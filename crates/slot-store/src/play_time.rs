use std::collections::HashMap;
use std::path::Path;

pub const PLAY_TIME_FILE: &str = "Config/play_time.txt";

pub fn read_play_times(root: &Path) -> HashMap<String, u64> {
    crate::ini::read(root, PLAY_TIME_FILE)
        .into_iter()
        .filter_map(|(stem, secs)| secs.parse().ok().map(|secs| (stem, secs)))
        .collect()
}

pub fn write_play_time(root: &Path, stem: &str, secs: u64) -> std::io::Result<()> {
    crate::ini::write(root, PLAY_TIME_FILE, stem, &secs.to_string())
}
