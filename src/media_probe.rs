//! Watch-later resume position, last-known `duration` from libmpv, and **raster** thumbnails (JPEG or PNG) in [crate::db]. The grid/quit paths use a **dedicated in-process [libmpv2::Mpv]** with `vo=image`. See [docs/features/21-recent-videos-launch.md].

include!("media_probe/card_data_resume_thumbs.rs");
include!("media_probe/thumb_pipeline_and_cards.rs");
