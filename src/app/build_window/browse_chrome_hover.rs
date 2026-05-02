struct BrowseChromeRefs {
    hdr_csd: Rc<Cell<Option<(bool, bool)>>>,
    nav_t: Rc<RefCell<Option<glib::SourceId>>>,
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
        root,
        gl,
        bar_show,
        recent,
        bottom,
        player,
        header,
    } = parts;
    Rc::new(move || {
        if let Some(id) = nav_t.borrow_mut().take() {
            id.remove();
        }
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
    })
}
