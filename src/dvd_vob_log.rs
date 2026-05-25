//! DVD unified-timeline debug lines (`RHINO_DVD_SEEK=1`, `RHINO_DVD_TRANSPORT_LOG=1`).

pub(crate) fn dvd_seek_log(msg: impl std::fmt::Display) {
    if std::env::var_os("RHINO_DVD_SEEK").is_some() {
        eprintln!("[rhino] dvd: {msg}");
    }
}

pub(crate) fn dvd_transport_log_enabled() -> bool {
    std::env::var_os("RHINO_DVD_TRANSPORT_LOG").is_some()
        || std::env::var_os("RHINO_DVD_SEEK").is_some()
}

pub(crate) fn dvd_transport_log(msg: impl std::fmt::Display) {
    if dvd_transport_log_enabled() {
        eprintln!("[rhino] dvd transport: {msg}");
    }
}
