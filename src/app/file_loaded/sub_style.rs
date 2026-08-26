struct SubStyleCtx {
    player: Rc<RefCell<Option<MpvBundle>>>,
    sub_pref: Rc<RefCell<db::SubPrefs>>,
    gl: gtk::GLArea,
    bar_show: Rc<Cell<bool>>,
    recent: gtk::Box,
    bottom: gtk::Box,
    sub_scale_adj: gtk::Adjustment,
    sub_color_btn: gtk::ColorDialogButton,
}

fn wire_sub_style_controls(ctx: SubStyleCtx) {
    let SubStyleCtx {
        player,
        sub_pref,
        gl: gl_area,
        bar_show,
        recent,
        bottom,
        sub_scale_adj,
        sub_color_btn,
    } = ctx;
    wire_sub_scale(&sub_scale_adj, &player, &sub_pref, &gl_area);
    wire_sub_color(
        &sub_color_btn,
        &player,
        &sub_pref,
        &gl_area,
        &bar_show,
        &recent,
        &bottom,
    );
}

fn wire_sub_scale(
    adj: &gtk::Adjustment,
    p: &Rc<RefCell<Option<MpvBundle>>>,
    sp: &Rc<RefCell<db::SubPrefs>>,
    gll: &gtk::GLArea,
) {
    let p = p.clone();
    let sp = sp.clone();
    let gll = gll.clone();
    let adj_h = adj.clone();
    adj.connect_value_changed(move |_| {
        let v = adj_h.value();
        sp.borrow_mut().scale = v;
        apply_sub_prefs_and_redraw(&p, &sp, &gll);
    });
}

fn wire_sub_color(
    btn: &gtk::ColorDialogButton,
    p: &Rc<RefCell<Option<MpvBundle>>>,
    sp: &Rc<RefCell<db::SubPrefs>>,
    gll: &gtk::GLArea,
    bshow: &Rc<Cell<bool>>,
    rec: &gtk::Box,
    bot: &gtk::Box,
) {
    let p = p.clone();
    let sp = sp.clone();
    let gll = gll.clone();
    let bshow = bshow.clone();
    let rec = rec.clone();
    let bot = bot.clone();
    let btn_h = btn.clone();
    btn.connect_rgba_notify(move |_| {
        sp.borrow_mut().color = sub_prefs::rgba_to_u32(&btn_h.rgba());
        apply_sub_prefs_with_pos(&p, &sp, &gll, &bshow, &rec, &bot);
    });
}

/// Re-applies subtitle prefs; scale changes do not need a toolbar reposition.
fn apply_sub_prefs_and_redraw(
    p: &Rc<RefCell<Option<MpvBundle>>>,
    sp: &Rc<RefCell<db::SubPrefs>>,
    gll: &gtk::GLArea,
) {
    if let Some(b) = p.borrow().as_ref() {
        let pr = sp.borrow();
        sub_prefs::apply_mpv(&b.mpv, &pr);
    }
    db::save_sub(&sp.borrow());
    gll.queue_render();
}

fn apply_sub_prefs_with_pos(
    p: &Rc<RefCell<Option<MpvBundle>>>,
    sp: &Rc<RefCell<db::SubPrefs>>,
    gll: &gtk::GLArea,
    bshow: &Rc<Cell<bool>>,
    rec: &gtk::Box,
    bot: &gtk::Box,
) {
    if let Some(b) = p.borrow().as_ref() {
        let pr = sp.borrow();
        sub_prefs::apply_mpv(&b.mpv, &pr);
        let show = if rec.is_visible() { true } else { bshow.get() };
        sub_prefs::apply_sub_pos_for_toolbar(&b.mpv, show, bot.height(), gll.height());
    }
    db::save_sub(&sp.borrow());
    gll.queue_render();
}
