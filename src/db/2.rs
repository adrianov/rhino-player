pub fn load_video() -> VideoPrefs {
    let mut p = VideoPrefs::default();
    if let Some(s) = get_setting_str(K_VIDEO_SMOOTH_60) {
        p.smooth_60 = s == "1" || s.eq_ignore_ascii_case("true");
    }
    if let Some(s) = get_setting_str(K_VIDEO_VS) {
        p.vs_path = s;
    }
    if let Some(s) = get_setting_str(K_VIDEO_MVTOOLS_LIB) {
        p.mvtools_lib = s;
    }
    p
}

pub fn save_video(p: &VideoPrefs) {
    put_setting(K_VIDEO_SMOOTH_60, if p.smooth_60 { "1" } else { "0" });
    put_setting(K_VIDEO_VS, &p.vs_path);
    put_setting(K_VIDEO_MVTOOLS_LIB, &p.mvtools_lib);
}

// --- subtitle appearance + last manual track label (see docs/features/24-subtitles.md) ---

const K_SUB_COLOR: &str = "sub_color";
const K_SUB_BORDER: &str = "sub_border_color";
const K_SUB_BSIZE: &str = "sub_border_size";
const K_SUB_SCALE: &str = "sub_scale";
const K_SUB_LAST: &str = "sub_last_label";
const K_SUB_OFF: &str = "sub_off";

/// SQLite-backed subtitle prefs (not every mpv `sub-*` key).
#[derive(Debug, Clone)]
pub struct SubPrefs {
    /// Text `0xRRGGBB`, warm yellow by default.
    pub color: u32,
    pub border_color: u32,
    pub border_size: f64,
    pub scale: f64,
    /// Last subtitle track the user picked in the popover (label text), for Levenshtein auto-pick.
    pub last_sub_label: String,
    /// User chose **Off**: do not run Levenshtein on new files; keep `sub-visibility` off after load.
    pub sub_off: bool,
}

impl Default for SubPrefs {
    fn default() -> Self {
        Self {
            color: 0xF0E4A0,
            border_color: 0x0A0A0A,
            border_size: 2.5,
            scale: 1.0,
            last_sub_label: String::new(),
            sub_off: false,
        }
    }
}

fn parse_u32(s: &str) -> Option<u32> {
    let t = s.trim();
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16).ok()
    } else {
        t.parse::<u32>().ok()
    }
}

fn get_setting_str(key: &str) -> Option<String> {
    with_conn(|c| {
        c.query_row("SELECT v FROM settings WHERE k = ?1", params![key], |row| {
            let s: String = row.get(0)?;
            Ok(s)
        })
        .optional()
    })
    .flatten()
}

/// Default loaded prefs (merged with [Default] for missing keys).
pub fn load_sub() -> SubPrefs {
    let mut p = SubPrefs::default();
    if let Some(s) = get_setting_str(K_SUB_COLOR) {
        if let Some(n) = parse_u32(&s) {
            p.color = n;
        }
    }
    if let Some(s) = get_setting_str(K_SUB_BORDER) {
        if let Some(n) = parse_u32(&s) {
            p.border_color = n;
        }
    }
    if let Some(s) = get_setting_str(K_SUB_BSIZE) {
        if let Ok(f) = s.parse::<f64>() {
            p.border_size = f.clamp(0.0, 8.0);
        }
    }
    if let Some(s) = get_setting_str(K_SUB_SCALE) {
        if let Ok(f) = s.parse::<f64>() {
            p.scale = f.clamp(0.2, 3.0);
        }
    }
    if let Some(s) = get_setting_str(K_SUB_LAST) {
        p.last_sub_label = s;
    }
    if let Some(s) = get_setting_str(K_SUB_OFF) {
        p.sub_off = s == "1" || s.eq_ignore_ascii_case("true");
    }
    p
}

fn put_setting(key: &str, val: &str) {
    let _ = with_conn(|c| {
        c.execute(
            "INSERT INTO settings (k, v) VALUES (?1, ?2)
             ON CONFLICT(k) DO UPDATE SET v = excluded.v",
            params![key, val],
        )?;
        Ok(())
    });
}

/// Persist; safe from quit and after each user edit.
pub fn save_sub(p: &SubPrefs) {
    let br = p.border_size.clamp(0.0, 8.0);
    let sc = p.scale.clamp(0.2, 3.0);
    put_setting(K_SUB_COLOR, &format!("{:#X}", p.color));
    put_setting(K_SUB_BORDER, &format!("{:#X}", p.border_color));
    put_setting(K_SUB_BSIZE, &format!("{br:.4}"));
    put_setting(K_SUB_SCALE, &format!("{sc:.4}"));
    put_setting(K_SUB_LAST, &p.last_sub_label);
    put_setting(K_SUB_OFF, if p.sub_off { "1" } else { "0" });
}

