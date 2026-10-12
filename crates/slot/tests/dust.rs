use slot::app::{App, Phase};
use slot::audio::Sfx;
use slot::dust::{glitch, Dust, PUFF_TAP};
use slot_gfx::{SRC_H, SRC_W};
use slot_input::{Action, Btn};
use slot_store::{Cart, Platform};
use slot_ui::Draw;

const DT: f32 = 1.0 / 60.0;

fn app_with(platform: Platform, stems: &[&str]) -> App {
    App::new(
        stems
            .iter()
            .map(|stem| Cart {
                platform,
                stem: (*stem).to_string(),
                rom: format!("Games/{stem}.rom").into(),
                label: None,
                code: String::new(),
                shell: None,
                title: stem.to_uppercase(),
            })
            .collect(),
    )
}

fn run(a: &mut App, secs: f32) {
    for _ in 0..(secs / DT) as usize {
        a.on_core_ready();
        a.update(DT);
    }
}

fn dud(dust: Dust) -> App {
    let mut a = app_with(Platform::Gba, &["Emerald", "Zelda"]);
    a.set_dust(dust);
    a.apply(Action::Insert);
    run(&mut a, 2.0);
    a
}

fn tap(a: &mut App) {
    a.apply(Action::GbaDown(Btn::B));
    a.apply(Action::GbaUp(Btn::B));
}

#[test]
fn a_clean_cart_always_boots() {
    let a = dud(Dust::clean());
    assert!(matches!(a.phase(), Phase::Playing { .. }));
}

#[test]
fn a_dusty_cart_stops_on_a_glitched_screen() {
    let a = dud(Dust::always(7));
    assert!(matches!(a.phase(), Phase::Dud { .. }), "{:?}", a.phase());
    assert!(a.glitch().is_some());
    let mut out = Vec::new();
    a.draw(&mut out);
    assert!(out.iter().any(|d| matches!(d, Draw::Game)));
}

#[test]
fn about_one_cart_in_ten_is_dusty() {
    let mut dust = Dust::seeded(42);
    let duds = (0..10_000).filter(|_| dust.roll().is_some()).count();
    assert!((800..1_200).contains(&duds), "{duds} duds in 10000");
}

#[test]
fn turning_dusty_carts_off_in_the_menu_stops_the_glitch() {
    let mut a = app_with(Platform::Gba, &["Emerald", "Zelda"]);
    a.set_dust(Dust::always(1));
    a.apply(Action::QuickMenu);
    a.apply(Action::GbaDown(Btn::Down));
    a.apply(Action::GbaDown(Btn::Down));
    a.apply(Action::GbaDown(Btn::A));
    a.apply(Action::GbaDown(Btn::Up));
    a.apply(Action::GbaDown(Btn::Up));
    a.apply(Action::GbaDown(Btn::Up));
    assert_eq!(
        a.quick_menu(),
        Some(slot_ui::QuickRow::Dusty),
        "walked to the wrong row"
    );
    a.apply(Action::GbaDown(Btn::Right));
    assert_eq!(
        a.quick_value(slot_ui::QuickRow::Dusty),
        Some(slot_ui::QuickValue::Off)
    );
    a.apply(Action::QuickMenu);
    a.apply(Action::QuickMenu);
    a.apply(Action::Insert);
    run(&mut a, 2.0);
    assert!(
        matches!(a.phase(), Phase::Playing { .. }),
        "{:?}",
        a.phase()
    );
}

#[test]
fn holding_b_blows_the_cart_out_and_back_in() {
    let mut a = dud(Dust::always(3));
    a.apply(Action::GbaDown(Btn::B));
    let mut held = 0.0;
    while matches!(a.phase(), Phase::Dud { .. }) {
        a.update(DT);
        held += DT;
        assert!(held < 3.0, "a held breath never reseated the cart");
    }
    assert!(held > 1.2, "blew clean in {held}s");
    assert!(matches!(a.phase(), Phase::Ejecting { .. }));
    a.apply(Action::GbaUp(Btn::B));
    a.set_dust(Dust::clean());
    run(&mut a, 1.0);
    assert!(
        matches!(a.phase(), Phase::Inserting { .. }),
        "{:?}",
        a.phase()
    );
    run(&mut a, 2.0);
    let Phase::Playing { cart } = a.phase() else {
        panic!("{:?}", a.phase());
    };
    assert_eq!(cart, "Emerald");
}

#[test]
fn mashing_b_works_as_well_as_holding_it() {
    let mut a = dud(Dust::always(5));
    let mut taps = 0;
    while matches!(a.phase(), Phase::Dud { .. }) {
        tap(&mut a);
        taps += 1;
        for _ in 0..8 {
            a.update(DT);
        }
        assert!(taps < 20, "mashing never reseated the cart");
    }
    assert!(taps >= (1.0 / PUFF_TAP) as usize - 1, "only {taps} taps");
}

#[test]
fn a_breath_that_stops_fades_away() {
    let mut a = dud(Dust::always(9));
    tap(&mut a);
    run(&mut a, 2.0);
    let Phase::Dud { puff, .. } = a.phase() else {
        panic!("{:?}", a.phase());
    };
    assert_eq!(*puff, 0.0);
}

#[test]
fn every_puff_makes_a_sound() {
    let mut a = dud(Dust::always(11));
    let _ = a.take_sfx();
    tap(&mut a);
    assert_eq!(a.take_sfx(), Some(Sfx::Blow));
}

#[test]
fn a_dusty_cart_can_still_be_ejected() {
    let mut a = dud(Dust::always(13));
    a.apply(Action::Eject);
    assert!(matches!(a.phase(), Phase::Ejecting { .. }));
    run(&mut a, 2.0);
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
}

#[test]
fn a_cart_resumed_at_boot_is_never_dusty() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("Games/GBA")).unwrap();
    std::fs::write(d.path().join("Games/GBA/Emerald.gba"), b"rom").unwrap();
    let mut a = App::boot(d.path());
    a.set_dust(Dust::always(17));
    run(&mut a, 2.0);
    assert!(!matches!(a.phase(), Phase::Dud { .. }));
}

#[test]
fn the_glitch_fills_only_the_picture() {
    for platform in Platform::ALL {
        let mut buf = Vec::new();
        glitch(&mut buf, platform, 99, 0, 0.0);
        assert_eq!(buf.len(), (SRC_W * SRC_H * 4) as usize);
        let (w, h) = platform.picture();
        let (ox, oy) = ((SRC_W - w) / 2, (SRC_H - h) / 2);
        let px = |x: u32, y: u32| {
            let at = ((y * SRC_W + x) * 4) as usize;
            [buf[at], buf[at + 1], buf[at + 2]]
        };
        let lit = (oy..oy + h)
            .flat_map(|y| (ox..ox + w).map(move |x| (x, y)))
            .filter(|&(x, y)| px(x, y) != [0, 0, 0])
            .count();
        assert!(lit > 0, "{platform:?} glitch is blank");
        if ox > 0 {
            assert_eq!(px(0, SRC_H / 2), [0, 0, 0], "{platform:?} spills left");
        }
    }
}

#[test]
fn the_glitch_moves() {
    let (mut a, mut b) = (Vec::new(), Vec::new());
    let moved = (0..20).any(|tick| {
        glitch(&mut a, Platform::Gb, 5, tick * 90, 0.5);
        glitch(&mut b, Platform::Gb, 5, (tick + 1) * 90, 0.5);
        a != b
    });
    assert!(moved);
}
