struct BrowseChromeRefs {
    hdr_csd: Rc<Cell<Option<(bool, bool)>>>,
    nav_t: Rc<RefCell<Option<glib::SourceId>>>,
    win: adw::ApplicationWindow,
    root: adw::ToolbarView,
    gl: gtk::GLArea,
    bar_show: Rc<Cell<bool>>,
    recent: gtk::Box,
    bottom: gtk::Box,
    player: Rc<RefCell<Option<MpvBundle>>>,
    header: adw::HeaderBar,
}

/// Chrome callback when returning to **Browse** after playback (Escape strip); cancels toolbar auto-hide idle.
fn rc_on_browse_chrome(parts: BrowseChromeRefs) -> Rc<dyn Fn()> {
    let BrowseChromeRefs {
        hdr_csd,
        nav_t,
        win,
        root,
        gl,
        bar_show,
        recent,
        bottom,
        player,
        header,
    } = parts;
    Rc::new(move || {
        drop_glib_source(nav_t.as_ref());
        bar_show.set(true);
        apply_chrome(ChromeApplyParts {
            hdr_csd_baseline: &hdr_csd,
            root: &root,
            header: &header,
            gl: &gl,
            bar_show: &bar_show,
            recent: &recent,
            bottom: &bottom,
            player: &player,
        });
        show_chrome_pointer(&win, &gl);
    })
}
