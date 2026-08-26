/// Path of a chapter `.vob` inside a disc directory.
fn dvd_vob(disc: &Path, vob_name: &str) -> PathBuf {
    disc.join("VIDEO_TS").join(vob_name)
}

fn write_min_dvd(disc: &Path, vob_name: &str) {
    let vts = disc.join("VIDEO_TS");
    ensure_dir(&vts);
    fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").unwrap();
    fs::write(vts.join(vob_name), b"v").unwrap();
}

/// Two minimal sibling disc dirs: DVD1 with two chapters, DVD2 with one.
fn seed_dvd_pair(island: &Path) -> (PathBuf, PathBuf) {
    let d1 = island.join("DVD1");
    let d2 = island.join("DVD2");
    write_min_dvd(&d1, "VTS_02_1.VOB");
    write_min_dvd(&d1, "VTS_02_2.VOB");
    write_min_dvd(&d2, "VTS_02_1.VOB");
    (d1, d2)
}

#[test]
fn dvd_advances_to_sibling_disc_dir_not_next_vob() {
    let island = scratch_island("dvd_sib", ScratchTmpOrder::First);
    let (d1, d2) = seed_dvd_pair(&island);
    let ch1 = dvd_vob(&d1, "VTS_02_1.VOB");
    let ch2 = dvd_vob(&d2, "VTS_02_1.VOB");
    assert_same_path(&next_after_eof(&ch1).unwrap(), &ch2);
    assert_same_path(
        &next_after_eof(&dvd_vob(&d1, "VTS_02_2.VOB")).unwrap(),
        &ch2,
    );
    assert_same_path(&prev_before_current(&ch2).unwrap(), &ch1);
    cleanup(&island);
}
