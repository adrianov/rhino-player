//! Decoding of individual audio / subpicture attribute rows into IFO stream entries.

use super::{DvdIfoAudio, DvdIfoSub, AUDIO_ATTR_SIZE, SUBP_ATTR_SIZE};
use crate::dvd_ifo_parse::bitreader::{read_audio_attr, read_subp_attr};

pub(super) fn parse_audio_attr(raw: &[u8], slot: u8) -> Option<DvdIfoAudio> {
    if raw.len() < AUDIO_ATTR_SIZE {
        return None;
    }
    let (format, lang_type, lang_code, ch_bits) = read_audio_attr(raw)?;
    let channels = ch_bits.saturating_add(1);
    if blank_audio_row(format, lang_type, channels, lang_code) {
        return None;
    }
    let lang = typed_lang(lang_type, lang_code);
    let (codec_key, format_label) = audio_format_label(format)?;
    Some(DvdIfoAudio {
        slot,
        label: compose_label(&lang, format_label, channel_label(channels)),
        lang,
        channels,
        codec_key,
    })
}

/// All-zero coding-mode placeholder row emitted for unused DVD audio slots.
fn blank_audio_row(format: u8, lang_type: u8, channels: u8, lang_code: u16) -> bool {
    format == 0 && lang_type == 0 && channels == 1 && lang_code == 0
}

pub(super) fn parse_subp_attr(raw: &[u8], slot: u8) -> Option<DvdIfoSub> {
    if raw.len() < SUBP_ATTR_SIZE {
        return None;
    }
    let (typ, lang_code) = read_subp_attr(raw)?;
    let lang = typed_lang(typ, lang_code);
    if lang.is_empty() && typ == 0 {
        return None;
    }
    let label = if lang.is_empty() {
        format!("Subtitle {}", slot + 1)
    } else {
        lang.clone()
    };
    Some(DvdIfoSub { slot, lang, label })
}

/// Language string for a DVD stream: two-letter code when the type marks it present.
fn typed_lang(lang_type: u8, lang_code: u16) -> String {
    if lang_type == 1 {
        lang_from_code(lang_code)
    } else {
        String::new()
    }
}

fn lang_from_code(code: u16) -> String {
    let hi = (code >> 8) as u8;
    let lo = (code & 0xff) as u8;
    if hi.is_ascii_alphabetic() && lo.is_ascii_alphabetic() {
        format!("{}{}", hi as char, lo as char).to_lowercase()
    } else {
        String::new()
    }
}

fn audio_format_label(format: u8) -> Option<(&'static str, &'static str)> {
    Some(match format {
        0 => ("ac3", "AC-3"),
        2 => ("mpeg1", "MPEG-1"),
        3 => ("mpeg2", "MPEG-2"),
        4 => ("lpcm", "LPCM"),
        6 => ("dts", "DTS"),
        _ => return None,
    })
}

fn channel_label(channels: u8) -> &'static str {
    match channels {
        0 => "unknown",
        1 => "mono",
        2 => "stereo",
        5 | 6 => "5.1",
        _ => "surround",
    }
}

fn compose_label(lang: &str, format: &str, channels: &str) -> String {
    if lang.is_empty() {
        return format!("{format} {channels}");
    }
    format!("{lang} · {format} {channels}")
}
