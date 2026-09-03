/// Hover move / leave wiring on the seek scale.
fn wire_motion_controllers(seek: &gtk::Scale, st: &Rc<SeekPreviewState>) {
    let mot = gtk::EventControllerMotion::new();
    {
        let st = Rc::clone(st);
        mot.connect_motion(move |_, x, y| on_preview_motion(&st, x, y));
    }
    {
        let st = Rc::clone(st);
        mot.connect_leave(move |_| on_preview_leave(&st));
    }
    seek.add_controller(mot);
}

/// Hover move along the seek bar: resolve hover time, update labels, size and open the
/// preview (reopening seeks immediately; otherwise arms the debounce).
fn on_preview_motion(st: &Rc<SeekPreviewState>, x: f64, y: f64) {
    if st.last_xy.borrow().is_some_and(|p| p == (x, y)) {
        return;
    }
    *st.last_xy.borrow_mut() = Some((x, y));

    let bar_d = st.seek_adj.upper();
    if bar_d <= 0.0 {
        crate::preview_debug::warn(format!("motion: bar upper={bar_d} — hide"));
        st.hide();
        return;
    }
    let Some(t) = resolve_hover_time(st, bar_d, x) else {
        crate::preview_debug::warn(format!(
            "motion: no hover time bar={bar_d:.2} w={}",
            st.seek.width()
        ));
        st.hide();
        return;
    };
    st.hover_t.set(t);
    update_hover_labels(st, t);
    reveal_preview(st, x);
}

/// Timestamp under the cursor plus the active chapter name.
fn update_hover_labels(st: &Rc<SeekPreviewState>, t: f64) {
    st.time_lbl.set_text(&format_time(t));
    update_chapter_label(st, t);
}

/// Enablement / open-readiness gate, then show and seek or arm the debounce.
fn reveal_preview(st: &Rc<SeekPreviewState>, x: f64) {
    if !st.enabled.get() {
        crate::preview_debug::info("motion: preview off in prefs — labels only");
        return;
    }

    set_preview_size(st);

    if !open_target_ready(st) {
        return;
    }
    show_and_seek(st, x);
}

/// Logs and hides when browse is active or no openable target is ready for the aux player.
fn open_target_ready(st: &Rc<SeekPreviewState>) -> bool {
    if st.recent_visible.get() {
        crate::preview_debug::info("motion: continue browse — no framed preview");
        st.hide();
        return false;
    }
    if preview_open_path(&st.player, &st.last_path).is_none() {
        crate::preview_debug::warn("motion: open target not ready — hide");
        st.hide();
        return false;
    }
    true
}

fn show_and_seek(st: &Rc<SeekPreviewState>, x: f64) {
    let reopening = !st.is_open();
    st.show_at(x);
    crate::glib_source_drop::drop_glib_source(st.pump.as_ref());
    if reopening {
        crate::preview_debug::info(format!(
            "reopen warm={} hover={:.2}",
            st.preview_media_warm(),
            st.hover_t.get()
        ));
        run_preview_seek_now(st);
    } else {
        arm_preview_debounce(Rc::clone(st));
    }
}

/// Content time under the cursor, honouring optical chain heads and the aux preview player.
fn resolve_hover_time(st: &Rc<SeekPreviewState>, bar_d: f64, x: f64) -> Option<f64> {
    let main = st.player.borrow();
    let main_mpv = main.as_ref();
    let shell = main_mpv.and_then(budget_shell_path);
    let w = st.seek.width();
    let preview = st.preview.borrow();
    seek_bar_label_time(
        bar_d,
        w,
        x,
        main_mpv.map(|b| &b.mpv),
        shell.as_deref(),
        preview.as_ref().map(|p| &p.mpv),
        Some(&st.dvd_bar),
    )
}

/// Shows the current chapter name under the hover position (blank label when none).
fn update_chapter_label(st: &Rc<SeekPreviewState>, t: f64) {
    let ch = st.chapters.borrow();
    let name = ch
        .iter()
        .rfind(|(ct, _)| *ct <= t)
        .map(|(_, n)| n.as_str())
        .unwrap_or("");
    st.chapter_lbl.set_text(name);
    st.chapter_lbl.set_visible(!name.is_empty());
}

/// Hover left the seek bar: invalidate in-flight work and hide.
fn on_preview_leave(st: &Rc<SeekPreviewState>) {
    st.serial.set(st.serial.get().wrapping_add(1));
    crate::glib_source_drop::drop_glib_source(st.deb.as_ref());
    crate::glib_source_drop::drop_glib_source(st.pump.as_ref());
    *st.last_xy.borrow_mut() = None;
    st.hide();
}
