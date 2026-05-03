/// Mirrors Transmission `techTags*` lists (resolution suffixes stripped separately).
const TECH_TAGS: &[&str] = &[
    "WEBDL",
    "WEB-DL",
    "WEBRip",
    "WEB-DLRip",
    "DLRip",
    "BDRip",
    "BDRemux",
    "BluRay",
    "HDRip",
    "DVDRip",
    "HDTV",
    "HEVC",
    "H264",
    "H.264",
    "H265",
    "H.265",
    "x264",
    "x265",
    "AVC",
    "10bit",
    "AAC",
    "AC3",
    "DTS",
    "Atmos",
    "TrueHD",
    "FLAC",
    "EAC3",
    "SDR",
    "HDR",
    "HDR10",
    "DV",
    "DoVi",
    "AMZN",
    "NF",
    "DSNP",
    "HMAX",
    "PCOK",
    "ATVP",
    "APTV",
    "ExKinoRay",
    "RuTracker",
    "LostFilm",
    "IMAX",
    "REPACK",
    "PROPER",
    "EXTENDED",
    "UNRATED",
    "REMUX",
    "HDCLUB",
    "Jaskier",
    "MVO",
    "DVD5",
    "DVD9",
    "DVD",
    "BD25",
    "BD50",
    "BD66",
    "BD100",
    "COMPLETE",
    "INTERNAL",
    "READNFO",
    "Subs",
    "TeamHD",
];

fn tech_regexes() -> &'static [Regex] {
    static V: OnceLock<Vec<Regex>> = OnceLock::new();
    V.get_or_init(|| {
        TECH_TAGS
            .iter()
            .map(|tag| {
                Regex::new(&format!(
                    r"(?i)(?:^|[.\s]){}(?:$|[.\s])",
                    regex::escape(tag)
                ))
                .unwrap_or_else(|_| panic!("tech tag regex {tag}"))
            })
            .collect()
    })
}

fn extra_regexes() -> &'static [Regex] {
    static V: OnceLock<Vec<Regex>> = OnceLock::new();
    V.get_or_init(|| {
        [
            r"(?i)\b\d+xRus\b",
            r"(?i)\b(?:Eng|Rus|Multi)\b",
            r"(?i)\b(?:XviD|DivX)\b",
            r"(?i)\(?МР3\)?",
            r"(?i)\(?МРЗ\)?",
        ]
        .into_iter()
        .map(|p| Regex::new(p).expect("extra pat"))
        .collect()
    })
}
