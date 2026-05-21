include!("chrome_fs_transition_gate.rs");
#[cfg(target_os = "macos")]
include!("chrome_macos_traffic_lights.rs");
include!("chrome_macos_unfullscreen_defer.rs");
#[cfg(target_os = "macos")]
include!("chrome_macos_toggle.rs");
#[cfg(target_os = "macos")]
include!("chrome_macos_header_popovers.rs");
include!("chrome_header_menubtns.rs");

/// Refs shared by menu / gesture fullscreen toggles (one bundle keeps wiring arity small).
struct FullscreenToggleRefs {
    fs_restore: Rc<RefCell<Option<(i32, i32)>>>,
    last_unmax: Rc<RefCell<(i32, i32)>>,
    skip_max_to_fs: Rc<Cell<bool>>,
    fs_transition_busy: Rc<Cell<bool>>,
}

#[cfg(not(target_os = "macos"))]
fn unfullscreen_safe_inner(win: &adw::ApplicationWindow) {
    win.unfullscreen();
}

/// Linux demaximize path: synchronous [`GtkWindowExt::unfullscreen`] after [`fs_transition_try_begin`].
#[cfg(not(target_os = "macos"))]
fn unfullscreen_safe(win: &adw::ApplicationWindow, fs_busy: &Cell<bool>) {
    if !fs_transition_try_begin(fs_busy) {
        return;
    }
    unfullscreen_safe_inner(win);
}

#[cfg(target_os = "macos")]
fn toggle_fullscreen(
    win: &adw::ApplicationWindow,
    fs_restore: &RefCell<Option<(i32, i32)>>,
    last_unmax: &RefCell<(i32, i32)>,
    skip_max_to_fs: &Cell<bool>,
    _fs_busy: &Cell<bool>,
) {
    macos_toggle_fullscreen(win, fs_restore, last_unmax, skip_max_to_fs);
}

#[cfg(not(target_os = "macos"))]
fn toggle_fullscreen(
    win: &adw::ApplicationWindow,
    fs_restore: &RefCell<Option<(i32, i32)>>,
    last_unmax: &RefCell<(i32, i32)>,
    skip_max_to_fs: &Cell<bool>,
    fs_busy: &Cell<bool>,
) {
    if !fs_transition_try_begin(fs_busy) {
        return;
    }
    if win.is_fullscreen() {
        skip_max_to_fs.set(true);
        unfullscreen_safe_inner(win);
    } else if !win.is_maximized() {
        *fs_restore.borrow_mut() = Some(win_normal_size(win));
        win.maximize();
    } else {
        if fs_restore.borrow().is_none() {
            *fs_restore.borrow_mut() = Some(*last_unmax.borrow());
        }
        win.fullscreen();
    }
}

include!("chrome_header_csd_controls.rs");

/// Bundle of refs for [`apply_chrome`].
struct ChromeApplyParts<'a, R>
where
    R: IsA<gtk::Widget>,
{
    hdr_csd_baseline: &'a Rc<Cell<Option<(bool, bool)>>>,
    root: &'a adw::ToolbarView,
    header: &'a adw::HeaderBar,
    gl: &'a gtk::GLArea,
    bar_show: &'a Rc<Cell<bool>>,
    recent: &'a R,
    bottom: &'a gtk::Box,
    player: &'a Rc<RefCell<Option<MpvBundle>>>,
}

/// Updates `ToolbarView` bar reveals, client-side decoration title slots, subtitles vs chrome, GL paint.
///
/// When the recent grid is visible, always reveal bars. When playing, visibility follows `bar_show`
/// (pointer motion clears [IDLE_3S]). Open header menus cancel auto-hide timer.
fn apply_chrome<R: IsA<gtk::Widget>>(c: ChromeApplyParts<'_, R>) {
    c.root.set_extend_content_to_top_edge(true);
    c.root.set_extend_content_to_bottom_edge(true);
    #[cfg(not(target_os = "macos"))]
    let show = c.recent.is_visible() || c.bar_show.get();
    #[cfg(target_os = "macos")]
    let mut show = c.recent.is_visible() || c.bar_show.get();
    #[cfg(target_os = "macos")]
    if crate::macos_fs_exit::exit_armed() {
        show = true;
    }
    let reveal_changed = set_toolbar_reveal(c.root, show);
    sync_header_window_controls(c.header, c.hdr_csd_baseline, show, c.root);
    if !reveal_changed {
        return;
    }
    c.gl.queue_render();
    if let Some(b) = c.player.borrow().as_ref() {
        sub_prefs::apply_sub_pos_for_toolbar(&b.mpv, show, c.bottom.height(), c.gl.height());
    }
}

include!("chrome_pointer_after_bars.rs");

fn replace_timeout(s: Rc<RefCell<Option<glib::SourceId>>>, f: impl Fn() + 'static) {
    drop_glib_source(s.as_ref());
    *s.borrow_mut() = Some(glib::timeout_add_local(
        IDLE_3S,
        glib::clone!(
            #[strong]
            s,
            move || {
                *s.borrow_mut() = None;
                f();
                glib::ControlFlow::Break
            }
        ),
    ));
}

