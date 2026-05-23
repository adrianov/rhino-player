// Window / header title from a playback entity (included by `playback_entity.rs`).

use std::path::Path;

use super::{PlaybackEntity, PlaybackEntityKind};

const APP_WIN_TITLE: &str = "Rhino Player";

fn file_base_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| path.display().to_string())
}

fn entity_display_name(entity: &PlaybackEntity) -> String {
    match &entity.kind {
        PlaybackEntityKind::SingleFile(p) => file_base_name(p),
        PlaybackEntityKind::DvdTitle { db_key, .. } => file_base_name(db_key),
    }
}

/// Human-readable window title for any openable path (entity key, not chapter `.vob` stub names).
#[must_use]
pub fn window_title_for(path: &Path) -> String {
    let entity = PlaybackEntity::resolve(path);
    let raw = entity_display_name(&entity);
    let human = crate::human_media_title::human_media_title(&raw);
    let label = {
        let h = human.trim();
        if h.is_empty() { raw.trim() } else { h }
    };
    let label = label.trim();
    if label.is_empty() {
        APP_WIN_TITLE.to_string()
    } else {
        format!("{label} — {APP_WIN_TITLE}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn dvd_entity_title_uses_disc_folder_not_vob_stub() {
        let disc = std::env::temp_dir().join(format!("rhino-pe-title-{}", std::process::id()));
        let _ = fs::remove_dir_all(&disc);
        let vts = disc.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        fs::write(vts.join("VTS_02_1.VOB"), b"v").expect("vob");
        let vob = vts.join("VTS_02_1.VOB");
        let t = window_title_for(&vob);
        let folder = disc
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("disc");
        assert!(t.contains(folder), "title was {t:?}");
        assert!(!t.trim().is_empty());
        let _ = fs::remove_dir_all(&disc);
    }
}
