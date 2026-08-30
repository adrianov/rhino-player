#[test]
fn homebrew_vapoursynth_lib_if_installed() {
    if !Path::new("/opt/homebrew/opt/vapoursynth").exists()
        && !Path::new("/usr/local/opt/vapoursynth").exists()
    {
        return;
    }
    let dir = macos_vapoursynth_lib_dir().expect("vapoursynth installed but lib dir missing");
    assert!(dir.join(VSSCRIPT_DYLIB).is_file());
    let dyld = macos_vapoursynth_dyld_paths().expect("dyld paths");
    assert!(!dyld.is_empty());
}

#[test]
fn ensure_repairs_stale_cellar_python() {
    if macos_vapoursynth_lib_dir().is_none() {
        return;
    }
    let _lock = TOML_LOCK.lock().unwrap();
    let _xdg = TempXdg::enter();
    let toml = macos_vapoursynth_toml_path().unwrap();
    let key_s = stale_cellar_key();
    write_stale_cellar_toml(&toml, &key_s);

    macos_ensure_vapoursynth_python_config();

    let text = std::fs::read_to_string(&toml).expect("toml readable");
    assert!(text.contains("# keep-me"));
    assert!(text.contains("\"/other/libvsscript.dylib\""));
    assert_eq!(
        text.lines()
            .filter_map(parse_vs_toml_line)
            .filter(|(k, _, _)| same_vsscript_key(k, &key_s))
            .count(),
        1,
        "duplicate keys collapsed"
    );
    assert_repaired_entry(&text, &key_s);
}

#[test]
fn ensure_is_noop_when_opt_mapping_ok() {
    if macos_vapoursynth_lib_dir().is_none() {
        return;
    }
    let _lock = TOML_LOCK.lock().unwrap();
    let _xdg = TempXdg::enter();
    let toml = macos_vapoursynth_toml_path().unwrap();
    macos_ensure_vapoursynth_python_config();
    assert_ensure_leaves_toml_unchanged(&toml);
}

#[test]
fn parse_vs_toml_line_smoke() {
    let (k, e, l) = parse_vs_toml_line(
        r#""/tmp/libvsscript.dylib" = ["/opt/exe/python3","/opt/python@3.14/Python"]"#,
    )
    .expect("parse");
    assert_eq!(k, "/tmp/libvsscript.dylib");
    assert_eq!(e, "/opt/exe/python3");
    assert_eq!(l, "/opt/python@3.14/Python");
}

#[test]
fn parse_vs_toml_line_with_vsscript_mtime() {
    let (k, e, l) = parse_vs_toml_line(
        r#""/tmp/libvsscript.dylib" = ["/opt/exe/python3","/Cellar/python@3.14/3.14.7/Python","1788090789"]"#,
    )
    .expect("parse");
    assert_eq!(k, "/tmp/libvsscript.dylib");
    assert_eq!(e, "/opt/exe/python3");
    assert_eq!(l, "/Cellar/python@3.14/3.14.7/Python");
}

#[test]
fn ensure_is_noop_when_live_cellar_mapping() {
    if macos_vapoursynth_lib_dir().is_none() {
        return;
    }
    let Some(cellar_lib) = live_cellar_python_lib() else {
        return;
    };
    let _lock = TOML_LOCK.lock().unwrap();
    let _xdg = TempXdg::enter();
    let toml = macos_vapoursynth_toml_path().unwrap();
    write_live_cellar_toml(
        &toml,
        &stale_cellar_key(),
        &macos_vs_python_exe().unwrap(),
        &cellar_lib,
    );
    assert_ensure_leaves_toml_unchanged(&toml);
}

#[test]
fn merge_vs_toml_mapping_preserves_other_lines() {
    let out = merge_vs_toml_mapping(
        "\
# comment\n\
\"/a/libvsscript.dylib\" = [\"/a/exe\",\"/a/lib\"]\n\
\"/b/libvsscript.dylib\" = [\"/bad\",\"/Cellar/python@3.14/x\"]\n\
\"/b/libvsscript.dylib\" = [\"/dup\",\"/Cellar/python@3.14/y\"]\n\
",
        "/b/libvsscript.dylib",
        "/opt/exe",
        "/opt/python@3.14/Python",
    );
    assert!(out.starts_with("# comment\n"));
    assert!(out.contains("\"/a/libvsscript.dylib\" = [\"/a/exe\",\"/a/lib\"]"));
    assert_eq!(
        out.matches("\"/b/libvsscript.dylib\"").count(),
        1,
        "duplicates dropped"
    );
    assert!(out.contains("\"/b/libvsscript.dylib\" = [\"/opt/exe\",\"/opt/python@3.14/Python\"]"));
    assert!(!out.contains("/Cellar/python@"));

    let appended = merge_vs_toml_mapping("# only\n", "/new", "/e", "/opt/python@3.14/P");
    assert!(appended.contains("# only\n"));
    assert!(appended.contains("\"/new\" = [\"/e\",\"/opt/python@3.14/P\"]\n"));
}
