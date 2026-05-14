// Register Now Playing metadata and MPRemoteCommandCenter handlers on macOS so system media keys
// route here during playback instead of launching Apple Music.

use std::ptr::NonNull;
use std::sync::Once;

use block2::RcBlock;
use objc2::runtime::AnyObject;
use objc2_foundation::{NSMutableDictionary, NSNumber, NSString};
use objc2_media_player::{
    MPNowPlayingInfoCenter, MPNowPlayingInfoMediaType, MPNowPlayingInfoPropertyElapsedPlaybackTime,
    MPNowPlayingInfoPropertyMediaType, MPNowPlayingInfoPropertyPlaybackRate,
    MPNowPlayingPlaybackState, MPMediaItemPropertyPlaybackDuration, MPMediaItemPropertyTitle,
    MPRemoteCommandCenter, MPRemoteCommandEvent, MPRemoteCommandHandlerStatus,
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

    let pk = play_key.clone();
    wire_remote_command(
        &center.togglePlayPauseCommand(),
        RcBlock::new(move |_ev: NonNull<MPRemoteCommandEvent>| {
            let _ = toggle_play_pause(&pk);
            MPRemoteCommandHandlerStatus::Success
        }),
    );

    let pk = play_key.clone();
    wire_remote_command(
        &center.playCommand(),
        RcBlock::new(move |_ev: NonNull<MPRemoteCommandEvent>| {
            let _ = apply_mpv_pause(&pk, false);
            MPRemoteCommandHandlerStatus::Success
        }),
    );

    let pk = play_key.clone();
    wire_remote_command(
        &center.pauseCommand(),
        RcBlock::new(move |_ev: NonNull<MPRemoteCommandEvent>| {
            let _ = apply_mpv_pause(&pk, true);
            MPRemoteCommandHandlerStatus::Success
        }),
    );

    let pk = play_key.clone();
    wire_remote_command(
        &center.stopCommand(),
        RcBlock::new(move |_ev: NonNull<MPRemoteCommandEvent>| {
            media_stop(&pk);
            MPRemoteCommandHandlerStatus::Success
        }),
    );

    let nav_n = nav.clone();
    wire_remote_command(
        &center.nextTrackCommand(),
        RcBlock::new(move |_ev: NonNull<MPRemoteCommandEvent>| {
            let r = nav_n.try_refs();
            try_load_sibling_pick(sibling_advance::next_after_eof, "next", &r);
            MPRemoteCommandHandlerStatus::Success
        }),
    );

    let nav_p = nav;
    wire_remote_command(
        &center.previousTrackCommand(),
        RcBlock::new(move |_ev: NonNull<MPRemoteCommandEvent>| {
            let r = nav_p.try_refs();
            try_load_sibling_pick(sibling_advance::prev_before_current, "previous", &r);
            MPRemoteCommandHandlerStatus::Success
        }),
    );
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

unsafe fn np_publish(title: &str, dur: f64, pos: f64, pause: bool, speed: f64) {
    let npc = MPNowPlayingInfoCenter::defaultCenter();
    let dict = NSMutableDictionary::<NSString, AnyObject>::new();
    let title_ns = NSString::from_str(title);
    // `NSMutableDictionary::insert` → `setObject:forKey:` retains each **value** and copies **keys**
    // (NSString adopts NSCopying). Rust `Retained` temps may drop after each insert; the dictionary + ARC
    // keep owning references.
    dict.insert(MPMediaItemPropertyTitle, title_ns.as_ref());

    let dur_ns = NSNumber::numberWithDouble(dur);
    dict.insert(MPMediaItemPropertyPlaybackDuration, dur_ns.as_ref());

    let pos_ns = NSNumber::numberWithDouble(pos);
    dict.insert(MPNowPlayingInfoPropertyElapsedPlaybackTime, pos_ns.as_ref());

    let rate = if pause { 0.0 } else { speed };
    let rate_ns = NSNumber::numberWithDouble(rate);
    dict.insert(MPNowPlayingInfoPropertyPlaybackRate, rate_ns.as_ref());

    let media_type_ns = NSNumber::numberWithUnsignedInteger(MPNowPlayingInfoMediaType::Video.0);
    dict.insert(MPNowPlayingInfoPropertyMediaType, media_type_ns.as_ref());

    npc.setNowPlayingInfo(Some(&dict));
    npc.setPlaybackState(if pause {
        MPNowPlayingPlaybackState::Paused
    } else {
        MPNowPlayingPlaybackState::Playing
    });
}

pub(crate) fn sync_macos_now_playing_for_transport(player: &Rc<RefCell<Option<MpvBundle>>>) {
    let Ok(g) = player.try_borrow() else {
        unsafe { np_clear() };
        return;
    };
    let Some(b) = g.as_ref() else {
        unsafe { np_clear() };
        return;
    };
    let dur = b.mpv.get_property::<f64>("duration").unwrap_or(0.0);
    let dur = if dur.is_finite() { dur.max(0.0) } else { 0.0 };
    if dur <= 0.0 {
        unsafe { np_clear() };
        return;
    }
    let pause = b.mpv.get_property::<bool>("pause").unwrap_or(false);
    let mut pos = b.mpv.get_property::<f64>("time-pos").unwrap_or(0.0);
    pos = if pos.is_finite() { pos.max(0.0) } else { 0.0 };
    let mut speed = b.mpv.get_property::<f64>("speed").unwrap_or(1.0);
    speed = if speed.is_finite() { speed.max(0.0) } else { 1.0 };

    let mut title = b
        .mpv
        .get_property::<String>("media-title")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_default();
    if title.is_empty() {
        title = local_file_from_mpv(&b.mpv)
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "Rhino Player".into());
    }

    unsafe {
        np_publish(&title, dur, pos, pause, speed);
    }
}
