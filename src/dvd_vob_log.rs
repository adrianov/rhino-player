//! DVD unified-timeline debug lines (`RHINO_DVD_SEEK=1`).

pub(crate) fn dvd_seek_log(msg: impl std::fmt::Display) {
    if std::env::var_os("RHINO_DVD_SEEK").is_some() {
        eprintln!("[rhino] dvd: {msg}");
    }
}
