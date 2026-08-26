/// Transient notice under the continue strip (open failures, empty/incomplete media).
#[derive(Clone)]
pub struct NoticeToast {
    pub shell: gtk::Box,
    pub label: gtk::Label,
    pub close: gtk::Button,
}

fn new_notice_toast() -> NoticeToast {
    let label = toast_label();

    let close = dismiss_button();

    let bar = toast_bar(&label, &close);
    let shell = toast_shell(&bar, &["rp-notice-shell"]);

    NoticeToast {
        shell,
        label,
        close,
    }
}

/// Horizontal pill row holding the message label and dismiss button.
fn toast_bar(label: &gtk::Label, close: &gtk::Button) -> gtk::Box {
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    bar.set_spacing(6);
    bar.set_halign(gtk::Align::Center);
    bar.set_valign(gtk::Align::Center);
    bar.append(label);
    bar.append(close);
    bar.add_css_class("rp-undo-toast");
    bar.add_css_class("rp-notice-toast");
    bar
}

/// Hidden-by-default vertical band under the continue strip; [css_classes] extend
/// `rp-undo-shell`.
fn toast_shell(bar: &gtk::Box, css_classes: &[&str]) -> gtk::Box {
    let shell = gtk::Box::new(gtk::Orientation::Vertical, 0);
    shell.set_hexpand(true);
    shell.set_halign(gtk::Align::Fill);
    shell.set_valign(gtk::Align::Start);
    shell.set_vexpand(false);
    shell.set_visible(false);
    shell.set_margin_top(4);
    shell.set_margin_start(16);
    shell.set_margin_end(16);
    shell.add_css_class("rp-undo-shell");
    for c in css_classes {
        shell.add_css_class(c);
    }
    shell.append(bar);
    shell
}

fn toast_label() -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    label.set_max_width_chars(64);
    label.set_xalign(0.0);
    label.set_halign(gtk::Align::Start);
    label.set_valign(gtk::Align::Center);
    label.set_wrap(false);
    label.set_single_line_mode(true);
    label.set_hexpand(true);
    label.add_css_class("rp-undo-toast-text");
    label.add_css_class("rp-notice-toast-text");
    label
}

fn dismiss_button() -> gtk::Button {
    let close = gtk::Button::from_icon_name("window-close-symbolic");
    close.set_valign(gtk::Align::Center);
    close.set_halign(gtk::Align::Center);
    close.set_tooltip_text(Some("Dismiss"));
    close.add_css_class("circular");
    close.add_css_class("flat");
    close.add_css_class("rp-undo-toast-close");
    close.set_cursor_from_name(Some("pointer"));
    close
}

/// Show / auto-dismiss controller for [NoticeToast].
pub struct NoticeToastCtrl {
    toast: NoticeToast,
    timer: Rc<RefCell<Option<glib::SourceId>>>,
}

impl NoticeToastCtrl {
    pub fn new(toast: NoticeToast) -> Rc<Self> {
        let timer = Rc::new(RefCell::new(None));
        let ctrl = Rc::new(Self {
            toast,
            timer: Rc::clone(&timer),
        });
        {
            let c = Rc::clone(&ctrl);
            ctrl.toast.close.connect_clicked(move |_| c.dismiss());
        }
        ctrl
    }

    pub fn show(&self, message: &str) {
        self.cancel_timer();
        self.toast.label.set_label(message);
        self.toast.label.set_tooltip_text(Some(message));
        self.toast.shell.set_visible(true);
        let shell = self.toast.shell.clone();
        let slot = Rc::clone(&self.timer);
        *self.timer.borrow_mut() =
            Some(glib::timeout_add_local(Duration::from_secs(6), move || {
                crate::glib_source_drop::finish_glib_source(slot.as_ref());
                shell.set_visible(false);
                glib::ControlFlow::Break
            }));
    }

    pub fn dismiss(&self) {
        self.cancel_timer();
        self.toast.shell.set_visible(false);
        self.toast.label.set_label("");
        self.toast.label.set_tooltip_text(None);
    }

    fn cancel_timer(&self) {
        crate::glib_source_drop::drop_glib_source(&self.timer);
    }
}
