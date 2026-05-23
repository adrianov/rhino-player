pub(super) fn dvdtime_to_sec(raw: &[u8]) -> f64 {
    if raw.len() < 4 {
        return 0.0;
    }
    let sec = bcd(raw[0]) as f64 * 3600.0
        + bcd(raw[1]) as f64 * 60.0
        + bcd(raw[2]) as f64;
    let fps = match (raw[3] & 0xc0) >> 6 {
        1 => 25.0,
        3 => 29.97,
        _ => 2500.0,
    };
    sec + bcd(raw[3] & 0x3f) as f64 * (1.0 / fps)
}

fn bcd(x: u8) -> u32 {
    ((x >> 4) as u32) * 10 + (x & 0x0f) as u32
}
