struct Patterns {
    strip_ext: Regex,
    glued_dots: Regex,
    tech_hint: Regex,
    se: Regex,
    season_range: Regex,
    n_by_ep: Regex,
    season_only: Regex,
    year_ellipsis: Regex,
    paren_open_space: Regex,
    paren_close_space: Regex,
    word_then_paren: Regex,
    curly: Regex,
    merged_rip: Regex,
    bluray: Regex,
    resolution: Regex,
    season_leftover: Regex,
    date_long: Regex,
    date_short: Regex,
    standalone_hyphen: Regex,
    dot_space_dot: Regex,
    space_dot_word: Regex,
    trailing_space_dot: Regex,
    space_dot_space: Regex,
    strip_end_dot_word: Regex,
    empty_parens: Regex,
    hd_sd_parens: Regex,
    comma_double: Regex,
    comma_after_open: Regex,
    comma_before_close: Regex,
}

fn regex(pat: &str, label: &str) -> Regex {
    Regex::new(pat).expect(label)
}

fn core_regexes() -> (Regex, Regex, Regex) {
    (
        regex(r"(?i)\.[a-z0-9]{2,5}$", "strip_ext"),
        regex(r"[\p{L}\p{N}]+\.[\p{L}\p{N}]+\.[\p{L}\p{N}]+", "glued"),
        regex(
            r"(?i)\b(?:2160p|1080p|720p|480p|8K|4K|UHD|S\d{1,2}(?:E\d+)?|(?:19|20)\d{2}|DVD|BD\d*|BD|WEB|Rip|BDRemux|Remux|HEVC|H264|H\.?264|H265|H\.?265|x264|x265|AAC|AC3|DTS|FLAC|MP3|MKV|MP4)\b",
            "tech_hint",
        ),
    )
}

fn season_regexes() -> (Regex, Regex, Regex, Regex) {
    (
        regex(r"(?i)\bS(\d{1,2})E(\d{1,3})\b", "se"),
        regex(r"(?i)\bS(\d{1,2})[-–](\d{1,2})\b", "sr"),
        // Two-digit episode segment avoids matching encoder tokens like `10x264`.
        regex(r"(?i)\b(\d{1,2})x(\d{2})\b", "nx"),
        regex(r"(?i)\bS(\d{1,2})\b", "sonly"),
    )
}

fn tag_regexes() -> (Regex, Regex, Regex, Regex, Regex, Regex) {
    (
        regex(r"(?:19|20)\d{2}(?:\.{2,}|\u{2026})(?:19|20)\d{2}", "yrell"),
        regex(r"\{[^}]*\}", "curly"),
        regex(
            r"(?i)(BDRip|HDRip|DVDRip|WEBRip)(1080p|720p|2160p|480p)",
            "mrip",
        ),
        regex(r"(?i)(?:^|[.\s])Blu[\s-]*Ray(?:$|[.\s])", "bluray"),
        regex(r"(?i)\.?#?\b(?:2160p|1080p|720p|480p|8K|4K|UHD)\b", "res"),
        regex(r"(?i)\.?S\d{1,2}(?:[-–]\d{1,2})?(?:E\d+)?\b", "sleft"),
    )
}

fn paren_regexes() -> (Regex, Regex, Regex) {
    (
        regex(r"\(\s+", "po"),
        regex(r"\s+\)", "pc"),
        regex(r"([\p{L}\p{N}])\(", "wtp"),
    )
}

fn date_regexes() -> (Regex, Regex, Regex) {
    (
        regex(r"\(?\d{2}\.\d{2}\.\d{4}\)?", "dlong"),
        regex(r"\(?\d{2}\.\d{2}\.\d{2}\)?", "dshort"),
        regex(r"(?:^|\s)-(?:\s|$)", "hyp"),
    )
}

fn dot_edge_regexes() -> (Regex, Regex, Regex, Regex, Regex) {
    (
        regex(r"\. +\.", "dsd"),
        regex(r" +\.(\w)", "sdw"),
        regex(r" +\.$", "tsd"),
        regex(r" +\. ", "sds"),
        regex(r"(?m)([^.])\.$", "sed"),
    )
}

fn paren_comma_regexes() -> (Regex, Regex, Regex, Regex, Regex) {
    (
        regex(r"\(\s*\)", "emp"),
        regex(r"\(\s*(?:HD|SD)\s*\)", "hdsd"),
        regex(r",\s*,", "cdbl"),
        regex(r"\(\s*,", "cao"),
        regex(r",\s*\)", "cbc"),
    )
}

fn build_patterns() -> Patterns {
    let core = core_regexes();
    let season = season_regexes();
    let tags = tag_regexes();
    let parens = paren_regexes();
    let dates = date_regexes();
    let dots = dot_edge_regexes();
    let commas = paren_comma_regexes();
    Patterns {
        strip_ext: core.0,
        glued_dots: core.1,
        tech_hint: core.2,
        se: season.0,
        season_range: season.1,
        n_by_ep: season.2,
        season_only: season.3,
        year_ellipsis: tags.0,
        curly: tags.1,
        merged_rip: tags.2,
        bluray: tags.3,
        resolution: tags.4,
        season_leftover: tags.5,
        paren_open_space: parens.0,
        paren_close_space: parens.1,
        word_then_paren: parens.2,
        date_long: dates.0,
        date_short: dates.1,
        standalone_hyphen: dates.2,
        dot_space_dot: dots.0,
        space_dot_word: dots.1,
        trailing_space_dot: dots.2,
        space_dot_space: dots.3,
        strip_end_dot_word: dots.4,
        empty_parens: commas.0,
        hd_sd_parens: commas.1,
        comma_double: commas.2,
        comma_after_open: commas.3,
        comma_before_close: commas.4,
    }
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(build_patterns)
}
