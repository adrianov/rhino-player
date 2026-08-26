pub(crate) fn wire_preview_gl(st: &Rc<SeekPreviewState>) {
    wire_gl_unrealize(st);
    wire_gl_render(st);
    wire_gl_realize(st);
}

/// Dispose the auxiliary mpv when the GL area goes away (load cache kept for re-hover).
fn wire_gl_unrealize(st: &Rc<SeekPreviewState>) {
    let pr_unrealize = Rc::clone(&st.preview);
    st.gl.connect_unrealize(move |a| {
        a.make_current();
        if let Some(old) = pr_unrealize.borrow_mut().take() {
            old.dispose(a);
        }
        crate::preview_debug::info("GLArea unrealised, preview mpv disposed (load cache kept)");
    });
}

fn wire_gl_render(st: &Rc<SeekPreviewState>) {
    let pr_draw = Rc::clone(&st.preview);
    let gl_draw = st.gl.clone();
    st.gl.connect_render(move |area, _| {
        area.make_current();
        if let Some(p) = pr_draw.borrow().as_ref() {
            p.draw(&gl_draw);
        }
        glib::Propagation::Stop
    });
}

fn wire_gl_realize(st: &Rc<SeekPreviewState>) {
    let st_realize = Rc::clone(st);
    st.gl.connect_realize(move |a| {
        a.make_current();
        let created = create_preview_on_realize(&st_realize, a);
        if created && seek_due_after_realise(&st_realize) {
            crate::preview_debug::info("realise while hover open — seek now");
            let st2 = Rc::clone(&st_realize);
            glib::idle_add_local_once(move || run_preview_seek_now(&st2));
        }
    });
}

/// Creates the preview mpv once; logs and recovers from init failure.
fn create_preview_on_realize(st: &SeekPreviewState, a: &gtk::GLArea) -> bool {
    let mut slot = st.preview.borrow_mut();
    if slot.is_some() {
        return false;
    }
    match MpvPreviewGl::new(a) {
        Ok(p) => {
            crate::preview_debug::info("GLArea realised, preview mpv ready");
            *slot = Some(p);
            true
        }
        Err(e) => {
            crate::preview_debug::warn(format!("GL/mpv init failed: {e}"));
            false
        }
    }
}

/// Hover is open, enabled, and nothing seeks or pumps yet.
fn seek_due_after_realise(st: &SeekPreviewState) -> bool {
    st.is_open()
        && st.last_xy.borrow().is_some()
        && st.enabled.get()
        && st.deb.borrow().is_none()
        && st.pump.borrow().is_none()
}
