/// Format seconds as a clock string (human-readable ranges; see `docs/product-and-use-cases.md` and `docs/features/04-transport-and-progress.md`).
pub fn format_time(seconds: f64) -> String {
    if !seconds.is_finite() || seconds <= 0.0 {
        return "0:00".to_string();
    }
    let sec = seconds.floor() as u64;
    let d = sec / 86_400;
    let h = (sec % 86_400) / 3_600;
    let m = (sec % 3_600) / 60;
    let s = sec % 60;
    if d > 0 {
        format!("{d}:{h:02}:{m:02}:{s:02}")
    } else if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_and_negative() {
        assert_eq!(format_time(0.0), "0:00");
        assert_eq!(format_time(-1.0), "0:00");
    }

    #[test]
    fn under_one_hour() {
        assert_eq!(format_time(45.0), "0:45");
        assert_eq!(format_time(9.0), "0:09");
        assert_eq!(format_time(59.0), "0:59");
        assert_eq!(format_time(60.0), "1:00");
        assert_eq!(format_time(125.0), "2:05");
    }

    #[test]
    fn one_hour_plus() {
        assert_eq!(format_time(3_600.0), "1:00:00");
        assert_eq!(format_time(3_661.0), "1:01:01");
    }

    #[test]
    fn days() {
        assert_eq!(format_time(90_000.0), "1:01:00:00");
    }
}
