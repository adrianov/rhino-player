// Event-driven thumb delivery: worker → ThumbInbox → MainContext::invoke → apply_ready_thumbs.
// No refill poll timer — same hop pattern as MpvBundle::install_event_drain.
// Flush callbacks live in a main-thread map keyed by inbox id (one entry per RecentContext).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

static THUMB_ID: AtomicU64 = AtomicU64::new(1);

/// Worker → main coalescing inbox + generation/cancel for thumbnail backfill.
pub(crate) struct ThumbBackfill {
    cancel: Arc<AtomicBool>,
    gen: Arc<std::sync::atomic::AtomicU64>,
    workers: RefCell<Vec<JoinHandle<()>>>,
    inbox: Arc<ThumbInbox>,
}

struct ThumbInbox {
    /// Stable id for the main-thread flush registry (per [RecentContext]).
    id: u64,
    pending: Mutex<Vec<std::path::PathBuf>>,
    /// True while a main-context flush is scheduled or running.
    flush_armed: AtomicBool,
}

type ThumbFlushFn = Box<dyn Fn(Vec<std::path::PathBuf>)>;

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
                Box::new(move |paths| {
                    if c.search.as_ref().is_some_and(|s| s.typing_pending()) {
                        return;
                    }
                    apply_ready_thumbs(&c.cards.borrow(), &c.media_paths.borrow(), &paths);
                }),
            );
        });
    }

    fn clear_flush(&self) {
        let id = self.inbox.id;
        THUMB_FLUSHES.with(|m| {
            m.borrow_mut().remove(&id);
        });
    }

    /// Capture missing stills on a worker. Ready paths coalesce into one main-context invoke.
    pub(crate) fn schedule(&self, paths: Vec<std::path::PathBuf>) {
        if paths.is_empty() {
            return;
        }
        let gen = self.gen.fetch_add(1, Ordering::AcqRel) + 1;
        let inbox = Arc::clone(&self.inbox);
        let c = self.cancel.clone();
        let gen_watch = self.gen.clone();
        let h = std::thread::spawn(move || run_thumb_worker(paths, gen, c, inbox, gen_watch));
        self.workers.borrow_mut().push(h);
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

fn note_thumb_ready(inbox: &Arc<ThumbInbox>, path: std::path::PathBuf) {
    if let Ok(mut g) = inbox.pending.lock() {
        if !g.iter().any(|q| q == &path) {
            g.push(path);
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
        if !p.exists() {
            continue;
        }
        let Ok(can) = std::fs::canonicalize(&p) else {
            continue;
        };
        if media_probe::thumb_backfill_satisfied(&can) {
            continue;
        }
        let _ = media_probe::ensure_thumbnail(&can);
        if thumb_gen_cancelled(&gen_watch, gen, &c) {
            return;
        }
        note_thumb_ready(&inbox, can);
    }
}
