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
        ent.save_global_resume(total, global);
        return Some((global, total));
    }
    let _ = db_key;
    None
}

/// Read `(global_sec, total_sec)` from the entity row only — resume and duration from the same key.
fn title_total_for_entity(entity: &Path, durs: &HashMap<String, f64>) -> Option<f64> {
    let ch = crate::dvd_entity::timeline_chapter_paths(entity)?.into_iter().next()?;
    let live = chapter_live_dur(&ch, durs);
    crate::dvd_entity::build_title_timeline_with(
        &ch,
        durs,
        live,
        crate::dvd_entity::TimelineBuildOpts::CACHE_ONLY,
    )
    .map(|tl| tl.total_sec)
}

fn entity_stored_total(
    stored_dur: f64,
    global: f64,
    entity: &Path,
    durs: &HashMap<String, f64>,
) -> f64 {
    let base = if stored_dur.is_finite() && stored_dur > 0.0 {
        stored_dur
    } else {
        global
    };
    if base >= global {
        return base;
    }
    title_total_for_entity(entity, durs)
        .map(|tl_total| tl_total.max(base).max(global))
        .unwrap_or(global)
}

fn entity_global_playback(
    entity: &Path,
    durs: &HashMap<String, f64>,
    tpos: &HashMap<String, f64>,
) -> Option<(f64, f64)> {
    for k in entity_media_keys(entity) {
        let Some(&global) = tpos.get(&k) else {
            continue;
        };
        if !global.is_finite() || global < 0.0 {
            continue;
        }
        let stored_dur = durs.get(&k).copied().unwrap_or(0.0);
        let total = entity_stored_total(stored_dur, global, entity, durs);
        return Some((global.clamp(0.0, total), total.max(global)));
    }
    None
}

impl PlaybackEntity {
    /// Unified timeline: persist whole-title seconds on the entity row (open maps global → `.vob` + seek).
    pub fn save_global_resume(&self, total_sec: f64, global_sec: f64) {
        if !self.has_unified_timeline() {
            return;
        }
        if !(total_sec.is_finite() && total_sec > 0.0 && global_sec.is_finite() && global_sec >= 0.0) {
            return;
        }
        let global = global_sec.min(total_sec);
        crate::db::set_playback(&self.db_path(), total_sec, global);
        self.purge_extra_db_rows();
        crate::media_probe::continue_grid_cache_note_playback(&self.db_path(), global, total_sec);
    }

    /// Map title-wide global seconds → chapter `.vob` + IFO-local seek (preview, continue grid).
    #[must_use]
    pub fn still_at_global(
        &self,
        probe: &Path,
        global_sec: f64,
        durs: &HashMap<String, f64>,
        bar: Option<&crate::dvd_vob_timeline::DvdBarState>,
        open_cap: Option<&crate::dvd_entity::StillOpenCap>,
    ) -> Option<crate::dvd_entity::DvdStillTarget> {
        if !self.has_unified_timeline() {
            return None;
        }
        let chapter =
            crate::dvd_entity::timeline_chapter_probe(probe).unwrap_or_else(|| probe.to_path_buf());
        crate::dvd_entity::still_at_global(chapter.as_path(), global_sec, durs, bar, open_cap)
    }
}

/// Whole-title resume + duration for the continue grid (entity row: global seconds on unified timeline).
#[must_use]
pub fn card_resume_duration(
    probe: &Path,
    durs: &HashMap<String, f64>,
    tpos: &HashMap<String, f64>,
) -> (f64, f64) {
    let ent = PlaybackEntity::resolve(probe);
    if ent.has_unified_timeline() {
        let entity = ent.db_path();
        if let Some((g, t)) = entity_global_playback(&entity, durs, tpos) {
            return (g, t);
        }
        if let Some((g, t)) = migrate_dvd_from_chapter_rows(&ent, durs, tpos) {
            return (g, t);
        }
        return (0.0, 0.0);
    }
    let keys = entity_media_keys(&ent.db_path());
    let resume = keys.iter().find_map(|k| tpos.get(k).copied());
    let duration = keys.iter().find_map(|k| durs.get(k).copied());
    (resume.unwrap_or(0.0), duration.unwrap_or(0.0))
}

#[cfg(test)]
mod card_tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;

    #[test]
    fn entity_global_playback_keeps_stored_global() {
        let base = std::env::temp_dir().join(format!("rhino-pe-global-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        fs::write(vts.join("VTS_02_1.VOB"), b"a").expect("vob1");
        fs::write(vts.join("VTS_02_2.VOB"), b"b").expect("vob2");
        let entity = crate::playback_entity::db_path_for(&base);
        let ek = entity.to_string_lossy().into_owned();
        let mut durs = HashMap::new();
        let mut tpos = HashMap::new();
        durs.insert(ek.clone(), 7289.0);
        tpos.insert(ek.clone(), 1746.5);
        let p2 = vts.join("VTS_02_2.VOB");
        durs.insert(p2.to_string_lossy().into_owned(), 1265.75);
        tpos.insert(p2.to_string_lossy().into_owned(), 1266.45);
        let (g, t) = entity_global_playback(&entity, &durs, &tpos).expect("entity row");
        assert!((g - 1746.5).abs() < 0.1, "global={g}");
        assert!((t - 7289.0).abs() < 0.1, "total={t}");
        let (resume, duration) = card_resume_duration(&base, &durs, &tpos);
        assert!((resume - 1746.5).abs() < 0.1, "resume={resume}");
        assert!((duration - 7289.0).abs() < 0.1, "duration={duration}");
        let _ = fs::remove_dir_all(&base);
    }
}
