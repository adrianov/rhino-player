/// Hover **Remove** / **Move to Trash** — shared flat circular chrome (`rp-recent-action`).
fn card_action_btn(icon: &str, tooltip: &str) -> gtk::Button {
    let btn = gtk::Button::from_icon_name(icon);
    btn.set_visible(false);
    btn.set_tooltip_text(Some(tooltip));
    btn.add_css_class("flat");
    btn.add_css_class("circular");
    btn.add_css_class("rp-recent-action");
    btn.set_cursor_from_name(Some("pointer"));
    btn
}

struct HistoryCardHandlers<'a> {
    on_open: Rc<dyn Fn(&Path)>,
    on_remove: Rc<dyn Fn(&Path)>,
    on_trash: Rc<dyn Fn(&Path)>,
    warm_hover: Option<&'a WarmHoverHooks>,
    kind: StripKind,
}

fn card_background(d: &CardData, miss: bool) -> gtk::Widget {
    if miss {
        return full_bleed_icon("image-missing-symbolic");
    }
    if let Some(ref bytes) = d.thumb {
        let key = crate::db::history_key(&d.path).unwrap_or_default();
        if let Some(tex) = crate::thumb_texture::texture_from_thumb_cached(&key, bytes.as_slice()) {
            return crate::thumb_texture::cover_picture(&tex).upcast();
        }
    }
    full_bleed_icon("camera-video-symbolic")
}

fn history_card_tooltip(c: &Path, a11y: &str, miss: bool) -> String {
    if miss {
        format!(
            "{}\n{} — file missing, click to remove from list",
            c.display(),
            a11y
        )
    } else {
        format!("{}\n{a11y}", c.display(), a11y = a11y)
    }
}

fn card_title_label(label_txt: &str, c: &Path) -> gtk::Label {
    let label = gtk::Label::new(Some(label_txt));
    no_target(&label);
    label.add_css_class("rp-recent-card-title");
    label.set_ellipsize(gtk::pango::EllipsizeMode::None);
    label.set_max_width_chars(-1);
    label.set_wrap(true);
    label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    label.set_natural_wrap_mode(gtk::NaturalWrapMode::Word);
    label.set_tooltip_text(c.to_str());
    label.set_halign(gtk::Align::Fill);
    label.set_hexpand(true);
    label.set_xalign(0.0);
    label
}

fn progress_row(p: f64) -> gtk::Box {
    let pro = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    no_target(&pro);
    pro.add_css_class("rp-recent-progress-row");
    pro.set_hexpand(true);
    let bar = progress_bar(p);
    let lp = progress_percent_label(p);
    pro.append(&bar);
    pro.append(&lp);
    pro
}

fn progress_bar(p: f64) -> gtk::ProgressBar {
    let bar = gtk::ProgressBar::new();
    no_target(&bar);
    bar.set_fraction(p / 100.0);
    bar.set_show_text(false);
    bar.set_hexpand(true);
    bar.set_hexpand_set(true);
    bar.add_css_class("rp-recent-bar");
    bar
}

fn progress_percent_label(p: f64) -> gtk::Label {
    let lp = gtk::Label::new(Some(&format!("{p:.0}%")));
    no_target(&lp);
    lp.add_css_class("rp-recent-percent");
    lp.set_hexpand(false);
    lp
}
