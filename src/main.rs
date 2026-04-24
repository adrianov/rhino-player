//! Rhino Player — binary entry.

fn main() -> ! {
    // libmpv checks the locale at init; keep numeric C rules before any other setup.
    std::env::set_var("LC_NUMERIC", "C");
    unsafe {
        libc::setlocale(libc::LC_NUMERIC, b"C\0".as_ptr().cast());
    }
    std::process::exit(rhino_player::run())
}
