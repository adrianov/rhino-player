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
        if !keys.iter().any(|k| k == s) {
            keys.push(s.to_owned());
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
    let PlaybackEntityKind::DvdTitle { chapters, .. } = &ent.kind else {
        return None;
    };
    for ch in chapters {
        let keys = chapter_media_keys(ch);
        let Some(loc_st) = keys.iter().find_map(|k| tpos.get(k).copied()) else {
            continue;
        };
        let Some(loc_dur) = keys.iter().find_map(|k| durs.get(k).copied()) else {
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
    None
}

/// Read `(global_sec, total_sec)` from the entity row only — resume and duration from the same key.
fn title_total_for_entity(entity: &Path, durs: &HashMap<String, f64>) -> Option<f64> {
    let ch = crate::dvd_entity::timeline_chapter_paths(entity)?
        .into_iter()
        .next()?;
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
        if !(total_sec.is_finite()
            && total_sec > 0.0
            && global_sec.is_finite()
            && global_sec >= 0.0)
        {
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

fn unified_card_resume(
    ent: &PlaybackEntity,
    durs: &HashMap<String, f64>,
    tpos: &HashMap<String, f64>,
) -> (f64, f64) {
    let entity = ent.db_path();
    entity_global_playback(&entity, durs, tpos)
        .or_else(|| migrate_dvd_from_chapter_rows(ent, durs, tpos))
        .unwrap_or((0.0, 0.0))
}

fn plain_card_resume(
    entity: &Path,
    durs: &HashMap<String, f64>,
    tpos: &HashMap<String, f64>,
) -> (f64, f64) {
    let keys = entity_media_keys(entity);
    let resume = keys.iter().find_map(|k| tpos.get(k).copied());
    let duration = keys.iter().find_map(|k| durs.get(k).copied());
    (resume.unwrap_or(0.0), duration.unwrap_or(0.0))
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
        return unified_card_resume(&ent, durs, tpos);
    }
    plain_card_resume(&ent.db_path(), durs, tpos)
}

#[cfg(test)]
mod card_tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;
    use std::path::{Path, PathBuf};

    /// Fresh DVD folder fixture with two 1-byte VOBs; returns `(base, second vob)`.
    fn global_fixture() -> (PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!("rhino-pe-global-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        fs::write(vts.join("VTS_02_1.VOB"), b"a").expect("vob1");
        let p2 = vts.join("VTS_02_2.VOB");
        fs::write(&p2, b"b").expect("vob2");
        (base, p2)
    }

    fn media_key(p: &Path) -> String {
        p.to_string_lossy().into_owned()
    }

    fn seeded_global_maps(ek: String, p2k: String) -> (HashMap<String, f64>, HashMap<String, f64>) {
        let mut durs = HashMap::new();
        let mut tpos = HashMap::new();
        durs.insert(ek.clone(), 7289.0);
        tpos.insert(ek, 1746.5);
        durs.insert(p2k.clone(), 1265.75);
        tpos.insert(p2k, 1266.45);
        (durs, tpos)
    }

    fn assert_close(actual: f64, expected: f64, label: &str) {
        assert!((actual - expected).abs() < 0.1, "{label}={actual}");
    }

    fn assert_card_matches(probe: &Path, durs: &HashMap<String, f64>, tpos: &HashMap<String, f64>) {
        let (resume, duration) = card_resume_duration(probe, durs, tpos);
        assert_close(resume, 1746.5, "resume");
        assert_close(duration, 7289.0, "duration");
    }

    #[test]
    fn entity_global_playback_keeps_stored_global() {
        let (base, p2) = global_fixture();
        let entity = crate::playback_entity::db_path_for(&base);
        let (durs, tpos) = seeded_global_maps(media_key(&entity), media_key(&p2));
        let (g, t) = entity_global_playback(&entity, &durs, &tpos).expect("entity row");
        assert_close(g, 1746.5, "global");
        assert_close(t, 7289.0, "total");
        assert_card_matches(&base, &durs, &tpos);
        let _ = fs::remove_dir_all(&base);
    }
}
