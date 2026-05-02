// Hardware media keys on macOS: GDK often never emits matching key events; AppKit delivers them as
// `NSEventType::SystemDefined`. Mirrors the GDK `Audio*` handling in `keys.rs`.

use block2::RcBlock;
use core::ptr;
use core::ptr::NonNull;
use objc2_app_kit::{NSEvent, NSEventMask, NSEventSubtype, NSEventType};

/// Key IDs in `data1` for auxiliary media keys ([`NX_KEYTYPE_*`] in `ev_keymap.h`).
/// Chromium maps [`NX_KEYTYPE_NEXT`] + [`NX_KEYTYPE_FAST`] → next track and
/// [`NX_KEYTYPE_PREVIOUS`] + [`NX_KEYTYPE_REWIND`] → previous (`media_keys_listener_mac.mm`).
const NX_KEYTYPE_PLAY: i64 = 16;
const NX_KEYTYPE_NEXT: i64 = 17;
const NX_KEYTYPE_PREVIOUS: i64 = 18;
const NX_KEYTYPE_FAST: i64 = 19;
const NX_KEYTYPE_REWIND: i64 = 20;

/// Auxiliary control “key down” in `data1` flags (high byte).
const MEDIA_KEY_DOWN: u8 = 0x0a;

fn wire_macos_media_keys(play_key: PlayToggleCtx, nav: SiblingNavCtx) {
    let block = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        let ev = unsafe { event.as_ref() };
        if dispatch_macos_media_key(ev, &play_key, &nav) {
            return ptr::null_mut();
        }
        event.as_ptr()
    });

    let mask = NSEventMask::SystemDefined;
    let monitor =
        unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &block) };
    if let Some(monitor) = monitor {
        std::mem::forget(monitor);
    }
}

fn dispatch_macos_media_key(ev: &NSEvent, play_key: &PlayToggleCtx, nav: &SiblingNavCtx) -> bool {
    if ev.r#type() != NSEventType::SystemDefined {
        return false;
    }
    if ev.subtype() != NSEventSubtype::ScreenChanged {
        return false;
    }

    let d = ev.data1() as i64;
    let key_code = (d >> 16) & 0xffff;
    let flags = (d & 0xffff) as u16;
    let key_state = ((flags >> 8) & 0xff) as u8;
    if key_state != MEDIA_KEY_DOWN {
        return false;
    }

    let refs = nav.try_refs();
    match key_code {
        NX_KEYTYPE_PLAY => {
            let _ = toggle_play_pause(play_key);
            true
        }
        NX_KEYTYPE_PREVIOUS | NX_KEYTYPE_REWIND => {
            try_load_sibling_pick(sibling_advance::prev_before_current, "previous", &refs);
            true
        }
        NX_KEYTYPE_NEXT | NX_KEYTYPE_FAST => {
            try_load_sibling_pick(sibling_advance::next_after_eof, "next", &refs);
            true
        }
        _ => false,
    }
}
