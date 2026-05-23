//! Header menu [`gtk::ScrolledWindow`] list caps (speed / sound / subtitles).

use gtk::prelude::*;

pub const SCROLL_CLASS_AUDIO: &str = "rp-header-scroll-audio";
pub const SCROLL_CLASS_SUB: &str = "rp-header-scroll-sub";
pub const SCROLL_CLASS_SPEED: &str = "rp-header-scroll-speed";

pub const AUDIO_MIN_W: i32 = 400;
pub const AUDIO_MAX_H: i32 = 480;
pub const SUB_MIN_W: i32 = 360;
pub const SUB_MAX_H: i32 = 280;
pub const SPEED_MAX_H: i32 = 320;

pub fn max_content_height_for(scrl: &gtk::ScrolledWindow) -> i32 {
    if scrl.has_css_class(SCROLL_CLASS_AUDIO) {
        AUDIO_MAX_H
    } else if scrl.has_css_class(SCROLL_CLASS_SUB) {
        SUB_MAX_H
    } else {
        SPEED_MAX_H
    }
}
