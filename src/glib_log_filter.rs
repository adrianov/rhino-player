//! Quiet **Gtk** “Theme parser … **gtk.css** …” warnings: they come from the **system** theme
//! (often one huge `gtk.css` on macOS Homebrew; mismatches / extensions the parser warns about).
//! Rhino’s own rules load as `<data>`, not `gtk.css`. Real app stylesheet problems still print.

use glib::{LogField, LogLevel, LogWriterOutput};

pub fn install() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        glib::log_set_writer_func(|level, fields| {
            if suppress_gtk_theme_css_warning(level, fields) {
                return LogWriterOutput::Handled;
            }
            glib::log_writer_default(level, fields)
        });
    });
}

fn suppress_gtk_theme_css_warning(level: LogLevel, fields: &[LogField<'_>]) -> bool {
    if level != LogLevel::Warning {
        return false;
    }
    let domain_is_gtk = fields
        .iter()
        .any(|f| f.key() == "GLIB_DOMAIN" && f.value_str() == Some("Gtk"));
    if !domain_is_gtk {
        return false;
    }
    for f in fields {
        if f.key() == "MESSAGE" {
            let Some(msg) = f.value_str() else { continue };
            return msg.contains("Theme parser warning: gtk.css");
        }
    }
    false
}
