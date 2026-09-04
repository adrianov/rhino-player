fn strip_year_ellipsis(s: &mut String) {
    *s = patterns().year_ellipsis.replace_all(s, " ").into_owned();
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
    *s = patterns().merged_rip.replace_all(s, "$1 $2").into_owned();
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
    *s = patterns().resolution.replace_all(s, " ").into_owned();
}

fn strip_leftover_season_tokens(s: &mut String) {
    *s = patterns().season_leftover.replace_all(s, " ").into_owned();
}

fn strip_dd_dot_dates(s: &mut String) {
    let p = patterns();
    *s = p.date_long.replace_all(s, " ").into_owned();
    *s = p.date_short.replace_all(s, " ").into_owned();
}

fn normalize_hyphen_spaces(s: &mut String) {
    let mut out = patterns()
        .standalone_hyphen
        .replace_all(
            &s.replace(" - ", &'\u{0001}'.to_string()),
            " ",
        )
        .into_owned();
    out = out.replace('\u{0001}', " - ");
    *s = out;
}

fn cleanup_dot_edges(s: &mut String) {
    repair_internal_dots(s);
    trim_dotted_edges(s);
}

/// Repair dots stranded inside the title after token removal.
fn repair_internal_dots(s: &mut String) {
    let p = patterns();
    *s = p.dot_space_dot.replace_all(s, ". ").into_owned();
    *s = p.space_dot_word.replace_all(s, " $1").into_owned();
    *s = p.trailing_space_dot.replace_all(s, "").into_owned();
}

/// Drop the leftover dot at the very end of the title.
fn trim_dotted_edges(s: &mut String) {
    let p = patterns();
    *s = p.space_dot_space.replace_all(s, " ").into_owned();
    *s = p.strip_end_dot_word.replace_all(s, "$1").into_owned();
}

fn strip_hd_sd_parens(s: &mut String) {
    let p = patterns();
    *s = p.empty_parens.replace_all(s, "").into_owned();
    *s = p.hd_sd_parens.replace_all(s, "").into_owned();
}

/// Collapse holes left when resolution tokens are removed inside `(…)`.
fn tidy_paren_commas(s: &mut String) {
    let p = patterns();
    for _ in 0..4 {
        let next = p.comma_double.replace_all(s, ",").into_owned();
        let next = p.comma_after_open.replace_all(&next, "(").into_owned();
        let next = p.comma_before_close.replace_all(&next, ")").into_owned();
        if next == *s {
            break;
        }
        *s = next;
    }
    *s = p.empty_parens.replace_all(s, "").into_owned();
    collapse_ws_inplace(s);
}

fn trim_edges_inplace(s: &mut String) {
    let mut t = s.trim().to_string();
    t = t.trim_matches(|c| c == '-' || c == ' ').to_string();
    *s = t;
}
