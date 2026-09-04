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
