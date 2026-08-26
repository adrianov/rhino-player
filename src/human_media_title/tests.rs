use super::*;

#[test]
fn bdremux_stripped() {
    assert_eq!(human_media_title("Movie Name BDRemux.mkv"), "Movie Name");
    assert_eq!(human_media_title("Some.Film.BDRemux.mkv"), "Some Film");
    assert_eq!(
        human_media_title("Women in Love Criterion Collection-BDRemux.mkv"),
        "Women in Love Criterion Collection"
    );
}

#[test]
fn americans_sample() {
    assert_eq!(
        human_media_title("The.Americans.S04E04.1080p.WEB-DL.4xRus.Eng.TeamHD.mkv"),
        "The Americans — Season 4, Episode 4"
    );
}

#[test]
fn glued_source_and_resolution() {
    assert_eq!(
        human_media_title("Legion.S01E01.WEB-DL1080p.Rus.Eng.DV.LostFilm.mkv"),
        "Legion — Season 1, Episode 1"
    );
    assert_eq!(
        human_media_title("Show.S02E03.WEBDL720p.mkv"),
        "Show — Season 2, Episode 3"
    );
    assert_eq!(human_media_title("Foo.Bar.WEB-DL4K.mkv"), "Foo Bar");
    assert_eq!(human_media_title("Foo.WEB-DLUHD.mkv"), "Foo");
    assert_eq!(
        human_media_title("Something.WEBDLRip720p.Group.mkv"),
        "Something Group"
    );
    assert_eq!(human_media_title("Movie.HDTV1080p.mkv"), "Movie");
    assert_eq!(human_media_title("Movie HDTV1080p.mkv"), "Movie");
}

#[test]
fn clean_name_unchanged() {
    assert_eq!(human_media_title("My Home Video.mp4"), "My Home Video");
}

#[test]
fn season_only_dot_separated() {
    let t = human_media_title("Some.Show.S02.720p.HDTV.x264-GROUP.mkv");
    assert!(t.contains("Season 2"));
    assert!(t.to_lowercase().contains("some show"));
    assert!(!t.to_lowercase().contains("720p"));
}

#[test]
fn alternate_nx_episode() {
    assert_eq!(
        human_media_title("Ponies.3x05.Episode.Title.1080p.mkv"),
        "Ponies Episode Title — Season 3, Episode 5"
    );
}

#[test]
fn empty_returns_empty() {
    assert_eq!(human_media_title(""), "");
    assert_eq!(human_media_title("   "), "");
}

#[test]
fn dvd_folder_basename_keeps_label() {
    let t = human_media_title("17_Mgnoveniy_DVD2");
    assert!(
        !t.trim().is_empty(),
        "DVD rip folder names must not humanize to empty: {t:?}"
    );
}

#[test]
fn keeps_release_year() {
    assert_eq!(
        human_media_title("Some.Film.2013.1080p.BluRay.x264.mkv"),
        "Some Film 2013"
    );
    assert_eq!(human_media_title("Movie (2013).mkv"), "Movie (2013)");
}

#[test]
fn in_progress_download_title_drops_temp_wrappers() {
    assert_eq!(
        human_media_title(
            "Связь (Coherence, 2013, 1080p).mkv.RSRXEZ4AWN67MGBANBT6YLR32JW32GVZSZLYN2Y.dctmp"
        ),
        "Связь (Coherence, 2013)"
    );
    assert_eq!(
        human_media_title(
            "Связь (Coherence, 2013, 1080p).mkv.RSRXEZ4AWN67MGBANBT6YLR32JW32GVZSZLYN2Y.DCTMP"
        ),
        "Связь (Coherence, 2013)"
    );
    assert_eq!(human_media_title("clip.mkv.dctmp"), "clip");
    assert_eq!(human_media_title("фильм.webm"), "фильм");
}
