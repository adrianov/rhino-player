/// Extra styling on top of libadwaita: opaque content area, custom seek / time look, pointer on controls.
/// Dark style comes from [adw::StyleManager::set_color_scheme] in `app.rs` — do not set
/// `gtk-application-prefer-dark-theme` (unsupported with libadwaita).
pub fn apply() {
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
        .rp-gl { background: #000000; min-height: 120px; }
        .rp-bottom {
            background-color: #1e1e1e;
            border-top: 1px solid #3d3d3d;
            padding: 8px 14px 12px 14px;
        }
        /* LTR: space before elapsed time; GTK CSS has no margin-end in all releases */
        .rp-bottom .rpb-prev { margin-right: 2px; }
        .rp-bottom .rpb-play { margin-left: 2px; margin-right: 2px; }
        .rp-bottom .rpb-next { margin-right: 6px; }
        .rp-time {
            color: #c0bfbc;
            font-family: monospace, monospace;
            font-feature-settings: "tnum";
            min-width: 3.2em;
        }
        .rp-time-dim { color: #9a9996; }
        scale.rp-seek { margin: 0 4px; }
        scale.rp-vol { margin: 0; }
        scale.rp-vol > trough { min-height: 4px; }
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
        .rp-page-stack, .rp-recent-scroll { background-color: #242424; }
        .rp-recent-scroll {
            min-height: 200px;
        }
        .rp-recent-card {
            padding: 0;
            background-color: #1e1e1e;
            border-radius: 8px;
            min-width: 200px;
            min-height: 120px;
        }
        .rp-recent-bg { border-radius: 0; }
        .rp-recent-bg-miss { background-color: #2d2d2d; }
        .rp-recent-card-footer {
            /* Solid strip + gradient so file name and progress read on any frame. */
            background-color: rgba(0, 0, 0, 0.55);
            background-image: linear-gradient(
                to top,
                rgba(0, 0, 0, 0.5) 0%,
                rgba(0, 0, 0, 0.12) 100%
            );
        }
        .rp-recent-card-footer label { color: #f6f5f4; }
        .rp-recent-card-footer label.dim-label { color: #deddda; }
        .rp-stale { opacity: 0.6; }
        .rp-recent-pict { color: #9a9996; }
        button.rp-recent-dismiss {
            min-width: 28px;
            min-height: 28px;
            padding: 0;
            background-color: rgba(0, 0, 0, 0.5);
        }
        button.rp-recent-dismiss:hover { background-color: rgba(0, 0, 0, 0.68); }
        /* Undo shell: zero paint so only the pill (.rp-undo-toast) is visible. */
        .rp-undo-shell {
            background: none;
            background-color: transparent;
            border: none;
            box-shadow: none;
            padding: 0;
            outline: none;
        }
        /* Continue list: file-manager style snack (pill, blur) — only on the inner box */
        .rp-undo-toast {
            min-height: 40px;
            padding: 6px 8px 6px 16px;
            /* Darker + frosted vs .rp-recent-scroll so the pill reads on top, not a flat 24/24/24 */
            background-color: rgba(18, 18, 20, 0.94);
            border-radius: 9999px;
            border: 1px solid rgba(255, 255, 255, 0.08);
            box-shadow: 0 4px 18px rgba(0, 0, 0, 0.5);
            backdrop-filter: blur(20px);
            color: #f6f5f4;
        }
        .rp-undo-toast:backdrop { background-color: rgba(18, 18, 20, 0.96); }
        .rp-undo-toast label.rp-undo-toast-text { color: #f6f5f4; }
        .rp-undo-toast-undo,
        .rp-undo-toast button.rp-undo-toast-undo {
            color: #f6f5f4;
            font-weight: 600;
            min-height: 32px;
            padding-left: 12px;
            padding-right: 12px;
            border-radius: 8px;
            background-color: rgba(255, 255, 255, 0.1);
        }
        .rp-undo-toast-undo:hover { background-color: rgba(255, 255, 255, 0.16); }
        button.rp-undo-toast-close {
            min-width: 32px;
            min-height: 32px;
            padding: 0;
            color: #f6f5f4;
        }
        button.rp-undo-toast-close:hover { background-color: rgba(255, 255, 255, 0.12); }
        progressbar.rp-recent-bar { min-height: 8px; }
        progressbar.rp-recent-bar trough { background-color: #3d3d3d; }
        progressbar.rp-recent-bar progress { background-color: #78aeed; }

        /* Hand on interactive controls; not-allowed when disabled. Skip entry, textview, drawing
           (video): they do not use these node names. */
        button:not(:disabled), menubutton:not(:disabled), modelbutton, togglebutton:not(:disabled),
        switch, checkbutton, radiobutton, link, scale:not(:disabled), spinbutton > button,
        listview > row, listbox > row {
            cursor: pointer;
        }
        button:disabled, menubutton:disabled, scale:disabled, togglebutton:disabled {
            cursor: not-allowed;
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
