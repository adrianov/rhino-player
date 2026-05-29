fn audio_tooltip_text(label: Option<&str>) -> String {
    match label.filter(|s| !s.is_empty()) {
        Some(l) => format!("Audio: {l}"),
        None => "Audio".to_string(),
    }
}

/// Header Sound button tooltip from the active track label (codec + layout when available).
pub fn refresh_audio_tooltip(mpv: &Mpv, btn: &gtk::MenuButton, shell: Option<&Path>) {
    let tip = audio_tooltip_text(current_audio_label(mpv, shell).as_deref());
    if btn.tooltip_text().as_deref() != Some(tip.as_str()) {
        btn.set_tooltip_text(Some(&tip));
    }
}

/// Like [refresh_audio_tooltip] but borrows the active player bundle when present.
pub fn refresh_audio_tooltip_for_player(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    btn: &gtk::MenuButton,
) {
    let Ok(g) = player.try_borrow() else {
        return;
    };
    let Some(b) = g.as_ref() else {
        if btn.tooltip_text().as_deref() != Some("Audio") {
            btn.set_tooltip_text(Some("Audio"));
        }
        return;
    };
    let shell = b.me_budget_shell_path.borrow();
    refresh_audio_tooltip(&b.mpv, btn, shell.as_deref());
}
