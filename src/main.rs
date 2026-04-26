//! Rhino Player — binary entry.
//!
//! Copyright © Peter Adrianov, 2026.

fn main() -> ! {
    // Before GLib / GTK: prefer CPU and (on Linux) I/O scheduling for smooth decode.
    rhino_player::sched::raise_process_priority();
    // libmpv checks the locale at init; keep numeric C rules before any other setup.
    std::env::set_var("LC_NUMERIC", "C");
    unsafe {
        libc::setlocale(libc::LC_NUMERIC, b"C\0".as_ptr().cast());
    }
    // Before GTK init: same string as the app id and `*.desktop` basename so the shell can match.
    glib::set_prgname(Some(rhino_player::APP_ID));
    std::process::exit(rhino_player::run())
}
