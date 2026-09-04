// Disc main-feature pick from `VIDEO_TS.IFO` TT_SRPT (included from `dvd_ifo_parse.rs`).

use buf::IfoBuf;

struct SrptTable {
    nr: usize,
    titles_off: usize,
}

fn parse_srpt_header(buf: &IfoBuf) -> Option<SrptTable> {
    let base = srpt_base(buf)?;
    let (nr, last) = srpt_extent(buf, base)?;
    let info_len = last as usize + 1 - 8;
    let table = SrptTable {
        nr,
        titles_off: base + 8,
    };
    if table.titles_off + info_len > buf.len() || nr > info_len / TITLE_INFO_SIZE {
        return None;
    }
    Some(table)
}

fn srpt_base(buf: &IfoBuf) -> Option<usize> {
    let sector = buf.be32(TT_SRPT_OFF) as usize;
    if sector == 0 {
        return None;
    }
    let base = sector * BLOCK;
    (base + 8 <= buf.len()).then_some(base)
}

fn srpt_extent(buf: &IfoBuf, base: usize) -> Option<(usize, u32)> {
    let nr = buf.be16(base) as usize;
    if nr == 0 || nr >= 100 {
        return None;
    }
    let mut last = buf.be32(base + 4);
    if last == 0 {
        last = (nr * TITLE_INFO_SIZE + 8 - 1) as u32;
    }
    Some((nr, last))
}

fn best_srpt_index(buf: &IfoBuf, nr: usize, titles_off: usize, skip_menu: bool) -> usize {
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
    best_idx
}

fn tt_srpt_best(buf: &IfoBuf, nr: usize, titles_off: usize, skip_menu: bool) -> (u32, u32) {
    let off = titles_off + best_srpt_index(buf, nr, titles_off, skip_menu) * TITLE_INFO_SIZE;
    let vts_id = buf.byte(off + 6) as u32;
    let ttn = buf.byte(off + 7).max(1) as u32;
    (vts_id, ttn)
}

fn any_non_menu_title(buf: &IfoBuf, table: &SrptTable) -> bool {
    (0..table.nr).any(|i| {
        let off = table.titles_off + i * TITLE_INFO_SIZE;
        buf.byte(off + 6) >= 2
    })
}

fn best_ttn_on_vts(vts_dir: &Path, vts_id: u32) -> Option<u32> {
    let ifo = vts_dir.join(format!("VTS_{vts_id:02}_0.IFO"));
    let mut best: Option<(u32, f64)> = None;
    for ttn in 1..=9_u32 {
        let dur = title_ttn_playback_sec(&ifo, ttn as usize).unwrap_or(0.0);
        if dur < MIN_SUBSTANTIAL_SEC {
            continue;
        }
        if best.map_or(true, |(_, d)| dur > d) {
            best = Some((ttn, dur));
        }
    }
    best.map(|(t, _)| t)
}

fn load_srpt(disc: &Path) -> Option<(std::path::PathBuf, IfoBuf, SrptTable)> {
    let vts_dir = crate::video_ext::dvd_video_ts_dir(disc)?;
    let buf = IfoBuf::load(&vts_dir.join("VIDEO_TS.IFO"))?;
    let table = parse_srpt_header(&buf)?;
    Some((vts_dir, buf, table))
}

/// VTS id / TTN after reconciling the SRPT pick with byte-scan and on-disk hints.
fn resolve_main_vts(vts_dir: &Path, srpt: (u32, u32)) -> (u32, u32) {
    let (srpt_vts, srpt_ttn) = srpt;
    let vts_id = crate::video_ext::resolve_dvd_main_vts(
        vts_dir,
        srpt_vts,
        crate::video_ext::feature_title_set_id(vts_dir).unwrap_or(srpt_vts),
    );
    (
        vts_id,
        best_ttn_on_vts(vts_dir, vts_id)
            .unwrap_or(if vts_id == srpt_vts { srpt_ttn } else { 1 }),
    )
}

/// Disc-level main feature from `VIDEO_TS.IFO` (`TT_SRPT`): `(VTS number, title within VTS)`.
pub fn main_title_from_disc(disc: &Path) -> Option<(u32, u32)> {
    let (vts_dir, buf, table) = load_srpt(disc)?;
    let (vts_id, ttn) = resolve_main_vts(
        &vts_dir,
        tt_srpt_best(
            &buf,
            table.nr,
            table.titles_off,
            any_non_menu_title(&buf, &table),
        ),
    );
    (1..=99).contains(&vts_id).then_some((vts_id, ttn))
}
