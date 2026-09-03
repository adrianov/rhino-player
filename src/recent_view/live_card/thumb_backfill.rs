// Event-driven thumb delivery: worker → ThumbInbox → MainContext::invoke → apply_ready_thumbs.
// No refill poll timer — same hop pattern as MpvBundle::install_event_drain.
// Flush callbacks live in a main-thread map keyed by inbox id (one entry per RecentContext).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

static THUMB_ID: AtomicU64 = AtomicU64::new(1);

/// Concurrent libmpv captures (Lucky / search often need a still per card).
const THUMB_WORKERS: usize = 3;

/// Worker → main coalescing inbox + generation/cancel for thumbnail backfill.
pub(crate) struct ThumbBackfill {
    cancel: Arc<AtomicBool>,
    gen: Arc<std::sync::atomic::AtomicU64>,
    workers: RefCell<Vec<JoinHandle<()>>>,
    inbox: Arc<ThumbInbox>,
}

enum ThumbNote {
    Ready(std::path::PathBuf),
    Drop(std::path::PathBuf),
}

struct ThumbInbox {
    /// Stable id for the main-thread flush registry (per [RecentContext]).
    id: u64,
    pending: Mutex<Vec<ThumbNote>>,
    /// True while a main-context flush is scheduled or running.
    flush_armed: AtomicBool,
}

type ThumbFlushFn = Box<dyn Fn(Vec<ThumbNote>)>;

thread_local! {
    /// Main-thread only: inbox id → apply hook for that window's [RecentContext].
    static THUMB_FLUSHES: RefCell<HashMap<u64, ThumbFlushFn>> = RefCell::new(HashMap::new());
}

impl ThumbBackfill {
    pub(crate) fn new() -> Self {
        Self {
            cancel: Arc::new(AtomicBool::new(true)),
            gen: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            workers: RefCell::new(Vec::new()),
            inbox: Arc::new(ThumbInbox {
                id: THUMB_ID.fetch_add(1, Ordering::Relaxed),
                pending: Mutex::new(Vec::new()),
                flush_armed: AtomicBool::new(false),
            }),
        }
    }

    /// Register this context's flush (call once after the context `Rc` is built).
    pub(crate) fn install_flush(ctx: &Rc<RecentContext>) {
        let id = ctx.thumbs.inbox.id;
        let c = Rc::clone(ctx);
        THUMB_FLUSHES.with(|m| {
            m.borrow_mut().insert(
                id,
                Box::new(move |notes| apply_thumb_notes(&c, notes)),
            );
        });
    }

    fn clear_flush(&self) {
        let id = self.inbox.id;
        THUMB_FLUSHES.with(|m| {
            m.borrow_mut().remove(&id);
        });
    }

    /// Capture missing stills on workers. Ready paths coalesce into one main-context invoke.
    /// Several workers so a Lucky / search handful of never-watched files is not strictly serial.
    pub(crate) fn schedule(&self, paths: Vec<std::path::PathBuf>) {
        if paths.is_empty() {
            return;
        }
        let gen = self.gen.fetch_add(1, Ordering::AcqRel) + 1;
        self.spawn_workers(paths, gen);
    }

    fn spawn_workers(&self, paths: Vec<std::path::PathBuf>, gen: u64) {
        self.workers.borrow_mut().retain(|h| !h.is_finished());
        for chunk in thumb_chunks(&paths) {
            let inbox = Arc::clone(&self.inbox);
            let c = self.cancel.clone();
            let gen_watch = self.gen.clone();
            let h = std::thread::spawn(move || run_thumb_worker(chunk, gen, c, inbox, gen_watch));
            self.workers.borrow_mut().push(h);
        }
    }

    pub(crate) fn shutdown(&self) {
        self.cancel.store(false, Ordering::Release);
        self.clear_flush();
        let workers: Vec<JoinHandle<()>> = self.workers.borrow_mut().drain(..).collect();
        if workers.is_empty() {
            return;
        }
        if let Err(e) = std::thread::Builder::new()
            .name("rhino-recent-join".to_string())
            .spawn(move || {
                for h in workers {
                    let _ = h.join();
                }
            })
        {
            eprintln!("[rhino] recent: joiner spawn: {e}");
        }
    }
}

fn note_thumb(inbox: &Arc<ThumbInbox>, note: ThumbNote) {
    if let Ok(mut g) = inbox.pending.lock() {
        let path = match &note {
            ThumbNote::Ready(p) | ThumbNote::Drop(p) => p,
        };
        if !g.iter().any(|q| note_path(q) == path) {
            g.push(note);
        }
    } else {
        eprintln!("[rhino] recent: thumb inbox lock poisoned");
        return;
    }
    if inbox.flush_armed.swap(true, Ordering::AcqRel) {
        return;
    }
    let inbox = Arc::clone(inbox);
    glib::MainContext::default().invoke(move || flush_thumb_inbox(&inbox));
}

fn note_path(note: &ThumbNote) -> &std::path::Path {
    match note {
        ThumbNote::Ready(p) | ThumbNote::Drop(p) => p,
    }
}

fn flush_thumb_inbox(inbox: &ThumbInbox) {
    inbox.flush_armed.store(false, Ordering::Release);
    let ready = match inbox.pending.lock() {
        Ok(mut g) => std::mem::take(&mut *g),
        Err(_) => {
            eprintln!("[rhino] recent: thumb inbox lock poisoned on flush");
            return;
        }
    };
    if ready.is_empty() {
        return;
    }
    let id = inbox.id;
    THUMB_FLUSHES.with(|m| {
        if let Some(f) = m.borrow().get(&id) {
            f(ready);
        }
    });
}

fn thumb_chunks(paths: &[std::path::PathBuf]) -> impl Iterator<Item = Vec<std::path::PathBuf>> + '_ {
    let n = THUMB_WORKERS.min(paths.len()).max(1);
    (0..n).filter_map(move |i| {
        let chunk: Vec<_> = paths.iter().skip(i).step_by(n).cloned().collect();
        (!chunk.is_empty()).then_some(chunk)
    })
}

fn thumb_gen_cancelled(gen_watch: &std::sync::atomic::AtomicU64, gen: u64, c: &AtomicBool) -> bool {
    gen_watch.load(Ordering::Acquire) != gen || !c.load(Ordering::Acquire)
}

fn run_thumb_worker(
    paths: Vec<std::path::PathBuf>,
    gen: u64,
    c: Arc<AtomicBool>,
    inbox: Arc<ThumbInbox>,
    gen_watch: Arc<std::sync::atomic::AtomicU64>,
) {
    for p in paths {
        if thumb_gen_cancelled(&gen_watch, gen, &c) {
            return;
        }
        let (thumb, key) = media_probe::ensure_listed_thumbnail(&p);
        let note = match thumb {
            media_probe::GridThumb::Ready => ThumbNote::Ready(key),
            media_probe::GridThumb::Unparseable => ThumbNote::Drop(key),
            media_probe::GridThumb::Miss => continue,
        };
        if thumb_gen_cancelled(&gen_watch, gen, &c) {
            return;
        }
        note_thumb(&inbox, note);
    }
}
