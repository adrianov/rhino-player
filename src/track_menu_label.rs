//! mpv `track-list` row labels for Sound / Subtitles menus (codec + layout, duplicate disambiguation).

use std::collections::HashMap;

/// Audio row label from mpv metadata (DVD IFO labels take precedence when present).
#[must_use]
pub fn mpv_audio_label(
    lang: Option<&str>,
    title: Option<&str>,
    codec: Option<&str>,
    demux_channel_count: Option<i64>,
    demux_channels: Option<&str>,
) -> String {
    if let Some(t) = title.map(str::trim).filter(|s| !s.is_empty()) {
        return t.to_string();
    }
    let lang = lang.map(str::trim).filter(|s| !s.is_empty()).unwrap_or("");
    let format = codec.and_then(mpv_codec_format_label);
    let ch = channel_label_from_mpv(demux_channel_count, demux_channels);
    match (lang.is_empty(), format) {
        (false, Some(fmt)) if !ch.is_empty() => format!("{lang} · {fmt} {ch}"),
        (false, Some(fmt)) => format!("{lang} · {fmt}"),
        (false, None) if !ch.is_empty() => format!("{lang} · {ch}"),
        (false, None) => lang.to_string(),
        (true, Some(fmt)) if !ch.is_empty() => format!("{fmt} {ch}"),
        (true, Some(fmt)) => fmt.to_string(),
        (true, None) if !ch.is_empty() => ch,
        (true, None) => String::new(),
    }
}

/// Subtitle row label from mpv metadata (DVD IFO labels take precedence when present).
#[must_use]
pub fn mpv_sub_label(
    lang: Option<&str>,
    title: Option<&str>,
    codec: Option<&str>,
    forced: bool,
    hearing_impaired: bool,
    visual_impaired: bool,
    default: bool,
) -> String {
    if let Some(t) = title.map(str::trim).filter(|s| !s.is_empty()) {
        return t.to_string();
    }
    let lang = lang.map(str::trim).filter(|s| !s.is_empty()).unwrap_or("");
    let kind = codec.and_then(sub_format_label);
    let mut out = match (lang.is_empty(), kind) {
        (false, Some(k)) => format!("{lang} · {k}"),
        (false, None) => lang.to_string(),
        (true, Some(k)) => k.to_string(),
        (true, None) => String::new(),
    };
    append_sub_tags(&mut out, forced, hearing_impaired, visual_impaired, default);
    out
}

fn append_sub_tags(
    out: &mut String,
    forced: bool,
    hearing_impaired: bool,
    visual_impaired: bool,
    default: bool,
) {
    if hearing_impaired {
        out.push_str(" (SDH)");
    }
    if visual_impaired {
        out.push_str(" (VI)");
    }
    if forced {
        out.push_str(" (forced)");
    }
    if default {
        out.push_str(" (default)");
    }
}

/// When several rows share the same label, suffix ` · 2`, ` · 3`, … (first row unchanged).
pub fn disambiguate_labels(labels: &mut [String]) {
    let mut totals: HashMap<String, usize> = HashMap::new();
    for l in labels.iter() {
        *totals.entry(l.clone()).or_default() += 1;
    }
    let mut seen: HashMap<String, usize> = HashMap::new();
    for label in labels.iter_mut() {
        if totals.get(label).copied().unwrap_or(1) <= 1 {
            continue;
        }
        let key = label.clone();
        let n = seen.entry(key).or_insert(0);
        *n += 1;
        if *n > 1 {
            label.push_str(&format!(" · {n}"));
        }
    }
}

fn mpv_codec_format_label(codec: &str) -> Option<&'static str> {
    match codec.trim().to_ascii_lowercase().as_str() {
        "ac3" | "ac-3" => Some("AC-3"),
        "eac3" | "e-ac-3" => Some("E-AC-3"),
        "dts" | "dca" => Some("DTS"),
        "truehd" => Some("TrueHD"),
        "flac" => Some("FLAC"),
        "aac" | "aac_latm" => Some("AAC"),
        "mp3" => Some("MP3"),
        "opus" => Some("Opus"),
        "vorbis" => Some("Vorbis"),
        "lpcm" | "pcm_s16le" | "pcm_s24le" | "pcm_s32le" => Some("LPCM"),
        _ => None,
    }
}

