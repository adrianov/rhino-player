//! Tick-callback state machine that mirrors frame + visibility onto the layer every
//! frame while skipping no-op ticks via cheap/full change keys.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use gtk::prelude::{Cast, WidgetExt, WidgetExtManual};
use objc2::rc::Retained;

use super::super::macos_video_displaylink::DriverStateHandle;
use super::super::macos_video_layer::RhinoMpvGlLayer;
use super::resync_wiring::resync_now;
use super::{nswindow_content_height_for, translate_to_window, OverlayCell};

const POS_PROBE_INTERVAL: u32 = 8;

type CheapKey = (i32, i32, i64, bool, bool);
type FullKey = (i32, i32, i64, i64, i64, bool, bool);

/// Tick-callback state for the per-frame position probe (see [`add_ticker`]).
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
            last: Cell::new((0, 0, i64::MIN, i64::MIN, i64::MIN, false, false)),
            last_cheap: Cell::new((0, 0, i64::MIN, false, false)),
            tick_n: Cell::new(0),
        }
    }

    /// NSWindow content height quantized to 1/4096 pt, with the GTK window height as fallback.
    fn height_snap(w: &gtk::Widget, window: &gtk::Window) -> i64 {
        nswindow_content_height_for(w)
            .map(|h| (h * 4096.0).round() as i64)
            .unwrap_or((window.height() as i64).saturating_mul(4096))
    }

    fn position_key(w: &gtk::Widget, window: &gtk::Window) -> (i64, i64) {
        translate_to_window(w, window)
            .map(|(x, y)| ((x * 4096.0).round() as i64, (y * 4096.0).round() as i64))
            .unwrap_or((i64::MIN, i64::MIN))
    }

    fn on_tick(&self, w: &gtk::Widget) -> glib::ControlFlow {
        let Some(window) = w.root().and_then(|r| r.downcast::<gtk::Window>().ok()) else {
            return glib::ControlFlow::Continue;
        };
        let ov = self.overlay.borrow().clone();
        let cheap_key = self.cheap_key(w, &window, &ov);
        if !self.probe_due(cheap_key) {
            return glib::ControlFlow::Continue;
        }
        self.sync_if_changed(w, &window, cheap_key)
    }

    fn cheap_key(
        &self,
        w: &gtk::Widget,
        window: &gtk::Window,
        ov: &Option<gtk::Widget>,
    ) -> CheapKey {
        (
            w.width(),
            w.height(),
            Self::height_snap(w, window),
            w.is_visible(),
            ov.as_ref().is_some_and(|v| v.is_visible()),
        )
    }

    /// Cheap short-circuit: only probe the (costlier) window position every N ticks
    /// or when something cheap changed. `last_cheap` is only committed on a probe.
    fn probe_due(&self, cheap_key: CheapKey) -> bool {
        let cheap_changed = cheap_key != self.last_cheap.get();
        let n = self.tick_n.get().wrapping_add(1);
        self.tick_n.set(n);
        cheap_changed || n.wrapping_rem(POS_PROBE_INTERVAL) == 0
    }

    fn sync_if_changed(
        &self,
        w: &gtk::Widget,
        window: &gtk::Window,
        cheap_key: CheapKey,
    ) -> glib::ControlFlow {
        let pos = Self::position_key(w, window);
        let key = (
            cheap_key.0,
            cheap_key.1,
            cheap_key.2,
            pos.0,
            pos.1,
            cheap_key.3,
            cheap_key.4,
        );
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

/// Mirror the sizer onto the layer every frame; the tick callback skips frames whose
/// size/visibility/position keys are unchanged so idle playback costs nothing.
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
