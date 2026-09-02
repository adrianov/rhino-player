// Continue / neighbour-search card action overlays (Trash / Remove).

use std::path::Path;
use std::rc::Rc;

use gtk::prelude::*;

use super::{HistoryCardHandlers, card_action_btn};

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

/// Top-right overlay buttons. Trash for present files on either strip; Remove on the
/// continue list and on I'm Feeling Lucky cards (name-search hits omit it).
pub(super) fn top_action_buttons(
    c: &Path,
    h: &HistoryCardHandlers<'_>,
    miss: bool,
) -> (gtk::Box, Vec<gtk::Button>) {
    let top_actions = action_overlay_box();
    let mut hover_btns = Vec::new();
    if !miss && c.is_file() {
        push_action(
            &top_actions,
            &mut hover_btns,
            c,
            h.on_trash.clone(),
            "user-trash-symbolic",
            "Move to Trash",
            "trash",
        );
    }
    if h.kind.shows_remove() {
        push_action(
            &top_actions,
            &mut hover_btns,
            c,
            h.on_remove.clone(),
            "window-close-symbolic",
            "Remove from list",
            "remove",
        );
    }
    (top_actions, hover_btns)
}

fn push_action(
    top: &gtk::Box,
    hover: &mut Vec<gtk::Button>,
    c: &Path,
    act: Rc<dyn Fn(&Path)>,
    icon: &str,
    tip: &str,
    verb: &'static str,
) {
    let btn = card_action_btn(icon, tip);
    wire_logged_action(&btn, c.to_path_buf(), act, verb);
    top.append(&btn);
    hover.push(btn);
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
