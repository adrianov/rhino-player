//! Integration: libmpv loads and reports a version string.

use libmpv2::Mpv;

#[test]
fn mpv_reports_version() {
    unsafe {
        libc::setlocale(libc::LC_NUMERIC, b"C\0".as_ptr().cast());
    }
    let m = Mpv::new().expect("create mpv");
    let v: String = m.get_property("mpv-version").expect("mpv-version");
    assert!(!v.is_empty(), "version string");
}
