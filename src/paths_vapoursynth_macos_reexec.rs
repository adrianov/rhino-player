// execve plumbing for the one-time DYLD_LIBRARY_PATH re-exec ([macos_reexec_for_vapoursynth_dyld_if_needed]).
/// VapourSynth dirs prepended to any inherited `DYLD_LIBRARY_PATH`.
#[cfg(target_os = "macos")]
fn merged_dyld_value(add: &str) -> String {
    match std::env::var_os("DYLD_LIBRARY_PATH") {
        Some(cur) if !cur.is_empty() => format!("{add}:{}", cur.to_string_lossy()),
        _ => add.to_string(),
    }
}

/// Current environment with `DYLD_LIBRARY_PATH` replaced and the primed marker added.
#[cfg(target_os = "macos")]
fn reexec_env(dyld: &str) -> Vec<(OsString, OsString)> {
    let mut env: Vec<(OsString, OsString)> = std::env::vars_os().collect();
    env.retain(|(k, _)| k != "DYLD_LIBRARY_PATH");
    env.push(("DYLD_LIBRARY_PATH".into(), OsString::from(dyld)));
    env.push((DYLD_PRIMED_VAR.into(), "1".into()));
    env
}

/// `execve` this binary with `env`; exits(1) when the exec fails or the exe path is gone.
#[cfg(target_os = "macos")]
fn execve_reexec(env: Vec<(OsString, OsString)>) {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[rhino] video: re-exec skipped (current_exe: {e})");
            return;
        }
    };
    let arg_c = reexec_args_c();
    let env_c = env_pairs_c(&env);

    eprintln!("[rhino] video: re-exec for VapourSynth (DYLD_LIBRARY_PATH at process start)");
    unsafe {
        libc::execve(
            cstring_lossy(exe.as_os_str()).as_ptr(),
            c_ptr_array(&arg_c).as_ptr(),
            c_ptr_array(&env_c).as_ptr(),
        );
    }
    eprintln!(
        "[rhino] video: re-exec failed: {}",
        std::io::Error::last_os_error()
    );
    std::process::exit(1);
}

/// argv for re-exec: the current process arguments as C strings.
#[cfg(target_os = "macos")]
fn reexec_args_c() -> Vec<CString> {
    std::env::args_os()
        .map(|a| cstring_lossy(a.as_os_str()))
        .collect()
}

#[cfg(target_os = "macos")]
fn env_pairs_c(env: &[(OsString, OsString)]) -> Vec<CString> {
    env.iter()
        .map(|(k, v)| env_pair_cstring(k, v.as_os_str()))
        .collect()
}

/// `"key=value"` C string; falls back to a harmless primed-marker pair on interior NULs.
#[cfg(target_os = "macos")]
fn env_pair_cstring(k: &std::ffi::OsStr, v: &std::ffi::OsStr) -> CString {
    use std::os::unix::ffi::OsStrExt;
    let mut pair = k.as_bytes().to_vec();
    pair.push(b'=');
    pair.extend_from_slice(v.as_bytes());
    CString::new(pair).unwrap_or_else(|_| CString::new(b"RHINO_DYLD_PRIMED=1").unwrap())
}

/// Null-terminated array of C-string pointers for [`libc::execve`] argv/envp.
#[cfg(target_os = "macos")]
fn c_ptr_array(items: &[CString]) -> Vec<*const libc::c_char> {
    let mut ptrs: Vec<*const libc::c_char> = items.iter().map(|a| a.as_ptr()).collect();
    ptrs.push(std::ptr::null());
    ptrs
}
