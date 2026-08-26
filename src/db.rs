//! Single SQLite file under XDG config: `~/.config/rhino/rhino.sqlite`.
//! Resume position is also persisted here (`media.time_pos_sec`) and applied via `loadfile … start=`.

include!("db/connection_init_and_audio.rs");
include!("db/video_sub_prefs.rs");
include!("db/history_and_media_playback.rs");
include!("db/rekey_continue_path.rs");
include!("db/media_me_budget.rs");
include!("db/media_source_fps.rs");
include!("db/media_fill_screen.rs");
#[cfg(test)]
mod media_me_budget_tests;
#[cfg(test)]
#[path = "db/rekey_continue_path_tests.rs"]
mod rekey_continue_path_tests;
include!("db/media_snapshots_thumbs.rs");
