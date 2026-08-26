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
        return prefix_title_lang(lang, t);
    }
    let lang = lang.map(str::trim).filter(|s| !s.is_empty());
    let format = codec.and_then(mpv_codec_format_label).map(str::to_string);
    let ch = channel_label_from_mpv(demux_channel_count, demux_channels);
    join_label(lang, format, ch)
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
        return prefix_title_lang(lang, t);
    }
    let lang = lang.map(str::trim).filter(|s| !s.is_empty());
    let kind = codec.and_then(sub_format_label).map(str::to_string);
    let mut out = join_label(lang, kind, String::new());
    append_sub_tags(&mut out, forced, hearing_impaired, visual_impaired, default);
    out
}

/// Join `lang · fmt ch`: the language leads, format and channel layout share the tail slot;
/// empty components are skipped entirely.
fn join_label(lang: Option<&str>, fmt: Option<String>, ch: String) -> String {
    let tail = match (fmt, ch.is_empty()) {
        (Some(f), true) => f,
        (Some(f), false) => format!("{f} {ch}"),
        (None, _) => ch,
    };
    match (lang.filter(|l| !l.is_empty()), tail.is_empty()) {
        (Some(l), false) => format!("{l} · {tail}"),
        (Some(l), true) => l.to_string(),
        (None, false) => tail,
        (None, true) => String::new(),
    }
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

/// Prefix a known language token onto a release-group title (e.g. `eng · DD 5.1 @ 384 Kbps`)
/// unless the title already mentions that language. Many rips set a descriptive title with no
/// language, which otherwise hides which track is which.
fn prefix_title_lang(lang: Option<&str>, title: &str) -> String {
    let tok = crate::sub_track_abbr::abbrev_track_lang(lang);
    if tok.is_empty() || title.to_lowercase().contains(&tok) {
        return title.to_string();
    }
    format!("{tok} · {title}")
}

/// When several rows share the same label, suffix ` · 2`, ` · 3`, … (first row unchanged).
pub fn disambiguate_labels(labels: &mut [String]) {
    let totals = duplicate_totals(labels);
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

/// How many rows carry each label (labels occurring once need no suffix).
fn duplicate_totals(labels: &[String]) -> HashMap<String, usize> {
    let mut totals: HashMap<String, usize> = HashMap::new();
    for l in labels.iter() {
        *totals.entry(l.clone()).or_default() += 1;
    }
    totals
}

/// mpv audio codec name → display token.
const CODEC_FORMAT_LABELS: &[(&str, &str)] = &[
    ("ac3", "AC-3"),
    ("ac-3", "AC-3"),
    ("eac3", "E-AC-3"),
    ("e-ac-3", "E-AC-3"),
    ("dts", "DTS"),
    ("dca", "DTS"),
    ("truehd", "TrueHD"),
    ("flac", "FLAC"),
    ("aac", "AAC"),
    ("aac_latm", "AAC"),
    ("mp3", "MP3"),
    ("opus", "Opus"),
    ("vorbis", "Vorbis"),
    ("lpcm", "LPCM"),
    ("pcm_s16le", "LPCM"),
    ("pcm_s24le", "LPCM"),
    ("pcm_s32le", "LPCM"),
];

fn mpv_codec_format_label(codec: &str) -> Option<&'static str> {
    let name = codec.trim().to_ascii_lowercase();
    CODEC_FORMAT_LABELS
        .iter()
        .find(|(raw, _)| *raw == name)
        .map(|(_, label)| *label)
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
        let head_lc = head.to_ascii_lowercase();
        if head_lc == "stereo" || head == "2.0" {
            return "stereo".into();
        }
        if head_lc == "unknown" || head_lc.starts_with("unknown") {
            return channel_label_from_count(count);
        }
        return head.to_string();
    }
    channel_label_from_count(count)
}

fn channel_label_from_count(count: Option<i64>) -> String {
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
    fn unknown_layout_uses_channel_count() {
        assert_eq!(
            mpv_audio_label(Some("eng"), None, Some("dts"), Some(6), Some("unknown6")),
            "eng · DTS 5.1"
        );
        assert_eq!(
            mpv_audio_label(Some("eng"), None, Some("dts"), Some(6), Some("unknown")),
            "eng · DTS 5.1"
        );
    }

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
    fn release_title_gets_language_prefix() {
        assert_eq!(
            mpv_audio_label(
                Some("eng"),
                Some("DD 5.1 @ 384 Kbps"),
                Some("ac3"),
                Some(6),
                None
            ),
            "eng · DD 5.1 @ 384 Kbps"
        );
        assert_eq!(
            mpv_audio_label(
                Some("rus"),
                Some("DD 5.1 @ 384 Kbps, LostFilm"),
                Some("ac3"),
                Some(6),
                None
            ),
            "rus · DD 5.1 @ 384 Kbps, LostFilm"
        );
    }

    #[test]
    fn title_already_naming_language_is_untouched() {
        assert_eq!(
            mpv_audio_label(
                Some("eng"),
                Some("English Commentary"),
                Some("ac3"),
                Some(2),
                None
            ),
            "English Commentary"
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
            mpv_sub_label(
                Some("eng"),
                None,
                Some("hdmv_pgs_subtitle"),
                false,
                false,
                false,
                false,
            ),
            mpv_sub_label(
                Some("eng"),
                None,
                Some("hdmv_pgs_subtitle"),
                false,
                false,
                false,
                false,
            ),
            mpv_sub_label(
                Some("eng"),
                None,
                Some("hdmv_pgs_subtitle"),
                false,
                false,
                false,
                false,
            ),
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
