//! Human-friendly labels from release-style video filenames (dots, SxxExx, tech tags).
//! Inspired by Transmission’s `formatHumanTitle`; omits resolution suffixes such as `#1080p`.

use regex::Regex;
use std::sync::OnceLock;

include!("human_media_title/patterns.rs");
include!("human_media_title/tech_strip.rs");
include!("human_media_title/cleanup.rs");

#[path = "human_media_title/download_temp.rs"]
mod download_temp;
use download_temp::peel_download_temp;
pub(crate) use download_temp::{finished_download_path, is_incomplete_download_path};

/// Display name for a file **basename** (with or without extension).
pub fn human_media_title(original: &str) -> String {
    let trimmed = original.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let peeled = peel_download_temp(trimmed);
    if let Some(s) = try_short_circuit(&peeled) {
        return s;
    }
    process_release_style(&peeled)
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
    strip_dd_dot_dates(&mut title);
    tidy_paren_commas(&mut title);

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
    patterns()
        .strip_ext
        .replace_all(&name, "")
        .trim()
        .to_string()
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

#[cfg(test)]
#[path = "human_media_title/tests.rs"]
mod tests;
