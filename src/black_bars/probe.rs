// Temporary lavfi `cropdetect` → mpv `video-crop` for Fill Screen.

type Player = Rc<std::cell::RefCell<Option<MpvBundle>>>;

/// Labeled filter so we can remove it without touching Smooth / deinterlace vf.
const CROPDETECT_LABEL: &str = "rhino-bars";
/// Delay before insert — avoids black opens / fades; timer is required (cropdetect needs frames).
const DETECT_DELAY: Duration = Duration::from_millis(2000);
const DETECT_GATHER: Duration = Duration::from_millis(1000);
/// Fallback when decode size is late after FileLoaded (VideoReconfig may already have fired).
const READY_RETRY: Duration = Duration::from_millis(250);
const READY_RETRY_MAX: u8 = 8;
const DETECT_LIMIT: &str = "24/255";
const DETECT_ROUND: i64 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CropMeta {
    w: i64,
    h: i64,
    x: i64,
    y: i64,
}

/// Start a delayed cropdetect for the current path. Earlier probes are cancelled via `gen`.
pub fn schedule_bar_probe(player: &Player, probe: &Rc<BarProbe>, on_done: Rc<dyn Fn()>) {
    let gen = probe.start_gen();
    let player = Rc::clone(player);
    let probe = Rc::clone(probe);
    glib::timeout_add_local_once(DETECT_DELAY, move || {
        if probe.gen.get() != gen {
            return;
        }
        probe.past_delay.set(true);
        begin_cropdetect(&player, &probe, gen, on_done);
    });
}

/// FileLoaded / VideoReconfig: if waiting on decode size, start cropdetect as soon as ready.
pub fn pump_bar_probe(player: &Player, probe: &Rc<BarProbe>, on_done: Rc<dyn Fn()>) {
    if !matches!(probe.state.get(), BarState::Pending) {
        return;
    }
    if !probe.past_delay.get() || probe.gathering.get() {
        return;
    }
    if video_ready_state(player) != ReadyState::Ready {
        return;
    }
    let gen = probe.gen.get();
    begin_cropdetect(player, probe, gen, on_done);
}

fn begin_cropdetect(player: &Player, probe: &Rc<BarProbe>, gen: u64, on_done: Rc<dyn Fn()>) {
    if probe.gathering.get() || probe.gen.get() != gen {
        return;
    }
    match video_ready_state(player) {
        ReadyState::NoPlayer => {
            probe.state.set(BarState::Clean);
            on_done();
            return;
        }
        ReadyState::Waiting => {
            defer_until_video_ready(player, probe, gen, on_done);
            return;
        }
        ReadyState::Ready => {}
    }
    let Some(hw_backup) = insert_cropdetect(player) else {
        probe.state.set(BarState::Clean);
        on_done();
        return;
    };
    probe.gathering.set(true);
    let player = Rc::clone(player);
    let probe = Rc::clone(probe);
    glib::timeout_add_local_once(DETECT_GATHER, move || {
        finish_cropdetect(&player, &probe, gen, hw_backup, on_done);
    });
}

#[derive(PartialEq, Eq)]
enum ReadyState {
    NoPlayer,
    Waiting,
    Ready,
}

fn video_ready_state(player: &Player) -> ReadyState {
    match player.borrow().as_ref() {
        None => ReadyState::NoPlayer,
        Some(b) if video_ready(&b.mpv) => ReadyState::Ready,
        Some(_) => ReadyState::Waiting,
    }
}

/// Bounded follow-ups after detect delay when width/height are still unset.
fn defer_until_video_ready(
    player: &Player,
    probe: &Rc<BarProbe>,
    gen: u64,
    on_done: Rc<dyn Fn()>,
) {
    let left = probe.ready_left.get();
    if left == 0 {
        eprintln!("[rhino] bars: probe gave up (video never ready)");
        probe.state.set(BarState::Clean);
        on_done();
        return;
    }
    probe.ready_left.set(left - 1);
    eprintln!(
        "[rhino] bars: probe deferred (no video yet), retry {} left",
        left - 1
    );
    let player = Rc::clone(player);
    let probe = Rc::clone(probe);
    glib::timeout_add_local_once(READY_RETRY, move || {
        if probe.gen.get() != gen {
            return;
        }
        begin_cropdetect(&player, &probe, gen, on_done);
    });
}

