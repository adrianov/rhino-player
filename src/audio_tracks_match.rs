// Saved-choice matching against sound-menu rows (included from `audio_tracks_popover.rs`).

fn norm_label(s: &str) -> String {
    s.trim().to_lowercase()
}

fn closest_label<'a>(rows: &'a [AudioMenuRow], want: &str) -> Option<&'a AudioMenuRow> {
    let want_n = norm_label(want);
    if want_n.is_empty() {
        return None;
    }
    let mut best_score = LabelMatchScore {
        word_intersection: 0,
        char_intersection: 0,
    };
    let mut picked: Option<&'a AudioMenuRow> = None;
    for row in rows {
        let s = match_score(&want_n, &norm_label(&row.label));
        if picked.is_none() || s > best_score {
            best_score = s;
            picked = Some(row);
        }
    }
    picked
}
