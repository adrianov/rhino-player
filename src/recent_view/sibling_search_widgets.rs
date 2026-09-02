// Neighbour-search row widgets: entry, I'm Feeling Lucky, inline hint.
// State and repaint decisions live in [SiblingSearchState].

/// Widest usual hint; floors the label so the row does not jump when the hint fills in.
const HINT_SIDE: &str = "Nothing to pick";

const LUCKY_LABEL: &str = "I'm Feeling Lucky";

/// Search row widgets; all repaint decisions live in [SiblingSearchState].
pub struct SiblingSearch {
    shell: gtk::Box,
    state: Rc<SiblingSearchState>,
}

impl SiblingSearch {
    pub(super) fn new() -> Self {
        let entry = search_entry();
        let hint = hint_label();
        let lucky = lucky_button();
        let shell = search_row_shell(&entry, &hint, &lucky);
        let state = SiblingSearchState::new(shell.clone(), entry, hint);
        state.wire_lucky(&lucky);
        SiblingSearch { shell, state }
    }

    /// Full-width band mounted directly above the card scroller.
    pub(crate) fn widget(&self) -> &gtk::Box {
        &self.shell
    }

    /// Shared rendering/query state handed to the strip painters.
    pub(crate) fn shared(&self) -> Rc<SiblingSearchState> {
        Rc::clone(&self.state)
    }
}

fn search_entry() -> gtk::SearchEntry {
    let entry = gtk::SearchEntry::new();
    entry.set_placeholder_text(Some("Search your video library…"));
    entry.add_css_class("rp-recent-search-entry");
    // Fixed width so the clear icon / typing does not reflow the strip below.
    entry.set_size_request(340, -1);
    entry.set_hexpand(false);
    clear_initial_search_focus(&entry);
    entry
}

fn lucky_button() -> gtk::Button {
    let btn = gtk::Button::with_label(LUCKY_LABEL);
    btn.add_css_class("rp-recent-lucky");
    btn.set_valign(gtk::Align::Center);
    btn.set_hexpand(false);
    btn.set_tooltip_text(Some("Show random videos from your library"));
    btn.set_cursor_from_name(Some("pointer"));
    btn
}

/// Invisible twin of the lucky button so the search field stays window-centered.
fn lucky_balance() -> gtk::Button {
    let bal = lucky_button();
    bal.set_opacity(0.0);
    bal.set_can_target(false);
    bal.set_can_focus(false);
    bal
}

/// GTK focuses the first focusable field on map; drop that so launch leaves the entry idle.
fn clear_initial_search_focus(entry: &gtk::SearchEntry) {
    let once = std::cell::Cell::new(true);
    entry.connect_map(move |e| {
        if !once.replace(false) {
            return;
        }
        let e = e.clone();
        glib::idle_add_local_once(move || {
            if !e.has_focus() {
                return;
            }
            let Some(win) = e.root().and_downcast::<gtk::Window>() else {
                return;
            };
            gtk::prelude::GtkWindowExt::set_focus(&win, gtk::Widget::NONE);
        });
    });
}

fn hint_label() -> gtk::Label {
    let hint = gtk::Label::new(None);
    hint.add_css_class("rp-recent-search-hint");
    hint.set_halign(gtk::Align::Center);
    hint.set_valign(gtk::Align::Center);
    hint.set_xalign(0.5);
    hint.set_hexpand(false);
    hint.set_width_chars(HINT_SIDE.chars().count() as i32);
    hint
}

fn search_row_shell(entry: &gtk::SearchEntry, hint: &gtk::Label, lucky: &gtk::Button) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.set_halign(gtk::Align::Center);
    row.set_valign(gtk::Align::Center);
    row.set_hexpand(false);
    row.append(&lucky_balance());
    row.append(entry);
    row.append(lucky);

    let shell = gtk::Box::new(gtk::Orientation::Vertical, 4);
    shell.set_halign(gtk::Align::Center);
    shell.set_valign(gtk::Align::Center);
    shell.set_hexpand(false);
    shell.append(&row);
    shell.append(hint);
    shell
}
