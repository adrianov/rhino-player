pub(crate) fn wire_preview_gl(st: &Rc<SeekPreviewState>) {
    let pr_unrealize = Rc::clone(&st.preview);
    st.gl.connect_unrealize(move |a| {
        a.make_current();
        if let Some(old) = pr_unrealize.borrow_mut().take() {
            old.dispose(a);
        }
        crate::preview_debug::info("GLArea unrealised, preview mpv disposed (load cache kept)");
    });

    let pr_draw = Rc::clone(&st.preview);
    let gl_draw = st.gl.clone();
    st.gl.connect_render(move |area, _| {
        area.make_current();
        if let Some(p) = pr_draw.borrow().as_ref() {
            p.draw(&gl_draw);
        }
        glib::Propagation::Stop
    });

    let st_realize = Rc::clone(st);
    st.gl.connect_realize(move |a| {
        a.make_current();
        let created = {
            let mut slot = st_realize.preview.borrow_mut();
            if slot.is_some() {
                false
            } else {
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
        };
        if created
            && st_realize.is_open()
            && st_realize.last_xy.borrow().is_some()
            && st_realize.enabled.get()
            && st_realize.deb.borrow().is_none()
            && st_realize.pump.borrow().is_none()
        {
            crate::preview_debug::info("realise while hover open — seek now");
            let st2 = Rc::clone(&st_realize);
            glib::idle_add_local_once(move || run_preview_seek_now(&st2));
        }
    });
}
