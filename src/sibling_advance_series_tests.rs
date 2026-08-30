// Unit tests for `sibling_advance_series` (included from that module's `#[cfg(test)]`).

use super::*;

#[test]
fn stem_strips_season_word() {
    assert_eq!(
        folder_series_stem("House of the Dragon Season 1"),
        "house of the dragon"
    );
    assert_eq!(
        folder_series_stem("House of the Dragon Season 02"),
        "house of the dragon"
    );
    assert_eq!(folder_series_stem("Legion Season 1"), "legion");
}

#[test]
fn stem_strips_sxx_and_dots() {
    assert_eq!(
        folder_series_stem("House.of.the.Dragon.S01"),
        "house of the dragon"
    );
    assert_eq!(
        folder_series_stem("House.of.the.Dragon.S02E01"),
        "house of the dragon"
    );
}

#[test]
fn stem_strips_russian_season() {
    assert_eq!(
        folder_series_stem("Игра престолов Сезон 1"),
        "игра престолов"
    );
}

#[test]
fn pure_season_labels_have_empty_stem() {
    assert!(folder_series_stem("S01").is_empty());
    assert!(folder_series_stem("S1").is_empty());
    assert!(folder_series_stem("Season 2").is_empty());
    assert!(folder_series_stem("Сезон 3").is_empty());
    assert!(folder_series_stem("01").is_empty());
    assert!(folder_series_stem("2").is_empty());
}

#[test]
fn pure_season_folders_match_each_other() {
    assert!(series_stems_match("S01", "S02"));
    assert!(series_stems_match("S1", "S02"));
    assert!(series_stems_match("Season 1", "Season 2"));
    assert!(series_stems_match("01", "02"));
    assert!(series_stems_match("S01", "Season 2"));
}

#[test]
fn same_series_seasons_match() {
    assert!(series_stems_match(
        "House of the Dragon Season 1",
        "House of the Dragon Season 2"
    ));
    assert!(series_stems_match(
        "House.of.the.Dragon.S01",
        "House.of.the.Dragon.S02"
    ));
}

#[test]
fn mixed_separator_season_formats_match() {
    assert_eq!(folder_series_stem("Show Season 1"), "show");
    assert_eq!(folder_series_stem("Show - Season 2"), "show");
    assert_eq!(folder_series_stem("Show – Season 3"), "show");
    assert!(series_stems_match("Show Season 1", "Show - Season 2"));
    assert!(series_stems_match("Show.S01", "Show - Season 2"));
    assert!(series_stems_match("Show: Season 1", "Show - Season 2"));
    assert!(series_stems_match("Show (2019) Season 1", "Show Season 2"));
}

#[test]
fn different_series_do_not_match() {
    assert!(!series_stems_match(
        "House of the Dragon Season 2",
        "Legion Season 1"
    ));
    assert!(!series_stems_match(
        "House of the Dragon Season 2",
        "Legion"
    ));
    assert!(!series_stems_match(
        "Breaking Bad Season 5",
        "Better Call Saul Season 1"
    ));
}

#[test]
fn pure_season_does_not_match_named_show() {
    assert!(!series_stems_match("S02", "Legion Season 1"));
    assert!(!series_stems_match("01", "Legion Season 1"));
}

#[test]
fn non_seasonal_siblings_still_match() {
    // Movie (or other) folders with no season markers keep classic advance.
    assert!(series_stems_match("Inception", "Interstellar"));
}
