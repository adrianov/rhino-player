//! Always-on user interaction lines on plain `cargo run` (`[rhino] ui:`).

pub(crate) fn act(msg: impl std::fmt::Display) {
    eprintln!("[rhino] ui: {msg}");
}
