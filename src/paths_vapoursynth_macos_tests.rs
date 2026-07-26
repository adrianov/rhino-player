use super::*;
use std::sync::Mutex;

static TOML_LOCK: Mutex<()> = Mutex::new(());

/// Isolate `vapoursynth.toml` under a temp `XDG_CONFIG_HOME` (never touch the real file).
struct TempXdg {
    dir: PathBuf,
    prev: Option<std::ffi::OsString>,
}

impl TempXdg {
    fn enter() -> Self {
        let dir = std::env::temp_dir().join(format!("rhino-vs-toml-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let prev = std::env::var_os("XDG_CONFIG_HOME");
        // SAFETY: callers hold `TOML_LOCK`; no concurrent env mutation.
        unsafe { std::env::set_var("XDG_CONFIG_HOME", &dir) };
        Self { dir, prev }
    }
}

impl Drop for TempXdg {
    fn drop(&mut self) {
        match &self.prev {
            Some(v) => unsafe { std::env::set_var("XDG_CONFIG_HOME", v) },
            None => unsafe { std::env::remove_var("XDG_CONFIG_HOME") },
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

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
    let vs = macos_vapoursynth_lib_dir().unwrap().join(VSSCRIPT_DYLIB);
    let key = std::fs::canonicalize(&vs).unwrap_or(vs);
    let key_s = key.to_string_lossy();
    std::fs::create_dir_all(toml.parent().unwrap()).unwrap();
    std::fs::write(
        &toml,
        format!(
            "# keep-me\n\
             \"/other/libvsscript.dylib\" = [\"/other/exe\",\"/other/lib\"]\n\
             {0} = [\"/bad/python\",\"/opt/homebrew/Cellar/python@3.14/3.14.5/Frameworks/Python.framework/Versions/3.14/Python\"]\n\
             {0} = [\"/also/bad\",\"/opt/homebrew/Cellar/python@3.14/3.14.0/Frameworks/Python.framework/Versions/3.14/Python\"]\n",
            toml_escape(&key_s)
        ),
    )
    .unwrap();

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
    let (_k, exe, lib) = text
        .lines()
        .find_map(|line| {
            parse_vs_toml_line(line).filter(|(k, _, _)| same_vsscript_key(k, &key_s))
        })
        .expect("toml entry for our key");
    assert!(macos_vs_python_mapping_ok(&exe, &lib), "exe={exe} lib={lib}");
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
    let before = std::fs::read_to_string(&toml).unwrap();
    let mtime = std::fs::metadata(&toml).unwrap().modified().unwrap();

    std::thread::sleep(std::time::Duration::from_millis(20));
    macos_ensure_vapoursynth_python_config();

    assert_eq!(before, std::fs::read_to_string(&toml).unwrap());
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

#[test]
fn merge_vs_toml_mapping_preserves_other_lines() {
    let existing = "\
# comment\n\
\"/a/libvsscript.dylib\" = [\"/a/exe\",\"/a/lib\"]\n\
\"/b/libvsscript.dylib\" = [\"/bad\",\"/Cellar/python@3.14/x\"]\n\
\"/b/libvsscript.dylib\" = [\"/dup\",\"/Cellar/python@3.14/y\"]\n\
";
    let out = merge_vs_toml_mapping(
        existing,
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
    assert!(out.contains(
        "\"/b/libvsscript.dylib\" = [\"/opt/exe\",\"/opt/python@3.14/Python\"]"
    ));
    assert!(!out.contains("/Cellar/python@"));

    let appended = merge_vs_toml_mapping("# only\n", "/new", "/e", "/opt/python@3.14/P");
    assert!(appended.contains("# only\n"));
    assert!(appended.contains("\"/new\" = [\"/e\",\"/opt/python@3.14/P\"]\n"));
}
