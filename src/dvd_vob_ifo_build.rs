impl DvdVobTimeline {
    /// Build from `VTS_XX_0.IFO` (PGC cell `playback_time` + PTT marks).
    pub fn from_chapter_ifo(current: &Path) -> Option<Self> {
        let ifo = crate::dvd_ifo_parse::timeline_from_vob(current)?;
        let title = crate::dvd_entity::vob_title_id(current)?;
        let vts = crate::dvd_entity::video_ts_for_vob(current)?;
        let on_disk = crate::dvd_entity::list_title_vobs(&vts, current);
        let mut vob_ids: Vec<u32> = ifo.vob_secs.iter().map(|(id, _)| *id).collect();
        vob_ids.sort_unstable();
        vob_ids.dedup();
        let mut vobs = Vec::new();
        let mut durs = Vec::new();
        for vid in vob_ids {
            let sec = ifo
                .vob_secs
                .iter()
                .find(|(id, _)| *id == vid)
                .map(|(_, s)| *s)
                .unwrap_or(0.0);
            if sec <= 0.0 || !sec.is_finite() {
                continue;
            }
            let path = on_disk
                .iter()
                .find(|p| crate::dvd_entity::vob_part_id(p) == Some(vid))
                .cloned()
                .or_else(|| crate::dvd_entity::chapter_vob_if_exists(&vts, title, vid))
                .unwrap_or_else(|| vts.join(format!("VTS_{title:02}_{vid}.VOB")));
            vobs.push(path);
            durs.push(sec);
        }
        if vobs.is_empty() {
            return None;
        }
        let mut tl = Self {
            vobs,
            starts: Vec::new(),
            durs,
            total_sec: 0.0,
            ptt_marks: ifo.ptt_marks,
        };
        tl.recompute_starts();
        tl.expand_on_disk_chapters(&on_disk);
        (tl.total_sec > 0.0).then_some(tl)
    }

    /// Rips often split one IFO VOB id into `VTS_XX_1` … `VTS_XX_N`; use on-disk files when IFO lists fewer.
    pub(crate) fn expand_on_disk_chapters(&mut self, on_disk: &[PathBuf]) {
        if on_disk.len() <= self.vobs.len() {
            return;
        }
        let old_vobs = std::mem::take(&mut self.vobs);
        let old_durs = std::mem::take(&mut self.durs);
        self.vobs = on_disk.to_vec();
        self.durs = vec![0.0; self.vobs.len()];
        for (vob, dur) in old_vobs.iter().zip(old_durs.iter()) {
            if *dur <= 0.0 || !dur.is_finite() {
                continue;
            }
            let Some(vid) = crate::dvd_entity::vob_part_id(vob) else {
                continue;
            };
            let parts: Vec<usize> = self
                .vobs
                .iter()
                .enumerate()
                .filter(|(_, p)| crate::dvd_entity::vob_part_id(p) == Some(vid))
                .map(|(i, _)| i)
                .collect();
            if parts.is_empty() {
                continue;
            }
            let bytes: Vec<u64> = parts
                .iter()
                .map(|&i| {
                    self.vobs[i]
                        .metadata()
                        .ok()
                        .map(|m| m.len())
                        .unwrap_or(0)
                })
                .collect();
            let sum: u64 = bytes.iter().copied().sum();
            if sum == 0 {
                let each = *dur / parts.len() as f64;
                for i in parts {
                    self.durs[i] = each;
                }
            } else {
                for (&i, b) in parts.iter().zip(bytes.iter()) {
                    self.durs[i] = *dur * (*b as f64) / (sum as f64);
                }
            }
        }
        if self.durs.iter().any(|d| *d <= 0.0) {
            let total: f64 = old_durs.iter().filter(|d| **d > 0.0).sum();
            if total > 0.0 {
                self.split_duration_by_file_bytes(total);
            }
        }
        self.recompute_starts();
    }

    /// Split `total` seconds across `vobs` by file size (equal split when sizes are unknown).
    fn split_duration_by_file_bytes(&mut self, total: f64) {
        if self.vobs.len() <= 1 || !(total.is_finite() && total > 0.0) {
            return;
        }
        let bytes: Vec<u64> = self
            .vobs
            .iter()
            .map(|p| p.metadata().ok().map(|m| m.len()).unwrap_or(0))
            .collect();
        let sum: u64 = bytes.iter().copied().sum();
        if sum == 0 {
            let each = total / self.vobs.len() as f64;
            self.durs.fill(each);
            return;
        }
        for (i, b) in bytes.iter().enumerate() {
            self.durs[i] = total * (*b as f64) / (sum as f64);
        }
    }

    /// Assign [total] across chapters with unknown length only; keep stored per-chapter values.
    fn fill_missing_chapter_durs(&mut self, total: f64, live_chapter: &Path, live_dur: f64) {
        let missing: Vec<usize> = self
            .durs
            .iter()
            .enumerate()
            .filter(|(_, d)| !d.is_finite() || **d <= 0.0)
            .map(|(i, _)| i)
            .collect();
        if missing.is_empty() {
            return;
        }
        let bytes: Vec<u64> = self
            .vobs
            .iter()
            .map(|p| p.metadata().ok().map(|m| m.len()).unwrap_or(0))
            .collect();
        let known: f64 = self.durs.iter().filter(|d| d.is_finite() && **d > 0.0).sum();
        if total.is_finite() && total > known + 0.5 {
            let remain = total - known;
            let miss_bytes: u64 = missing.iter().map(|&i| bytes[i]).sum();
            if miss_bytes > 0 {
                for &i in &missing {
                    self.durs[i] = remain * bytes[i] as f64 / miss_bytes as f64;
                }
                return;
            }
        }
        if known <= 0.0 {
            if total.is_finite() && total > 0.0 {
                self.split_duration_by_file_bytes(total);
            } else {
                self.bootstrap_from_bytes(live_chapter, live_dur);
            }
            return;
        }
        let miss_n = missing.len() as f64;
        let each = (self.total_sec.max(known) - known).max(0.0) / miss_n;
        if each > 0.0 {
            for i in missing {
                self.durs[i] = each;
            }
        }
    }

    /// Fill missing per-chapter lengths when a title spans several `.vob` files.
    pub(crate) fn ensure_chapter_dur_coverage(
        &mut self,
        title_total: f64,
        live_chapter: &Path,
        live_dur: f64,
    ) {
        if self.vobs.len() <= 1 {
            return;
        }
        let known: f64 = self.durs.iter().filter(|d| **d > 0.0).copied().sum();
        let need = self.durs.iter().any(|d| *d <= 0.0)
            || !(self.total_sec.is_finite() && self.total_sec > 0.0)
            || (title_total.is_finite() && title_total > known + 0.5);
        if !need {
            return;
        }
        self.fill_missing_chapter_durs(title_total, live_chapter, live_dur);
        self.recompute_starts();
    }

    /// Overwrite chapter lengths from SQLite / mpv per-file entries.
    pub(crate) fn apply_map_chapter_durs(&mut self, dur_by_path: &HashMap<String, f64>) {
        for (i, vob) in self.vobs.iter().enumerate() {
            let mapped = crate::dvd_vob_timeline::dur_from_map(dur_by_path, vob);
            if mapped > 0.0 {
                self.durs[i] = mapped;
            }
        }
        self.recompute_starts();
    }

    /// Prefer mpv `duration` on the open chapter when it is shorter than the byte estimate.
    pub(crate) fn apply_live_chapter_dur(&mut self, chapter: &Path, live_dur: f64) {
        if !(live_dur.is_finite() && live_dur > 0.0) {
            return;
        }
        let Some(i) = self.index_of(chapter) else {
            return;
        };
        if live_dur + 0.5 < self.durs[i] {
            self.durs[i] = live_dur;
        } else {
            self.grow_chapter_dur(chapter, live_dur);
        }
        self.recompute_starts();
    }
}
