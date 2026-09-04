use std::borrow::Cow;

fn mime_base(mime: &str) -> &str {
    mime.split(';').next().unwrap_or(mime).trim()
}

fn paths_from_uri_list_text(s: &str) -> Vec<PathBuf> {
    s.lines().flat_map(paths_from_uri_line).collect()
}

fn paths_from_uri_line(trimmed_line: &str) -> Vec<PathBuf> {
    let line = trimmed_line.trim_end_matches('\r').trim_start();
    if line.is_empty() || line.starts_with('#') {
        return Vec::new();
    }
    uri_to_local_path(line.split_whitespace().next().unwrap_or(line))
}

fn uri_to_local_path(uri: &str) -> Vec<PathBuf> {
    let uri = uri.trim();
    if uri.is_empty() {
        return Vec::new();
    }
    if let Some(p) = gio::File::for_uri(uri).path() {
        return vec![p];
    }
    if let Ok((path, _)) = glib::filename_from_uri(uri) {
        return vec![path];
    }
    Vec::new()
}

fn paths_from_x_special(raw: &str) -> Vec<PathBuf> {
    let mut ln = raw.lines();
    match ln.next() {
        None => Vec::new(),
        Some(h) => match h.trim() {
            "copy" | "cut" | "link" => ln.flat_map(paths_from_uri_line).collect(),
            _ => std::iter::once(h)
                .chain(ln)
                .flat_map(paths_from_uri_line)
                .collect(),
        },
    }
}

fn paths_from_received_bytes(raw: &[u8], mime: &str) -> Vec<PathBuf> {
    let s = match std::str::from_utf8(raw) {
        Ok(s) => Cow::Borrowed(s),
        Err(_) => return Vec::new(),
    };
    match mime_base(mime).to_ascii_lowercase().as_str() {
        "x-special/gnome-copied-files" => paths_from_x_special(s.as_ref()),
        _ => paths_from_uri_list_text(s.as_ref()),
    }
}
