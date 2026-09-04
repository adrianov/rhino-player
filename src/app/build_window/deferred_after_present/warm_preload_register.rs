/// Register warm-preload callbacks; kick the startup preload when launching without a file.
fn register_warm_preload_step(args: WindowAfterPresentArgs) {
    // Same as continue-strip launch (`file_boot` none); do not use `recent_visible.get()`
    // here — it may still be false before the window is mapped.
    let want_warm_preload = args.file_boot.borrow().is_none() && args.last_path.borrow().is_none();
    let Some(ctx) = args.warm_preload else {
        return;
    };
    register_warm_preload_ctx(Rc::clone(&ctx));
    register_warm_preload_loaded_slot(&ctx);
    if want_warm_preload {
        let ctx = Rc::clone(&ctx);
        let _ = glib::idle_add_local_once(move || run_continue_warm_preload(&ctx, false));
    }
}

/// Loaded-hook: completes the warm-preload gate into a real path run.
fn register_warm_preload_loaded_slot(ctx: &Rc<WarmPreloadCtx>) {
    let done_ctx = Rc::clone(ctx);
    register_warm_preload_loaded(Rc::new(move || {
        let run = Rc::clone(&done_ctx);
        Rc::clone(&done_ctx.gate).complete(move |p| WarmPreloadCtx::run_path(&run, p));
    }));
}
