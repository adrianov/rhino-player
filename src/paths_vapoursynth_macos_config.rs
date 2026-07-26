// Keep `~/.config/vapoursynth/vapoursynth.toml` pointed at a live Homebrew Python.
// VSScript caches absolute Cellar paths (`python@3.14/3.14.5/...`); `brew upgrade python`
// removes that keg and Smooth dies with "Python library failed to load".

#[cfg(target_os = "macos")]
fn macos_vs_python_ver(vs_lib: &Path) -> Option<String> {
    // …/lib/python3.14/site-packages/vapoursynth → "3.14"
    let name = vs_lib.parent()?.parent()?.file_name()?.to_str()?;
    name.strip_prefix("python").map(str::to_string)
}

#[cfg(target_os = "macos")]
fn macos_opt_python_lib(ver: &str) -> Option<PathBuf> {
    for prefix in VS_HOMEBREW_PREFIXES {
        let p = Path::new(prefix).join(format!(
            "opt/python@{ver}/Frameworks/Python.framework/Versions/{ver}/Python"
        ));
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn macos_vs_python_exe() -> Option<PathBuf> {
    for prefix in VS_HOMEBREW_PREFIXES {
        let exe = Path::new(prefix).join("opt/vapoursynth/libexec/bin/python3");
        if exe.is_file() {
            return Some(exe);
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn macos_vapoursynth_toml_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("vapoursynth/vapoursynth.toml"))
}

#[cfg(target_os = "macos")]
fn toml_escape(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('\"', "\\\""))
}

/// Parse `"key" = ["exe","lib"]` → `(key, exe, lib)`.
#[cfg(target_os = "macos")]
fn parse_vs_toml_line(line: &str) -> Option<(String, String, String)> {
    let line = line.trim();
    let (key_part, rest) = line.split_once("\" = [\"")?;
    let key = key_part.strip_prefix('\"')?.to_string();
    let rest = rest.strip_suffix(']')?;
    let (exe, lib) = rest.split_once("\",\"")?;
    Some((key, exe.to_string(), lib.trim_end_matches('\"').to_string()))
}

#[cfg(target_os = "macos")]
fn same_vsscript_key(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

/// Live + not a Cellar patch pin (those break on `brew upgrade python`).
#[cfg(target_os = "macos")]
fn macos_vs_python_mapping_ok(exe: &str, lib: &str) -> bool {
    Path::new(exe).is_file()
        && Path::new(lib).is_file()
        && !lib.contains("/Cellar/python@")
        && lib.contains("/opt/python@")
}

/// Rewrite VSScript → Python mapping to stable Homebrew **`opt/python@*`** when stale/missing.
#[cfg(target_os = "macos")]
pub(crate) fn macos_ensure_vapoursynth_python_config() {
    let Some(vs_lib) = macos_vapoursynth_lib_dir() else {
        return;
    };
    let vsscript = vs_lib.join(VSSCRIPT_DYLIB);
    if !vsscript.is_file() {
        return;
    }
    let key = std::fs::canonicalize(&vsscript).unwrap_or(vsscript);
    let Some(ver) = macos_vs_python_ver(&vs_lib) else {
        eprintln!(
            "[rhino] video: cannot derive Python version from {}",
            vs_lib.display()
        );
        return;
    };
    let Some(py_lib) = macos_opt_python_lib(&ver) else {
        eprintln!(
            "[rhino] video: Homebrew Python.framework missing for python@{ver} — \
             `brew install python@{ver}`"
        );
        return;
    };
    let Some(py_exe) = macos_vs_python_exe() else {
        eprintln!(
            "[rhino] video: VapourSynth libexec python3 missing (brew install vapoursynth)"
        );
        return;
    };
    let Some(toml_path) = macos_vapoursynth_toml_path() else {
        return;
    };

    let key_s = key.to_string_lossy();
    let want_lib = py_lib.to_string_lossy();
    let want_exe = py_exe.to_string_lossy();

    if let Ok(text) = std::fs::read_to_string(&toml_path) {
        for line in text.lines() {
            let Some((k, exe, lib)) = parse_vs_toml_line(line) else {
                continue;
            };
            if !same_vsscript_key(&k, key_s.as_ref()) {
                continue;
            }
            if macos_vs_python_mapping_ok(&exe, &lib) {
                return;
            }
            break;
        }
    }

    if let Some(parent) = toml_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let line = format!(
        "{} = [{},{}]\n",
        toml_escape(&key_s),
        toml_escape(&want_exe),
        toml_escape(&want_lib)
    );
    match std::fs::write(&toml_path, line) {
        Ok(()) => eprintln!(
            "[rhino] video: refreshed {} → {}",
            toml_path.display(),
            py_lib.display()
        ),
        Err(e) => eprintln!(
            "[rhino] video: failed to write {}: {e}",
            toml_path.display()
        ),
    }
}
