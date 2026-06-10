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

fn pin_once() -> Result<&'static str, String> {
    for name in VSSCRIPT_LIB_NAMES {
        let Ok(cname) = std::ffi::CString::new(*name) else {
            continue;
        };
        let handle = unsafe { libc::dlopen(cname.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL) };
        if handle.is_null() {
            continue;
        }
        let sym = unsafe { libc::dlsym(handle, c"getVSScriptAPI".as_ptr()) };
        if sym.is_null() {
            continue;
        }
        let get_api: unsafe extern "C" fn(libc::c_int) -> *const VsScriptApi =
            unsafe { std::mem::transmute(sym) };
        let api = unsafe { get_api(VSSCRIPT_API_4_1) };
        if api.is_null() {
            return Err(format!(
                "getVSScriptAPI(4.1) returned NULL via {name}{}",
                vsscript_last_error(handle)
            ));
        }
        // Intentionally leaked: this environment is the pin that keeps Python initialized.
        let script = unsafe { ((*api).create_script)(std::ptr::null_mut()) };
        if script.is_null() {
            return Err(format!("createScript failed via {name}"));
        }
        return Ok(name);
    }
    Err("no VSScript library with getVSScriptAPI found via dlopen".into())
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
