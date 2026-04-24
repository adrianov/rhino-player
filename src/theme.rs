/// Extra styling on top of libadwaita: opaque content area, custom seek / time look.
pub fn apply() {
    if let Some(s) = gtk::Settings::default() {
        s.set_gtk_application_prefer_dark_theme(true);
    }

    let p = gtk::CssProvider::new();
    p.load_from_string(
        r#"
        window.rp-win {
            background-color: #242424;
            color: #eeeeec;
        }
        .rpb-header {
            background-color: #2d2d2d;
            color: #eeeeec;
            box-shadow: none;
            border: none;
        }
        .rp-stack {
            background-color: #242424;
        }
        .rp-status {
            color: #9a9996;
            font-size: 0.9em;
            margin: 8px 14px 4px 14px;
        }
        .rp-gl { background: #000000; min-height: 120px; }
        .rp-bottom {
            background-color: #1e1e1e;
            border-top: 1px solid #3d3d3d;
            padding: 8px 14px 12px 14px;
        }
        .rp-time {
            color: #c0bfbc;
            font-family: monospace, monospace;
            font-feature-settings: "tnum";
            min-width: 3.2em;
        }
        .rp-time-dim { color: #9a9996; }
        scale.rp-seek { margin: 0 4px; }
        scale.rp-seek > trough {
            background-color: #3d3d3d;
            min-height: 6px;
            border-radius: 3px;
        }
        scale.rp-seek > trough > highlight {
            background-color: #78aeed;
            border-radius: 3px;
        }
        scale.rp-seek > trough > fill {
            background-color: #78aeed;
        }
    "#,
    );
    if let Some(d) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &d,
            &p,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
