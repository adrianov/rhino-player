// Neighbour-search row widgets: entry + inline hint. Split out of `sibling_search.rs`
// to keep both sides reviewable; state and repaint decisions live in [SiblingSearchState].

/// Widest usual hint; both side slots measure this so the entry stays window-centered.
const HINT_SIDE: &str = "40+ matches";

/// Search row widgets; all repaint decisions live in [SiblingSearchState].
pub struct SiblingSearch {
    shell: gtk::Box,
    state: Rc<SiblingSearchState>,
}

impl SiblingSearch {
    pub(super) fn new() -> Self {
        let entry = search_entry();
        let hint = hint_label();
        let shell = search_row_shell(&entry, &hint);
        let state = SiblingSearchState::new(entry, hint);
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
    entry.set_placeholder_text(Some("Find neighbours of your list…"));
    entry.add_css_class("rp-recent-search-entry");
    // Fixed width so the clear icon / typing does not reflow the strip below.
    entry.set_size_request(340, -1);
    entry.set_hexpand(false);
    entry
}

fn hint_label() -> gtk::Label {
    let hint = gtk::Label::new(None);
    hint.add_css_class("rp-recent-search-hint");
    hint.set_valign(gtk::Align::Center);
    hint.set_xalign(0.0);
    hint.set_hexpand(false);
    // Floor width to the longest hint so an empty side still matches the balancer.
    hint.set_width_chars(HINT_SIDE.chars().count() as i32);
    hint
}

/// Invisible twin of the widest hint — empty Label + opacity often collapses to 0 width.
fn hint_balance() -> gtk::Label {
    let bal = gtk::Label::new(Some(HINT_SIDE));
    bal.add_css_class("rp-recent-search-hint");
    bal.set_valign(gtk::Align::Center);
    bal.set_opacity(0.0);
    bal.set_can_target(false);
    bal.set_hexpand(false);
    bal
}

fn search_row_shell(entry: &gtk::SearchEntry, hint: &gtk::Label) -> gtk::Box {
    let shell = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    shell.set_halign(gtk::Align::Center);
    shell.set_valign(gtk::Align::Center);
    shell.set_hexpand(false);
    // balance | entry | hint — equal side slots keep the field centered when the hint has text.
    shell.append(&hint_balance());
    shell.append(entry);
    shell.append(hint);
    shell
}
