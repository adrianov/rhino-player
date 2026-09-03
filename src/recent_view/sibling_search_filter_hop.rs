// Worker → main filter hop (feature 33). Same pattern as thumb backfill.
// `#[path]` submodule of [sibling_search_state.rs].

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use super::super::FilterOutcome;
use super::SiblingSearchState;

static FILTER_ID: AtomicU64 = AtomicU64::new(1);

struct FilterMsg {
    gen: u64,
    draft: String,
    outcome: FilterOutcome,
}

type FilterFlushFn = Box<dyn Fn(FilterMsg)>;
type FilterFlushMap = HashMap<u64, FilterFlushFn>;

/// Worker → main hop. Inbox is Send; flush map is main-only.
pub(super) struct FilterInbox {
    id: u64,
    pending: Mutex<Option<FilterMsg>>,
    armed: AtomicBool,
}

impl FilterInbox {
    pub(super) fn new() -> Arc<Self> {
        Arc::new(Self {
            id: FILTER_ID.fetch_add(1, Ordering::Relaxed),
            pending: Mutex::new(None),
            armed: AtomicBool::new(false),
        })
    }
}

thread_local! {
    static FILTER_FLUSHES: RefCell<FilterFlushMap> = RefCell::new(HashMap::new());
}

pub(super) fn install_filter_flush(state: &Rc<SiblingSearchState>) {
    let id = state.filter_inbox.id;
    let weak = Rc::downgrade(state);
    FILTER_FLUSHES.with(|m| {
        m.borrow_mut().insert(
            id,
            Box::new(move |msg| {
                let Some(s) = weak.upgrade() else {
                    return;
                };
                s.on_filter_done(msg.gen, msg.draft, msg.outcome);
            }),
        );
    });
}

pub(super) fn note_filter_done(
    inbox: &Arc<FilterInbox>,
    gen: u64,
    draft: String,
    outcome: FilterOutcome,
) {
    let msg = FilterMsg {
        gen,
        draft,
        outcome,
    };
    match inbox.pending.lock() {
        Ok(mut g) => {
            // Keep the newest generation; a late older worker must not clobber it.
            let keep = match g.as_ref() {
                None => true,
                Some(old) => gen >= old.gen,
            };
            if keep {
                *g = Some(msg);
            }
        }
        Err(_) => {
            eprintln!("[rhino] search: filter inbox lock poisoned");
            return;
        }
    }
    if inbox.armed.swap(true, Ordering::AcqRel) {
        return;
    }
    let inbox = Arc::clone(inbox);
    glib::MainContext::default().invoke(move || flush_filter_inbox(&inbox));
}

fn flush_filter_inbox(inbox: &FilterInbox) {
    inbox.armed.store(false, Ordering::Release);
    let msg = match inbox.pending.lock() {
        Ok(mut g) => g.take(),
        Err(_) => {
            eprintln!("[rhino] search: filter inbox lock poisoned on flush");
            return;
        }
    };
    let Some(msg) = msg else {
        return;
    };
    let id = inbox.id;
    FILTER_FLUSHES.with(|m| {
        if let Some(f) = m.borrow().get(&id) {
            f(msg);
        }
    });
}
