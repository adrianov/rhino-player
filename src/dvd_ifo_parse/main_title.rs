// Disc main-feature pick from `VIDEO_TS.IFO` TT_SRPT (included from `dvd_ifo_parse.rs`).

use buf::IfoBuf;

fn tt_srpt_best(buf: &IfoBuf, _base: usize, nr: usize, titles_off: usize, skip_menu: bool) -> (u32, u32) {
    let mut best_idx = 0usize;
    let mut best_ptt = -1i32;
    let mut best_vts = 99u32;
    for i in 0..nr {
        let off = titles_off + i * TITLE_INFO_SIZE;
        let vts = buf.byte(off + 6) as u32;
        if skip_menu && vts < 2 {
            continue;
        }
        let ptt = buf.be16(off + 2) as i32;
        if ptt > best_ptt || (ptt == best_ptt && vts < best_vts) {
            best_ptt = ptt;
            best_vts = vts;
            best_idx = i;
        }
    }
    let off = titles_off + best_idx * TITLE_INFO_SIZE;
    let vts_id = buf.byte(off + 6) as u32;
    let ttn = buf.byte(off + 7).max(1) as u32;
    (vts_id, ttn)
}

fn best_ttn_on_vts(vts_dir: &Path, vts_id: u32) -> Option<u32> {
    let ifo = vts_dir.join(format!("VTS_{vts_id:02}_0.IFO"));
    let mut best: Option<(u32, f64)> = None;
    for ttn in 1..=9_u32 {
        let dur = title_ttn_playback_sec(&ifo, ttn as usize).unwrap_or(0.0);
        if dur < MIN_SUBSTANTIAL_SEC {
            continue;
        }
        if best.is_none_or(|(_, d)| dur > d) {
            best = Some((ttn, dur));
        }
    }
    best.map(|(t, _)| t)
}

/// Disc-level main feature from `VIDEO_TS.IFO` (`TT_SRPT`): `(VTS number, title within VTS)`.
pub fn main_title_from_disc(disc: &Path) -> Option<(u32, u32)> {
    let vts_dir = crate::video_ext::dvd_video_ts_dir(disc)?;
    let ifo = vts_dir.join("VIDEO_TS.IFO");
    let buf = IfoBuf::load(&ifo)?;
    let sector = buf.be32(TT_SRPT_OFF) as usize;
    if sector == 0 {
        return None;
    }
    let base = sector * BLOCK;
    if base + 8 > buf.len() {
        return None;
    }
    let nr = buf.be16(base) as usize;
    if nr == 0 || nr >= 100 {
        return None;
    }
    let mut last = buf.be32(base + 4);
    if last == 0 {
        last = (nr * TITLE_INFO_SIZE + 8 - 1) as u32;
    }
    let info_len = last as usize + 1 - 8;
    let titles_off = base + 8;
    if titles_off + info_len > buf.len() || nr > info_len / TITLE_INFO_SIZE {
        return None;
    }
    let mut skip_menu = false;
    for i in 0..nr {
        let off = titles_off + i * TITLE_INFO_SIZE;
        if buf.byte(off + 6) >= 2 {
            skip_menu = true;
            break;
        }
    }
    let (srpt_vts, srpt_ttn) = tt_srpt_best(&buf, base, nr, titles_off, skip_menu);
    let bytes_vts = crate::video_ext::feature_title_set_id(&vts_dir).unwrap_or(srpt_vts);
    let vts_id = crate::video_ext::resolve_dvd_main_vts(&vts_dir, srpt_vts, bytes_vts);
    let ttn = best_ttn_on_vts(&vts_dir, vts_id).unwrap_or(if vts_id == srpt_vts { srpt_ttn } else { 1 });
    (1..=99).contains(&vts_id).then_some((vts_id, ttn))
}
