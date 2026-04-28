//! Single SQLite file under XDG config: `~/.config/rhino/rhino.sqlite`.
//! Resume position is also persisted here (`media.time_pos_sec`) and applied via `loadfile … start=`.

include!("db/connection_init_and_audio.rs");
include!("db/video_sub_prefs.rs");
include!("db/history_and_media_playback.rs");
include!("db/media_snapshots_thumbs.rs");