/// Append cropdetect after any Bob/Smooth filters so detection sees progressive frames.
fn insert_cropdetect(player: &Player) -> Option<Option<String>> {
    player.borrow().as_ref().and_then(|b| {
        let mpv = &b.mpv;
        remove_cropdetect(mpv);
        let hw_backup = pause_noncopy_hwdec(mpv);
        let spec = format!(
            "@{CROPDETECT_LABEL}:cropdetect=limit={DETECT_LIMIT}:round={DETECT_ROUND}:reset=0"
        );
        match mpv.command("vf", &["add", &spec]) {
            Ok(()) => Some(hw_backup),
            Err(e) => {
                eprintln!("[rhino] bars: cropdetect insert failed: {e}");
                restore_hwdec(mpv, hw_backup.as_deref());
                None
            }
        }
    })
}

fn finish_cropdetect(
    player: &Player,
    probe: &Rc<BarProbe>,
    gen: u64,
    hw_backup: Option<String>,
    on_done: Rc<dyn Fn()>,
) {
    if probe.gen.get() != gen {
        abort_stale_probe(player, hw_backup.as_deref());
        return;
    }
    let (state, saw_deint) = take_probe_result(player, hw_backup.as_deref());
    probe.gathering.set(false);
    probe.saw_deint.set(saw_deint);
    probe.state.set(state);
    on_done();
}

fn abort_stale_probe(player: &Player, hw_backup: Option<&str>) {
    if let Some(b) = player.borrow().as_ref() {
        remove_cropdetect(&b.mpv);
        restore_hwdec(&b.mpv, hw_backup);
    }
}

fn take_probe_result(player: &Player, hw_backup: Option<&str>) -> (BarState, bool) {
    player.borrow().as_ref().map_or((BarState::Clean, false), |b| {
        let mpv = &b.mpv;
        let saw_deint = crate::video_pref::bob_deinterlace_in_vf(
            &mpv.get_property::<String>("vf").unwrap_or_default(),
        );
        let meta = read_cropdetect_meta(mpv);
        remove_cropdetect(mpv);
        restore_hwdec(mpv, hw_backup);
        let state = match meta.and_then(|m| crop_from_meta(mpv, m)) {
            Some(rect) => {
                eprintln!(
                    "[rhino] bars: detected crop={}x{}+{}+{}",
                    rect.w, rect.h, rect.x, rect.y
                );
                BarState::Crop(rect)
            }
            None => {
                eprintln!("[rhino] bars: probe clean (no strips)");
                BarState::Clean
            }
        };
        (state, saw_deint)
    })
}

fn video_ready(mpv: &Mpv) -> bool {
    mpv.get_property::<i64>("width").unwrap_or(0) > 0
        && mpv.get_property::<i64>("height").unwrap_or(0) > 0
}

fn pause_noncopy_hwdec(mpv: &Mpv) -> Option<String> {
    let current = mpv
        .get_property::<String>("hwdec-current")
        .unwrap_or_else(|_| "no".into());
    let ok = current == "no"
        || current == "crystalhd"
        || current == "rkmpp"
        || current.ends_with("-copy");
    if ok {
        return None;
    }
    let backup = mpv.get_property::<String>("hwdec").ok();
    crate::video_pref::ensure_hwdec_vf_copy(mpv);
    backup
}

fn restore_hwdec(mpv: &Mpv, backup: Option<&str>) {
    if let Some(mode) = backup {
        if let Err(e) = mpv.set_property("hwdec", mode) {
            eprintln!("[rhino] bars: hwdec restore failed: {e}");
        }
    }
}

fn remove_cropdetect(mpv: &Mpv) {
    if !mpv
        .get_property::<String>("vf")
        .unwrap_or_default()
        .contains(CROPDETECT_LABEL)
    {
        return;
    }
    let label = format!("@{CROPDETECT_LABEL}");
    if let Err(e) = mpv.command("vf", &["remove", &label]) {
        eprintln!("[rhino] bars: cropdetect remove failed: {e}");
    }
}

pub fn apply_video_crop(mpv: &Mpv, rect: Option<CropRect>) {
    if let Err(e) = mpv.set_property(
        "video-crop",
        rect.map(CropRect::as_video_crop).unwrap_or_default().as_str(),
    ) {
        eprintln!("[rhino] bars: video-crop set failed: {e}");
    }
}

pub fn clear_video_crop(mpv: &Mpv) {
    apply_video_crop(mpv, None);
}

#[cfg(test)]
mod probe_tests {
    use super::*;

    #[test]
    fn crop_rect_formats_mpv_video_crop() {
        assert_eq!(
            CropRect {
                w: 1920,
                h: 800,
                x: 0,
                y: 140
            }
            .as_video_crop(),
            "1920x800+0+140"
        );
    }
}

// Parse lavfi cropdetect from `vf-metadata/<label>` as `MPV_FORMAT_NODE` only.
// Per-key `…/lavfi.cropdetect.*` get_property can SIGSEGV in libmpv (`mp_tags_get_bstr`
// with NULL tags) while Smooth rebuilds the vf chain.

