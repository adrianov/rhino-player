// Fuzzy subtitle auto-pick after [loadfile] (included from `sub_tracks.rs`).

/// Seeding text for fuzzy match: last hand-picked track label, else a short [LANG] hint.
pub fn autoseed(prefs: &SubPrefs) -> String {
    let t = prefs.last_sub_label.trim();
    if !t.is_empty() {
        return t.to_lowercase();
    }
    std::env::var("LANG")
        .ok()
        .and_then(|s| s.split('.').next().map(str::to_string))
        .unwrap_or_else(|| "en".into())
        .split('_')
        .next()
        .unwrap_or("en")
        .to_lowercase()
}

/// After a new [loadfile], pick the subtitle track whose label best matches [autoseed]
/// (word multiset overlap first, then alphanumeric character multiset overlap).
pub fn autopick_sub_track(mpv: &Mpv, prefs: &SubPrefs, shell: Option<&std::path::Path>) {
    if prefs.sub_off {
        set_sub_off(mpv);
        return;
    }
    let rows = sub_rows(mpv, shell);
    if rows.is_empty() {
        return;
    }
    let seed = autoseed(prefs);
    if seed.is_empty() {
        return;
    }
    let Some((row, score)) = best_matching_row(&seed, &rows) else {
        return;
    };
    if !subtitle_autopick_qualifies(score) {
        return;
    }
    let sid = resolve_sub_id(mpv, row.id, row.ifo_slot, shell).unwrap_or(row.id);
    let _ = mpv.set_property("sub-visibility", true);
    let _ = mpv.set_property("sid", sid);
    reapply_styling(mpv);
}

/// Highest-scoring row for `seed`: word multiset overlap first, then character overlap.
fn best_matching_row<'a>(seed: &str, rows: &'a [Row]) -> Option<(&'a Row, LabelMatchScore)> {
    let mut best_score = LabelMatchScore {
        word_intersection: 0,
        char_intersection: 0,
    };
    let mut best: Option<&Row> = None;
    for r in rows {
        let s = seed_row_score(seed, &r.text, &r.lang);
        if best.is_none() || s > best_score {
            best_score = s;
            best = Some(r);
        }
    }
    best.map(|r| (r, best_score))
}
