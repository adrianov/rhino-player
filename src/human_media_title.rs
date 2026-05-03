//! Human-friendly labels from release-style video filenames (dots, SxxExx, tech tags).
//! Inspired by Transmission’s `formatHumanTitle`; omits resolution suffixes such as `#1080p`.

use regex::Regex;
use std::sync::OnceLock;

include!("human_media_title/patterns.rs");
include!("human_media_title/tech_strip.rs");

/// Display name for a file **basename** (with or without extension).
pub fn human_media_title(original: &str) -> String {
    let trimmed = original.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if let Some(s) = try_short_circuit(trimmed) {
        return s;
    }
    process_release_style(trimmed)
}

fn try_short_circuit(trimmed: &str) -> Option<String> {
    let no_ext = patterns().strip_ext.replace(trimmed, "");
    let no_ext = no_ext.trim();
    if no_ext.contains(['.', '_', '|']) {
        return None;
    }
    if patterns().tech_hint.is_match(no_ext) {
        return None;
    }
    Some(collapse_ws(&normalize_commas(no_ext.to_string())))
}

fn process_release_style(trimmed: &str) -> String {
    let mut title = normalize_commas(trimmed.replace(['_', '|'], " "));
    title = collapse_ws(&title);
    title = strip_extension_owned(title);

    let had_glued = patterns().glued_dots.is_match(&title);
    let tail = parse_tail_strip_markers(&mut title);

    strip_year_ellipsis(&mut title);
    fix_paren_edges(&mut title);
    insert_space_before_word_paren(&mut title);
    strip_curly_groups(&mut title);
    brackets_to_spaces(&mut title);
    collapse_ws_inplace(&mut title);
    merged_rip_spacing(&mut title);
    strip_bluray(&mut title);
    strip_extra_word_tags(&mut title);
    strip_tech_tags(&mut title);
    strip_resolution_tokens(&mut title);
    strip_leftover_season_tokens(&mut title);
    strip_year_tokens(&mut title);
    strip_dd_dot_dates(&mut title);

    if had_glued || (!title.contains(' ') && title.contains('.')) {
        title = title.replace('.', " ");
    }
    normalize_hyphen_spaces(&mut title);
    cleanup_dot_edges(&mut title);
    strip_hd_sd_parens(&mut title);
    trim_edges_inplace(&mut title);

    if title.is_empty() {
        title = naive_fallback(trimmed);
    }
    compose_tail(title, tail)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tail {
    None,
    SeasonEp { s: u32, e: u32 },
    SeasonRange { a: u32, b: u32 },
    SeasonOnly(u32),
}

fn parse_tail_strip_markers(work: &mut String) -> Tail {
    let p = patterns();
    if let Some(c) = p.se.captures(work.as_str()) {
        let s = c[1].parse().unwrap_or(0);
        let e = c[2].parse().unwrap_or(0);
        *work = p.se.replace_all(work, " ").into_owned();
        return Tail::SeasonEp { s, e };
    }
    if let Some(c) = p.season_range.captures(work.as_str()) {
        let a = c[1].parse().unwrap_or(0);
        let b = c[2].parse().unwrap_or(0);
        *work = p.season_range.replace_all(work, " ").into_owned();
        return Tail::SeasonRange { a, b };
    }
    if let Some(c) = p.n_by_ep.captures(work.as_str()) {
        let s = c[1].parse().unwrap_or(0);
        let e = c[2].parse().unwrap_or(0);
        *work = p.n_by_ep.replace_all(work, " ").into_owned();
        return Tail::SeasonEp { s, e };
    }
    if let Some(c) = p.season_only.captures(work.as_str()) {
        let s = c[1].parse().unwrap_or(0);
        *work = p.season_only.replace_all(work, " ").into_owned();
        return Tail::SeasonOnly(s);
    }
    Tail::None
}

fn compose_tail(mut base: String, tail: Tail) -> String {
    match tail {
        Tail::None => {}
        Tail::SeasonEp { s, e } => {
            base.push_str(&format!(" — Season {s}, Episode {e}"));
        }
        Tail::SeasonRange { a, b } => {
            base.push_str(&format!(" — Season {a}-{b}"));
        }
        Tail::SeasonOnly(s) => {
            base.push_str(&format!(" — Season {s}"));
        }
    }
    collapse_ws_inplace(&mut base);
    base.trim().to_string()
}

fn naive_fallback(trimmed: &str) -> String {
    let no_ext = patterns().strip_ext.replace(trimmed, "");
    collapse_ws(&no_ext.trim().replace(['.', '_'], " "))
}

fn strip_extension_owned(name: String) -> String {
    patterns().strip_ext.replace_all(&name, "").trim().to_string()
}

fn normalize_commas(s: String) -> String {
    s.replace(',', ", ")
}

fn collapse_ws(s: &str) -> String {
    split_join_spaces(s)
}

fn collapse_ws_inplace(s: &mut String) {
    let t = split_join_spaces(s);
    *s = t;
}

fn split_join_spaces(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_year_ellipsis(s: &mut String) {
    *s = patterns()
        .year_ellipsis
        .replace_all(s, " ")
        .into_owned();
}

fn fix_paren_edges(s: &mut String) {
    let p = patterns();
    *s = p.paren_open_space.replace_all(s, "(").into_owned();
    *s = p.paren_close_space.replace_all(s, ")").into_owned();
}

fn insert_space_before_word_paren(s: &mut String) {
    *s = patterns()
        .word_then_paren
        .replace_all(s, "$1 (")
        .into_owned();
}

fn strip_curly_groups(s: &mut String) {
    *s = patterns().curly.replace_all(s, " ").into_owned();
}

fn brackets_to_spaces(s: &mut String) {
    *s = s.replace(['[', ']'], " ");
}

fn merged_rip_spacing(s: &mut String) {
    *s = patterns()
        .merged_rip
        .replace_all(s, "$1 $2")
        .into_owned();
}

fn strip_bluray(s: &mut String) {
    *s = patterns().bluray.replace_all(s, " ").into_owned();
}

fn strip_extra_word_tags(s: &mut String) {
    for re in extra_regexes() {
        *s = re.replace_all(s, " ").into_owned();
    }
}

fn strip_tech_tags(s: &mut String) {
    for re in tech_regexes() {
        *s = re.replace_all(s, " ").into_owned();
    }
}

fn strip_resolution_tokens(s: &mut String) {
    let p = patterns();
    *s = p.resolution.replace_all(s, " ").into_owned();
}

fn strip_leftover_season_tokens(s: &mut String) {
    *s = patterns()
        .season_leftover
        .replace_all(s, " ")
        .into_owned();
}

fn strip_year_tokens(s: &mut String) {
    *s = patterns().year_token.replace_all(s, " ").into_owned();
}

fn strip_dd_dot_dates(s: &mut String) {
    let p = patterns();
    *s = p.date_long.replace_all(s, " ").into_owned();
    *s = p.date_short.replace_all(s, " ").into_owned();
}

fn normalize_hyphen_spaces(s: &mut String) {
    let marker = '\u{0001}';
    let tmp = s.replace(" - ", &marker.to_string());
    let p = patterns();
    let mut out = p.standalone_hyphen.replace_all(&tmp, " ").into_owned();
    out = out.replace(marker, " - ");
    *s = out;
}

fn cleanup_dot_edges(s: &mut String) {
    let p = patterns();
    *s = p.dot_space_dot.replace_all(s, ". ").into_owned();
    *s = p.space_dot_word.replace_all(s, " $1").into_owned();
    *s = p.trailing_space_dot.replace_all(s, "").into_owned();
    *s = p.space_dot_space.replace_all(s, " ").into_owned();
    *s = p.strip_end_dot_word.replace_all(s, "$1").into_owned();
}

fn strip_hd_sd_parens(s: &mut String) {
    let p = patterns();
    *s = p.empty_parens.replace_all(s, "").into_owned();
    *s = p.hd_sd_parens.replace_all(s, "").into_owned();
}

fn trim_edges_inplace(s: &mut String) {
    let mut t = s.trim().to_string();
    t = t.trim_matches(|c| c == '-' || c == ' ').to_string();
    *s = t;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn americans_sample() {
        assert_eq!(
            human_media_title("The.Americans.S04E04.1080p.WEB-DL.4xRus.Eng.TeamHD.mkv"),
            "The Americans — Season 4, Episode 4"
        );
    }

    #[test]
    fn clean_name_unchanged() {
        assert_eq!(human_media_title("My Home Video.mp4"), "My Home Video");
    }

    #[test]
    fn season_only_dot_separated() {
        let t = human_media_title("Some.Show.S02.720p.HDTV.x264-GROUP.mkv");
        assert!(t.contains("Season 2"));
        assert!(t.to_lowercase().contains("some show"));
        assert!(!t.to_lowercase().contains("720p"));
    }

    #[test]
    fn alternate_nx_episode() {
        assert_eq!(
            human_media_title("Ponies.3x05.Episode.Title.1080p.mkv"),
            "Ponies Episode Title — Season 3, Episode 5"
        );
    }

    #[test]
    fn empty_returns_empty() {
        assert_eq!(human_media_title(""), "");
        assert_eq!(human_media_title("   "), "");
    }
}
