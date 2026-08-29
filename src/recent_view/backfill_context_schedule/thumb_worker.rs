/// Worker-thread body for [schedule_thumb_backfill]: exits as soon as a newer generation
/// starts or [RecentContext::cancel] clears.
fn run_thumb_worker(
    paths: Vec<std::path::PathBuf>,
    gen: u64,
    c: Arc<AtomicBool>,
    tx: mpsc::Sender<()>,
    gen_watch: Arc<std::sync::atomic::AtomicU64>,
) {
    for p in paths {
        if thumb_gen_cancelled(&gen_watch, gen, &c) {
            return;
        }
        if !p.exists() {
            continue;
        }
        let can = match std::fs::canonicalize(&p) {
            Ok(c) => c,
            _ => continue,
        };
        if media_probe::thumb_backfill_satisfied(&can) {
            continue;
        }
        let _ = media_probe::ensure_thumbnail(&can);
        if thumb_gen_cancelled(&gen_watch, gen, &c) {
            return;
        }
        if tx.send(()).is_err() {
            return;
        }
    }
}
