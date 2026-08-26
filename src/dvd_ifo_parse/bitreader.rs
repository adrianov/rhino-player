//! MSB-first bit reader matching libdvdread `dvdread_getbits`.

pub(super) struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_pos: u8,
    byte: u8,
}

impl<'a> BitReader<'a> {
    pub(super) fn new(data: &'a [u8]) -> Option<Self> {
        let byte = *data.first()?;
        Some(Self {
            data,
            byte_pos: 0,
            bit_pos: 0,
            byte,
        })
    }

    fn advance_byte(&mut self) {
        self.bit_pos = 0;
        self.byte_pos += 1;
        self.byte = *self.data.get(self.byte_pos).unwrap_or(&0);
    }

    fn take_partial(&mut self, left: u32) -> u32 {
        let mut byte = self.byte;
        self.byte <<= left as u8;
        byte >>= 8 - left as u8;
        self.bit_pos += left as u8;
        if self.bit_pos == 8 {
            self.advance_byte();
        }
        u32::from(byte)
    }
    fn consume_mid_byte_bits(&mut self, left: &mut u32, result: &mut u32) {
        if *left <= (8 - self.bit_pos) as u32 {
            self.finish_in_partial_byte(left, result);
        } else {
            self.restart_from_next_byte(left, result);
        }
    }

    /// Remaining bits of a half-consumed byte complete the request.
    fn finish_in_partial_byte(&mut self, left: &mut u32, result: &mut u32) {
        let byte = self.take_partial(*left);
        *result = (*result << *left) | byte;
        *left = 0;
    }

    /// Consume the untouched high bits of the current byte and move on.
    fn restart_from_next_byte(&mut self, left: &mut u32, result: &mut u32) {
        let byte = self.byte >> self.bit_pos;
        *result = u32::from(byte);
        *left -= (8 - self.bit_pos) as u32;
        self.advance_byte();
    }

    fn consume_aligned_bits(&mut self, left: &mut u32, result: &mut u32) {
        self.take_whole_bytes(left, result);
        if *left > 0 {
            let byte = self.take_partial(*left);
            *result = (*result << *left) | byte;
            *left = 0;
        }
    }

    fn take_whole_bytes(&mut self, left: &mut u32, result: &mut u32) {
        while *left >= 8 {
            *result = (*result << 8) | u32::from(self.byte);
            *left -= 8;
            self.advance_byte();
        }
    }

    pub(super) fn getbits(&mut self, n: u32) -> Option<u32> {
        if n == 0 || n > 32 {
            return None;
        }
        let mut left = n;
        let mut result = 0u32;
        while left > 0 {
            if self.bit_pos > 0 {
                self.consume_mid_byte_bits(&mut left, &mut result);
            } else {
                self.consume_aligned_bits(&mut left, &mut result);
            }
        }
        Some(result)
    }
}

pub(super) fn read_audio_attr(raw: &[u8]) -> Option<(u8, u8, u16, u8)> {
    let mut bits = BitReader::new(raw)?;
    let (format, lang_type) = read_audio_format(&mut bits)?;
    skip_audio_attr_tail(&mut bits)?;
    let channels = bits.getbits(3)? as u8;
    let lang_code = bits.getbits(16)? as u16;
    Some((format, lang_type, lang_code, channels))
}

/// Coding mode, multichannel flag, language type and application mode fields.
fn read_audio_format(bits: &mut BitReader<'_>) -> Option<(u8, u8)> {
    let format = bits.getbits(3)? as u8;
    let _multichannel = bits.getbits(1)?;
    let lang_type = bits.getbits(2)? as u8;
    let _app_mode = bits.getbits(2)?;
    Some((format, lang_type))
}

/// Quantization, sample-rate and reserved bits between the format head and channel count.
fn skip_audio_attr_tail(bits: &mut BitReader<'_>) -> Option<()> {
    let _quant = bits.getbits(2)?;
    let _sample = bits.getbits(2)?;
    let _unknown1 = bits.getbits(1)?;
    Some(())
}

pub(super) fn read_subp_attr(raw: &[u8]) -> Option<(u8, u16)> {
    let mut bits = BitReader::new(raw)?;
    let _code_mode = bits.getbits(3)?;
    let _zero1 = bits.getbits(3)?;
    let typ = bits.getbits(2)? as u8;
    let _zero2 = bits.getbits(8)?;
    let lang_code = bits.getbits(16)? as u16;
    Some((typ, lang_code))
}
