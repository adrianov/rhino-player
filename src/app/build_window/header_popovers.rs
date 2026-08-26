include!("header_popover_sound.rs");
include!("header_popover_subtitles.rs");

/// Sound (volume + audio tracks) and subtitle (style + tracks) header popovers.
struct HeaderPopovers {
    vol_adj: gtk::Adjustment,
    vol_header_img: gtk::Image,
    vol_readout: gtk::Label,
    vol_mute_btn: gtk::ToggleButton,
    audio_tracks_block: Rc<Cell<bool>>,
    audio_tracks_box: gtk::Box,
    audio_tracks_section: gtk::Box,
    vol_pop: gtk::Popover,
    vol_menu: gtk::MenuButton,
    sub_tracks_block: Rc<Cell<bool>>,
    sub_tracks_box: gtk::Box,
    sub_tracks_section: gtk::Box,
    sub_scale_adj: gtk::Adjustment,
    sub_color_btn: gtk::ColorDialogButton,
    sub_color_row: gtk::Box,
    sub_pop: gtk::Popover,
    sub_menu: gtk::MenuButton,
    sub_readout: gtk::Label,
}

fn build_header_popovers(sub_pref: &Rc<RefCell<db::SubPrefs>>) -> HeaderPopovers {
    let sound = build_sound_popover();
    let subs = build_subtitle_popover(sub_pref);
    HeaderPopovers {
        vol_adj: sound.vol_adj,
        vol_header_img: sound.vol_header_img,
        vol_readout: sound.vol_readout,
        vol_mute_btn: sound.vol_mute_btn,
        audio_tracks_block: sound.audio_tracks_block,
        audio_tracks_box: sound.audio_tracks_box,
        audio_tracks_section: sound.audio_tracks_section,
        vol_pop: sound.vol_pop,
        vol_menu: sound.vol_menu,
        sub_tracks_block: subs.sub_tracks_block,
        sub_tracks_box: subs.sub_tracks_box,
        sub_tracks_section: subs.sub_tracks_section,
        sub_scale_adj: subs.sub_scale_adj,
        sub_color_btn: subs.sub_color_btn,
        sub_color_row: subs.sub_color_row,
        sub_pop: subs.sub_pop,
        sub_menu: subs.sub_menu,
        sub_readout: subs.sub_readout,
    }
}
