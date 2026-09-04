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
    // Glued tags (`HDTV1080p`) miss `tech_hint` word boundaries.
    if tech_regexes().iter().any(|re| re.is_match(no_ext)) {
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

    strip_structural_noise(&mut title);
    strip_tag_tokens(&mut title);

    title = split_dots_if_glued(title, had_glued);
    polish_edges(&mut title);

    if title.is_empty() {
        title = naive_fallback(trimmed);
    }
    compose_tail(title, tail)
}

/// Glued filenames (`Foo.Bar.Baz`) keep their dots as word separators only when
/// nothing spaced them out earlier.
fn split_dots_if_glued(title: String, had_glued: bool) -> String {
    if had_glued || (!title.contains(' ') && title.contains('.')) {
        title.replace('.', " ")
    } else {
        title
    }
}

/// Parenthesis / bracket / curly layout noise left behind after marker stripping.
fn strip_structural_noise(s: &mut String) {
    strip_year_ellipsis(s);
    fix_paren_edges(s);
    insert_space_before_word_paren(s);
    strip_curly_groups(s);
    brackets_to_spaces(s);
    collapse_ws_inplace(s);
}

fn strip_tag_tokens(s: &mut String) {
    merged_rip_spacing(s);
    strip_bluray(s);
    strip_extra_word_tags(s);
    strip_tech_tags(s);
    strip_resolution_tokens(s);
    strip_leftover_season_tokens(s);
    strip_dd_dot_dates(s);
    tidy_paren_commas(s);
}

/// Final edge cleanup once the tag vocabulary is gone.
fn polish_edges(s: &mut String) {
    normalize_hyphen_spaces(s);
    cleanup_dot_edges(s);
    strip_hd_sd_parens(s);
    trim_edges_inplace(s);
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
    if let Some(n) = take_tail_nums(work, &p.se, 2) {
        return Tail::SeasonEp { s: n[0], e: n[1] };
    }
    if let Some(n) = take_tail_nums(work, &p.season_range, 2) {
        return Tail::SeasonRange { a: n[0], b: n[1] };
    }
    if let Some(n) = take_tail_nums(work, &p.n_by_ep, 2) {
        return Tail::SeasonEp { s: n[0], e: n[1] };
    }
    if let Some(n) = take_tail_nums(work, &p.season_only, 1) {
        return Tail::SeasonOnly(n[0]);
    }
    Tail::None
}

/// Capture `count` numeric groups from `re`'s first match, then blank every match out.
fn take_tail_nums(work: &mut String, re: &Regex, count: usize) -> Option<Vec<u32>> {
    let c = re.captures(work.as_str())?;
    let nums = (1..=count).map(|i| c[i].parse().unwrap_or(0)).collect();
    *work = re.replace_all(work.as_str(), " ").into_owned();
    Some(nums)
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
    collapse_ws(&patterns().strip_ext.replace(trimmed, "").trim().replace(['.', '_'], " "))
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
    *s = split_join_spaces(s);
}

fn split_join_spaces(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
#[path = "human_media_title/tests.rs"]
mod tests;
