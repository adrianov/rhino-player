//! Rebuild audio/subtitle track rows when a header menu opens (windowed popover or macOS theater overlay).

use std::cell::RefCell;
use std::rc::Rc;

thread_local! {
    static REFRESH_AUDIO: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
    static REFRESH_SUB: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
}

pub struct HeaderMenuTrackHooks {
    pub audio: Rc<dyn Fn()>,
    pub sub: Rc<dyn Fn()>,
}

pub fn register_refresh(hooks: HeaderMenuTrackHooks) {
    REFRESH_AUDIO.with(|s| *s.borrow_mut() = Some(hooks.audio));
    REFRESH_SUB.with(|s| *s.borrow_mut() = Some(hooks.sub));
}

/// Sound menu opened — rebuild track list (popover show or fullscreen overlay).
#[cfg(target_os = "macos")]
pub fn refresh_audio_on_open() {
    REFRESH_AUDIO.with(|s| {
        if let Some(f) = s.borrow().as_ref() {
            f();
        }
    });
}

/// Subtitles menu opened — rebuild track list (popover show or fullscreen overlay).
#[cfg(target_os = "macos")]
pub fn refresh_sub_on_open() {
    REFRESH_SUB.with(|s| {
        if let Some(f) = s.borrow().as_ref() {
            f();
        }
    });
}
