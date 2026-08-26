include!("final_actions_wire/simple_actions.rs");

fn wire_final_platform_accels(ctx: &FinalActionCtx) {
    #[cfg(target_os = "macos")]
    {
        ctx.app.set_menubar(Some(&ctx.main_menu));
        ctx.app.set_accels_for_action("app.open", &["<Meta>o"]);
        ctx.app
            .set_accels_for_action("app.close-video", &["<Meta>w"]);
        ctx.app.set_accels_for_action(
            "app.move-to-trash",
            &["Delete", "KP_Delete", "<Meta>BackSpace"],
        );
        ctx.app.set_accels_for_action("app.quit", &["<Meta>q", "q"]);
        ctx.app
            .set_accels_for_action("app.toggle-fullscreen", &["<Meta><Control>f"]);
    }
    #[cfg(not(target_os = "macos"))]
    {
        ctx.app.set_accels_for_action("app.open", &["<Primary>o"]);
        ctx.app
            .set_accels_for_action("app.close-video", &["<Primary>w"]);
        ctx.app
            .set_accels_for_action("app.move-to-trash", &["Delete", "KP_Delete"]);
        ctx.app.set_accels_for_action("app.about", &["F1"]);
        ctx.app
            .set_accels_for_action("app.quit", &["<Primary>q", "q"]);
        ctx.app
            .set_accels_for_action("app.toggle-fullscreen", &["F11"]);
    }
}

fn wire_final_idle_chrome_resize(ctx: &FinalActionCtx) {
    apply_chrome(ChromeApplyParts {
        hdr_csd_baseline: &ctx.hdr_csd_baseline,
        root: &ctx.root,
        header: &ctx.header,
        gl: &ctx.gl,
        bar_show: &ctx.bar_show,
        recent: &ctx.recent,
        bottom: &ctx.bottom,
        player: &ctx.player,
    });
    wire_smooth_resize_and_subtitle_pos(
        &ctx.gl,
        &ctx.bottom,
        &ctx.player,
        &ctx.bar_show,
        &ctx.recent,
    );
    let idle_t = Rc::clone(&ctx.idle_inhib);
    let p_t = Rc::clone(&ctx.player);
    let r_t = ctx.recent.clone();
    let a_t = ctx.app.clone();
    let w_t = ctx.win.clone();
    glib::source::timeout_add_local(
        Duration::from_millis(500),
        glib::clone!(
            #[strong]
            a_t,
            #[strong]
            w_t,
            #[strong]
            p_t,
            #[strong]
            r_t,
            #[strong]
            idle_t,
            move || {
                let should = idle_inhibit::should_inhibit(&p_t, r_t.is_visible());
                let gtk_a: &gtk::Application = a_t.upcast_ref();
                idle_inhibit::sync(gtk_a, Some(&w_t), should, &idle_t);
                glib::ControlFlow::Continue
            }
        ),
    );
}
