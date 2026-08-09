/// Playback chrome: shell layout registration, bar show/autohide, menu-open hold.
struct VideoChrome {
    hdr_csd_baseline: Rc<Cell<Option<(bool, bool)>>>,
    ch_hide: Rc<ChromeBarHide>,
    /// Reveal bars and cancel autohide (call after leaving the continue strip).
    on_show: Rc<dyn Fn()>,
}

struct VideoChromeParts<'a> {
    win: &'a adw::ApplicationWindow,
    root: &'a adw::ToolbarView,
    header: &'a adw::HeaderBar,
    outer_ovl: &'a gtk::Overlay,
    video_handle: &'a gtk::WindowHandle,
    gl: &'a gtk::GLArea,
    recent: &'a gtk::Box,
    bottom: &'a gtk::Box,
    #[cfg(target_os = "macos")]
    bottom_shell: &'a gtk::Box,
    player: &'a Rc<RefCell<Option<MpvBundle>>>,
    bar_show: &'a Rc<Cell<bool>>,
    nav_t: &'a Rc<RefCell<Option<glib::SourceId>>>,
    motion_squelch: &'a Rc<Cell<Option<Instant>>>,
    seek_grabbed: &'a Rc<Cell<bool>>,
    vol_menu: &'a gtk::MenuButton,
    sub_menu: &'a gtk::MenuButton,
    speed_mbtn: &'a gtk::MenuButton,
    menu_btn: &'a gtk::MenuButton,
}

impl VideoChrome {
    fn attach(p: VideoChromeParts<'_>) -> Self {
        attach_window_shell(&WindowInputShell {
            win: p.win.clone(),
            root: p.root.clone(),
            header: p.header.clone(),
            outer_ovl: p.outer_ovl.clone(),
            video_handle: p.video_handle.clone(),
            bottom: p.bottom.clone(),
            #[cfg(target_os = "macos")]
            bottom_shell: p.bottom_shell.clone(),
            gl: p.gl.clone(),
            recent: p.recent.clone(),
        });
        let shell_layout = Rc::new(ShellLayoutCtx {
            win: p.win.clone(),
            root: p.root.clone(),
            header: p.header.clone(),
            video_handle: p.video_handle.clone(),
            gl: p.gl.clone(),
            bottom: p.bottom.clone(),
            #[cfg(target_os = "macos")]
            bottom_shell: p.bottom_shell.clone(),
            recent: p.recent.clone(),
            bar_show: Rc::clone(p.bar_show),
            player: Rc::clone(p.player),
            touch_chrome: RefCell::new(None),
        });
        register_shell_layout(Rc::clone(&shell_layout));
        #[cfg(target_os = "macos")]
        {
            wire_macos_recent_hide_refresh(p.win, p.gl, p.recent, p.player);
            wire_macos_surface_compositing_refresh(&shell_layout);
        }

        let hdr_csd_baseline = Rc::new(Cell::new(None));
        wire_header_csd_baseline_snap(&hdr_csd_baseline, p.header);

        let ch_hide = Rc::new(ChromeBarHide {
            nav: p.nav_t.clone(),
            vol: p.vol_menu.clone(),
            sub: p.sub_menu.clone(),
            speed: p.speed_mbtn.clone(),
            main: p.menu_btn.clone(),
            win: p.win.clone(),
            root: p.root.clone(),
            header: p.header.clone(),
            gl: p.gl.clone(),
            bar_show: p.bar_show.clone(),
            recent: p.recent.clone(),
            bottom: p.bottom.clone(),
            player: p.player.clone(),
            squelch: p.motion_squelch.clone(),
            seek_grabbed: p.seek_grabbed.clone(),
            hdr_csd_baseline: Rc::clone(&hdr_csd_baseline),
        });
        #[cfg(target_os = "macos")]
        {
            let chc = Rc::clone(&ch_hide);
            let chc_pop = Rc::clone(&chc);
            crate::macos_header_menu::register_checks(
                Rc::new(move || {
                    chc.vol.is_active()
                        || chc.sub.is_active()
                        || chc.speed.is_active()
                        || chc.vol.popover().is_some_and(|p| p.is_visible())
                        || chc.sub.popover().is_some_and(|p| p.is_visible())
                        || chc.speed.popover().is_some_and(|p| p.is_visible())
                        || crate::macos_header_menu_overlay::overlay_visible()
                }),
                Rc::new(move || {
                    chc_pop.vol.popover().is_some_and(|p| p.is_visible())
                        || chc_pop.sub.popover().is_some_and(|p| p.is_visible())
                        || chc_pop.speed.popover().is_some_and(|p| p.is_visible())
                }),
            );
        }
        let on_show: Rc<dyn Fn()> = {
            let (csp, root, gl, b, recent, bot, player, hdr, chh, win_ov) = (
                Rc::clone(&hdr_csd_baseline),
                p.root.clone(),
                p.gl.clone(),
                p.bar_show.clone(),
                p.recent.clone(),
                p.bottom.clone(),
                p.player.clone(),
                p.header.clone(),
                Rc::clone(&ch_hide),
                p.win.clone(),
            );
            Rc::new(move || {
                b.set(true);
                apply_chrome(ChromeApplyParts {
                    hdr_csd_baseline: &csp,
                    root: &root,
                    header: &hdr,
                    gl: &gl,
                    bar_show: &b,
                    recent: &recent,
                    bottom: &bot,
                    player: &player,
                });
                schedule_bars_autohide(Rc::clone(&chh));
                show_chrome_pointer(&win_ov, &gl);
            })
        };
        wire_shell_layout_chrome(Rc::clone(&on_show));
        wire_menu_chrome(
            Rc::clone(&ch_hide),
            p.vol_menu,
            p.sub_menu,
            p.speed_mbtn,
            p.menu_btn,
        );
        Self {
            hdr_csd_baseline,
            ch_hide,
            on_show,
        }
    }
}
