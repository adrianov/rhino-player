//! libmpv OpenGL output in a [`gtk::GLArea`]. See `docs/features/03-mpv-embedding.md` and `docs/product-and-use-cases.md`.

mod gl_platform;

#[cfg(target_os = "macos")]
mod macos_video_cgl;
#[cfg(target_os = "macos")]
mod macos_video_layer;
#[cfg(target_os = "macos")]
pub(crate) mod macos_video_attach;
#[cfg(target_os = "macos")]
mod macos_video_displaylink;
#[cfg(target_os = "macos")]
pub(crate) mod macos_video_bundle;

include!("mpv_embed/linux_egl_helpers.rs");
include!("mpv_embed/main_bundle_egl_render.rs");
include!("mpv_embed/mpv_persistence.rs");
include!("mpv_embed/preview_gl_set_tracks.rs");
