// Same-series check for sibling folder advance (feature 07).
//
// Flat libraries put season folders beside unrelated shows
// (`House of the Dragon Season 2` next to `Legion Season 1`). Strip season
// markers and require equal remaining stems. Season-only labels (`S01`, `01`)
// match each other under a shared parent. Folders with no season markers
// (typical movie dirs) keep the old any-sibling advance.

use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

struct SeasonStrip {
    season_word: Regex,
    sxx: Regex,
    year: Regex,
    bare_num: Regex,
}

fn strip_patterns() -> &'static SeasonStrip {
    static P: OnceLock<SeasonStrip> = OnceLock::new();
    P.get_or_init(|| SeasonStrip {
        season_word: Regex::new(
            r"(?i)\b(?:seasons?|сезон(?:ы|а|ов)?)\s*\d{1,2}(?:\s*[-–—]\s*\d{1,2})?\b",
        )
        .expect("season_word"),
        sxx: Regex::new(r"(?i)\bs\d{1,2}(?:[-–—]\d{1,2})?(?:e\d{1,3})?\b").expect("sxx"),
        year: Regex::new(r"(?i)\((?:19|20)\d{2}\)|\b(?:19|20)\d{2}\b").expect("year"),
        bare_num: Regex::new(r"^\d{1,2}$").expect("bare_num"),
    })
}

fn fold_folder_chars(name: &str) -> String {
    let mut s = String::with_capacity(name.len());
    for c in name.chars() {
        if c == '.' || c == '_' {
            s.push(' ');
        } else {
            for lc in c.to_lowercase() {
                s.push(lc);
            }
        }
    }
    collapse_ws(&s)
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// After season/year tokens are blanked, turn leftover title separators into spaces.
fn collapse_separators(s: &str) -> String {
    collapse_ws(
        &s.chars()
            .map(|c| {
                if c.is_whitespace()
                    || matches!(
                        c,
                        '-' | '–' | '—' | '.' | '_' | '(' | ')' | '[' | ']' | '|' | ':' | ';' | ','
                    )
                {
                    ' '
                } else {
                    c
                }
            })
            .collect::<String>(),
    )
}

fn looks_seasonal(folded: &str) -> bool {
    let p = strip_patterns();
    p.bare_num.is_match(folded) || p.season_word.is_match(folded) || p.sxx.is_match(folded)
}

fn stem_from_folded(folded: &str) -> String {
    let p = strip_patterns();
    if p.bare_num.is_match(folded) {
        return String::new();
    }
    let next = p.season_word.replace_all(folded, " ");
    let next = p.sxx.replace_all(&next, " ");
    let next = p.year.replace_all(&next, " ");
    collapse_separators(&next)
}

/// Series stem after season / year noise is removed. Empty = season-only label.
pub(crate) fn folder_series_stem(name: &str) -> String {
    stem_from_folded(&fold_folder_chars(name))
}

/// Folder basename carries a season marker (`Season 2`, `S01`, bare `01`).
pub(crate) fn folder_looks_seasonal(name: &str) -> bool {
    looks_seasonal(&fold_folder_chars(name))
}

/// Whether two sibling folder basenames may be queued across.
pub(super) fn series_stems_match(a_name: &str, b_name: &str) -> bool {
    let a_fold = fold_folder_chars(a_name);
    let b_fold = fold_folder_chars(b_name);
    // No season markers on either side → keep classic sibling-folder advance.
    if !looks_seasonal(&a_fold) && !looks_seasonal(&b_fold) {
        return true;
    }
    let a = stem_from_folded(&a_fold);
    let b = stem_from_folded(&b_fold);
    (a.is_empty() && b.is_empty()) || (!a.is_empty() && !b.is_empty() && a == b)
}

pub(super) fn same_series_dirs(a: &Path, b: &Path) -> bool {
    let Some(an) = a.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    let Some(bn) = b.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    series_stems_match(an, bn)
}

#[cfg(test)]
mod series_tests {
    include!("sibling_advance_series_tests.rs");
}
