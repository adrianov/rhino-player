// Copy-path shortcut helpers (included from `input.rs`). Spec: `docs/features/13-input-shortcuts.md`.

#[cfg(target_os = "macos")]
fn copy_path_modifier_held(m: gtk::gdk::ModifierType) -> bool {
    m.contains(gtk::gdk::ModifierType::META_MASK)
}

#[cfg(not(target_os = "macos"))]
fn copy_path_modifier_held(m: gtk::gdk::ModifierType) -> bool {
    m.contains(gtk::gdk::ModifierType::CONTROL_MASK)
}

fn try_copy_playing_path(player: &Rc<RefCell<Option<MpvBundle>>>) -> bool {
    let g = player.borrow();
    let Some(b) = g.as_ref() else {
        eprintln!("[rhino] copy-path: no player");
        return false;
    };
    let shell = b.me_budget_shell_path.borrow();
    let Some(path) = crate::media_probe::shell_media_path(&b.mpv, shell.as_deref()) else {
        eprintln!("[rhino] copy-path: no open media path");
        return false;
    };
    let Some(display) = gtk::gdk::Display::default() else {
        eprintln!("[rhino] copy-path: no display");
        return false;
    };
    display.clipboard().set_text(&path.display().to_string());
    true
}
