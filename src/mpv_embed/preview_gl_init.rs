// Thumbnail-player construction helpers: isolated mpv init options, EGL render context, and
// the GLArea queue-render callback. Included at module level from `preview_gl_set_tracks.rs`.

/// Isolated thumbnail-player core options: no user config, scripts, tracks, or resume state;
/// aggressive decoder skipping + tiny demuxer caches since thumbnails are seek-and-grab.
fn init_preview_options(init: libmpv2::MpvInitializer) -> Result<(), libmpv2::Error> {
    init_preview_player_options(&init)?;
    init_preview_track_options(&init)?;
    init_preview_decode_options(init)?;
    Ok(())
}

/// Core player identity: video-only, silent, no user config or scripts.
fn init_preview_player_options(init: &libmpv2::MpvInitializer) -> Result<(), libmpv2::Error> {
    init.set_option("vo", "libmpv")?;
    init.set_option("ao", "null")?;
    init.set_option("osc", "no")?;
    init.set_option("load-scripts", false)?;
    init.set_option("config", "no")?;
    init.set_option("ytdl", false)?;
    init.set_option("pause", true)?;
    Ok(())
}

/// Track / resume isolation: the thumbnail player shares nothing with user playback.
fn init_preview_track_options(init: &libmpv2::MpvInitializer) -> Result<(), libmpv2::Error> {
    // Thumbnail seeks near EOF must not unload the clip (default EOF → idle/black).
    let _ = init.set_option("keep-open", "always");
    let _ = init.set_option("autoload-files", "no");
    let _ = init.set_option("audio-file-auto", "no");
    let _ = init.set_option("sub-auto", "no");
    let _ = init.set_option("aid", "no");
    let _ = init.set_option("sid", "no");
    let _ = init.set_option("secondary-sid", "no");
    let _ = init.set_option("resume-playback", "no");
    let _ = init.set_option("save-position-on-quit", "no");
    Ok(())
}

/// Decode + demux tuning for seek-and-grab thumbnails, plus quiet/OFF OSD extras.
fn init_preview_decode_options(init: libmpv2::MpvInitializer) -> Result<(), libmpv2::Error> {
    let _ = init.set_option("hwdec", "no");
    let _ = init.set_option("terminal", false);
    let _ = init.set_option("msg-level", "all=no");
    let _ = init.set_option("vd-lavc-threads", 1i64);
    let _ = init.set_option("vd-lavc-fast", true);
    let _ = init.set_option("vd-lavc-skiploopfilter", "all");
    let _ = init.set_option("vd-lavc-skipidct", "nonkey");
    let _ = init.set_option("vd-lavc-skipframe", "nonkey");
    let _ = init.set_option("vd-lavc-software-fallback", 1i64);
    let _ = init.set_option("sws-scaler", "fast-bilinear");
    let _ = init.set_option("demuxer-readahead-secs", 0.0f64);
    let _ = init.set_option("demuxer-max-bytes", "128KiB");
    let _ = init.set_option("hr-seek", false);
    let _ = init.set_option("gpu-dumb-mode", true);
    init_preview_quiet_options(init)
}

/// Keep the thumbnail player invisible: no OSD console, stats overlay, or auto profiles.
fn init_preview_quiet_options(init: libmpv2::MpvInitializer) -> Result<(), libmpv2::Error> {
    let _ = init.set_option("load-osd-console", "no");
    let _ = init.set_option("load-stats-overlay", "no");
    let _ = init.set_option("load-auto-profiles", "no");
    let _ = init.set_option("really-quiet", "yes");
    Ok(())
}

/// Build the mpv render context against the process-loaded EGL library.
fn new_preview_render_context(mpv: &mut Mpv, gl_libs: &GlDynLib) -> Result<RenderContext, String> {
    let egl_state = EglState {
        get: gl_libs.get_proc,
    };

    let params: Vec<RenderParam<EglState>> = vec![
        RenderParam::ApiType(RenderParamApiType::OpenGl),
        RenderParam::InitParams(OpenGLInitParams {
            get_proc_address: egl_proc,
            ctx: egl_state,
        }),
    ];

    RenderContext::new(unsafe { mpv.ctx.as_mut() }, params)
        .map_err(|e| format!("render context: {e:?}"))
}

/// Queue a GLArea redraw whenever the preview player reports a new frame.
fn install_preview_queue_render(render: &mut RenderContext, gl_ptr: usize) {
    let mctx = glib::MainContext::default();
    render.set_update_callback(move || {
        let p = gl_ptr;
        mctx.clone().invoke(move || {
            let gl = unsafe {
                from_glib_borrow::<*mut gtk::ffi::GtkGLArea, gtk::GLArea>(
                    p as *mut gtk::ffi::GtkGLArea,
                )
            };
            gl.queue_render();
        });
    });
}