fn sub_format_label(codec: &str) -> Option<&'static str> {
    match codec.trim().to_ascii_lowercase().as_str() {
        "hdmv_pgs_subtitle" | "pgs" | "pgssub" => Some("PGS"),
        "dvd_sub" => Some("VOBSUB"),
        "dvb_sub" | "dvbsub" | "dvb_teletext" | "teletext" => Some("DVB"),
        "subrip" | "srt" => Some("SRT"),
        "ass" | "ssa" => Some("ASS"),
        "mov_text" => Some("Text"),
        _ => None,
    }
}

fn channel_label_from_mpv(count: Option<i64>, layout: Option<&str>) -> String {
    if let Some(l) = layout.map(str::trim).filter(|s| !s.is_empty()) {
        let head = l.split('(').next().unwrap_or(l).trim();
        if head.eq_ignore_ascii_case("stereo") || head == "2.0" {
            return "stereo".into();
        }
        return head.to_string();
    }
    match count.unwrap_or(0).max(0) as u8 {
        0 => String::new(),
        1 => "mono".into(),
        2 => "stereo".into(),
        5 | 6 => "5.1".into(),
        7 | 8 => "7.1".into(),
        n => format!("{n}ch"),
    }
}

#[cfg(test)]
mod tests {
    use super::{disambiguate_labels, mpv_audio_label, mpv_sub_label};

    #[test]
    fn youth_in_revolt_audio_labels() {
        assert_eq!(
            mpv_audio_label(Some("rus"), None, Some("dts"), Some(6), Some("5.1(side)")),
            "rus · DTS 5.1"
        );
        assert_eq!(
            mpv_audio_label(Some("rus"), None, Some("ac3"), Some(2), Some("stereo")),
            "rus · AC-3 stereo"
        );
        assert_eq!(
            mpv_audio_label(Some("eng"), None, Some("dts"), Some(6), Some("5.1(side)")),
            "eng · DTS 5.1"
        );
        assert_eq!(
            mpv_audio_label(Some("eng"), None, Some("ac3"), Some(2), Some("stereo")),
            "eng · AC-3 stereo"
        );
    }

    #[test]
    fn sub_tags_stack() {
        assert_eq!(
            mpv_sub_label(Some("eng"), None, Some("subrip"), true, true, false, false),
            "eng · SRT (SDH) (forced)"
        );
        assert_eq!(
            mpv_sub_label(Some("deu"), None, None, false, false, true, true),
            "deu (VI) (default)"
        );
    }

    #[test]
    fn duplicate_sub_labels_numbered() {
        let mut labels = vec![
            "rus".into(),
            mpv_sub_label(Some("eng"), None, Some("hdmv_pgs_subtitle"), false, false, false, false),
            mpv_sub_label(Some("eng"), None, Some("hdmv_pgs_subtitle"), false, false, false, false),
            mpv_sub_label(Some("eng"), None, Some("hdmv_pgs_subtitle"), false, false, false, false),
        ];
        disambiguate_labels(&mut labels);
        assert_eq!(labels[0], "rus");
        assert_eq!(labels[1], "eng · PGS");
        assert_eq!(labels[2], "eng · PGS · 2");
        assert_eq!(labels[3], "eng · PGS · 3");
    }

    #[test]
    fn unique_audio_skips_suffix() {
        let mut labels = vec![
            mpv_audio_label(Some("rus"), None, Some("dts"), Some(6), Some("5.1(side)")),
            mpv_audio_label(Some("rus"), None, Some("ac3"), Some(2), Some("stereo")),
        ];
        disambiguate_labels(&mut labels);
        assert_eq!(labels[0], "rus · DTS 5.1");
        assert_eq!(labels[1], "rus · AC-3 stereo");
    }
}
