//! Short subtitle language tokens for the header (e.g. `eng`, `rus`).

/// ISO 639-1 code → short display token.
const TWO_LETTER_LANGS: &[(&str, &str)] = &[
    ("en", "eng"),
    ("ru", "rus"),
    ("ja", "jpn"),
    ("ko", "kor"),
    ("zh", "zho"),
    ("pt", "por"),
    ("es", "spa"),
    ("fr", "fra"),
    ("de", "deu"),
    ("it", "ita"),
    ("uk", "ukr"),
    ("pl", "pol"),
    ("tr", "tur"),
    ("ar", "ara"),
    ("hi", "hin"),
];

pub fn abbrev_track_lang(raw: Option<&str>) -> String {
    let Some(s) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return String::new();
    };
    let tok = primary_lang_token(s).to_lowercase();
    match tok.len() {
        0 => String::new(),
        3 if tok.bytes().all(|b: u8| b.is_ascii_lowercase()) => tok,
        2 => expand_two_letter(&tok),
        _ => fallback_token(&tok),
    }
}

fn expand_two_letter(tok: &str) -> String {
    TWO_LETTER_LANGS
        .iter()
        .find(|(two, _)| *two == tok)
        .map_or_else(|| tok.to_string(), |(_, three)| (*three).to_string())
}

/// Non-ISO tokens: numeric track names collapse to an ellipsis, words truncate alphabetically.
fn fallback_token(tok: &str) -> String {
    if tok.chars().all(|c| c.is_ascii_digit()) && tok.len() <= 8 {
        return "…".to_string();
    }
    let slug: String = tok
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .take(12)
        .collect();
    match slug.len() {
        0 => "…".to_string(),
        1 | 2 => slug,
        _ => slug.chars().take(3).collect(),
    }
}

fn primary_lang_token(raw: &str) -> &str {
    raw.split(['-', '_', ' '])
        .find(|t| !t.is_empty())
        .map(str::trim)
        .unwrap_or(raw)
        .trim()
}

#[cfg(test)]
mod tests {
    use super::abbrev_track_lang;

    #[test]
    fn iso639_1_to_short_display() {
        assert_eq!(abbrev_track_lang(Some("en")), "eng");
        assert_eq!(abbrev_track_lang(Some("RU")), "rus");
        assert_eq!(abbrev_track_lang(Some("ja")), "jpn");
    }

    #[test]
    fn bcp47_primary_subtag() {
        assert_eq!(abbrev_track_lang(Some("en-US")), "eng");
    }

    #[test]
    fn three_letter_pass_through() {
        assert_eq!(abbrev_track_lang(Some("eng")), "eng");
        assert_eq!(abbrev_track_lang(Some("rus")), "rus");
    }

    #[test]
    fn long_word_truncates_alphabetically() {
        assert_eq!(abbrev_track_lang(Some("English")), "eng");
        assert_eq!(abbrev_track_lang(Some("russian")), "rus");
    }
}
