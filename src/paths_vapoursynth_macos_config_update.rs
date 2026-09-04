// Resolution and application of one VSScript→Python mapping rewrite in `vapoursynth.toml`.

/// Inputs for one VSScript→Python mapping rewrite.
#[cfg(target_os = "macos")]
struct VsTomlUpdate {
    key: String,
    exe: String,
    lib: PathBuf,
    toml_path: PathBuf,
}

#[cfg(target_os = "macos")]
fn resolve_vs_toml_update(vs_lib: &Path) -> Option<VsTomlUpdate> {
    let key = vs_script_key(vs_lib)?;
    let Some(ver) = macos_vs_python_ver(vs_lib) else {
        eprintln!(
            "[rhino] video: cannot derive Python version from {}",
            vs_lib.display()
        );
        return None;
    };
    let (py_exe, py_lib) = homebrew_python_paths(&ver)?;
    let toml_path = macos_vapoursynth_toml_path()?;
    Some(VsTomlUpdate {
        key,
        exe: py_exe.to_string_lossy().into_owned(),
        lib: py_lib,
        toml_path,
    })
}

/// Canonicalized [`VSSCRIPT_DYLIB`] path — VSScript's own mapping key.
#[cfg(target_os = "macos")]
fn vs_script_key(vs_lib: &Path) -> Option<String> {
    let vsscript = vs_lib.join(VSSCRIPT_DYLIB);
    if !vsscript.is_file() {
        return None;
    }
    Some(
        std::fs::canonicalize(&vsscript)
            .unwrap_or(vsscript)
            .to_string_lossy()
            .into_owned(),
    )
}

/// Stable Homebrew `opt/python@*` framework library and the libexec `python3`.
#[cfg(target_os = "macos")]
fn homebrew_python_paths(ver: &str) -> Option<(PathBuf, PathBuf)> {
    let Some(py_lib) = macos_opt_python_lib(ver) else {
        eprintln!(
            "[rhino] video: Homebrew Python.framework missing for python@{ver} — \
             `brew install python@{ver}`"
        );
        return None;
    };
    let Some(py_exe) = macos_vs_python_exe() else {
        eprintln!("[rhino] video: VapourSynth libexec python3 missing (brew install vapoursynth)");
        return None;
    };
    Some((py_exe, py_lib))
}

#[cfg(target_os = "macos")]
fn apply_vs_toml_update(update: &VsTomlUpdate) {
    let existing = std::fs::read_to_string(&update.toml_path).unwrap_or_default();
    if vs_toml_mapping_already_ok(&existing, &update.key) {
        return;
    }

    if let Some(parent) = update.toml_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(
        &update.toml_path,
        merge_vs_toml_mapping(
            &existing,
            &update.key,
            &update.exe,
            &update.lib.to_string_lossy(),
        ),
    ) {
        Ok(()) => eprintln!(
            "[rhino] video: refreshed {} → {}",
            update.toml_path.display(),
            update.lib.display()
        ),
        Err(e) => eprintln!(
            "[rhino] video: failed to write {}: {e}",
            update.toml_path.display()
        ),
    }
}
