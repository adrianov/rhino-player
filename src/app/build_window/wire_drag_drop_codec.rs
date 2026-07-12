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
            _ => std::iter::once(h).chain(ln).flat_map(paths_from_uri_line).collect(),
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
    let gf = gio::File::static_type();
    let gs = glib::types::Type::STRING;

    if !typ.is_valid() || typ == glib::types::Type::INVALID {
        return Vec::new();
    }

    if typ == fl || val.is::<gtk::gdk::FileList>() {
        let Ok(list) = val.get_owned::<gtk::gdk::FileList>() else {
            return Vec::new();
        };
        local_paths_from_gfiles(&list.files())
    } else if typ == gf || val.is::<gio::File>() {
        let Ok(f) = val.get_owned::<gio::File>() else {
            return Vec::new();
        };
        f.path().into_iter().collect()
    } else if typ == gs || val.is::<String>() || val.is::<glib::GString>() {
        uri_paths_from_utf8_value(val)
    } else if let Ok(var) = val.get_owned::<glib::Variant>() {
        paths_from_uri_list_text(var.to_string().trim())
    } else {
        Vec::new()
    }
}

/// Every MIME advertised by `GdkDrop`: known-good types first (see `DROP_READ_MIME_PREF`), rest in
/// offer order (`read_async` tries in sequence).
fn mime_types_ordered_for_drop_read(dk: &gtk::gdk::Drop) -> Vec<String> {
    let mime_offer = dk.formats().mime_types();
    let offered: Vec<&str> = mime_offer.iter().map(|m| m.as_str()).collect();
    let mut out = Vec::new();
    let mut seen = HashSet::<String>::new();

    fn push_seen(out: &mut Vec<String>, seen: &mut HashSet<String>, s: impl Into<String>) {
        let s = s.into();
        if seen.insert(s.clone()) {
            out.push(s);
        }
    }

    for cand in DROP_READ_MIME_PREF {
        if offered.contains(cand) {
            push_seen(&mut out, &mut seen, *cand);
        }
    }
    for gs in dk.formats().mime_types() {
        let raw = gs.as_str();
        if mime_base(raw).to_ascii_lowercase().starts_with("text/") {
            push_seen(&mut out, &mut seen, raw.to_string());
        }
    }
    for gs in dk.formats().mime_types() {
        push_seen(&mut out, &mut seen, gs.as_str().to_owned());
    }
    out
}

fn drag_dest_formats_union() -> gtk::gdk::ContentFormats {
    gtk::gdk::ContentFormats::for_type(gtk::gdk::FileList::static_type())
        .union(&gtk::gdk::ContentFormats::for_type(gio::File::static_type()))
        .union(&gtk::gdk::ContentFormats::new(DROP_READ_MIME_PREF))
}
