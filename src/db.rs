//! Single SQLite file under XDG config: `~/.config/rhino/rhino.sqlite`.
//! mpv [paths::watch_later] files stay separate because libmpv needs a directory.

include!("db/connection_init_and_audio.rs");
include!("db/video_sub_prefs.rs");
include!("db/history_and_media_playback.rs");
include!("db/media_snapshots_thumbs.rs");
