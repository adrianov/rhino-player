// Fullscreen header menu overlay: anchor panel under the pressed MenuButton.

const MENU_GAP_PX: i32 = 4;
const PANEL_MIN_W: i32 = 180;
const PANEL_MIN_H: i32 = 80;

const AUDIO_TRACKS_MAX_H: i32 = 480;
const SUB_TRACKS_MAX_H: i32 = 280;
const SPEED_LIST_MAX_H: i32 = 320;

pub(super) fn prep_fs_menu_layout(root: &adw::ToolbarView, header: &adw::HeaderBar, shell: &gtk::Overlay) {
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
    let min_w = scrl.min_content_width();
    let restore = match min_w {
        400 => AUDIO_TRACKS_MAX_H,
        360 => SUB_TRACKS_MAX_H,
        _ => SPEED_LIST_MAX_H,
    };
    scrl.set_max_content_height(restore);
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

pub(super) fn raise_panel_top(shell: &gtk::Overlay, panel: &gtk::Frame) {
    panel.unparent();
    shell.add_overlay(panel);
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

pub(super) fn place_panel_clamped(panel: &gtk::Frame, btn: &gtk::MenuButton, shell: &gtk::Overlay) {
    let Some((bx, by, btn_w, btn_h)) = btn_box_in_shell(btn, shell) else {
        return;
    };
    let (shell_w, shell_h) = shell_size(shell);
    let gap = f64::from(MENU_GAP_PX);
    let menu_top = by + btn_h + gap;
    let max_panel_h = ((shell_h - menu_top - gap).max(f64::from(PANEL_MIN_H))) as i32;
    if let Some(child) = panel.child() {
        cap_scrolled_heights(&child, max_panel_h.saturating_sub(24));
    }
    let (panel_w, panel_h) = panel_natural_size(panel, max_panel_h);
    let pw = f64::from(panel_w);
    let ph = f64::from(panel_h);

    let mut x = bx + btn_w - pw;
    let y = menu_top.clamp(0.0, (shell_h - ph - gap).max(0.0));
    x = x.clamp(0.0, (shell_w - pw).max(0.0));

    panel.set_halign(gtk::Align::Start);
    panel.set_valign(gtk::Align::Start);
    panel.set_hexpand(false);
    panel.set_vexpand(false);
    panel.set_size_request(panel_w, panel_h);
    panel.set_margin_start(x.round() as i32);
    panel.set_margin_top(y.round() as i32);
    panel.set_margin_end(0);
    panel.set_margin_bottom(0);
}
