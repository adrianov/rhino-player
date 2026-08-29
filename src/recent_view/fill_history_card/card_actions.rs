// Continue / neighbour-search card action overlays (Trash / Remove).

use std::path::Path;
use std::rc::Rc;

use gtk::prelude::*;

use super::{HistoryCardHandlers, StripKind, card_action_btn};

/// Logs the continue-strip verb, then runs [act].
fn wire_logged_action(
    btn: &gtk::Button,
    path: std::path::PathBuf,
    act: Rc<dyn Fn(&Path)>,
    verb: &'static str,
) {
    btn.connect_clicked(move |_| {
        crate::user_action_log::act(format!("continue {verb} {}", path.display()));
        act(&path);
    });
}

/// Top-right overlay buttons. Trash for present files on either strip; Remove only on the
/// continue list. Returns the overlay box plus the buttons that toggle with hover.
pub(super) fn top_action_buttons(
    c: &Path,
    h: &HistoryCardHandlers<'_>,
    miss: bool,
) -> (gtk::Box, Vec<gtk::Button>) {
    let top_actions = action_overlay_box();
    let mut hover_btns = Vec::new();
    if !miss && c.is_file() {
        let trash = card_action_btn("user-trash-symbolic", "Move to Trash");
        wire_logged_action(&trash, c.to_path_buf(), h.on_trash.clone(), "trash");
        top_actions.append(&trash);
        hover_btns.push(trash);
    }
    if h.kind == StripKind::ContinueList {
        let remove = card_action_btn("window-close-symbolic", "Remove from list");
        wire_logged_action(&remove, c.to_path_buf(), h.on_remove.clone(), "remove");
        top_actions.append(&remove);
        hover_btns.push(remove);
    }
    (top_actions, hover_btns)
}

fn action_overlay_box() -> gtk::Box {
    let top_actions = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    top_actions.set_spacing(2);
    top_actions.set_halign(gtk::Align::End);
    top_actions.set_valign(gtk::Align::Start);
    top_actions.set_margin_top(2);
    top_actions.set_margin_end(2);
    top_actions
}
