use std::collections::HashMap;
use std::path::Path;

use super::{PlaybackEntity, PlaybackEntityKind};

/// Path strings used in SQLite `media` rows for one entity (never per-chapter aliases).
fn entity_media_keys(entity: &Path) -> Vec<String> {
    let mut keys = Vec::new();
    let mut push = |s: &str| {
        if !keys.iter().any(|k| k == s) {
            keys.push(s.to_owned());
        }
    };
    if let Some(s) = entity.to_str() {
        push(s);
    }
    if let Ok(c) = std::fs::canonicalize(entity) {
        if let Some(cs) = c.to_str() {
            push(cs);
        }
    }
    keys
}

/// Entity row keys plus legacy first-chapter `.vob` keys from before disc-root entity.
fn entity_db_lookup_keys(entity: &Path) -> Vec<String> {
    let mut keys = entity_media_keys(entity);
    let Some(disc) = crate::video_ext::dvd_disc_root(entity).or_else(|| {
        crate::video_ext::is_dvd_vob_path(entity)
            .then(|| entity.to_path_buf())
            .and_then(|p| crate::video_ext::dvd_disc_root(&p))
    }) else {
        return keys;
    };
    let Some(vob) = crate::video_ext::dvd_main_chapter_vob(&disc) else {
        return keys;
    };
    let Some(legacy) = crate::dvd_entity::title_entity_path(&vob) else {
        return keys;
    };
    if crate::video_ext::paths_same_file(&legacy, entity) {
        return keys;
    }
    for k in entity_media_keys(&legacy) {
        if !keys.iter().any(|x| x == &k) {
            keys.push(k);
        }
    }
    keys
}

fn chapter_media_keys(chapter: &Path) -> Vec<String> {
    let mut keys = entity_media_keys(chapter);
    if let Some(s) = chapter.to_str() {
        let owned = s.to_owned();
        if !keys.iter().any(|k| k == &owned) {
            keys.push(owned);
        }
    }
    keys
}

fn chapter_live_dur(chapter: &Path, durs: &HashMap<String, f64>) -> f64 {
    chapter_media_keys(chapter)
        .iter()
        .find_map(|k| durs.get(k).copied())
        .filter(|d| d.is_finite() && *d > 0.0)
        .unwrap_or(0.0)
}

fn migrate_dvd_from_chapter_rows(
    ent: &PlaybackEntity,
    durs: &HashMap<String, f64>,
    tpos: &HashMap<String, f64>,
) -> Option<(f64, f64)> {
    let PlaybackEntityKind::DvdTitle { chapters, db_key } = &ent.kind else {
        return None;
    };
    for ch in chapters {
        let Some(loc_st) = chapter_media_keys(ch)
            .iter()
            .find_map(|k| tpos.get(k).copied())
        else {
            continue;
        };
        let Some(loc_dur) = chapter_media_keys(ch)
            .iter()
            .find_map(|k| durs.get(k).copied())
        else {
            continue;
        };
        let Some((total, global)) =
            crate::dvd_entity::playback_snapshot(ch.as_path(), loc_st, loc_dur, durs)
        else {
            continue;
        };
        crate::db::set_playback(db_key, total, global);
        ent.purge_extra_db_rows();
        return Some((global, total));
    }
    None
}

fn dvd_timeline_probe(ent: &PlaybackEntity, probe: &Path) -> std::path::PathBuf {
    if let PlaybackEntityKind::DvdTitle { chapters, .. } = &ent.kind {
        if !crate::video_ext::is_dvd_vob_path(probe) {
            if let Some(ch) = chapters.first() {
                return ch.clone();
            }
        }
    }
    probe.to_path_buf()
}

fn normalize_dvd_entity_row(
    ent: &PlaybackEntity,
    probe: &Path,
    resume: f64,
    duration: f64,
    durs: &HashMap<String, f64>,
) -> (f64, f64) {
    let chapter = dvd_timeline_probe(ent, probe);
    let live = chapter_live_dur(&chapter, durs);
    let Some(tl) = crate::dvd_entity::build_title_timeline(&chapter, durs, live) else {
        return (resume, duration);
    };
    if tl.vobs.len() <= 1 {
        return (resume, duration);
    }
    let mut total = duration.max(tl.total_sec);
    let mut global = resume;
    let idx0 = tl
        .vobs
        .iter()
        .position(|p| crate::video_ext::paths_same_file(p, &chapter))
        .unwrap_or(0);
    let ch0_dur = tl.chapter_dur_at(idx0);
    if resume <= ch0_dur + 5.0 {
        global = tl.global_pos(&chapter, resume);
    }
    if global > total {
        total = global.max(tl.total_sec);
    }
    if (total - duration).abs() > 0.5 || (global - resume).abs() > 0.5 {
        crate::db::set_playback(&ent.db_path(), total, global);
        ent.purge_extra_db_rows();
    }
    (global, total)
}

/// Whole-title resume + duration for the continue grid (entity row only, not per-chapter `.vob`).
#[must_use]
pub fn card_resume_duration(
    probe: &Path,
    durs: &HashMap<String, f64>,
    tpos: &HashMap<String, f64>,
) -> (f64, f64) {
    let ent = PlaybackEntity::resolve(probe);
    let entity = ent.db_path();
    let keys = entity_db_lookup_keys(&entity);
    let resume = keys.iter().find_map(|k| tpos.get(k).copied());
    let duration = keys.iter().find_map(|k| durs.get(k).copied());

    if ent.has_unified_timeline() {
        if resume.is_none() || duration.is_none() {
            if let Some((g, t)) = migrate_dvd_from_chapter_rows(&ent, durs, tpos) {
                return (g, t);
            }
        }
        if let (Some(s), Some(d)) = (resume, duration) {
            let (g, t) = normalize_dvd_entity_row(&ent, probe, s, d, durs);
            return (g, t);
        }
        if let Some((g, t)) = migrate_dvd_from_chapter_rows(&ent, durs, tpos) {
            return (g, t);
        }
    }

    (resume.unwrap_or(0.0), duration.unwrap_or(0.0))
}
