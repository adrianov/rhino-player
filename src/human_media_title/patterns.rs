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
    year_token: Regex,
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
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        strip_ext: Regex::new(r"(?i)\.[a-z0-9]{2,5}$").expect("strip_ext"),
        glued_dots: Regex::new(r"[\p{L}\p{N}]+\.[\p{L}\p{N}]+\.[\p{L}\p{N}]+").expect("glued"),
        tech_hint: Regex::new(
            r"(?i)\b(?:2160p|1080p|720p|480p|8K|4K|UHD|S\d{1,2}(?:E\d+)?|(?:19|20)\d{2}|DVD|BD\d*|BD|WEB|Rip|HEVC|H264|H\.?264|H265|H\.?265|x264|x265|AAC|AC3|DTS|FLAC|MP3|MKV|MP4)\b",
        )
        .expect("tech_hint"),
        se: Regex::new(r"(?i)\bS(\d{1,2})E(\d{1,3})\b").expect("se"),
        season_range: Regex::new(r"(?i)\bS(\d{1,2})[-–](\d{1,2})\b").expect("sr"),
        // Two-digit episode segment avoids matching encoder tokens like `10x264`.
        n_by_ep: Regex::new(r"(?i)\b(\d{1,2})x(\d{2})\b").expect("nx"),
        season_only: Regex::new(r"(?i)\bS(\d{1,2})\b").expect("sonly"),
        year_ellipsis: Regex::new(r"(?:19|20)\d{2}(?:\.{2,}|\u{2026})(?:19|20)\d{2}").expect("yrell"),
        paren_open_space: Regex::new(r"\(\s+").expect("po"),
        paren_close_space: Regex::new(r"\s+\)").expect("pc"),
        word_then_paren: Regex::new(r"([\p{L}\p{N}])\(").expect("wtp"),
        curly: Regex::new(r"\{[^}]*\}").expect("curly"),
        merged_rip: Regex::new(r"(?i)(BDRip|HDRip|DVDRip|WEBRip)(1080p|720p|2160p|480p)").expect("mrip"),
        bluray: Regex::new(r"(?i)(?:^|[.\s])Blu[\s-]*Ray(?:$|[.\s])").expect("bluray"),
        resolution: Regex::new(
            r"(?i)\.?#?\b(?:2160p|1080p|720p|480p|8K|4K|UHD)\b",
        )
        .expect("res"),
        season_leftover: Regex::new(r"(?i)\.?S\d{1,2}(?:[-–]\d{1,2})?(?:E\d+)?\b").expect("sleft"),
        year_token: Regex::new(r"(?i)\.?\(?\b(?:19\d{2}|20\d{2})\b\)?").expect("year"),
        date_long: Regex::new(r"\(?\d{2}\.\d{2}\.\d{4}\)?").expect("dlong"),
        date_short: Regex::new(r"\(?\d{2}\.\d{2}\.\d{2}\)?").expect("dshort"),
        standalone_hyphen: Regex::new(r"(?:^|\s)-(?:\s|$)").expect("hyp"),
        dot_space_dot: Regex::new(r"\. +\.").expect("dsd"),
        space_dot_word: Regex::new(r" +\.(\w)").expect("sdw"),
        trailing_space_dot: Regex::new(r" +\.$").expect("tsd"),
        space_dot_space: Regex::new(r" +\. ").expect("sds"),
        strip_end_dot_word: Regex::new(r"(?m)([^.])\.$").expect("sed"),
        empty_parens: Regex::new(r"\(\s*\)").expect("emp"),
        hd_sd_parens: Regex::new(r"\(\s*(?:HD|SD)\s*\)").expect("hdsd"),
    })
}
