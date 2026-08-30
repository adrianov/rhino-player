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

/// Parse `"key" = ["exe","lib"]` or VSScript's 3-field form `["exe","lib","mtime"]` → `(key, exe, lib)`.
#[cfg(target_os = "macos")]
fn parse_vs_toml_line(line: &str) -> Option<(String, String, String)> {
    let (key_part, rest) = line.trim().split_once("\" = [\"")?;
    let key = key_part.strip_prefix('\"')?.to_string();
    let (exe, lib) = parse_vs_toml_paths(rest.strip_suffix(']')?)?;
    Some((key, exe, lib))
}

#[cfg(target_os = "macos")]
fn parse_vs_toml_paths(body: &str) -> Option<(String, String)> {
    let (exe, after_exe) = body.split_once("\",\"")?;
    let lib = after_exe.split("\",\"").next()?.trim_end_matches('\"');
    Some((exe.to_string(), lib.to_string()))
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

/// Mapping is usable when both paths still exist. Prefer rewriting only when a Cellar pin is
/// dead (`brew upgrade python`); do not fight VSScript each launch when it rewrites a live
/// Cellar path (+ optional mtime field) back into the toml.
#[cfg(target_os = "macos")]
fn macos_vs_python_mapping_ok(exe: &str, lib: &str) -> bool {
    Path::new(exe).is_file() && Path::new(lib).is_file()
}

/// Replace or append the VSScript→Python mapping; keep every other line.
#[cfg(target_os = "macos")]
fn merge_vs_toml_mapping(existing: &str, key: &str, exe: &str, lib: &str) -> String {
    let mapping = format!(
        "{} = [{},{}]",
        toml_escape(key),
        toml_escape(exe),
        toml_escape(lib)
    );
    let mut out = String::new();
    let mut replaced = false;
    for line in existing.lines() {
        if let Some((k, _, _)) = parse_vs_toml_line(line) {
            if same_vsscript_key(&k, key) {
                if !replaced {
                    out.push_str(&mapping);
                    out.push('\n');
                    replaced = true;
                }
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    if !replaced {
        out.push_str(&mapping);
        out.push('\n');
    }
    out
}

#[cfg(target_os = "macos")]
fn vs_toml_mapping_already_ok(text: &str, key: &str) -> bool {
    text.lines().any(|line| {
        parse_vs_toml_line(line).is_some_and(|(k, exe, lib)| {
            same_vsscript_key(&k, key) && macos_vs_python_mapping_ok(&exe, &lib)
        })
    })
}

/// Rewrite VSScript → Python mapping to stable Homebrew **`opt/python@*`** when stale/missing.
#[cfg(target_os = "macos")]
pub(crate) fn macos_ensure_vapoursynth_python_config() {
    let Some(vs_lib) = macos_vapoursynth_lib_dir() else {
        return;
    };
    let Some(update) = resolve_vs_toml_update(&vs_lib) else {
        return;
    };
    apply_vs_toml_update(&update);
}

include!("paths_vapoursynth_macos_config_update.rs");
