//! Single SQLite file under XDG config: `~/.config/rhino/rhino.sqlite`.
//! mpv [paths::watch_later] files stay separate because libmpv needs a directory.

include!("db/1.rs");
include!("db/2.rs");
include!("db/3.rs");
include!("db/4.rs");
