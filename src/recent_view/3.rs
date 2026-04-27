
/// Replace all children with cards. [on_remove] drops an entry from the list; [on_trash] moves a file
/// to the system Trash then removes it from the list.
pub fn fill_row(
    row: &gtk::Box,
    items: Vec<CardData>,
    on_open: Rc<dyn Fn(&Path)>,
    on_remove: Rc<dyn Fn(&Path)>,
    on_trash: Rc<dyn Fn(&Path)>,
) {
    clear(row);
    let cards = Rc::new(RefCell::new(Vec::<gtk::Overlay>::new()));
    for d in items {
        let c = d.path.clone();
        let miss = d.missing;
        let p = d.percent;
        let name = c
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let a11y = format!("{name}, {p:.0} percent played");

        let card = gtk::Overlay::new();
        card.set_vexpand(false);
        card.set_hexpand(false);
        card.set_overflow(gtk::Overflow::Hidden);
        card.add_css_class("rp-recent-card");
        if miss {
            card.add_css_class("rp-stale");
        }
        let tip = if miss {
            format!(
                "{}\n{} — file missing, click to remove from recent",
                c.display(),
                a11y
            )
        } else {
            format!("{}\n{a11y}", c.display(), a11y = a11y)
        };
        card.set_tooltip_text(Some(tip.as_str()));

        let bg: gtk::Widget = if miss {
            full_bleed_icon("image-missing-symbolic")
        } else if let Some(ref bytes) = d.thumb {
            if let Some(tex) = crate::jpeg_texture::texture_from_jpeg(bytes.as_slice()) {
                let pic = gtk::Picture::for_paintable(&tex);
                pic.set_content_fit(gtk::ContentFit::Cover);
                pic.set_can_shrink(true);
                pic.set_vexpand(true);
                pic.set_hexpand(true);
                pic.set_halign(gtk::Align::Fill);
                pic.set_valign(gtk::Align::Fill);
                no_target(&pic);
                pic.add_css_class("rp-recent-bg");
                pic.upcast()
            } else {
                full_bleed_icon("video-x-generic")
            }
        } else {
            full_bleed_icon("video-x-generic")
        };
        card.set_child(Some(&bg));

        let footer = gtk::Box::new(gtk::Orientation::Vertical, 6);
        footer.set_halign(gtk::Align::Fill);
        footer.set_valign(gtk::Align::End);
        no_target(&footer);
        footer.add_css_class("rp-recent-card-footer");

        let label = gtk::Label::new(Some(&name));
        no_target(&label);
        label.add_css_class("rp-recent-card-title");
        label.set_ellipsize(gtk::pango::EllipsizeMode::None);
        label.set_max_width_chars(-1);
        label.set_wrap(true);
        label.set_natural_wrap_mode(gtk::NaturalWrapMode::Word);
        label.set_tooltip_text(c.to_str());
        label.set_halign(gtk::Align::Fill);
        label.set_xalign(0.0);

        let pro = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        no_target(&pro);
        pro.add_css_class("rp-recent-progress-row");
        let bar = gtk::ProgressBar::new();
        no_target(&bar);
        bar.set_fraction(p / 100.0);
        bar.set_show_text(false);
        bar.set_hexpand(true);
        bar.add_css_class("rp-recent-bar");
        let lp = gtk::Label::new(Some(&format!("{p:.0}%")));
        no_target(&lp);
        lp.add_css_class("rp-recent-percent");
        pro.append(&bar);
        pro.append(&lp);

        footer.append(&label);
        footer.append(&pro);
        card.add_overlay(&footer);

        let top_actions = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        top_actions.set_spacing(2);
        top_actions.set_halign(gtk::Align::End);
        top_actions.set_valign(gtk::Align::Start);
        top_actions.set_margin_top(2);
        top_actions.set_margin_end(2);

        let dismiss = gtk::Button::from_icon_name("window-close-symbolic");
        dismiss.set_visible(false);
        dismiss.set_tooltip_text(Some("Remove from list"));
        dismiss.add_css_class("flat");
        dismiss.add_css_class("circular");
        dismiss.add_css_class("rp-recent-dismiss");
        {
            let path = c.clone();
            let rem = on_remove.clone();
            dismiss.connect_clicked(move |_| {
                eprintln!("[rhino] recent: dismiss path={}", path.display());
                rem(&path);
            });
        }

        let hover_btns: Vec<gtk::Button> = if !miss && c.is_file() {
            let trash = gtk::Button::from_icon_name("user-trash-symbolic");
            trash.set_visible(false);
            trash.set_tooltip_text(Some("Move to trash"));
            trash.add_css_class("flat");
            trash.add_css_class("circular");
            trash.add_css_class("rp-recent-trash");
            {
                let path = c.clone();
                let tr = on_trash.clone();
                trash.connect_clicked(move |_| {
                    eprintln!("[rhino] recent: trash path={}", path.display());
                    tr(&path);
                });
            }
            top_actions.append(&trash);
            top_actions.append(&dismiss);
            vec![trash, dismiss]
        } else {
            top_actions.append(&dismiss);
            vec![dismiss.clone()]
        };
        card.add_overlay(&top_actions);

        if miss {
            let path = c.clone();
            let rem = on_remove.clone();
            add_click_and_pointer(
                &card,
                &c.display().to_string(),
                Rc::new(move |()| {
                    eprintln!(
                        "[rhino] recent: stale remove callback path={}",
                        path.display()
                    );
                    rem(&path);
                }),
                &hover_btns,
            );
        } else {
            let path = c.clone();
            let op = on_open.clone();
            add_click_and_pointer(
                &card,
                &c.display().to_string(),
                Rc::new(move |()| {
                    eprintln!("[rhino] recent: open callback path={}", path.display());
                    op(&path);
                }),
                &hover_btns,
            );
        }

        let wrap = adw::Clamp::new();
        wrap.set_maximum_size(CARD_MAX_W);
        wrap.set_child(Some(&card));
        cards.borrow_mut().push(card.clone());
        row.append(&wrap);
    }
    sync_card_sizes(row, &cards.borrow());
    let cards2 = Rc::clone(&cards);
    row.connect_notify_local(Some("width"), move |r, _| {
        sync_card_sizes(r, &cards2.borrow());
    });
}
