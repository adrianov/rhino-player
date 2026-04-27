#[derive(Clone)]
struct SiblingNavUi {
    prev_btn: gtk::Button,
    next_btn: gtk::Button,
    prev_wrap: gtk::Box,
    next_wrap: gtk::Box,
    prev_tip: Rc<RefCell<String>>,
    next_tip: Rc<RefCell<String>>,
}

impl SiblingNavUi {
    fn new(
        prev_btn: &gtk::Button,
        next_btn: &gtk::Button,
        prev_wrap: &gtk::Box,
        next_wrap: &gtk::Box,
    ) -> Self {
        let ui = Self {
            prev_btn: prev_btn.clone(),
            next_btn: next_btn.clone(),
            prev_wrap: prev_wrap.clone(),
            next_wrap: next_wrap.clone(),
            prev_tip: Rc::new(RefCell::new(String::new())),
            next_tip: Rc::new(RefCell::new(String::new())),
        };
        ui.set_no_media();
        ui
    }

    fn refresh(&self, cur: Option<&Path>, seof: &SiblingEofState) {
        let (cur, can_prev, can_next) = if let Some(c) = cur.filter(|p| p.is_file()) {
            let (prev, next) = seof.nav_sensitivity(c);
            (Some(c), prev, next)
        } else {
            seof.clear_nav_sensitivity();
            (None, false, false)
        };
        self.sync_prev(can_prev, sibling_bar_tooltip(true, can_prev, cur));
        self.sync_next(can_next, sibling_bar_tooltip(false, can_next, cur));
    }

    fn set_no_media(&self) {
        self.sync_prev(false, "No media".to_string());
        self.sync_next(false, "No media".to_string());
    }

    fn sync_prev(&self, can_skip: bool, tip: String) {
        sync_nav_button(&self.prev_btn, &self.prev_wrap, &self.prev_tip, can_skip, tip.as_str());
    }

    fn sync_next(&self, can_skip: bool, tip: String) {
        sync_nav_button(&self.next_btn, &self.next_wrap, &self.next_tip, can_skip, tip.as_str());
    }
}

fn sync_nav_button(
    button: &gtk::Button,
    wrapper: &gtk::Box,
    tip_state: &Rc<RefCell<String>>,
    can_skip: bool,
    tip: &str,
) {
    if tip_state.borrow().as_str() != tip {
        *tip_state.borrow_mut() = tip.to_string();
        button.set_tooltip_text(Some(tip));
        wrapper.set_tooltip_text(Some(tip));
    }
    wrapper.set_can_target(true);
    button.set_sensitive(can_skip);
    button.set_can_target(can_skip);
}
