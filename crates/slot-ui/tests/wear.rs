use slot_store::{Cart, Platform};
use slot_ui::{cart_face_worn, label_size, wear_for, wear_label, wear_seed, writes_by_hand};

const HOUR: u64 = 3600;

#[test]
fn a_cart_barely_played_shows_no_wear() {
    assert_eq!(wear_for(0), 0.0);
    assert_eq!(wear_for(HOUR - 1), 0.0);
    assert!(wear_for(HOUR) > 0.0);
}

#[test]
fn wear_only_ever_grows_and_tops_out() {
    let mut last = 0.0;
    for secs in (0..200 * HOUR).step_by(600) {
        let w = wear_for(secs);
        assert!(w >= last, "wear fell back at {secs}s");
        last = w;
    }
    assert_eq!(wear_for(100 * HOUR), 1.0);
    assert_eq!(wear_for(10_000 * HOUR), 1.0);
}

#[test]
fn wear_moves_in_steps_so_a_few_minutes_change_nothing() {
    assert_eq!(wear_for(6 * HOUR), wear_for(6 * HOUR + 900));
}

#[test]
fn the_same_cart_wears_the_same_way_every_time() {
    let label = |seed| {
        let mut rgba = vec![0x80u8; 64 * 48 * 4];
        wear_label(&mut rgba, 64, 48, 0.9, seed);
        rgba
    };
    assert_eq!(label(wear_seed("Emerald")), label(wear_seed("Emerald")));
    assert_ne!(label(wear_seed("Emerald")), label(wear_seed("Fusion")));
}

fn emerald() -> Cart {
    Cart {
        platform: Platform::Gba,
        stem: "Emerald".into(),
        rom: "/x/Emerald.gba".into(),
        label: None,
        title: String::new(),
        code: String::new(),
        shell: None,
        tags: Vec::new(),
    }
}

#[test]
fn an_unworn_face_is_the_plain_face() {
    let cart = emerald();
    let (lw, lh) = label_size(&cart);
    let art = || Some([40u8, 160, 90, 255].repeat((lw * lh) as usize));
    let plain = slot_ui::cart_face_with(&cart, art());
    let unworn = cart_face_worn(&cart, art(), 0.0);
    let worn = cart_face_worn(&cart, art(), 0.5);
    assert_eq!(plain.rgba, unworn.rgba);
    assert_ne!(plain.rgba, worn.rgba);
}

#[test]
fn a_handwritten_label_does_not_wear() {
    let cart = emerald();
    if !writes_by_hand(&cart) {
        return;
    }
    let plain = slot_ui::cart_face_with(&cart, None);
    let worn = cart_face_worn(&cart, None, 1.0);
    assert_eq!(plain.rgba, worn.rgba);
}
