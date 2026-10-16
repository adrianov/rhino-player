// Fill Screen strip probe: samples displayed frames (`screenshot-raw`) → mpv `video-crop`.
// Never edits the vf chain: a filter rebuild restarts Smooth's VapourSynth graph (seconds
// of frozen video) and its black warm-up frames read as huge fake strips.

type Player = Rc<std::cell::RefCell<Option<MpvBundle>>>;

/// Delay before sampling — avoids black opens / fades.
const DETECT_DELAY: Duration = Duration::from_millis(2000);
/// Frames per gather; their picture bounds are united so one dark-edged frame cannot
/// shrink the crop. Spaced out so the samples rarely share one dark scene.
const GATHER_SAMPLES: u8 = 4;
const SAMPLE_GAP: Duration = Duration::from_millis(2000);
/// Start a delayed strip probe for the current path. Earlier probes are cancelled via `gen`.
pub fn schedule_bar_probe(player: &Player, probe: &Rc<BarProbe>, on_done: Rc<dyn Fn()>) {
    let gen = probe.start_gen();
    eprintln!("[rhino] bars: probe scheduled gen {gen}");
    let player = Rc::clone(player);
    let probe = Rc::clone(probe);
    glib::timeout_add_local_once(DETECT_DELAY, move || {
        if probe.gen.get() != gen {
            eprintln!(
                "[rhino] bars: probe delay stale: gen {gen} vs live {}",
                probe.gen.get()
            );
            return;
        }
        probe.past_delay.set(true);
        begin_gather(&player, &probe, gen, on_done);
    });
}

/// FileLoaded / VideoReconfig / unpause: if a probe is still pending, sample as soon as ready.
pub fn pump_bar_probe(player: &Player, probe: &Rc<BarProbe>, on_done: Rc<dyn Fn()>) {
    if !probe.past_delay.get() || probe.gathering.get() {
        return;
    }
    if video_ready_state(player) != ReadyState::Ready {
        return;
    }
    let gen = probe.gen.get();
    begin_gather(player, probe, gen, on_done);
}

/// True when this gen's probe is stale, in flight, or already finished:
/// a deferred retry whose result completed via a pump-driven run must not
/// start another gather (its own NoData outcome would overwrite the final state).
fn probe_start_blocked(probe: &Rc<BarProbe>, gen: u64) -> bool {
    probe.gathering.get()
        || probe.gen.get() != gen
        || !matches!(probe.state.get(), BarState::Pending)
}

fn begin_gather(player: &Player, probe: &Rc<BarProbe>, gen: u64, on_done: Rc<dyn Fn()>) {
    if probe_start_blocked(probe, gen) {
        eprintln!(
            "[rhino] bars: probe begin blocked: gen {gen} vs live {}, state {:?}",
            probe.gen.get(),
            probe.state.get()
        );
        return;
    }
    match video_ready_state(player) {
        ReadyState::NoPlayer => {
            eprintln!("[rhino] bars: probe begin gen {gen}: no player; next media re-arms");
            return; // player gone; next media invalidates the gen
        }
        ReadyState::Waiting => {
            defer_until_video_ready(player, probe, gen, on_done);
            return;
        }
        ReadyState::Ready => {}
    }
    // A paused start shows one frame (often a fade or title card) — wait for playback.
    if probe_paused(player) {
        defer_no_data(player, probe, gen, on_done, "paused");
        return;
    }
    probe.gathering.set(true);
    gather_step(
        Rc::new(GatherRun {
            player: Rc::clone(player),
            probe: Rc::clone(probe),
            gen,
            on_done,
        }),
        Gather::default(),
        GATHER_SAMPLES,
    );
}

/// One in-flight gather: what the sample chain needs to finish or bail out.
struct GatherRun {
    player: Player,
    probe: Rc<BarProbe>,
    gen: u64,
    on_done: Rc<dyn Fn()>,
}

/// Picture bounds united across a gather's samples, in VO-image space.
#[derive(Clone, Copy, Default)]
struct Gather {
    sizes: Option<ChainSizes>,
    content: Option<CropRect>,
}

impl Gather {
    /// Fold one sample in; a changed chain (new sizes) restarts the union.
    fn absorb(&mut self, sizes: ChainSizes, content: Option<CropRect>) {
        if self.sizes != Some(sizes) {
            *self = Self {
                sizes: Some(sizes),
                content: None,
            };
        }
        self.content = match (self.content, content) {
            (Some(a), Some(b)) => Some(union_rect(a, b)),
            (a, b) => a.or(b),
        };
    }
}

fn gather_step(run: Rc<GatherRun>, mut gather: Gather, left: u8) {
    if run.probe.gen.get() != run.gen {
        eprintln!(
            "[rhino] bars: probe gather stale: gen {} vs live {}",
            run.gen,
            run.probe.gen.get()
        );
        return;
    }
    match sample_frame(&run.player) {
        Some((sizes, content)) => gather.absorb(sizes, content),
        None => eprintln!("[rhino] bars: frame sample unavailable"),
    }
    if left > 1 {
        glib::timeout_add_local_once(SAMPLE_GAP, move || gather_step(run, gather, left - 1));
        return;
    }
    finish_gather(&run, gather);
}

/// Screenshot the displayed frame; picture bounds mapped into the full VO image
/// (`None` content for an all-dark frame).
fn sample_frame(player: &Player) -> Option<(ChainSizes, Option<CropRect>)> {
    let g = player.borrow();
    let mpv = &g.as_ref()?.mpv;
    let sizes = chain_sizes(mpv)?;
    let shown = shown_vo_region(mpv, sizes.vo);
    let (shot, content) = crate::media_probe::with_raw_frame(mpv, |v| {
        (
            (v.w as i64, v.h as i64),
            content_bounds(v.w, v.h, v.row_stride, v.bpp, v.fmt, v.data),
        )
    })?;
    Some((sizes, content.map(|c| shot_to_vo(c, shot, shown))))
}

