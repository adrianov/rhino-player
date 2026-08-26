fn mpv_gl_realize_attach(
    area: &gtk::GLArea,
    ctx: &MpvRealizeCtx,
    file_boot_rz: &Rc<RefCell<Option<PathBuf>>>,
    vp_realize: &Rc<RefCell<db::VideoPrefs>>,
) {
    let area = area.clone();
    let ctx = ctx.clone();
    let file_boot_rz = Rc::clone(file_boot_rz);
    let vp_realize = Rc::clone(vp_realize);
    glib::idle_add_local_once(move || {
        mpv_gl_realize_attach_now(&area, &ctx, &file_boot_rz, &vp_realize)
    });
}

fn mpv_gl_realize_attach_now(
    area: &gtk::GLArea,
    ctx: &MpvRealizeCtx,
    file_boot_rz: &Rc<RefCell<Option<PathBuf>>>,
    vp_realize: &Rc<RefCell<db::VideoPrefs>>,
) {
    area.make_current();
    let init = {
        let mut v = vp_realize.borrow_mut();
        MpvBundle::new(area, &mut v)
    };
    match init {
        Ok((b, auto_off)) => {
            gl_realize_bundle_ready(area, ctx, b, auto_off);
            run_stashed_after_present_wire();
            if let Some(p) = file_boot_rz.replace(None) {
                load_startup_file(ctx, vp_realize, p);
            }
        }
        Err(e) => eprintln!("[rhino] OpenGL / mpv: {e}"),
    }
}

fn load_startup_file(ctx: &MpvRealizeCtx, vp_realize: &Rc<RefCell<db::VideoPrefs>>, p: PathBuf) {
    let mut o = LoadOpts::replace_media(ReplaceMediaBundled {
        video_pref: Rc::clone(vp_realize),
        last_path: ctx.st.last_path.clone(),
        on_start: Some(Rc::clone(&ctx.st.on_video_chrome)),
        win_aspect: Rc::clone(&ctx.st.win_aspect),
        on_loaded: Some(Rc::clone(&ctx.st.on_file_loaded)),
        play_on_start: true,
        reset_speed_to_normal: false,
        hdr_title_mirror: ctx.shell.hdr_title_mirror.clone(),
    });
    o.playback_focus = Some(Rc::clone(&ctx.st.playback_focus));
    o.on_open_fail = Some(Rc::clone(&ctx.st.on_open_fail));
    if let Err(e) = try_load(
        &p,
        &ctx.shell.player,
        &ctx.shell.win,
        &ctx.shell.gl,
        &ctx.shell.recent,
        &o,
    ) {
        eprintln!("[rhino] try_load (startup): {e}");
    }
}

/// Linux passes `win_hide = Some(window)` so teardown can hide the GTK shell before quit; macOS passes `None`.
fn mpv_gl_render_frame(
    area: &gtk::GLArea,
    td: &Rc<Cell<bool>>,
    p_draw: &Rc<RefCell<Option<MpvBundle>>>,
    app_rd: &adw::Application,
    gl_bundle_drop: &gtk::GLArea,
    win_hide: Option<&adw::ApplicationWindow>,
) -> glib::Propagation {
    area.make_current();
    if td.replace(false) {
        gl_quit_teardown(area, p_draw, app_rd, gl_bundle_drop, win_hide);
        return glib::Propagation::Stop;
    }
    #[cfg(target_os = "macos")]
    if let Some(b) = p_draw.borrow().as_ref() {
        b.clear_glarea_transparent();
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(b) = p_draw.borrow().as_ref() {
        b.draw(area);
    }
    glib::Propagation::Stop
}

fn gl_quit_teardown(
    area: &gtk::GLArea,
    p_draw: &Rc<RefCell<Option<MpvBundle>>>,
    app_rd: &adw::Application,
    gl_bundle_drop: &gtk::GLArea,
    win_hide: Option<&adw::ApplicationWindow>,
) {
    if let Some(b) = p_draw.borrow().as_ref() {
        b.teardown_gl_paint(area);
    }
    if let Some(w) = win_hide {
        w.set_visible(false);
    }
    let to_drop = p_draw.borrow_mut().take();
    let app_q = app_rd.clone();
    let gl_q = gl_bundle_drop.clone();
    glib::idle_add_local_once(move || {
        gl_q.make_current();
        if let Some(b) = to_drop {
            b.dispose_for_quit(&gl_q);
        }
        app_q.quit();
    });
}
