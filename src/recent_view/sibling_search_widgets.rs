// Neighbour-search row widgets: pill entry + inline hint. Split out of `sibling_search.rs`
// to keep both sides reviewable; state and repaint decisions live in [SiblingSearchState].

/// Side slot width floor (longest usual hint: `40+ matches`).
const HINT_SIDE_CHARS: i32 = 12;

/// Search row widgets; all repaint decisions live in [SiblingSearchState].
pub struct SiblingSearch {
    shell: gtk::Box,
    state: Rc<SiblingSearchState>,
    /// Keeps the invisible start balancer as wide as the end hint.
    _hint_sync: gtk::SizeGroup,
}

impl SiblingSearch {
    pub(super) fn new() -> Self {
        let entry = search_entry();
        let hint = hint_label();
        let (shell, hint_sync) = search_row_shell(&entry, &hint);
        let state = SiblingSearchState::new(entry, hint);
        SiblingSearch {
            shell,
            state,
            _hint_sync: hint_sync,
        }
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
    hint.set_width_chars(HINT_SIDE_CHARS);
    hint.set_hexpand(false);
    hint
}

/// Invisible twin of the hint so pill | hint never shifts the entry off center.
fn hint_balance() -> gtk::Label {
    let bal = gtk::Label::new(None);
    bal.add_css_class("rp-recent-search-hint");
    bal.set_valign(gtk::Align::Center);
    bal.set_width_chars(HINT_SIDE_CHARS);
    bal.set_opacity(0.0);
    bal.set_can_target(false);
    bal.set_hexpand(false);
    bal
}

fn search_row_shell(entry: &gtk::SearchEntry, hint: &gtk::Label) -> (gtk::Box, gtk::SizeGroup) {
    let shell = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    shell.set_halign(gtk::Align::Center);
    shell.set_valign(gtk::Align::Center);
    shell.set_hexpand(false);
    shell.add_css_class("rp-recent-search-row");
    let balance = hint_balance();
    let sync = gtk::SizeGroup::new(gtk::SizeGroupMode::Horizontal);
    sync.add_widget(&balance);
    sync.add_widget(hint);
    // balance | pill | hint — equal side slots keep the pill centered when the hint has text.
    shell.append(&balance);
    shell.append(&search_pill(entry));
    shell.append(hint);
    (shell, sync)
}

fn search_pill(entry: &gtk::SearchEntry) -> gtk::Box {
    let pill = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    pill.add_css_class("rp-recent-search-pill");
    pill.set_halign(gtk::Align::Center);
    pill.set_valign(gtk::Align::Center);
    pill.append(entry);
    pill
}