fn read_cropdetect_meta(mpv: &Mpv) -> Option<CropMeta> {
    let name = CString::new(format!("vf-metadata/{CROPDETECT_LABEL}")).ok()?;
    let mut root = std::mem::MaybeUninit::<libmpv2_sys::mpv_node>::uninit();
    let err = unsafe {
        libmpv2_sys::mpv_get_property(
            mpv.ctx.as_ptr(),
            name.as_ptr(),
            libmpv2_sys::mpv_format_MPV_FORMAT_NODE,
            root.as_mut_ptr().cast(),
        )
    };
    if err < 0 {
        return None;
    }
    let mut root = unsafe { root.assume_init() };
    let meta = unsafe { parse_crop_map(&root) };
    unsafe { libmpv2_sys::mpv_free_node_contents(&mut root) };
    meta
}

unsafe fn parse_crop_map(root: &libmpv2_sys::mpv_node) -> Option<CropMeta> {
    let (keys, vals) = map_keys_values(root)?;
    let mut fields = CropFields::default();
    for (key_ptr, vn) in keys.iter().zip(vals.iter()) {
        if key_ptr.is_null() {
            continue;
        }
        let key = unsafe { CStr::from_ptr(*key_ptr) }.to_bytes();
        if let Some(n) = node_number(vn) {
            fields.absorb(key, n);
        }
    }
    fields.into_meta()
}

#[derive(Default)]
struct CropFields {
    w: Option<i64>,
    h: Option<i64>,
    x: Option<i64>,
    y: Option<i64>,
}

impl CropFields {
    fn absorb(&mut self, key: &[u8], n: i64) {
        match key {
            b"lavfi.cropdetect.w" => self.w = Some(n),
            b"lavfi.cropdetect.h" => self.h = Some(n),
            b"lavfi.cropdetect.x" => self.x = Some(n),
            b"lavfi.cropdetect.y" => self.y = Some(n),
            _ => {}
        }
    }

    fn into_meta(self) -> Option<CropMeta> {
        Some(CropMeta {
            w: self.w?,
            h: self.h?,
            x: self.x?,
            y: self.y?,
        })
    }
}

fn node_number(vn: &libmpv2_sys::mpv_node) -> Option<i64> {
    if vn.format == libmpv2_sys::mpv_format_MPV_FORMAT_INT64 {
        return Some(unsafe { vn.u.int64 });
    }
    if vn.format == libmpv2_sys::mpv_format_MPV_FORMAT_DOUBLE {
        return Some(unsafe { vn.u.double_ } as i64);
    }
    if vn.format == libmpv2_sys::mpv_format_MPV_FORMAT_STRING {
        let sp = unsafe { vn.u.string };
        if sp.is_null() {
            return None;
        }
        return unsafe { CStr::from_ptr(sp) }
            .to_string_lossy()
            .trim()
            .parse()
            .ok();
    }
    None
}

unsafe fn map_keys_values(
    entry: &libmpv2_sys::mpv_node,
) -> Option<(&[*const std::os::raw::c_char], &[libmpv2_sys::mpv_node])> {
    if entry.format != libmpv2_sys::mpv_format_MPV_FORMAT_NODE_MAP {
        return None;
    }
    let list_ptr = entry.u.list;
    if list_ptr.is_null() {
        return None;
    }
    let n = unsafe { (*list_ptr).num } as usize;
    let (keys, vals) = unsafe { ((*list_ptr).keys, (*list_ptr).values) };
    if keys.is_null() || vals.is_null() || n == 0 {
        return None;
    }
    Some((
        unsafe { std::slice::from_raw_parts(keys.cast(), n) },
        unsafe { std::slice::from_raw_parts(vals, n) },
    ))
}

fn crop_from_meta(mpv: &Mpv, meta: CropMeta) -> Option<CropRect> {
    let width = mpv.get_property::<i64>("width").ok()?;
    let height = mpv.get_property::<i64>("height").ok()?;
    let ok = width > 0
        && height > 0
        && crop_meta_in_frame(width, height, meta)
        && crop_meaningful(width, height, meta.w, meta.h)
        && !(meta.x == 0 && meta.y == 0 && meta.w == width && meta.h == height);
    ok.then_some(CropRect {
        w: meta.w,
        h: meta.h,
        x: meta.x,
        y: meta.y,
    })
}

fn crop_meta_in_frame(width: i64, height: i64, meta: CropMeta) -> bool {
    meta.x >= 0
        && meta.y >= 0
        && meta.w > 0
        && meta.h > 0
        && meta.x + meta.w <= width
        && meta.y + meta.h <= height
}
