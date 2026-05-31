//! Raw **JPEG** bytes → [gdk::Texture]. `gdk::Texture::from_bytes` is for serialized GDK textures, not
//! encoded image file bytes; use [gdk_pixbuf::PixbufLoader] for JPEG.

use gdk_pixbuf::prelude::PixbufLoaderExt;
use gtk::gdk;

/// Decode a JPEG in memory; returns `None` on invalid data or if GTK is not initialised.
pub fn texture_from_jpeg(bytes: &[u8]) -> Option<gdk::Texture> {
    let loader = gdk_pixbuf::PixbufLoader::new();
    loader.write(bytes).ok()?;
    loader.close().ok()?;
    let pb = loader.pixbuf()?;
    Some(gdk::Texture::for_pixbuf(&pb))
}
