// Chained-title head VOB duration from sibling byte rates (included from `session.rs`).

/// Bytes-per-second of every sibling part after the chain head.
fn sibling_bytes_per_sec(m: &mut Mpv, chapters: &[std::path::PathBuf]) -> Vec<f64> {
    chapters
        .iter()
        .skip(1)
        .filter_map(|sib| {
            let dur = probe_with_session(m, sib)?;
            if !valid_duration(dur) {
                return None;
            }
            let bytes = sib.metadata().ok()?.len();
            (bytes > 0).then_some(bytes as f64 / dur)
        })
        .collect()
}

/// Middle element of the collected rates; `None` when nothing probed successfully.
fn median_rate(mut rates: Vec<f64>) -> Option<f64> {
    if rates.is_empty() {
        return None;
    }
    rates.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Some(rates[rates.len() / 2])
}

/// First `.vob` in a chained title reports the whole program; derive length from siblings.
fn chain_head_duration(m: &mut Mpv, path: &Path) -> Option<f64> {
    if !is_title_chain_head(path) {
        return None;
    }
    let chapters = crate::dvd_entity::title_chapter_paths(path)?;
    let head_bytes = path.metadata().ok()?.len();
    if head_bytes == 0 {
        return None;
    }
    let rate = median_rate(sibling_bytes_per_sec(m, &chapters))?;
    let est = head_bytes as f64 / rate;
    valid_duration(est).then_some(est)
}
