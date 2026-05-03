//! Short subtitle language tokens for the header (e.g. `eng`, `rus`).

pub fn abbrev_track_lang(raw: Option<&str>) -> String {
    let Some(s) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return String::new();
    };
    let tok = primary_lang_token(s).to_lowercase();
    if tok.is_empty() {
        return String::new();
    }
    if tok.len() == 3 && tok.bytes().all(|b: u8| b.is_ascii_lowercase()) {
        return tok;
    }
    if tok.len() == 2 {
        let three = match tok.as_str() {
            "en" => "eng",
            "ru" => "rus",
            "ja" => "jpn",
            "ko" => "kor",
            "zh" => "zho",
            "pt" => "por",
            "es" => "spa",
            "fr" => "fra",
            "de" => "deu",
            "it" => "ita",
            "uk" => "ukr",
            "pl" => "pol",
            "tr" => "tur",
            "ar" => "ara",
            "hi" => "hin",
            _ => return tok,
        };
        return three.to_string();
    }
    if tok.chars().all(|c| c.is_ascii_digit()) && tok.len() <= 8 {
        return "…".to_string();
    }
    let slug: String = tok
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .take(12)
        .collect::<String>()
        .to_lowercase();
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
