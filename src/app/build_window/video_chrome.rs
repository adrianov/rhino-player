include!("video_chrome_macos.rs");

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

fn attach_shell_and_layout(p: &VideoChromeParts<'_>) {
    attach_window_shell(&new_window_input_shell(p));
    let shell_layout = Rc::new(new_shell_layout_ctx(p));
    register_shell_layout(Rc::clone(&shell_layout));
    #[cfg(target_os = "macos")]
    {
        wire_macos_recent_hide_refresh(p.win, p.gl, p.recent, p.player);
        wire_macos_surface_compositing_refresh(&shell_layout);
    }
}

fn new_window_input_shell(p: &VideoChromeParts<'_>) -> WindowInputShell {
    WindowInputShell {
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
    }
}

fn new_shell_layout_ctx(p: &VideoChromeParts<'_>) -> ShellLayoutCtx {
    ShellLayoutCtx {
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
    }
}
impl VideoChrome {
    fn attach(p: VideoChromeParts<'_>) -> Self {
        attach_shell_and_layout(&p);

        let hdr_csd_baseline = Rc::new(Cell::new(None));
        wire_header_csd_baseline_snap(&hdr_csd_baseline, p.header);

        let ch_hide = Rc::new(Self::bar_hide(&p, &hdr_csd_baseline));
        #[cfg(target_os = "macos")]
        register_macos_menu_checks(&ch_hide);
        let on_show: Rc<dyn Fn()> = Self::make_on_show(&p, &hdr_csd_baseline, &ch_hide);
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

    fn bar_hide(
        p: &VideoChromeParts<'_>,
        hdr_csd_baseline: &Rc<Cell<Option<(bool, bool)>>>,
    ) -> ChromeBarHide {
        ChromeBarHide {
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
            hdr_csd_baseline: Rc::clone(hdr_csd_baseline),
        }
    }

    fn make_on_show(
        p: &VideoChromeParts<'_>,
        csp: &Rc<Cell<Option<(bool, bool)>>>,
        chh: &Rc<ChromeBarHide>,
    ) -> Rc<dyn Fn()> {
        let c = ChromeShowCtx {
            csp: Rc::clone(csp),
            chh: Rc::clone(chh),
            root: p.root.clone(),
            hdr: p.header.clone(),
            gl: p.gl.clone(),
            b: p.bar_show.clone(),
            recent: p.recent.clone(),
            bot: p.bottom.clone(),
            player: p.player.clone(),
            win_ov: p.win.clone(),
        };
        Rc::new(move || run_chrome_show(&c))
    }
}

/// Cloned widget refs for one chrome reveal.
struct ChromeShowCtx {
    csp: Rc<Cell<Option<(bool, bool)>>>,
    chh: Rc<ChromeBarHide>,
    root: adw::ToolbarView,
    hdr: adw::HeaderBar,
    gl: gtk::GLArea,
    b: Rc<Cell<bool>>,
    recent: gtk::Box,
    bot: gtk::Box,
    player: Rc<RefCell<Option<MpvBundle>>>,
    win_ov: adw::ApplicationWindow,
}

/// One chrome reveal: show bars, refresh autohide timer, and make the pointer visible.
fn run_chrome_show(c: &ChromeShowCtx) {
    c.b.set(true);
    apply_chrome(ChromeApplyParts {
        hdr_csd_baseline: &c.csp,
        root: &c.root,
        header: &c.hdr,
        gl: &c.gl,
        bar_show: &c.b,
        recent: &c.recent,
        bottom: &c.bot,
        player: &c.player,
    });
    schedule_bars_autohide(Rc::clone(&c.chh));
    show_chrome_pointer(&c.win_ov, &c.gl);
}
