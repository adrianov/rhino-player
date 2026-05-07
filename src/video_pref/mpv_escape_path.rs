/// Wrap paths for mpv `vf` / `vapoursynth:file=` when they contain characters that split sub-options
/// (`:`, `,`, `=`) or start a bracket string (`[`, `]`, space). Inside `[…]`, `\` and `]` are escaped
/// per mpv’s string rules so a trailing `]` in a path does not truncate the filter.
pub(crate) fn mpv_escape_path(p: &str) -> String {
    let needs_brackets = p.contains(':')
        || p.contains(' ')
        || p.contains('[')
        || p.contains(']')
        || p.contains(',')
        || p.contains('=')
        || p.contains('\\');
    if !needs_brackets {
        return p.to_string();
    }
    let mut inner = String::with_capacity(p.len() + 8);
    for ch in p.chars() {
        match ch {
            '\\' => inner.push_str(r"\\"),
            ']' => inner.push_str(r"\]"),
            _ => inner.push(ch),
        }
    }
    format!("[{inner}]")
}

#[cfg(test)]
mod mpv_escape_path_tests {
    use super::mpv_escape_path;

    #[test]
    fn unix_path_without_meta_is_unchanged() {
        assert_eq!(
            mpv_escape_path("/home/u/vs/rhino_60_mvtools.vpy"),
            "/home/u/vs/rhino_60_mvtools.vpy"
        );
    }

    #[test]
    fn space_colon_eq_comma_use_brackets() {
        assert_eq!(
            mpv_escape_path("/a b/c:d=e,f.vpy"),
            r"[/a b/c:d=e,f.vpy]"
        );
    }

    #[test]
    fn close_bracket_is_escaped_inside_brackets() {
        assert_eq!(mpv_escape_path(r"/x]y.vpy"), r"[/x\]y.vpy]");
    }

    #[test]
    fn backslash_doubled_inside_brackets() {
        assert_eq!(mpv_escape_path(r"/a\b.vpy"), r"[/a\\b.vpy]");
    }
}
