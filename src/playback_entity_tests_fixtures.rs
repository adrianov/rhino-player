/// Fresh DVD-folder fixture: `<base>/VIDEO_TS/VIDEO_TS.IFO` plus the named VOBs.
fn pe_dvd_dir(tag: &str, vobs: &[&str]) -> (PathBuf, PathBuf) {
    let base = std::env::temp_dir().join(format!("rhino-pe-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    let vts = base.join("VIDEO_TS");
    fs::create_dir_all(&vts).expect("mkdir");
    fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
    for n in vobs {
        fs::write(vts.join(n), b"v").expect("write");
    }
    (base, vts)
}

fn pe_remove(base: &Path) {
    let _ = fs::remove_dir_all(base);
}

fn pe_key(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Card fixture: title set with a `VTS_02_0.IFO` index and two 1000/2000-byte VOBs.
fn card_fixture() -> (PathBuf, PathBuf, PathBuf) {
    let (base, vts) = pe_dvd_dir("card", &[]);
    fs::write(vts.join("VTS_02_0.IFO"), b"IFO").expect("ifo");
    for (n, size) in [("VTS_02_1.VOB", 1000), ("VTS_02_2.VOB", 2000)] {
        fs::write(vts.join(n), vec![0u8; size]).expect("write");
    }
    (base, vts.join("VTS_02_1.VOB"), vts.join("VTS_02_2.VOB"))
}

fn chapter_maps(p1: &Path, p2: &Path) -> (HashMap<String, f64>, HashMap<String, f64>) {
    let mut durs = HashMap::new();
    let mut tpos = HashMap::new();
    durs.insert(pe_key(p1), 100.0);
    durs.insert(pe_key(p2), 100.0);
    tpos.insert(pe_key(p2), 50.0);
    (durs, tpos)
}

/// Duration map seeding the entity row (150s) above chapter rows (100/50s).
fn global_dur_map(entity: &PlaybackEntity, p1: &Path, p2: &Path) -> HashMap<String, f64> {
    let mut durs = HashMap::new();
    durs.insert(pe_key(&entity.db_path()), 150.0);
    durs.insert(pe_key(p1), 100.0);
    durs.insert(pe_key(p2), 50.0);
    durs
}

/// Stale-entity fixture: four `x`-filled VOBs sized `[100, 200, 300, 400]`; returns `(base, first vob)`.
fn sized_vobs_fixture(tag: &str) -> (PathBuf, PathBuf) {
    let (base, vts) = pe_dvd_dir(tag, &[]);
    for (i, n) in [100usize, 200, 300, 400].into_iter().enumerate() {
        fs::write(vts.join(format!("VTS_02_{}.VOB", i + 1)), vec![b'x'; n]).expect("vob");
    }
    (base, vts.join("VTS_02_1.VOB"))
}
