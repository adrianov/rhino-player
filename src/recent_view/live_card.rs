// In-place continue-strip thumbnail refresh (feature owner).
// Delivery: live_card/thumb_backfill.rs (worker → inbox → MainContext::invoke).
// See docs/features/21-recent-videos-launch.md (thumbnail scroll scenario).

use std::path::Path;

/// Swap ready stills onto strip overlays. `cards[0]` is Open Video; `media_paths` align with `cards[1..]`.
fn apply_ready_thumbs(
    cards: &[gtk::Overlay],
    media_paths: &[std::path::PathBuf],
    ready: &[std::path::PathBuf],
) {
    for path in ready {
        let Some(i) = media_index(media_paths, path) else {
            continue;
        };
        let Some(overlay) = cards.get(i + 1) else {
            continue;
        };
        apply_live_thumb(overlay, media_paths[i].as_path());
    }
}

fn media_index(media_paths: &[std::path::PathBuf], path: &Path) -> Option<usize> {
    let key = crate::db::history_key(path);
    media_paths.iter().position(|cp| {
        cp == path || (key.is_some() && crate::db::history_key(cp) == key)
    })
}

fn apply_live_thumb(card: &gtk::Overlay, path: &Path) {
    let Some(bytes) = media_probe::cached_thumbnail_for_display(path) else {
        return;
    };
    let key = crate::db::history_key(path).unwrap_or_default();
    let Some(tex) = crate::thumb_texture::texture_from_thumb_cached(&key, bytes.as_slice()) else {
        return;
    };
    if crate::thumb_texture::overlay_shows_texture(card, &tex) {
        return;
    }
    card.set_child(Some(&crate::thumb_texture::cover_picture(&tex)));
}

include!("live_card/thumb_backfill.rs");

/// Public entry used by strip paint and browse-back.
pub fn schedule_thumb_backfill(ctx: Rc<RecentContext>, paths: Vec<std::path::PathBuf>) {
    ctx.thumbs.schedule(paths);
}
