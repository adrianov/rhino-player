//! Continue-card **Rename file** dialog (feature 37).

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib::object::IsA;
use gtk::prelude::{EditableExt, EntryExt, WidgetExt};

/// Open the rename dialog for [path]; parent is taken from [anchor]'s root window.
pub(crate) fn prompt_card_rename(anchor: &impl IsA<gtk::Widget>, path: &Path) {
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        eprintln!("[rhino] rename: no stem path={}", path.display());
        return;
    };
    let parent = anchor.root();
    let path = path.to_path_buf();
    let entry = rename_entry(stem);
    let err = error_label();
    let dialog = rename_dialog(&entry, &err);
    wire_rename_dialog(&dialog, &entry, &err, path, parent);
    focus_stem(&entry);
    dialog.present(anchor.root().as_ref());
}

fn rename_entry(stem: &str) -> gtk::Entry {
    let entry = gtk::Entry::new();
    entry.set_text(stem);
    entry.set_hexpand(true);
    entry
}

fn error_label() -> gtk::Label {
    let err = gtk::Label::new(None);
    err.add_css_class("error");
    err.set_wrap(true);
    err.set_xalign(0.0);
    err.set_visible(false);
    err
}

fn rename_dialog(entry: &gtk::Entry, err: &gtk::Label) -> adw::AlertDialog {
    let col = gtk::Box::new(gtk::Orientation::Vertical, 8);
    col.append(entry);
    col.append(err);
    let dialog = adw::AlertDialog::new(Some("Rename file"), None);
    dialog.set_extra_child(Some(&col));
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("rename", "Rename");
    dialog.set_default_response(Some("rename"));
    dialog.set_close_response("cancel");
    dialog.set_response_appearance("rename", adw::ResponseAppearance::Suggested);
    dialog
}

fn wire_rename_dialog(
    dialog: &adw::AlertDialog,
    entry: &gtk::Entry,
    err: &gtk::Label,
    path: PathBuf,
    parent: Option<gtk::Root>,
) {
    let apply = rename_apply(path, entry.clone(), err.clone());
    let apply_r = apply.clone();
    let parent_r = parent.clone();
    dialog.connect_response(None, move |d, id| {
        if id == "rename" && !apply_r() {
            d.present(parent_r.as_ref());
        }
    });
    let dialog_a = dialog.clone();
    entry.connect_activate(move |_| {
        if apply() {
            dialog_a.force_close();
        }
    });
}

fn rename_apply(path: PathBuf, entry: gtk::Entry, err: gtk::Label) -> Rc<dyn Fn() -> bool> {
    let done = Rc::new(Cell::new(false));
    Rc::new(move || {
        if done.get() {
            return true;
        }
        match super::super::with_rename_search(|s| s.rename_card_file(&path, &entry.text())) {
            Some(Ok(())) => {
                clear_entry_error(&entry, &err);
                done.set(true);
                true
            }
            Some(Err(msg)) => {
                eprintln!("[rhino] rename: {msg} path={}", path.display());
                show_entry_error(&entry, &err, &msg);
                false
            }
            None => {
                eprintln!(
                    "[rhino] rename: apply skipped (search unbound) path={}",
                    path.display()
                );
                show_entry_error(&entry, &err, "Could not update the library.");
                false
            }
        }
    })
}

fn show_entry_error(entry: &gtk::Entry, err: &gtk::Label, msg: &str) {
    entry.add_css_class("error");
    err.set_text(msg);
    err.set_visible(true);
    entry.grab_focus();
}

fn clear_entry_error(entry: &gtk::Entry, err: &gtk::Label) {
    entry.remove_css_class("error");
    err.set_visible(false);
    err.set_text("");
}

fn focus_stem(entry: &gtk::Entry) {
    let entry = entry.clone();
    glib::idle_add_local_once(move || {
        entry.grab_focus();
        entry.select_region(0, -1);
    });
}
