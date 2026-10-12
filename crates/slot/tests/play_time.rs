mod common;

use common::{app_playing_in, tmp_root_with_carts};
use slot::app::App;
use slot_input::Action;
use slot_store::{read_play_times, write_play_time};

fn play_for(a: &mut App, secs: u32) {
    for _ in 0..secs * 4 {
        a.update(0.25);
    }
}

fn played(root: &std::path::Path, stem: &str) -> u64 {
    read_play_times(root).get(stem).copied().unwrap_or(0)
}

#[test]
fn ejecting_writes_down_how_long_the_cart_was_played() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    play_for(&mut a, 45);
    a.apply(Action::Eject);
    let secs = played(d.path(), "Emerald");
    assert!((45..=47).contains(&secs), "played {secs}s, not about 45");
    assert_eq!(played(d.path(), "Fusion"), 0);
}

#[test]
fn play_time_adds_up_across_sessions() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_play_time(d.path(), "Emerald", 1000).unwrap();
    let mut a = app_playing_in(d.path(), "Emerald");
    play_for(&mut a, 30);
    a.apply(Action::Eject);
    let secs = played(d.path(), "Emerald");
    assert!((1030..=1032).contains(&secs), "played {secs}s in all");
}

#[test]
fn the_autosave_keeps_the_count_when_the_cart_is_never_ejected() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    play_for(&mut a, 70);
    a.settle_saves();
    let secs = played(d.path(), "Emerald");
    assert!(
        (55..=62).contains(&secs),
        "the minute's autosave wrote {secs}s"
    );
}

#[test]
fn closing_the_lid_writes_the_count() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    play_for(&mut a, 20);
    a.apply(Action::LidClose);
    let secs = played(d.path(), "Emerald");
    assert!((20..=22).contains(&secs), "the lid wrote {secs}s");
}

#[test]
fn time_in_the_game_menu_does_not_count() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    play_for(&mut a, 10);
    a.apply(Action::GameMenu);
    assert!(a.game_menu_open());
    play_for(&mut a, 40);
    a.apply(Action::GameMenu);
    assert!(!a.game_menu_open());
    a.apply(Action::Eject);
    let secs = played(d.path(), "Emerald");
    assert!(
        (10..=12).contains(&secs),
        "counted {secs}s with the menu up"
    );
}

#[test]
fn a_frame_that_stalls_for_minutes_counts_as_a_second() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.update(0.25);
    a.update(600.0);
    a.apply(Action::Eject);
    assert!(played(d.path(), "Emerald") <= 3);
}

#[test]
fn crossing_a_wear_step_asks_for_the_cart_face_again() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_play_time(d.path(), "Emerald", 3590).unwrap();
    let mut a = app_playing_in(d.path(), "Emerald");
    assert_eq!(a.cart_wear("Emerald"), 0.0);
    assert!(a.take_worn().is_empty());
    play_for(&mut a, 20);
    a.apply(Action::Eject);
    assert!(a.cart_wear("Emerald") > 0.0);
    let worn = a.take_worn();
    assert_eq!(worn.len(), 1);
    let (shelf, index) = worn[0];
    assert_eq!(
        a.face_cart(shelf, index).map(|c| c.stem.as_str()),
        Some("Emerald")
    );
    assert!(a.take_worn().is_empty());
}

#[test]
fn playing_within_a_wear_step_leaves_the_face_alone() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_play_time(d.path(), "Emerald", 4000).unwrap();
    let mut a = app_playing_in(d.path(), "Emerald");
    play_for(&mut a, 20);
    a.apply(Action::Eject);
    assert!(a.take_worn().is_empty());
}
