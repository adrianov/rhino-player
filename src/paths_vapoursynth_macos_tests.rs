use super::*;

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
    let toml = macos_vapoursynth_toml_path().expect("toml path");
    let vs = macos_vapoursynth_lib_dir().unwrap().join(VSSCRIPT_DYLIB);
    let key = std::fs::canonicalize(&vs).unwrap_or(vs);
    std::fs::create_dir_all(toml.parent().unwrap()).unwrap();
    std::fs::write(
        &toml,
        format!(
            "{} = [\"/bad/python\",\"/opt/homebrew/Cellar/python@3.14/3.14.5/Frameworks/Python.framework/Versions/3.14/Python\"]\n",
            toml_escape(&key.to_string_lossy())
        ),
    )
    .unwrap();

    macos_ensure_vapoursynth_python_config();

    let text = std::fs::read_to_string(&toml).expect("toml readable");
    let (_k, exe, lib) = text
        .lines()
        .find_map(parse_vs_toml_line)
        .expect("toml entry");
    assert!(macos_vs_python_mapping_ok(&exe, &lib), "exe={exe} lib={lib}");
}

#[test]
fn ensure_is_noop_when_opt_mapping_ok() {
    if macos_vapoursynth_lib_dir().is_none() {
        return;
    }
    macos_ensure_vapoursynth_python_config();
    let toml = macos_vapoursynth_toml_path().expect("toml path");
    let before = std::fs::read_to_string(&toml).unwrap();
    let meta = std::fs::metadata(&toml).unwrap();
    let mtime = meta.modified().unwrap();

    std::thread::sleep(std::time::Duration::from_millis(20));
    macos_ensure_vapoursynth_python_config();

    let after = std::fs::read_to_string(&toml).unwrap();
    assert_eq!(before, after);
    assert_eq!(std::fs::metadata(&toml).unwrap().modified().unwrap(), mtime);
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
