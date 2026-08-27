// Neighbour-search row widgets: pill entry + inline hint. Split out of `sibling_search.rs`
// to keep both sides reviewable; state and repaint decisions live in [SiblingSearchState].

/// Search row widgets; all repaint decisions live in [SiblingSearchState].
pub struct SiblingSearch {
    shell: gtk::Box,
    state: Rc<SiblingSearchState>,
}

impl SiblingSearch {
    pub(super) fn new() -> Self {
        let entry = search_entry();
        let hint = hint_label();
        let state = SiblingSearchState::new(entry.clone(), hint.clone());
        SiblingSearch {
            shell: search_row_shell(&entry, &hint),
            state,
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
    entry
}

fn hint_label() -> gtk::Label {
    let hint = gtk::Label::new(None);
    hint.add_css_class("rp-recent-search-hint");
    hint.set_valign(gtk::Align::Center);
    hint
}

fn search_row_shell(entry: &gtk::SearchEntry, hint: &gtk::Label) -> gtk::Box {
    let shell = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    shell.set_halign(gtk::Align::Center);
    shell.set_valign(gtk::Align::Start);
    shell.add_css_class("rp-recent-search-row");
    shell.append(entry);
    shell.append(hint);
    shell
}
