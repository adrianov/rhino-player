// Now Playing info-center publishing: build the metadata dictionary from live transport
// properties, or clear it when nothing is playing.

unsafe fn np_publish(title: &str, dur: f64, pos: f64, pause: bool, speed: f64) {
    let npc = MPNowPlayingInfoCenter::defaultCenter();
    let dict = NSMutableDictionary::<NSString, AnyObject>::new();
    np_insert_title(&dict, title);
    np_insert_double(&dict, MPMediaItemPropertyPlaybackDuration, dur);
    np_insert_double(&dict, MPNowPlayingInfoPropertyElapsedPlaybackTime, pos);
    np_insert_rate_and_type(&dict, pause, speed);
    npc.setNowPlayingInfo(Some(&dict));
    npc.setPlaybackState(if pause {
        MPNowPlayingPlaybackState::Paused
    } else {
        MPNowPlayingPlaybackState::Playing
    });
}

unsafe fn np_insert_title(dict: &NSMutableDictionary<NSString, AnyObject>, title: &str) {
    let title_ns = NSString::from_str(title);
    // `NSMutableDictionary::insert` → `setObject:forKey:` retains each **value** and copies **keys**
    // (NSString adopts NSCopying). Rust `Retained` temps may drop after each insert; the dictionary + ARC
    // keep owning references.
    dict.insert(MPMediaItemPropertyTitle, title_ns.as_ref());
}

unsafe fn np_insert_double(
    dict: &NSMutableDictionary<NSString, AnyObject>,
    key: &'static NSString,
    value: f64,
) {
    let n = NSNumber::numberWithDouble(value);
    // See the retention note in [`np_insert_title`]: same `insert` semantics.
    dict.insert(key, n.as_ref());
}

unsafe fn np_insert_rate_and_type(
    dict: &NSMutableDictionary<NSString, AnyObject>,
    pause: bool,
    speed: f64,
) {
    np_insert_double(
        dict,
        MPNowPlayingInfoPropertyPlaybackRate,
        if pause { 0.0 } else { speed },
    );
    let media_type_ns = NSNumber::numberWithUnsignedInteger(MPNowPlayingInfoMediaType::Video.0);
    dict.insert(MPNowPlayingInfoPropertyMediaType, media_type_ns.as_ref());
}

/// Collect transport properties for publishing; `None` clears Now Playing (no live player or no
/// finite positive duration).
fn np_transport_snapshot(
    player: &Rc<RefCell<Option<MpvBundle>>>,
) -> Option<(String, f64, f64, bool, f64)> {
    let g = player.try_borrow().ok()?;
    let b = g.as_ref()?;
    let dur = np_duration_secs(b);
    if dur <= 0.0 {
        return None;
    }
    let (pause, pos, speed) = np_transport_props(b);
    Some((np_media_title(b), dur, pos, pause, speed))
}

/// Pause flag plus sanitized elapsed time / rate for the Now Playing dictionary.
fn np_transport_props(b: &MpvBundle) -> (bool, f64, f64) {
    let pause = b.mpv.get_property::<bool>("pause").unwrap_or(false);
    let pos = np_sanitized(b.mpv.get_property::<f64>("time-pos").unwrap_or(0.0), 0.0);
    let speed = np_sanitized(b.mpv.get_property::<f64>("speed").unwrap_or(1.0), 1.0);
    (pause, pos, speed)
}

pub(crate) fn sync_macos_now_playing_for_transport(player: &Rc<RefCell<Option<MpvBundle>>>) {
    match np_transport_snapshot(player) {
        Some((title, dur, pos, pause, speed)) => unsafe {
            np_publish(&title, dur, pos, pause, speed);
        },
        None => unsafe {
            np_clear();
        },
    }
}
