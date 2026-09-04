// Fullscreen header clock: GNOME org.gnome.desktop.interface (clock-format, clock-show-seconds).
// Falls back to 24-hour HH:MM when the schema is unavailable.
//
// The resolved strftime pattern is read once on first use (`gio::Settings` is not kept).
// The fullscreen ticker (~1 Hz) only borrows that cached string (no schema lookup per tick).

use gio::prelude::SettingsExt;

const SCHEMA_DESKTOP_IFACE: &str = "org.gnome.desktop.interface";

thread_local! {
    static WALL_CLOCK_PATTERN: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn gnome_interface_available() -> bool {
    gio::SettingsSchemaSource::default()
        .and_then(|s| s.lookup(SCHEMA_DESKTOP_IFACE, true))
        .is_some()
}

fn gnome_wall_clock_pattern(settings: &gio::Settings) -> String {
    let twelve_h = settings.string("clock-format").as_str() == "12h";
    match (twelve_h, settings.boolean("clock-show-seconds")) {
        (true, true) => "%l:%M:%S %p".to_string(),
        (true, false) => "%l:%M %p".to_string(),
        (false, true) => "%H:%M:%S".to_string(),
        (false, false) => "%H:%M".to_string(),
    }
}

fn resolve_wall_clock_pattern_once() -> String {
    if gnome_interface_available() {
        return gnome_wall_clock_pattern(&gio::Settings::new(SCHEMA_DESKTOP_IFACE));
    }
    "%H:%M".to_string()
}

fn format_dt_with_pattern(dt: &glib::DateTime, pattern: &str) -> glib::GString {
    match dt.format(pattern) {
        Ok(gs) => {
            if pattern.contains("%l") {
                glib::GString::from(gs.trim_start())
            } else {
                gs
            }
        }
        Err(_) => dt
            .format(if pattern.contains("%p") {
                "%I:%M %p"
            } else {
                "%H:%M"
            })
            .unwrap_or_else(|_| glib::GString::from("—")),
    }
}

/// Current local time using the same GNOME desktop rules as the shell clock when possible.
pub(crate) fn format_wall_clock_now() -> glib::GString {
    let Ok(dt) = glib::DateTime::now_local() else {
        return glib::GString::from("—");
    };
    WALL_CLOCK_PATTERN.with(|cell| {
        let mut slot = cell.borrow_mut();
        format_dt_with_pattern(&dt, slot.get_or_insert_with(resolve_wall_clock_pattern_once).as_str())
    })
}
