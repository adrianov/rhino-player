//! VTSM_PGCI/PTT byte-table decoding (`VTS_xx_0.IFO`).

use super::PttTitle;
use crate::dvd_ifo_parse::buf::IfoBuf;
use crate::dvd_ifo_parse::BLOCK;

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

fn ptt_info_len(last: u32) -> usize {
    last as usize + 1 - 8
}

pub(super) fn parse_titles(buf: &IfoBuf, sector: usize) -> Option<Vec<PttTitle>> {
    let table = PttOffsets::parse(buf, sector)?;
    let mut titles = Vec::with_capacity(table.offsets.len());
    for i in 0..table.offsets.len() {
        let n = table.title_byte_len(i);
        if n % 4 != 0 {
            continue;
        }
        let nr_ptt = n / 4;
        let ptt = table.read_ptt(buf, table.offsets[i], nr_ptt);
        titles.push(PttTitle { ptt });
    }
    Some(titles)
}
