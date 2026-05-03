fn set_toolbar_reveal(root: &adw::ToolbarView, show: bool) -> bool {
    let changed = root.reveals_top_bars() != show || root.reveals_bottom_bars() != show;
    root.set_reveal_top_bars(show);
    root.set_reveal_bottom_bars(show);
    changed
}
