//! Episode marker in a release name (`E06`, `EP06`, `episode 6`, `11 сер.`).

use regex::Regex;
use std::sync::OnceLock;

pub(super) fn is_match(name: &str) -> bool {
    regs().iter().any(|re| re.is_match(name))
}

/// First episode number, then blank every match of that form.
pub(super) fn take(work: &mut String) -> Option<u32> {
    regs().iter().find_map(|re| pull(work, re))
}

fn pull(work: &mut String, re: &Regex) -> Option<u32> {
    let n = re.captures(work)?.get(1)?.as_str().parse().ok()?;
    *work = re.replace_all(work, " ").into_owned();
    Some(n)
}

fn regs() -> &'static [Regex; 4] {
    static RE: OnceLock<[Regex; 4]> = OnceLock::new();
    RE.get_or_init(|| {
        [
            compile(
                r"(?i)\b(?:(?:episode|серия|серии|сер)\b[\s._-]*|ep\.?[\s._-]*)(\d{1,3})\b",
            ),
            compile(r"(?i)\bE[\s._-]?(\d{1,3})\b"),
            compile(r"(?i)\(\s*(\d{1,3})[\s._-]*(?:episode|ep\.?|серия|серии|сер\.?)\s*\)"),
            compile(r"(?i)\b(\d{1,3})[\s._-]*(?:(?:episode|серия|серии|сер)\b|ep\.?\b)"),
        ]
    })
}

fn compile(pat: &str) -> Regex {
    Regex::new(pat).expect("episode")
}
