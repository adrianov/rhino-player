fn set_toolbar_reveal(root: &adw::ToolbarView, show: bool) -> bool {
    #[cfg(target_os = "macos")]
    let show = show && !crate::macos_fs_exit::exit_armed();
    let changed = root.reveals_top_bars() != show || root.reveals_bottom_bars() != show;
    root.set_reveal_top_bars(show);
    root.set_reveal_bottom_bars(show);
    changed
}
