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

fn stale_cellar_key() -> String {
    let vs = macos_vapoursynth_lib_dir().unwrap().join(VSSCRIPT_DYLIB);
    std::fs::canonicalize(&vs)
        .unwrap_or(vs)
        .to_string_lossy()
        .into_owned()
}

/// Two Cellar-pinned mappings for our key plus one unrelated entry to preserve.
fn write_stale_cellar_toml(toml: &Path, key_s: &str) {
    std::fs::create_dir_all(toml.parent().unwrap()).unwrap();
    std::fs::write(
        toml,
        format!(
            "# keep-me\n\
             \"/other/libvsscript.dylib\" = [\"/other/exe\",\"/other/lib\"]\n\
             {0} = [\"/bad/python\",\"/opt/homebrew/Cellar/python@3.14/3.14.5/Frameworks/Python.framework/Versions/3.14/Python\"]\n\
             {0} = [\"/also/bad\",\"/opt/homebrew/Cellar/python@3.14/3.14.0/Frameworks/Python.framework/Versions/3.14/Python\"]\n",
            toml_escape(key_s)
        ),
    )
    .unwrap();
}

fn assert_repaired_entry(text: &str, key_s: &str) {
    let (_k, exe, lib) = text
        .lines()
        .find_map(|line| parse_vs_toml_line(line).filter(|(k, _, _)| same_vsscript_key(k, key_s)))
        .expect("toml entry for our key");
    assert!(
        macos_vs_python_mapping_ok(&exe, &lib),
        "exe={exe} lib={lib}"
    );
}

fn toml_snapshot(toml: &Path) -> (String, std::time::SystemTime) {
    (
        std::fs::read_to_string(toml).unwrap(),
        std::fs::metadata(toml).unwrap().modified().unwrap(),
    )
}

fn assert_ensure_leaves_toml_unchanged(toml: &Path) {
    let (before_text, before_mtime) = toml_snapshot(toml);
    std::thread::sleep(std::time::Duration::from_millis(20));
    macos_ensure_vapoursynth_python_config();
    assert_eq!(before_text, std::fs::read_to_string(toml).unwrap());
    assert_eq!(
        std::fs::metadata(toml).unwrap().modified().unwrap(),
        before_mtime
    );
}

/// First live Homebrew Cellar `python@3.14/…/Python` framework lib, if any.
fn live_cellar_python_lib() -> Option<PathBuf> {
    Path::new("/opt/homebrew/Cellar/python@3.14")
        .read_dir()
        .ok()?
        .flatten()
        .map(|e| e.path().join("Frameworks/Python.framework/Versions/3.14/Python"))
        .find(|p| p.is_file())
}

fn write_live_cellar_toml(toml: &Path, key: &str, exe: &Path, lib: &Path) {
    std::fs::create_dir_all(toml.parent().unwrap()).unwrap();
    std::fs::write(
        toml,
        format!(
            "{} = [{},{},\"1788090789\"]\n",
            toml_escape(key),
            toml_escape(&exe.to_string_lossy()),
            toml_escape(&lib.to_string_lossy()),
        ),
    )
    .unwrap();
}
