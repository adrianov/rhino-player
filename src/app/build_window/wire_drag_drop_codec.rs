use std::borrow::Cow;
use std::collections::HashSet;

const DROP_READ_MIME_PREF: &[&str] = &[
    "text/uri-list",
    "text/plain;charset=utf-8",
    "text/plain",
    "x-special/gnome-copied-files",
];

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
    let uri_part = line.split_whitespace().next().unwrap_or(line);
    uri_to_local_path(uri_part)
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

fn local_paths_from_gfiles(files: &[gio::File]) -> Vec<PathBuf> {
    files.iter().filter_map(|f| f.path()).collect()
}

fn uri_paths_from_utf8_value(val: &glib::Value) -> Vec<PathBuf> {
    if let Ok(s) = val.get_owned::<String>() {
        paths_from_uri_list_text(s.as_str())
    } else if let Ok(gs) = val.get_owned::<glib::GString>() {
        paths_from_uri_list_text(gs.as_str())
    } else {
        Vec::new()
    }
}

fn paths_from_gvalue(typ: glib::types::Type, val: &glib::Value) -> Vec<PathBuf> {
    use glib::types::StaticType;

    let fl = gtk::gdk::FileList::static_type();
    if !typ.is_valid() || typ == glib::types::Type::INVALID {
        return Vec::new();
    }
    if typ == fl || val.is::<gtk::gdk::FileList>() {
        return paths_from_gvalue_file_list(val);
    }
    paths_from_gvalue_typed(typ, val)
}

/// Remaining typed branches after the file-list check: `GFile`, URI text, or variant fallback.
fn paths_from_gvalue_typed(typ: glib::types::Type, val: &glib::Value) -> Vec<PathBuf> {
    use glib::types::StaticType;

    if typ == gio::File::static_type() || val.is::<gio::File>() {
        return paths_from_gvalue_gfile(val);
    }
    if typ == glib::types::Type::STRING || val.is::<String>() || val.is::<glib::GString>() {
        return uri_paths_from_utf8_value(val);
    }
    match val.get_owned::<glib::Variant>() {
        Ok(var) => paths_from_uri_list_text(var.to_string().trim()),
        Err(_) => Vec::new(),
    }
}

fn paths_from_gvalue_file_list(val: &glib::Value) -> Vec<PathBuf> {
    match val.get_owned::<gtk::gdk::FileList>() {
        Ok(list) => local_paths_from_gfiles(&list.files()),
        Err(_) => Vec::new(),
    }
}

fn paths_from_gvalue_gfile(val: &glib::Value) -> Vec<PathBuf> {
    val.get_owned::<gio::File>()
        .ok()
        .and_then(|f| f.path())
        .into_iter()
        .collect()
}

/// Every MIME advertised by `GdkDrop`: known-good types first (see `DROP_READ_MIME_PREF`), rest in
/// offer order (`read_async` tries in sequence).
fn mime_types_ordered_for_drop_read(dk: &gtk::gdk::Drop) -> Vec<String> {
    let mimes = dk.formats().mime_types();
    let raws: Vec<&str> = mimes.iter().map(|m| m.as_str()).collect();

    let mut ordered = preferred_drop_mimes(&raws);
    ordered.extend(text_drop_mimes(&raws));
    ordered.extend(raws.iter().copied());
    dedup_preserving_order(ordered.into_iter())
}

/// Known-good MIME types (see `DROP_READ_MIME_PREF`) that the drop actually offers.
fn preferred_drop_mimes<'a>(raws: &'a [&'a str]) -> Vec<&'a str> {
    DROP_READ_MIME_PREF
        .iter()
        .copied()
        .filter(|cand| raws.contains(cand))
        .collect()
}

/// Offered MIME types usable as URI/plain text payloads.
fn text_drop_mimes<'a>(raws: &'a [&'a str]) -> Vec<&'a str> {
    raws.iter()
        .copied()
        .filter(|raw| mime_base(raw).to_ascii_lowercase().starts_with("text/"))
        .collect()
}

fn dedup_preserving_order<'a>(cands: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::<String>::new();
    for s in cands {
        if seen.contains(s) {
            continue;
        }
        seen.insert(s.to_owned());
        out.push(s.to_owned());
    }
    out
}

fn drag_dest_formats_union() -> gtk::gdk::ContentFormats {
    gtk::gdk::ContentFormats::for_type(gtk::gdk::FileList::static_type())
        .union(&gtk::gdk::ContentFormats::for_type(gio::File::static_type()))
        .union(&gtk::gdk::ContentFormats::new(DROP_READ_MIME_PREF))
}
