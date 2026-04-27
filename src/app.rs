use adw::prelude::*;
use gio::prelude::{
    ActionExt as GioActionExt, ActionMapExt as GioActionMapExt, ApplicationExtManual, FileExt,
};
use glib::prelude::{ObjectExt, ToVariant};
use gtk::gio;
use gtk::glib;
use gtk::prelude::{
    ActionableExt, EventControllerExt, GestureExt, GtkWindowExt, NativeExt, WidgetExt,
};
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use crate::audio_tracks;
use crate::continue_undo::{apply as apply_bar_undo, ContinueBarUndo};
use crate::db;
use crate::format_time;
use crate::history;
use crate::icons;
use crate::idle_inhibit;
use crate::sub_prefs;
use crate::sub_tracks;
use crate::video_ext;
use libmpv2::Mpv;

use crate::media_probe::{
    capture_list_remove_undo, card_data_list, is_done_enough_to_drop_continue, local_file_from_mpv,
    remove_continue_entry, CardData,
};
use crate::mpv_embed::MpvBundle;
use crate::playback_speed;
use crate::recent_view;
use crate::recent_view::RecentContext;
use crate::seek_bar_preview;
use crate::sibling_advance;
use crate::theme;
use crate::trash_xdg;
use crate::video_pref;

/// Application and icon name ([reverse-DNS] for GTK, desktop, and AppStream).
///
/// [reverse-DNS]: https://developer.gnome.org/documentation/tutorials/application-id.html
pub const APP_ID: &str = "ch.rhino.RhinoPlayer";
include!("app/base.rs");
include!("app/load.rs");
include!("app/realize.rs");
include!("app/final_actions.rs");
include!("app/input.rs");
include!("app/file_actions.rs");
include!("app/file_loaded.rs");
include!("app/recent_undo.rs");
include!("app/chrome_wiring.rs");
include!("app/open_handler.rs");
include!("app/build_window.rs");
