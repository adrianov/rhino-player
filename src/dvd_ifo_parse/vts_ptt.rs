use super::buf::IfoBuf;
use super::pgc::Pgcit;
use super::{BLOCK, VTS_PGCIT_OFF, VTS_PTT_OFF};

pub(super) struct PttTitle {
    pub ptt: Vec<(u16, u16)>,
}

pub(super) struct VtsPtt {
    pub titles: Vec<PttTitle>,
}

pub(super) struct VtsTables {
    pub ptt: VtsPtt,
    pub pgcit: Pgcit,
}

pub(super) fn load_vts_tables(ifo_path: &std::path::Path) -> Option<VtsTables> {
    let buf = IfoBuf::load(ifo_path)?;
    let ptt_sec = buf.be32(VTS_PTT_OFF) as usize;
    let pgcit_sec = buf.be32(VTS_PGCIT_OFF) as usize;
    if ptt_sec == 0 || pgcit_sec == 0 {
        return None;
    }
    Some(VtsTables {
        ptt: parse_vts_ptt(&buf, ptt_sec)?,
        pgcit: super::pgc::parse_pgcit(&buf, pgcit_sec, BLOCK)?,
    })
}

/// Byte layout of one `VTSM_PGCI/PTT` table: base offset plus per-title start offsets.
struct PttOffsets {
    base: usize,
    last: u32,
    offsets: Vec<usize>,
}

impl PttOffsets {
    fn parse(buf: &IfoBuf, sector: usize) -> Option<Self> {
        let base = sector * BLOCK;
        if base + 8 > buf.len() {
            return None;
        }
        let (nr, last) = Self::extent(buf, base)?;
        let data_off = base + 8;
        if data_off + ptt_info_len(last) > buf.len() {
            return None;
        }
        Some(Self {
            base,
            last,
            offsets: Self::offsets(buf, data_off, nr, last)?,
        })
    }

    fn extent(buf: &IfoBuf, base: usize) -> Option<(usize, u32)> {
        let nr = buf.be16(base) as usize;
        if nr == 0 || nr >= 100 {
            return None;
        }
        let mut last = buf.be32(base + 4);
        if last == 0 {
            last = (nr * 4 + 8 - 1) as u32;
        }
        Some((nr, last))
    }

    fn offsets(buf: &IfoBuf, data_off: usize, nr: usize, last: u32) -> Option<Vec<usize>> {
        let mut offsets = Vec::with_capacity(nr);
        for i in 0..nr {
            let off = data_off + i * 4;
            let start = buf.be32(off);
            if start as usize + 4 > last as usize + 1 {
                return None;
            }
            offsets.push(start as usize);
        }
        Some(offsets)
    }

    fn title_byte_len(&self, i: usize) -> usize {
        let start = self.offsets[i];
        if i + 1 < self.offsets.len() {
            self.offsets[i + 1].saturating_sub(start)
        } else {
            self.last as usize + 1 - start
        }
    }

    fn read_ptt(&self, buf: &IfoBuf, start: usize, nr_ptt: usize) -> Vec<(u16, u16)> {
        let mut ptt = Vec::with_capacity(nr_ptt);
        for j in 0..nr_ptt {
            let o = self.base + start + j * 4;
            if o + 4 > buf.len() {
                break;
            }
            ptt.push((buf.be16(o), buf.be16(o + 2)));
        }
        ptt
    }
}

fn parse_vts_ptt(buf: &IfoBuf, sector: usize) -> Option<VtsPtt> {
    let table = PttOffsets::parse(buf, sector)?;
    let mut titles = Vec::with_capacity(table.offsets.len());
    for i in 0..table.offsets.len() {
        let n = table.title_byte_len(i);
        if n % 4 != 0 {
            continue;
        }
        let nr_ptt = n / 4;
        titles.push(PttTitle {
            ptt: table.read_ptt(buf, table.offsets[i], nr_ptt),
        });
    }
    Some(VtsPtt { titles })
}

fn ptt_info_len(last: u32) -> usize {
    last as usize + 1 - 8
}
