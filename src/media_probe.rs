//! Watch-later resume position, last-known `duration` from libmpv, and **raster** thumbnails (JPEG or PNG) in [crate::db]. The grid/quit paths use a **dedicated in-process [libmpv2::Mpv]** with `vo=image`. See [docs/features/21-recent-videos-launch.md].

include!("media_probe/1.rs");
include!("media_probe/2.rs");
