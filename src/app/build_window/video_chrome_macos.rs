/// Registers the macOS header-menu open checks: any menu active, or any popover visible.
#[cfg(target_os = "macos")]
fn register_macos_menu_checks(chc: &Rc<ChromeBarHide>) {
    crate::macos_header_menu::register_checks(
        Rc::new(macos_any_menu_active(Rc::clone(chc))),
        Rc::new(macos_popovers_visible(Rc::clone(chc))),
    );
}

#[cfg(target_os = "macos")]
fn macos_any_menu_active(chc: Rc<ChromeBarHide>) -> impl Fn() -> bool + 'static {
    move || {
        chc.vol.is_active()
            || chc.sub.is_active()
            || chc.speed.is_active()
            || chc.vol.popover().is_some_and(|p| p.is_visible())
            || chc.sub.popover().is_some_and(|p| p.is_visible())
            || chc.speed.popover().is_some_and(|p| p.is_visible())
            || crate::macos_header_menu_overlay::overlay_visible()
    }
}

#[cfg(target_os = "macos")]
fn macos_popovers_visible(chc: Rc<ChromeBarHide>) -> impl Fn() -> bool + 'static {
    move || {
        chc.vol.popover().is_some_and(|p| p.is_visible())
            || chc.sub.popover().is_some_and(|p| p.is_visible())
            || chc.speed.popover().is_some_and(|p| p.is_visible())
    }
}
