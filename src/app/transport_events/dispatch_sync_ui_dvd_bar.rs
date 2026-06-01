// DVD chain-bar cache, seek-chapter labels, and window-title sync for `FileLoaded` dispatch.
// Split out of `dispatch_sync_ui_file_loaded.rs`; included in the same module scope.

fn sync_seek_chapters(ctx: &Rc<TransportCtx>) {
    let mut list = Vec::new();
    if let Some(bar) = ctx.dvd_bar.borrow().as_ref() {
        list = bar.chapter_preview_labels();
    } else if let Ok(g) = ctx.player.try_borrow() {
        if let Some(b) = g.as_ref() {
            list = crate::chapter_list::mpv_chapter_list(&b.mpv);
        }
    }
    *ctx.seek_chapters.borrow_mut() = list;
}

fn dvd_bar_duration(ctx: &TransportCtx) -> Option<f64> {
    let chapter = transport_chapter_path_for_ctx(ctx)?;
    let bar = ctx.dvd_bar.borrow();
    let bar = bar.as_ref()?;
    crate::playback_entity::PlaybackEntity::resolve(&chapter)
        .transport_duration_from_bar(&chapter, bar)
}

fn transport_chapter_path_for_ctx(ctx: &TransportCtx) -> Option<std::path::PathBuf> {
    if ctx.recent_visible.get() {
        return ctx.eof.last_path.borrow().clone();
    }
    let g = ctx.player.try_borrow().ok()?;
    let b = g.as_ref()?;
    let shell = b.me_budget_shell_path.borrow().clone();
    crate::playback_entity::transport_chapter_path(false, None, Some(&b.mpv), shell.as_deref())
}

/// Refresh window / header title when mpv `path` changes (sibling DVD advance, etc.).
fn sync_window_title_from_context(ctx: &Rc<TransportCtx>) {
    if ctx.recent_visible.get() {
        return;
    }
    let path = ctx
        .player
        .borrow()
        .as_ref()
        .and_then(|b| {
            crate::media_probe::shell_media_path(
                &b.mpv,
                b.me_budget_shell_path.borrow().as_deref(),
            )
        })
        .or_else(|| ctx.eof.last_path.borrow().clone());
    let Some(path) = path else {
        return;
    };
    let ttl = crate::playback_entity::window_title_for(&path);
    sync_app_window_title(
        &ctx.eof.win,
        ctx.eof.hdr_title_mirror.as_deref(),
        Some(&ttl),
    );
}

fn refresh_dvd_bar_cache(ctx: &Rc<TransportCtx>) {
    let Ok(g) = ctx.player.try_borrow() else {
        return;
    };
    let Some(b) = g.as_ref() else {
        *ctx.dvd_bar.borrow_mut() = None;
        return;
    };
    let shell = b.me_budget_shell_path.borrow();
    crate::dvd_vob_timeline::refresh_dvd_bar(&ctx.dvd_bar, &b.mpv, shell.as_deref());
    sync_seek_chapters(ctx);
}

fn maybe_refresh_dvd_bar_cache(ctx: &Rc<TransportCtx>) {
    let Ok(g) = ctx.player.try_borrow() else {
        return;
    };
    let Some(b) = g.as_ref() else {
        return;
    };
    let shell = b.me_budget_shell_path.borrow();
    crate::dvd_vob_timeline::maybe_refresh_dvd_bar(&ctx.dvd_bar, &b.mpv, shell.as_deref());
    sync_seek_chapters(ctx);
}
