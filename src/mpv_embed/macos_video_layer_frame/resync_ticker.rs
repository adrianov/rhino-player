//! Tick-callback state machine that mirrors frame + visibility onto the layer every
//! frame while skipping no-op ticks via cheap/full change keys.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use gtk::prelude::{WidgetExt, WidgetExtManual};
use objc2::rc::Retained;

use super::super::macos_video_displaylink::DriverStateHandle;
use super::super::macos_video_layer::RhinoMpvGlLayer;
use super::resync_wiring::resync_now;
use super::{content_view_size, OverlayCell};

const SIZE_PROBE_INTERVAL: u32 = 8;

type CheapKey = (i32, i32, bool, bool);
type FullKey = (i64, i64, bool, bool);

/// Tick-callback state for the per-frame contentView size probe (see [`add_ticker`]).
struct ResyncTicker {
    layer: Retained<RhinoMpvGlLayer>,
    overlay: OverlayCell,
    repaint: Arc<DriverStateHandle>,
    last: Cell<FullKey>,
    last_cheap: Cell<CheapKey>,
    tick_n: Cell<u32>,
}

impl ResyncTicker {
    fn new(
        layer: Retained<RhinoMpvGlLayer>,
        overlay: OverlayCell,
        repaint: Arc<DriverStateHandle>,
    ) -> Self {
        Self {
            layer,
            overlay,
            repaint,
            last: Cell::new((i64::MIN, i64::MIN, false, false)),
            last_cheap: Cell::new((0, 0, false, false)),
            tick_n: Cell::new(0),
        }
    }

    /// ContentView size quantized to 1/4096 pt (matches the layer frame source).
    fn content_snap(w: &gtk::Widget) -> (i64, i64) {
        content_view_size(w)
            .map(|(cw, ch)| ((cw * 4096.0).round() as i64, (ch * 4096.0).round() as i64))
            .unwrap_or((i64::MIN, i64::MIN))
    }

    fn on_tick(&self, w: &gtk::Widget) -> glib::ControlFlow {
        let ov = self.overlay.borrow().clone();
        let cheap_key = self.cheap_key(w, &ov);
        if !self.probe_due(cheap_key) {
            return glib::ControlFlow::Continue;
        }
        self.sync_if_changed(w, cheap_key)
    }

    fn cheap_key(&self, w: &gtk::Widget, ov: &Option<gtk::Widget>) -> CheapKey {
        (
            w.width(),
            w.height(),
            w.is_visible(),
            ov.as_ref().is_some_and(|v| v.is_visible()),
        )
    }

    /// Cheap short-circuit: only probe the (costlier) contentView size every N ticks
    /// or when something cheap changed. `last_cheap` is only committed on a probe.
    fn probe_due(&self, cheap_key: CheapKey) -> bool {
        let cheap_changed = cheap_key != self.last_cheap.get();
        let n = self.tick_n.get().wrapping_add(1);
        self.tick_n.set(n);
        cheap_changed || n.wrapping_rem(SIZE_PROBE_INTERVAL) == 0
    }

    fn sync_if_changed(&self, w: &gtk::Widget, cheap_key: CheapKey) -> glib::ControlFlow {
        let (cw, ch) = Self::content_snap(w);
        let key = (cw, ch, cheap_key.2, cheap_key.3);
        if key != self.last.get() {
            resync_now(&self.layer, w, &self.overlay, &self.repaint);
            self.last.set(key);
        }
        self.last_cheap.set(cheap_key);
        glib::ControlFlow::Continue
    }
}

fn install_resync_ticker(sizer_widget: &gtk::Widget, handler: Rc<ResyncTicker>) {
    sizer_widget.add_tick_callback(move |w, _| handler.on_tick(w));
}

/// Mirror the contentView onto the layer every frame; the tick callback skips frames whose
/// size/visibility keys are unchanged so idle playback costs nothing.
pub(super) fn add_ticker(
    sizer_widget: &gtk::Widget,
    layer: Retained<RhinoMpvGlLayer>,
    overlay: OverlayCell,
    repaint: Arc<DriverStateHandle>,
) {
    install_resync_ticker(
        sizer_widget,
        Rc::new(ResyncTicker::new(layer, overlay, repaint)),
    );
}
