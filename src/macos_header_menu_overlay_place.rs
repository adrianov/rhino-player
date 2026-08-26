// Fullscreen header menu overlay: anchor panel under the pressed MenuButton.

const MENU_GAP_PX: i32 = 4;
const PANEL_MIN_W: i32 = 180;
const PANEL_MIN_H: i32 = 80;

pub(super) fn prep_fs_menu_layout(
    root: &adw::ToolbarView,
    header: &adw::HeaderBar,
    shell: &gtk::Overlay,
) {
    root.set_reveal_top_bars(true);
    header.queue_allocate();
    root.queue_allocate();
    shell.queue_allocate();
}

pub(super) fn prep_overlay_child(child: &gtk::Widget) {
    child.set_vexpand(false);
    child.set_hexpand(false);
}

fn shell_size(shell: &gtk::Overlay) -> (f64, f64) {
    (
        f64::from(shell.width().max(1)),
        f64::from(shell.height().max(1)),
    )
}

fn widget_origin_in_shell(w: &impl IsA<gtk::Widget>, shell: &gtk::Overlay) -> Option<(f64, f64)> {
    let origin = gtk::graphene::Point::new(0.0, 0.0);
    let pt = w.compute_point(&shell.clone().upcast::<gtk::Widget>(), &origin)?;
    Some((f64::from(pt.x()), f64::from(pt.y())))
}

fn btn_box_in_shell(btn: &gtk::MenuButton, shell: &gtk::Overlay) -> Option<(f64, f64, f64, f64)> {
    let (x, y) = widget_origin_in_shell(btn, shell)?;
    Some((x, y, f64::from(btn.width()), f64::from(btn.height())))
}

fn panel_natural_size(panel: &gtk::Frame, max_h: i32) -> (i32, i32) {
    let Some(child) = panel.child() else {
        return (
            panel.width().max(PANEL_MIN_W),
            panel.height().max(PANEL_MIN_H).min(max_h),
        );
    };
    let (_, nat_w, _, _) = child.measure(gtk::Orientation::Horizontal, -1);
    let (_, nat_h, _, _) = child.measure(gtk::Orientation::Vertical, nat_w);
    (nat_w.max(PANEL_MIN_W), nat_h.max(PANEL_MIN_H).min(max_h))
}

fn cap_scrolled_heights(w: &gtk::Widget, max_h: i32) {
    if let Ok(scrl) = w.clone().downcast::<gtk::ScrolledWindow>() {
        scrl.set_max_content_height(max_h.max(PANEL_MIN_H));
        return;
    }
    let mut child = w.first_child();
    while let Some(c) = child {
        cap_scrolled_heights(&c, max_h);
        child = c.next_sibling();
    }
}

fn restore_scrolled_max(scrl: &gtk::ScrolledWindow) {
    scrl.set_max_content_height(crate::header_menu_scroll::max_content_height_for(scrl));
}

pub(super) fn reset_scrolled_heights(w: &gtk::Widget) {
    if let Ok(scrl) = w.clone().downcast::<gtk::ScrolledWindow>() {
        restore_scrolled_max(&scrl);
        return;
    }
    let mut child = w.first_child();
    while let Some(c) = child {
        reset_scrolled_heights(&c);
        child = c.next_sibling();
    }
}

pub(super) fn enable_target_tree(w: &gtk::Widget) {
    w.set_can_target(true);
    let mut child = w.first_child();
    while let Some(c) = child {
        enable_target_tree(&c);
        child = c.next_sibling();
    }
}

pub(crate) fn raise_overlay_top(shell: &gtk::Overlay, w: &impl IsA<gtk::Widget>) {
    w.unparent();
    shell.add_overlay(w);
}

pub(super) fn raise_panel_top(shell: &gtk::Overlay, panel: &gtk::Frame) {
    raise_overlay_top(shell, panel);
}

pub(super) fn show_panel(panel: &gtk::Frame, shell: &gtk::Overlay) {
    panel.set_can_target(true);
    raise_panel_top(shell, panel);
    panel.set_visible(true);
}

pub(super) fn hide_panel_widget(panel: &gtk::Frame) {
    panel.set_visible(false);
    panel.set_can_target(false);
}

/// Largest height the panel may take below the pressed button.
fn max_panel_height(menu_top: f64, shell_h: f64) -> i32 {
    let gap = f64::from(MENU_GAP_PX);
    ((shell_h - menu_top - gap).max(f64::from(PANEL_MIN_H))) as i32
}

/// Cap scrolled children to fit, then measure the panel's natural size.
fn fit_panel(panel: &gtk::Frame, max_h: i32) -> (i32, i32) {
    if let Some(child) = panel.child() {
        cap_scrolled_heights(&child, max_h.saturating_sub(24));
    }
    panel_natural_size(panel, max_h)
}

/// Vertical margin under the button, clamped to the shell.
fn clamped_top(menu_top: f64, ph: f64, shell_h: f64) -> f64 {
    let gap = f64::from(MENU_GAP_PX);
    menu_top.clamp(0.0, (shell_h - ph - gap).max(0.0))
}

/// Top-left margins placing the panel under the button box, clamped inside the shell.
/// `btn_box` = (x, y, w, h) of the button; `panel_px` = (w, h); `shell_px` = (w, h).
fn clamped_origin(
    btn_box: (f64, f64, f64, f64),
    panel_px: (i32, i32),
    shell_px: (f64, f64),
) -> (i32, i32) {
    let gap = f64::from(MENU_GAP_PX);
    let menu_top = btn_box.1 + btn_box.3 + gap;
    let y = clamped_top(menu_top, f64::from(panel_px.1), shell_px.1);
    let pw = f64::from(panel_px.0);
    let x = (btn_box.0 + btn_box.2 - pw).clamp(0.0, (shell_px.0 - pw).max(0.0));
    (x.round() as i32, y.round() as i32)
}

fn apply_panel_placement(panel: &gtk::Frame, w: i32, h: i32, x: i32, y: i32) {
    panel.set_halign(gtk::Align::Start);
    panel.set_valign(gtk::Align::Start);
    panel.set_hexpand(false);
    panel.set_vexpand(false);
    panel.set_size_request(w, h);
    panel.set_margin_start(x);
    panel.set_margin_top(y);
    panel.set_margin_end(0);
    panel.set_margin_bottom(0);
}

pub(super) fn place_panel_clamped(panel: &gtk::Frame, btn: &gtk::MenuButton, shell: &gtk::Overlay) {
    let Some(btn_box) = btn_box_in_shell(btn, shell) else {
        return;
    };
    let shell_px = shell_size(shell);
    let menu_top = btn_box.1 + btn_box.3 + f64::from(MENU_GAP_PX);
    let max_h = max_panel_height(menu_top, shell_px.1);
    let panel_px = fit_panel(panel, max_h);
    let (x, y) = clamped_origin(btn_box, panel_px, shell_px);
    apply_panel_placement(panel, panel_px.0, panel_px.1, x, y);
}
