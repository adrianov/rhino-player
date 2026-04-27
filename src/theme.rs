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
        /* Header popovers: one shared layout for sound, subtitles, and speed. */
        popover.rp-header-popover > contents {
            padding: 0;
        }
        .rp-popover-box {
            padding: 12px;
            border-spacing: 10px;
            min-width: 240px;
        }
        .rp-popover-box scrolledwindow {
            min-height: 0;
        }
        .rp-popover-box list.rich-list {
            background: none;
        }
        /* Seek hover preview (see docs/features/18-thumbnail-preview.md) */
        popover.rp-seek-popover {
            background: none;
            background-color: transparent;
            border: none;
            box-shadow: none;
            padding: 0;
        }
        popover.rp-seek-popover > contents {
            background: none;
            background-color: transparent;
            border: none;
            box-shadow: none;
            padding: 0;
        }
        frame.rp-seek-thumb-frame {
            padding: 3px;
            background-color: #2d2d2d;
            border: 1px solid rgba(255, 255, 255, 0.16);
            border-radius: 8px;
            box-shadow: 0 8px 22px rgba(0, 0, 0, 0.45);
        }
        frame.rp-seek-thumb-frame > border {
            border: none;
        }
        frame.rp-seek-thumb-frame glarea {
            border-radius: 5px;
        }
        label.rp-seek-thumb-time {
            color: #c0bfbc;
            font-size: 0.82em;
            font-family: monospace, monospace;
            font-feature-settings: "tnum";
            padding: 0 2px 1px 2px;
        }
        .rp-page-stack, .rp-recent-scroll { background-color: #242424; }
        .rp-recent-scroll {
            min-height: 200px;
        }
        .rp-recent-row {
            padding: 10px;
        }
        .rp-recent-card {
            padding: 0;
            background-color: rgba(18, 18, 20, 0.94);
            border-radius: 12px;
            border: 1px solid rgba(255, 255, 255, 0.10);
            box-shadow: 0 10px 28px rgba(0, 0, 0, 0.50);
            min-width: 220px;
            min-height: 132px;
        }
        .rp-recent-card:hover {
            border-color: rgba(255, 255, 255, 0.22);
        }
        .rp-recent-bg { border-radius: 0; }
        .rp-recent-bg-miss { background-color: #2d2d2d; }
        .rp-recent-card-footer {
            padding: 30px 12px 10px 12px;
            background-color: transparent;
            background-image: linear-gradient(
                to top,
                rgba(0, 0, 0, 0.76) 0%,
                rgba(0, 0, 0, 0.54) 46%,
                rgba(0, 0, 0, 0.00) 100%
            );
            border-radius: 0 0 12px 12px;
        }
        label.rp-recent-card-title {
            color: #ffffff;
            font-weight: 600;
            font-size: 0.98em;
            text-shadow: 0 1px 3px rgba(0, 0, 0, 0.9);
        }
        .rp-recent-progress-row {
            min-height: 18px;
        }
        label.rp-recent-percent {
            min-width: 42px;
            padding: 2px 7px;
            border-radius: 9999px;
            background-color: rgba(255, 255, 255, 0.12);
            color: #f6f5f4;
            font-size: 0.82em;
            font-weight: 600;
        }
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
        progressbar.rp-recent-bar { min-height: 5px; }
        progressbar.rp-recent-bar trough {
            min-height: 5px;
            border-radius: 9999px;
            background-color: rgba(255, 255, 255, 0.22);
        }
        progressbar.rp-recent-bar progress {
            min-height: 5px;
            border-radius: 9999px;
            background-color: #62a0ea;
        }

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
