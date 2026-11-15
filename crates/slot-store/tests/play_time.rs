use slot_store::{read_play_times, write_play_time, PLAY_TIME_FILE};
use tempfile::tempdir;

#[test]
fn a_card_with_no_file_has_played_nothing() {
    let d = tempdir().unwrap();
    assert!(read_play_times(d.path()).is_empty());
}

#[test]
fn written_times_read_back_per_cart() {
    let d = tempdir().unwrap();
    write_play_time(d.path(), "Pokemon - Emerald Version (USA, Europe)", 3600).unwrap();
    write_play_time(d.path(), "Metroid Fusion", 42).unwrap();
    write_play_time(d.path(), "Metroid Fusion", 90).unwrap();
    let times = read_play_times(d.path());
    assert_eq!(
        times.get("Pokemon - Emerald Version (USA, Europe)"),
        Some(&3600)
    );
    assert_eq!(times.get("Metroid Fusion"), Some(&90));
    assert_eq!(times.len(), 2);
}

#[test]
fn a_hand_edited_line_that_is_not_a_number_is_skipped() {
    let d = tempdir().unwrap();
    std::fs::create_dir(d.path().join("Config")).unwrap();
    std::fs::write(
        d.path().join(PLAY_TIME_FILE),
        "Emerald = lots\nFusion = 120\n",
    )
    .unwrap();
    let times = read_play_times(d.path());
    assert_eq!(times.get("Emerald"), None);
    assert_eq!(times.get("Fusion"), Some(&120));
}
