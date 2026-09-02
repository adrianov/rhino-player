// Continue-card pointer interaction: primary-click activation plus the hover wiring that
// reveals Remove / Move to Trash and drives warm preload. Split out of
// `backfill_context_schedule.rs` (backfill scheduling owns the rest).

/// Hand on hover, primary click triggers [act]. [show_on_hover] (Remove / Trash / quality) shows on hover.
/// Uses [PropagationPhase::Target] so nested [gtk::Button]s receive the click first.
fn add_click_and_pointer(
    card: &impl IsA<gtk::Widget>,
    path: &Path,
    act: UnitFn,
    show_on_hover: &[gtk::Widget],
    warm_hover: Option<&WarmHoverHooks>,
) {
    attach_click_gesture(card, act);
    attach_hover_pointer(card, path, show_on_hover, warm_hover);
}

fn attach_click_gesture(card: &impl IsA<gtk::Widget>, act: UnitFn) {
    card.as_ref().set_can_target(true);
    let g = gtk::GestureClick::new();
    g.set_button(1);
    g.set_propagation_phase(gtk::PropagationPhase::Target);
    let act = act.clone();
    g.connect_pressed(move |_, n, _x, _y| {
        if n == 1 {
            act(());
        }
    });
    card.as_ref().add_controller(g);
}

/// Pointer cursor while hovering; reveals [show_on_hover] buttons and fires warm hooks.
fn attach_hover_pointer(
    card: &impl IsA<gtk::Widget>,
    path: &Path,
    show_on_hover: &[gtk::Widget],
    warm_hover: Option<&WarmHoverHooks>,
) {
    let m = gtk::EventControllerMotion::new();
    wire_hover_enter(&m, card, path, show_on_hover, warm_hover);
    wire_hover_leave(&m, card, show_on_hover, warm_hover);
    card.as_ref().add_controller(m);
}

fn wire_hover_enter(
    m: &gtk::EventControllerMotion,
    card: &impl IsA<gtk::Widget>,
    path: &Path,
    show_on_hover: &[gtk::Widget],
    warm_hover: Option<&WarmHoverHooks>,
) {
    let c = card.as_ref().clone();
    let show: Vec<gtk::Widget> = show_on_hover.to_vec();
    let warm_enter = warm_hover.map(|h| h.enter.clone());
    let warm_path = path.to_path_buf();
    m.connect_enter(move |_, _x, _y| hover_enter(&c, &show, warm_enter.as_ref(), &warm_path));
}

fn wire_hover_leave(
    m: &gtk::EventControllerMotion,
    card: &impl IsA<gtk::Widget>,
    show_on_hover: &[gtk::Widget],
    warm_hover: Option<&WarmHoverHooks>,
) {
    let c = card.as_ref().clone();
    let hide: Vec<gtk::Widget> = show_on_hover.to_vec();
    let warm_leave = warm_hover.map(|h| h.leave.clone());
    m.connect_leave(move |_| hover_leave(&c, &hide, warm_leave.as_ref()));
}

/// Enter the card: pointer cursor, reveal hover actions, fire the warm-preload hook.
fn hover_enter(c: &gtk::Widget, show: &[gtk::Widget], warm_enter: Option<&RcPathFn>, path: &Path) {
    c.set_cursor_from_name(Some("pointer"));
    for b in show {
        b.set_visible(true);
    }
    if let Some(f) = warm_enter {
        f(path);
    }
}

/// Leave the card: reset cursor, hide hover actions, end warm preload.
fn hover_leave(c: &gtk::Widget, hide: &[gtk::Widget], warm_leave: Option<&WarmHoverLeave>) {
    c.set_cursor_from_name(None);
    for b in hide {
        b.set_visible(false);
    }
    if let Some(f) = warm_leave {
        f();
    }
}
