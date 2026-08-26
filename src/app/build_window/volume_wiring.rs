/// Wires the volume slider, mute toggle, and the wheel-on-video volume nudge.
///
/// All three handlers read mpv as the source of truth and use [VolumeCtx::vol_sync]
/// as a single re-entrancy guard so a programmatic property write does not retrigger
/// the GTK callback that just made the write.
struct VolumeCtx {
    player: Rc<RefCell<Option<MpvBundle>>>,
    recent: gtk::Box,
    gl: gtk::GLArea,
    vol_header_img: gtk::Image,
    vol_readout: gtk::Label,
    vol_adj: gtk::Adjustment,
    vol_mute_btn: gtk::ToggleButton,
    vol_sync: Rc<Cell<bool>>,
}

fn wire_volume_controls(ctx: VolumeCtx) {
    wire_vol_adj(&ctx);
    wire_mute_toggle(&ctx);
    wire_wheel_volume(&ctx);
}

fn wire_vol_adj(ctx: &VolumeCtx) {
    let p = ctx.player.clone();
    let vi = ctx.vol_header_img.clone();
    let vr = ctx.vol_readout.clone();
    let vm = ctx.vol_mute_btn.clone();
    let vsx = ctx.vol_sync.clone();
    ctx.vol_adj.connect_value_changed(move |a| {
        if vsx.get() {
            return;
        }
        let g = p.borrow();
        let Some(b) = g.as_ref() else {
            return;
        };
        push_adj_volume(b, a, &vi, &vr, &vm, &vsx);
    });
}

fn push_adj_volume(
    b: &MpvBundle,
    a: &gtk::Adjustment,
    vi: &gtk::Image,
    vr: &gtk::Label,
    vm: &gtk::ToggleButton,
    vsx: &Rc<Cell<bool>>,
) {
    let v = a.value();
    let vmax = a.upper();
    let _ = b.mpv.set_property("volume", v);
    if v > 0.5 {
        let _ = b.mpv.set_property("mute", false);
    }
    let m = b.mpv.get_property::<bool>("mute").unwrap_or(false);
    let cur = b.mpv.get_property::<f64>("volume").unwrap_or(v);
    vi.set_icon_name(Some(vol_icon(m, cur)));
    stamp_vol_percent_readout(vr, cur, vmax);
    sync_mute_button(vm, m, vsx);
}

/// Mirrors the mute state onto the toolbar toggle under the re-entrancy guard.
fn sync_mute_button(vm: &gtk::ToggleButton, m: bool, vsx: &Rc<Cell<bool>>) {
    vsx.set(true);
    if vm.is_active() != m {
        vm.set_active(m);
    }
    paint_mute_button(vm, m);
    vsx.set(false);
}

/// Icon + tooltip shared by the adjustment and toggle paths.
fn paint_mute_button(btn: &gtk::ToggleButton, m: bool) {
    btn.set_icon_name(vol_mute_pop_icon(m));
    btn.set_tooltip_text(Some(if m { "Unmute" } else { "Mute" }));
}

fn wire_mute_toggle(ctx: &VolumeCtx) {
    let p = ctx.player.clone();
    let vi = ctx.vol_header_img.clone();
    let vr = ctx.vol_readout.clone();
    let vad = ctx.vol_adj.clone();
    let vsx = ctx.vol_sync.clone();
    ctx.vol_mute_btn.connect_toggled(move |ch| {
        if vsx.get() {
            return;
        }
        if let Some(b) = p.borrow().as_ref() {
            apply_mute_state(b, ch, &vi, &vr, &vad);
        }
    });
}

fn apply_mute_state(
    b: &MpvBundle,
    ch: &gtk::ToggleButton,
    vi: &gtk::Image,
    vr: &gtk::Label,
    vad: &gtk::Adjustment,
) {
    let m = ch.is_active();
    let _ = b.mpv.set_property("mute", m);
    let vol = b.mpv.get_property::<f64>("volume").unwrap_or(0.0);
    vi.set_icon_name(Some(vol_icon(m, vol)));
    stamp_vol_percent_readout(vr, vol, vad.upper());
    paint_mute_button(ch, m);
}

fn wire_wheel_volume(ctx: &VolumeCtx) {
    let p = ctx.player.clone();
    let r = ctx.recent.clone();
    let vmi = ctx.vol_header_img.clone();
    let vr = ctx.vol_readout.clone();
    let vad = ctx.vol_adj.clone();
    let sc = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
    sc.connect_scroll(move |_, _dx, dy| {
        if r.is_visible() {
            return glib::Propagation::Proceed;
        }
        let g = p.borrow();
        let Some(b) = g.as_ref() else {
            return glib::Propagation::Proceed;
        };
        wheel_nudge_volume(b, dy, &vmi, &vr, &vad)
    });
    ctx.gl.add_controller(sc);
}

fn wheel_nudge_volume(
    b: &MpvBundle,
    dy: f64,
    vmi: &gtk::Image,
    vr: &gtk::Label,
    vad: &gtk::Adjustment,
) -> glib::Propagation {
    let step = if dy.abs() < 0.5 { -dy * 4.0 } else { -dy * 5.0 };
    nudge_mpv_volume(&b.mpv, step);
    let vol = b.mpv.get_property::<f64>("volume").unwrap_or(0.0);
    let m = b.mpv.get_property::<bool>("mute").unwrap_or(false);
    vmi.set_icon_name(Some(vol_icon(m, vol)));
    stamp_vol_percent_readout(vr, vol, vad.upper());
    glib::Propagation::Stop
}