fn schedule_bars_autohide(ctx: Rc<ChromeBarHide>) {
    replace_timeout(Rc::clone(&ctx.nav), {
        let ctx2 = Rc::clone(&ctx);
        move || {
            if ctx2.vol.is_active()
                || ctx2.sub.is_active()
                || ctx2.speed.is_active()
                || ctx2.main.is_active()
                || ctx2.seek_grabbed.get()
            {
                schedule_bars_autohide(Rc::clone(&ctx2));
            } else {
                ctx2.bar_show.set(false);
                apply_chrome(ChromeApplyParts {
                    hdr_csd_baseline: &ctx2.hdr_csd_baseline,
                    root: &ctx2.root,
                    header: &ctx2.header,
                    gl: &ctx2.gl,
                    bar_show: &ctx2.bar_show,
                    recent: &ctx2.recent,
                    bottom: &ctx2.bottom,
                    player: &ctx2.player,
                });
                ctx2.squelch.set(Some(Instant::now() + LAYOUT_SQUELCH));
                hide_cursor_after_bars_hide(&ctx2.win, &ctx2.gl, &ctx2.recent, &ctx2.player);
            }
        }
    });
}

/// Clicks to another header [gtk::MenuButton] are blocked while a **modal** popover is open.
/// [gtk::Popover:modal] on GTK 4.14+ — set to false so the rest of the window (including
/// the other header buttons) stays clickable; [gtk::Popover:autohide] still dismisses on outside press.
fn header_popover_non_modal(pop: &impl IsA<gtk::Popover>) {
    use glib::prelude::Cast;
    let pop = pop.upcast_ref::<gtk::Popover>();
    if pop.find_property("modal").is_none() {
        return;
    }
    pop.set_property("modal", false);
}

/// Display (or stream) size in pixels from mpv, if known.
fn video_display_dims(mpv: &Mpv) -> Option<(i64, i64)> {
    let pair = |mw: &Mpv, wk: &str, hk: &str| {
        let w = mw.get_property::<i64>(wk).ok()?;
        let h = mw.get_property::<i64>(hk).ok()?;
        (w > 0 && h > 0).then_some((w, h))
    };
    pair(mpv, "dwidth", "dheight").or_else(|| pair(mpv, "width", "height"))
}

fn window_size_for_horizontal_video(vw: i64, vh: i64) -> (i32, i32) {
    let wf = vw as f64;
    let hf = vh as f64;
    let mut nw = FIT_H_VIDEO_W;
    let mut nh = (FIT_H_VIDEO_W as f64 * hf / wf).round() as i32;
    if nh > FIT_H_VIDEO_MAX_H {
        nh = FIT_H_VIDEO_MAX_H;
        nw = (FIT_H_VIDEO_MAX_H as f64 * wf / hf).round() as i32;
    }
    nw = nw.clamp(320, 4096);
    nh = nh.clamp(200, 4096);
    (nw, nh)
}

/// Resize the window to match a **landscape** video aspect (wider than tall). No-op in fullscreen, when maximized, for portrait or square, or if dimensions are unknown.
fn schedule_window_fit_h_video(
    player: Rc<RefCell<Option<MpvBundle>>>,
    win: adw::ApplicationWindow,
) {
    let w = win.clone();
    let _ = glib::timeout_add_local(
        Duration::from_millis(u64::from(FIT_WINDOW_DELAY_MS)),
        move || {
            if w.is_fullscreen() || w.is_maximized() {
                return glib::ControlFlow::Break;
            }
            let b = match player.try_borrow() {
                Ok(b) => b,
                Err(_) => return glib::ControlFlow::Break,
            };
            let Some(pl) = b.as_ref() else {
                return glib::ControlFlow::Break;
            };
            let Some((px, py)) = video_display_dims(&pl.mpv) else {
                return glib::ControlFlow::Break;
            };
            if px <= py {
                return glib::ControlFlow::Break;
            }
            let (nw, nh) = window_size_for_horizontal_video(px, py);
            w.set_default_size(nw, nh);
            glib::ControlFlow::Break
        },
    );
}

fn schedule_or_defer_recent_backfill(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    pending: &Rc<RefCell<Option<RecentBackfillJob>>>,
    ctx: Rc<RecentContext>,
    paths: Vec<PathBuf>,
) {
    if player.borrow().is_some() {
        recent_view::schedule_thumb_backfill(ctx, paths);
    } else {
        *pending.borrow_mut() = Some((ctx, paths));
    }
}

fn drain_recent_backfill(pending: &Rc<RefCell<Option<RecentBackfillJob>>>) {
    if let Some((ctx, paths)) = pending.borrow_mut().take() {
        recent_view::schedule_thumb_backfill(ctx, paths);
    }
}

fn schedule_sub_button_scan(player: Rc<RefCell<Option<MpvBundle>>>, button: gtk::MenuButton) {
    button.set_visible(false);
    let tries = Rc::new(Cell::new(0u8));
    let _ = glib::timeout_add_local(Duration::from_millis(SUB_SCAN_MS), move || {
        let has_subs = player
            .borrow()
            .as_ref()
            .is_some_and(|b| sub_tracks::has_subtitle_tracks(&b.mpv));
        button.set_visible(has_subs);
        if has_subs {
            return glib::ControlFlow::Break;
        }
        let next = tries.get().saturating_add(1);
        tries.set(next);
        if next >= SUB_SCAN_TICKS {
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });
}
