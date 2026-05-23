use std::path::Path;

pub(super) struct IfoBuf(pub(super) Vec<u8>);

impl IfoBuf {
    pub(super) fn load(path: &Path) -> Option<Self> {
        let data = std::fs::read(path).ok()?;
        (!data.is_empty()).then_some(IfoBuf(data))
    }

    pub(super) fn len(&self) -> usize {
        self.0.len()
    }

    pub(super) fn be16(&self, off: usize) -> u16 {
        let b = &self.0;
        if off + 2 > b.len() {
            return 0;
        }
        u16::from_be_bytes([b[off], b[off + 1]])
    }

    pub(super) fn be32(&self, off: usize) -> u32 {
        let b = &self.0;
        if off + 4 > b.len() {
            return 0;
        }
        u32::from_be_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
    }

    pub(super) fn slice(&self, off: usize, len: usize) -> Option<&[u8]> {
        self.0.get(off..off + len)
    }

    pub(super) fn byte(&self, off: usize) -> u8 {
        self.0.get(off).copied().unwrap_or(0)
    }
}
