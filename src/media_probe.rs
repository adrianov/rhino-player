//! Watch-later resume position, last-known `duration` from libmpv, and **raster** thumbnails (WebP) in [crate::db]. The grid backfill path uses a **dedicated in-process [libmpv2::Mpv]** with `screenshot-raw` (in-memory, no temp files). See [docs/features/21-recent-videos-launch.md].

mod continue_grid_cache_hook;

include!("media_probe/card_data_resume_thumbs.rs");
include!("media_probe/thumb_pipeline_and_cards.rs");
