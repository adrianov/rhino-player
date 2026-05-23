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

fn title_timeline_for_entity(
    ent: &PlaybackEntity,
    dur_by_path: &HashMap<String, f64>,
) -> Option<crate::dvd_vob_timeline::DvdVobTimeline> {
    let PlaybackEntityKind::DvdTitle { db_key, chapters } = &ent.kind else {
        return None;
    };
    let probe = chapters.first()?.as_path();
    let live = entity_db_lookup_keys(db_key)
        .iter()
        .find_map(|k| {
            dur_by_path
                .get(k)
                .copied()
                .filter(|d| d.is_finite() && *d > 0.0)
        })
        .unwrap_or(0.0);
    let mut tl = crate::dvd_vob_timeline::DvdVobTimeline::from_chapter_ifo(probe)
        .or_else(|| {
            crate::dvd_vob_timeline::DvdVobTimeline::from_chapter(
                probe,
                dur_by_path,
                probe,
                live,
            )
        })?;
    if let Some(on_disk) = crate::dvd_entity::title_chapter_paths(probe) {
        tl.expand_on_disk_chapters(&on_disk);
    }
    for k in entity_db_lookup_keys(db_key) {
        if let Some(total) = dur_by_path.get(&k).copied() {
            if total.is_finite() && total > tl.total_sec {
                tl.apply_entity_total(total);
                break;
            }
        }
    }
    (tl.total_sec > 0.0).then_some(tl)
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
    let Some(tl) = title_timeline_for_entity(ent, durs) else {
        return (resume, duration);
    };
    if tl.vobs.len() <= 1 {
        return (resume, duration);
    }
    let chapter = dvd_timeline_probe(ent, probe);
    let mut total = duration;
    let mut global = resume;
    if tl.total_sec > duration + 60.0 {
        total = tl.total_sec;
        let idx = tl
            .vobs
            .iter()
            .position(|p| crate::video_ext::paths_same_file(p, &chapter))
            .unwrap_or(0);
        let ch_dur = tl.chapter_dur_at(idx);
        if resume <= ch_dur + 5.0 {
            global = tl.global_pos(&chapter, resume);
        }
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
