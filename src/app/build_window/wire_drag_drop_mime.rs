use std::collections::HashSet;

const DROP_READ_MIME_PREF: &[&str] = &[
    "text/uri-list",
    "text/plain;charset=utf-8",
    "text/plain",
    "x-special/gnome-copied-files",
];

/// Every MIME advertised by `GdkDrop`: known-good types first (see `DROP_READ_MIME_PREF`), rest in
/// offer order (`read_async` tries in sequence).
fn mime_types_ordered_for_drop_read(dk: &gtk::gdk::Drop) -> Vec<String> {
    let raws: Vec<&str> = dk.formats().mime_types().iter().map(|m| m.as_str()).collect();

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
