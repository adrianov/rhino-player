/// Local paths whose extension counts as external subtitles for [sub-add].
const SUBTITLE_EXT: &[&str] = &[
    "srt", "vtt", "ass", "ssa", "smi", "sub", "sup", "idx", "mpl2", "mks",
];

fn is_subtitle_path(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| SUBTITLE_EXT.iter().any(|s| *s == e.to_ascii_lowercase()))
}

fn drop_subtitles_on_mpv(mpv: &Mpv, subs: &[PathBuf]) {
    for utf8 in subs.iter().filter_map(|p| p.to_str()) {
        let _ = mpv.command("sub-add", &[utf8]);
    }
}

fn playlist_append_utf8_paths(mpv: &Mpv, paths: &[PathBuf]) {
    for utf8 in paths.iter().filter_map(|p| p.to_str()) {
        let _ = mpv.command("loadfile", &[utf8, "append"]);
    }
}

fn consume_dropped_paths(
    paths: Vec<PathBuf>,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    sub_menu: &gtk::MenuButton,
    on_open: &RcPathFn,
) {
    if paths.is_empty() {
        eprintln!("[rhino] dnd: empty path list");
        return;
    }
    let dropped_n = paths.len();

    let mpv_loaded = player.borrow().as_ref().is_some_and(|b| {
        crate::media_probe::local_file_from_mpv(&b.mpv).is_some()
    });

    let mut subs = Vec::new();
    let mut media = Vec::new();
    for p in paths {
        if is_subtitle_path(&p) {
            subs.push(p);
        } else if crate::video_ext::is_openable_media_path(&p) {
            media.push(crate::video_ext::resolve_open_media_path(&p));
        }
    }

    if mpv_loaded && !subs.is_empty() {
        if let Some(b) = player.borrow().as_ref() {
            drop_subtitles_on_mpv(&b.mpv, &subs);
        }
        schedule_sub_button_scan(Rc::clone(player), sub_menu.clone());
        if media.is_empty() {
            return;
        }
    }

    if media.is_empty() {
        eprintln!("[rhino] dnd: no openable media ({dropped_n} path(s) dropped)");
        return;
    }

    on_open(media[0].as_path());

    let extra = media.len().saturating_sub(1);
    if extra != 0 && player.borrow().is_some() {
        let tail = media[1..].to_vec();
        let pl = Rc::clone(player);
        let _ = glib::idle_add_local_once(move || {
            if let Some(b) = pl.borrow().as_ref() {
                playlist_append_utf8_paths(&b.mpv, &tail);
            }
        });
    }
}

#[cfg(not(target_os = "macos"))]
fn dispatch_paths_and_finish_drop(
    paths: Vec<PathBuf>,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    sub_menu: &gtk::MenuButton,
    on_open: &RcPathFn,
    drop_done: &gtk::gdk::Drop,
) {
    consume_dropped_paths(paths, player, sub_menu, on_open);
    finish_drop(drop_done);
}
