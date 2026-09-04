// Pin the VSScript runtime so mpv can re-create the `vapoursynth` filter.
//
// mpv's `vf=vapoursynth` uses VSScript API4: `createScript()` per filter instance,
// `freeScript()` on destroy. When the last script environment is freed (Smooth off, vf strip),
// VapourSynth finalizes its embedded Python interpreter — which cannot be initialized again in
// the same process (`Failed to initialize the VapourSynth Python module for VSScript use`).
// Holding one extra script environment from Rhino keeps Python alive for the whole process,
// so Smooth can be toggled off and on freely.

/// Candidate VSScript library names for `dlopen` (resolved via the normal loader search path —
/// on macOS the re-exec already put the VapourSynth dir on `DYLD_LIBRARY_PATH`).
#[cfg(target_os = "macos")]
const VSSCRIPT_LIB_NAMES: &[&str] = &["libvapoursynth-script.dylib", "libvsscript.dylib"];

#[cfg(not(target_os = "macos"))]
const VSSCRIPT_LIB_NAMES: &[&str] = &["libvapoursynth-script.so.0", "libvapoursynth-script.so"];

/// `VS_MAKE_VERSION(4, 1)` — the base VSScript API4 version every R55+ install accepts.
const VSSCRIPT_API_4_1: libc::c_int = (4 << 16) | 1;

/// Leading fields of `VSSCRIPTAPI` (VSScript4.h); only `create_script` is called.
#[repr(C)]
struct VsScriptApi {
    get_api_version: unsafe extern "C" fn() -> libc::c_int,
    get_vsapi: unsafe extern "C" fn(libc::c_int) -> *const libc::c_void,
    create_script: unsafe extern "C" fn(*mut libc::c_void) -> *mut libc::c_void,
}

/// Call once before the first `vf add vapoursynth`; later calls are free.
pub(crate) fn pin_vsscript_python() {
    static PINNED: std::sync::Once = std::sync::Once::new();
    PINNED.call_once(|| match pin_once() {
        Ok(name) => eprintln!(
            "[rhino] video: VSScript runtime pinned via {name} (Python stays alive across vf remove)"
        ),
        Err(e) => eprintln!(
            "[rhino] video: VSScript pin failed: {e} — Smooth re-enable after off may fail"
        ),
    });
}

/// CLI diagnostics: resolve VSScript and hold one script env (same pin Smooth uses).
pub(crate) fn diagnose_vsscript() -> Result<String, String> {
    pin_once().map(|name| format!("pinned via {name}"))
}

fn pin_once() -> Result<&'static str, String> {
    for name in VSSCRIPT_LIB_NAMES {
        // Err aborts the search (API answered but failed); Ok(None) skips to the next candidate.
        match try_pin_via_lib(name) {
            Ok(Some(pinned)) => return Ok(pinned),
            Ok(None) => continue,
            Err(e) => return Err(e),
        }
    }
    Err("no VSScript library with getVSScriptAPI found via dlopen".into())
}

/// Try one candidate library: `dlopen` → `getVSScriptAPI(4.1)` → leak one script environment.
/// `Ok(Some(name))` = pinned; `Ok(None)` = unusable candidate, try the next; `Err` = hard failure.
fn try_pin_via_lib(name: &&'static str) -> Result<Option<&'static str>, String> {
    match probe_vsscript_api(name) {
        VsscriptProbe::Skip => Ok(None),
        VsscriptProbe::ApiNull(handle) => Err(format!(
            "getVSScriptAPI(4.1) returned NULL via {name}{}",
            vsscript_last_error(handle)
        )),
        VsscriptProbe::Api(api) => create_pin_script(api, name),
    }
}

/// Outcome of resolving one candidate library.
enum VsscriptProbe {
    /// Unusable candidate — try the next library name.
    Skip,
    /// `getVSScriptAPI` answered `NULL`; carries the handle for last-error detail.
    ApiNull(*mut libc::c_void),
    /// API4.1 table resolved.
    Api(*const VsScriptApi),
}

/// `dlopen` the candidate and resolve `getVSScriptAPI(VSSCRIPT_API_4_1)`.
fn probe_vsscript_api(name: &&'static str) -> VsscriptProbe {
    let Ok(cname) = std::ffi::CString::new(*name) else {
        return VsscriptProbe::Skip;
    };
    unsafe {
        let handle = libc::dlopen(cname.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL);
        if handle.is_null() {
            return VsscriptProbe::Skip;
        }
        let sym = libc::dlsym(handle, c"getVSScriptAPI".as_ptr());
        if sym.is_null() {
            return VsscriptProbe::Skip;
        }
        let api = (std::mem::transmute::<
            *mut libc::c_void,
            unsafe extern "C" fn(libc::c_int) -> *const VsScriptApi,
        >(sym))(VSSCRIPT_API_4_1);
        if api.is_null() {
            VsscriptProbe::ApiNull(handle)
        } else {
            VsscriptProbe::Api(api)
        }
    }
}

/// Create (and intentionally leak) one script environment — the pin that keeps Python alive.
fn create_pin_script(
    api: *const VsScriptApi,
    name: &&'static str,
) -> Result<Option<&'static str>, String> {
    let script = unsafe { ((*api).create_script)(std::ptr::null_mut()) };
    if script.is_null() {
        return Err(format!("createScript failed via {name}"));
    }
    Ok(Some(name))
}

/// Best-effort detail from `getVSScriptAPILastError` (VSScript API 4.3+).
fn vsscript_last_error(handle: *mut libc::c_void) -> String {
    let sym = unsafe { libc::dlsym(handle, c"getVSScriptAPILastError".as_ptr()) };
    if sym.is_null() {
        return String::new();
    }
    let last_err: unsafe extern "C" fn() -> *const libc::c_char =
        unsafe { std::mem::transmute(sym) };
    let msg = unsafe { last_err() };
    if msg.is_null() {
        return String::new();
    }
    let s = unsafe { std::ffi::CStr::from_ptr(msg) }.to_string_lossy();
    format!(" ({s})")
}
