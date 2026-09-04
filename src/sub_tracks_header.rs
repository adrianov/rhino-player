// Compact subtitles-header caption from the active / saved track (included from `sub_tracks.rs`).

fn track_header_token(r: &Row) -> String {
    let l = r.lang.trim();
    if !l.is_empty() {
        let a = abbrev_track_lang(Some(l));
        if !a.is_empty() {
            return a;
        }
    }
    abbrev_track_lang(Some(
        r.text.split(" – ").next().unwrap_or(r.text.as_str()).trim(),
    ))
}

fn compact_header_label_row(
    sid: i64,
    rows: &[Row],
    mpv: &Mpv,
    shell: Option<&std::path::Path>,
) -> String {
    let Some(row) = row_for_sid(sid, rows, mpv, shell) else {
        return "…".to_string();
    };
    let t = track_header_token(row);
    if t.is_empty() {
        "…".into()
    } else {
        t
    }
}

fn row_for_sid<'a>(
    sid: i64,
    rows: &'a [Row],
    mpv: &Mpv,
    shell: Option<&std::path::Path>,
) -> Option<&'a Row> {
    rows.iter().find(|r| r.id == sid).or_else(|| {
        let slot = ifo_slot_for_sid(mpv, sid, shell)?;
        rows.iter().find(|r| r.ifo_slot == Some(slot))
    })
}

/// Updates the subtitles header caption from the current subtitle track (`Off` when hidden).
pub fn refresh_sub_header(mpv: &Mpv, label: &gtk::Label, shell: Option<&std::path::Path>) {
    let s = sub_header_compact(mpv, shell);
    if label.text().as_str() != s.as_str() {
        label.set_text(&s);
    }
}

fn sub_header_compact(mpv: &Mpv, shell: Option<&std::path::Path>) -> String {
    if !sub_visibility(mpv) {
        return "Off".to_string();
    }
    let rows = sub_rows(mpv, shell);
    if let Some(sid) = current_sid(mpv) {
        return compact_header_label_row(sid, &rows, mpv, shell);
    }
    let saved = crate::db::load_sub().last_sub_label;
    if !saved.trim().is_empty() {
        if let Some(t) = saved_header_token(&rows, saved.trim()) {
            return t;
        }
    }
    "Auto".to_string()
}

/// Header token for the last hand-picked label: the matching row's token, else its abbreviation.
fn saved_header_token(rows: &[Row], saved: &str) -> Option<String> {
    for r in rows {
        if r.text.eq_ignore_ascii_case(saved)
            || r.lang.eq_ignore_ascii_case(saved)
            || r.text.contains(saved)
        {
            return Some(track_header_token(r));
        }
    }
    let a = abbrev_track_lang(Some(saved));
    (!a.is_empty()).then_some(a)
}
