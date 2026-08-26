// Register Now Playing metadata and MPRemoteCommandCenter handlers on macOS so system media keys
// route here during playback instead of launching Apple Music.
include!("macos_now_playing_info.rs");

use std::ptr::NonNull;
use std::sync::Once;

use block2::RcBlock;
use objc2::runtime::AnyObject;
use objc2_foundation::{NSMutableDictionary, NSNumber, NSString};
use objc2_media_player::{
    MPMediaItemPropertyPlaybackDuration, MPMediaItemPropertyTitle, MPNowPlayingInfoCenter,
    MPNowPlayingInfoMediaType, MPNowPlayingInfoPropertyElapsedPlaybackTime,
    MPNowPlayingInfoPropertyMediaType, MPNowPlayingInfoPropertyPlaybackRate,
    MPNowPlayingPlaybackState, MPRemoteCommandCenter, MPRemoteCommandEvent,
    MPRemoteCommandHandlerStatus,
};

/// Install one remote-command handler and give Rust-side ownership of the block/token to ObjC for the
/// rest of process lifetime.
///
/// **Leak / retention (deliberate, bounded):** [`MPRemoteCommand::addTargetWithHandler`] retains the
/// handler block and returns an opaque target object used internally by MediaPlayer. We never call
/// `removeTarget:`. Registration happens exactly **six** times under [`Once::call_once`] in
/// [`register_remote_commands`], so leaked Objective‑C objects are bounded and fixed at startup.
/// [`std::mem::forget`] on both values avoids pairing bugs if Rust’s [`RcBlock`] drop raced or
/// over‑released relative to MediaPlayer’s retain semantics — same practical trade‑off as leaving
/// targets registered until exit in Objective‑C.
fn wire_remote_command(
    cmd: &objc2_media_player::MPRemoteCommand,
    handler: RcBlock<dyn Fn(NonNull<MPRemoteCommandEvent>) -> MPRemoteCommandHandlerStatus>,
) {
    let tok = unsafe { cmd.addTargetWithHandler(&handler) };
    std::mem::forget(tok);
    std::mem::forget(handler);
}

unsafe fn register_remote_commands(play_key: PlayToggleCtx, nav: SiblingNavCtx) {
    let center = MPRemoteCommandCenter::sharedCommandCenter();

    unsafe {
        register_playback_remote_commands(&center, &play_key);
        register_track_remote_commands(&center, &nav);
    }
}

/// Register one remote command whose handler receives the shared context by reference.
unsafe fn wire_ctx_command<C, F>(cmd: &objc2_media_player::MPRemoteCommand, ctx: C, handler: F)
where
    C: 'static,
    F: Fn(&C, NonNull<MPRemoteCommandEvent>) -> MPRemoteCommandHandlerStatus + 'static,
{
    wire_remote_command(cmd, RcBlock::new(move |ev| handler(&ctx, ev)));
}

/// Toggle / play / pause / stop handlers on the shared [`PlayToggleCtx`].
unsafe fn register_playback_remote_commands(
    center: &MPRemoteCommandCenter,
    play_key: &PlayToggleCtx,
) {
    unsafe {
        wire_ctx_command(
            &center.togglePlayPauseCommand(),
            play_key.clone(),
            |pk, _| {
                let _ = toggle_play_pause(pk);
                MPRemoteCommandHandlerStatus::Success
            },
        );
        wire_ctx_command(&center.playCommand(), play_key.clone(), |pk, _| {
            let _ = apply_mpv_pause(pk, false);
            MPRemoteCommandHandlerStatus::Success
        });
        wire_ctx_command(&center.pauseCommand(), play_key.clone(), |pk, _| {
            let _ = apply_mpv_pause(pk, true);
            MPRemoteCommandHandlerStatus::Success
        });
        wire_ctx_command(&center.stopCommand(), play_key.clone(), |pk, _| {
            media_stop(pk);
            MPRemoteCommandHandlerStatus::Success
        });
    }
}

/// Next / previous track handlers on the shared [`SiblingNavCtx`].
unsafe fn register_track_remote_commands(center: &MPRemoteCommandCenter, nav: &SiblingNavCtx) {
    unsafe {
        wire_ctx_command(&center.nextTrackCommand(), nav.clone(), |nav, _| {
            let r = nav.try_refs();
            try_load_sibling_pick(sibling_advance::next_after_eof, "next", &r);
            MPRemoteCommandHandlerStatus::Success
        });
        wire_ctx_command(&center.previousTrackCommand(), nav.clone(), |nav, _| {
            let r = nav.try_refs();
            try_load_sibling_pick(sibling_advance::prev_before_current, "previous", &r);
            MPRemoteCommandHandlerStatus::Success
        });
    }
}

fn wire_macos_now_playing_remote(play_key: PlayToggleCtx, nav: SiblingNavCtx) {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        register_remote_commands(play_key, nav);
    });
}

unsafe fn np_clear() {
    let npc = MPNowPlayingInfoCenter::defaultCenter();
    npc.setNowPlayingInfo(None);
    npc.setPlaybackState(MPNowPlayingPlaybackState::Stopped);
}

/// Clamp a finite, non-negative mpv seconds value; fall back when missing or non-finite.
fn np_sanitized(v: f64, fallback: f64) -> f64 {
    if v.is_finite() {
        v.max(0.0)
    } else {
        fallback
    }
}

/// Positive duration or `0.0` (which clears Now Playing — no media to describe).
fn np_duration_secs(b: &MpvBundle) -> f64 {
    let d = b.mpv.get_property::<f64>("duration").unwrap_or(0.0);
    np_sanitized(d, 0.0)
}

/// Media title, falling back to the file name and then the app name.
fn np_media_title(b: &MpvBundle) -> String {
    let streamed = np_streamed_title(b);
    if !streamed.is_empty() {
        return streamed;
    }
    np_fallback_title(b)
}

/// `media-title` when mpv provides a non-blank one.
fn np_streamed_title(b: &MpvBundle) -> String {
    b.mpv
        .get_property::<String>("media-title")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_default()
}

/// File name of the playing local file, else the app name.
fn np_fallback_title(b: &MpvBundle) -> String {
    local_file_from_mpv(&b.mpv)
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "Rhino Player".into())
}