/// Screenshots honor an active crop: map a screenshot rect into the shown VO region.
fn shot_to_vo(c: CropRect, shot: (i64, i64), shown: CropRect) -> CropRect {
    let r = scale_rect_between(c, shot, (shown.w, shown.h));
    CropRect {
        x: r.x + shown.x,
        y: r.y + shown.y,
        ..r
    }
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

fn video_ready(mpv: &Mpv) -> bool {
    mpv.get_property::<i64>("width").unwrap_or(0) > 0
        && mpv.get_property::<i64>("height").unwrap_or(0) > 0
}

pub fn apply_video_crop(mpv: &Mpv, rect: Option<CropRect>) {
    if let Err(e) = mpv.set_property(
        "video-crop",
        rect.map(|r| crop_rect_in_vo_space(mpv, r).as_video_crop())
            .unwrap_or_default()
            .as_str(),
    ) {
        eprintln!("[rhino] bars: video-crop set failed: {e}");
    }
}

pub fn clear_video_crop(mpv: &Mpv) {
    apply_video_crop(mpv, None);
}

/// Strip reading classification on a `(fw, fh)` chain-output image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReadingVerdict {
    /// Meaningful strip crop in VO space (caller maps it to decode space).
    Crop(CropRect),
    /// Full-frame result: genuinely no strips — the only probed-clean outcome.
    Clean,
    /// Implausible reading: outside the measured frame, or a sub-region lock-on
    /// from near-black content (dark openings, small logos). Never cached — the
    /// caller retries bounded and re-arms on later events.
    Garbage,
}

fn classify_reading(rect: CropRect, frame: (i64, i64)) -> ReadingVerdict {
    let (fw, fh) = frame;
    if !rect_in_frame(fw, fh, rect) {
        return ReadingVerdict::Garbage;
    }
    let rect = centered_rect(rect, frame);
    // Edges thinner than MIN_BAR_FRAC (scan borders, soft rows) are the full frame.
    let thin = |full: i64, kept: i64| ((full - kept) as f64) < full as f64 * MIN_BAR_FRAC;
    if thin(fw, rect.w) && thin(fh, rect.h) {
        return ReadingVerdict::Clean;
    }
    if !crop_meaningful(fw, fh, rect.w, rect.h) {
        return ReadingVerdict::Garbage;
    }
    ReadingVerdict::Crop(rect)
}

#[cfg(test)]
mod probe_tests {
    use super::*;

    fn rect(w: i64, h: i64, x: i64, y: i64) -> CropRect {
        CropRect { w, h, x, y }
    }

    #[test]
    fn crop_rect_formats_mpv_video_crop() {
        assert_eq!(rect(1920, 800, 0, 140).as_video_crop(), "1920x800+0+140");
    }

    #[test]
    fn classify_full_vo_frame_under_downscale_is_clean() {
        // A clean file probed while Smooth60 downscales: the reading covers the
        // full chain-output image — genuinely no strips, probed clean.
        assert_eq!(classify_reading(rect(1552, 872, 0, 0), (1552, 872)), ReadingVerdict::Clean);
    }

    #[test]
    fn classify_keeps_real_strips_measured_in_vo_space() {
        let r = rect(1552, 720, 0, 76);
        assert_eq!(classify_reading(r, (1552, 872)), ReadingVerdict::Crop(r));
    }

    #[test]
    fn classify_trims_only_the_thinner_margin() {
        // Night scene: right half dark, no real strips — the full frame.
        assert_eq!(classify_reading(rect(818, 607, 0, 0), (1600, 680)), ReadingVerdict::Clean);
        // Letterbox with a dark lower third: only the top strip's width is cropped.
        assert_eq!(
            classify_reading(rect(1920, 700, 0, 140), (1920, 1080)),
            ReadingVerdict::Crop(rect(1920, 800, 0, 140))
        );
    }

    #[test]
    fn classify_rejects_readings_outside_the_measured_frame() {
        assert_eq!(
            classify_reading(rect(1553, 800, 0, 40), (1552, 872)),
            ReadingVerdict::Garbage
        );
    }

    #[test]
    fn classify_treats_thin_edges_as_clean() {
        // Soft first row, and a film scan with 8 px dark side borders: full frame.
        assert_eq!(classify_reading(rect(852, 479, 0, 1), (853, 480)), ReadingVerdict::Clean);
        assert_eq!(classify_reading(rect(1904, 1080, 8, 0), (1920, 1080)), ReadingVerdict::Clean);
    }

    #[test]
    fn classify_rejects_subregion_lockon_from_dark_openings() {
        // Near-black opening frames: only a small bright logo reads as picture.
        // In-frame but implausible — never "clean".
        assert_eq!(
            classify_reading(rect(188, 172, 858, 588), (1920, 1080)),
            ReadingVerdict::Garbage
        );
    }

    #[test]
    fn gather_unites_samples_and_restarts_on_chain_change() {
        let a = ChainSizes { decode: (1920, 1080), vo: (1920, 1080) };
        let b = ChainSizes { decode: (1920, 1080), vo: (1552, 872) };
        let mut g = Gather::default();
        g.absorb(a, Some(rect(1800, 800, 60, 140)));
        g.absorb(a, None);
        g.absorb(a, Some(rect(1920, 790, 0, 145)));
        assert_eq!(g.content, Some(rect(1920, 800, 0, 140)));
        g.absorb(b, Some(rect(1552, 650, 0, 111)));
        assert_eq!((g.sizes, g.content), (Some(b), Some(rect(1552, 650, 0, 111))));
    }
}
