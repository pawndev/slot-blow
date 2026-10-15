use slot::app::{App, Phase};
use slot::audio::Sfx;
use slot::dry::{paint, text_w, Bench, Dry, Joint, Note, Stage, FIXED, NOTICE, ZONE_W};
use slot::dust::Dust;
use slot::pixel_font;
use slot_gfx::{SRC_H, SRC_W};
use slot_input::{Action, Btn};
use slot_store::{Cart, Platform};
use slot_ui::Draw;

const DT: f32 = 1.0 / 60.0;

fn app_with(platform: Platform, stems: &[&str]) -> App {
    let mut a = App::new(
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
    );
    a.set_platform(platform);
    a
}

fn run(a: &mut App, secs: f32) {
    for _ in 0..(secs / DT) as usize {
        a.on_core_ready();
        a.update(DT);
    }
}

fn dry_on(platform: Platform, dry: Dry) -> App {
    let mut a = app_with(platform, &["Emerald", "Zelda"]);
    a.set_dry(dry);
    a.apply(Action::Insert);
    run(&mut a, 2.0);
    a
}

fn dry(dry: Dry) -> App {
    dry_on(Platform::Gba, dry)
}

fn bench(a: &App) -> &Bench {
    a.bench()
        .unwrap_or_else(|| panic!("no bench in {:?}", a.phase()))
}

fn tap(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn read_through(a: &mut App, until: Stage) {
    for _ in 0..40 {
        if a.bench().is_none_or(|b| b.stage() == until) {
            return;
        }
        tap(a, Btn::A);
        run(a, 0.1);
    }
    panic!("never reached {until:?}: {:?}", a.phase());
}

fn hold_a_until(a: &mut App, done: impl Fn(&Bench) -> bool) {
    a.apply(Action::GbaDown(Btn::A));
    let mut held = 0.0;
    while !done(bench(a)) {
        a.update(DT);
        held += DT;
        assert!(held < 4.0, "held A forever in {:?}", a.phase());
    }
    a.apply(Action::GbaUp(Btn::A));
}

fn desolder(a: &mut App) {
    for _ in 0..2 {
        let at = bench(a).selected();
        hold_a_until(a, |b| b.joints()[at] == Joint::Lifted);
    }
    assert_eq!(bench(a).stage(), Stage::Swap);
    for (i, secs) in [0.35, 0.4, 0.4].into_iter().enumerate() {
        run(a, secs);
        snap(a, &format!("swap-{i}"));
    }
    run(a, 1.0);
    assert_eq!(bench(a).stage(), Stage::Solder);
}

fn solder_one(a: &mut App) {
    let at = bench(a).selected();
    let (lo, hi) = bench(a).zone(at);
    hold_a_until(a, |b| b.heat() >= (lo + hi) / 2.0);
}

#[test]
fn a_clean_battery_always_boots() {
    let a = dry(Dry::clean());
    assert!(
        matches!(a.phase(), Phase::Playing { .. }),
        "{:?}",
        a.phase()
    );
}

#[test]
fn a_dry_battery_stops_on_the_notice() {
    let a = dry(Dry::always(7));
    assert_eq!(bench(&a).stage(), Stage::Notice);
    assert_eq!(
        bench(&a).page(),
        [
            "The internal battery has run dry.",
            "The game can be played."
        ]
    );
    let mut out = Vec::new();
    a.draw(&mut out);
    assert!(out.iter().any(|d| matches!(d, Draw::Game)));
}

#[test]
fn about_one_battery_in_ten_is_dry() {
    let mut dry = Dry::seeded(42);
    let dead = (0..10_000).filter(|_| dry.roll().is_some()).count();
    assert!((800..1_200).contains(&dead), "{dead} dry in 10000");
}

#[test]
fn a_first_a_finishes_the_line_and_a_second_turns_the_page() {
    let mut b = Bench::new(Platform::Gba, 3);
    b.step(0.1);
    b.press(Btn::A);
    assert_eq!(b.page()[0], "The internal battery has run dry.");
    assert_eq!(b.take_cue(), None);
    b.press(Btn::A);
    assert_eq!(
        b.page(),
        ["However, clock-based events will", "no longer occur."]
    );
    assert_eq!(b.take_cue(), Some(slot::dry::Cue::Blip));
}

#[test]
fn reading_on_past_the_notice_leads_to_the_bench_not_the_game() {
    let mut a = dry(Dry::always(5));
    read_through(&mut a, Stage::Desolder);
    assert_eq!(bench(&a).joints(), [Joint::Old; 2]);
    assert!(matches!(a.phase(), Phase::Dry { .. }));
}

#[test]
fn replacing_the_battery_boots_the_game() {
    let mut a = dry(Dry::always(11));
    read_through(&mut a, Stage::Desolder);
    desolder(&mut a);
    solder_one(&mut a);
    assert_eq!(bench(&a).note(), Some(Note::Good));
    assert_eq!(bench(&a).selected(), 1, "the iron did not move on");
    solder_one(&mut a);
    assert_eq!(bench(&a).joints(), [Joint::Good; 2]);
    assert_eq!(bench(&a).stage(), Stage::Settle);
    snap(&a, "settle");
    run(&mut a, 1.5);
    assert_eq!(bench(&a).stage(), Stage::Fixed);
    snap(&a, "fixed");
    assert_eq!(bench(&a).page()[0], "The new battery is soldered in.");
    read_through(&mut a, Stage::Done);
    run(&mut a, 0.1);
    let Phase::Playing { cart } = a.phase() else {
        panic!("{:?}", a.phase());
    };
    assert_eq!(cart, "Emerald");
}

#[test]
fn letting_go_too_soon_leaves_a_cold_joint() {
    let mut a = dry(Dry::always(13));
    read_through(&mut a, Stage::Desolder);
    desolder(&mut a);
    let (lo, _) = bench(&a).zone(0);
    hold_a_until(&mut a, |b| b.heat() >= lo - 0.15);
    assert_eq!(bench(&a).joints()[0], Joint::Cold);
    assert_eq!(bench(&a).note(), Some(Note::Cold));
    assert_eq!(bench(&a).selected(), 0, "a cold joint moved the iron on");
    solder_one(&mut a);
    assert_eq!(bench(&a).joints()[0], Joint::Good);
}

#[test]
fn holding_on_too_long_burns_the_pad_until_it_cools() {
    let mut a = dry(Dry::always(17));
    read_through(&mut a, Stage::Desolder);
    desolder(&mut a);
    hold_a_until(&mut a, |b| b.joints()[0] == Joint::Burnt);
    assert_eq!(bench(&a).note(), Some(Note::Burnt));
    assert_eq!(a.take_sfx(), Some(Sfx::Sizzle));
    a.apply(Action::GbaDown(Btn::A));
    run(&mut a, 0.3);
    assert!(
        bench(&a).heat() < 0.7,
        "a burnt pad heated before it cooled"
    );
    a.apply(Action::GbaUp(Btn::A));
    run(&mut a, 1.0);
    solder_one(&mut a);
    assert_eq!(bench(&a).joints()[0], Joint::Good);
}

#[test]
fn a_tap_that_barely_touches_the_pad_changes_nothing() {
    let mut a = dry(Dry::always(19));
    read_through(&mut a, Stage::Desolder);
    desolder(&mut a);
    tap(&mut a, Btn::A);
    assert_eq!(bench(&a).joints()[0], Joint::Bare);
    assert_eq!(bench(&a).note(), None);
}

#[test]
fn the_iron_moves_with_the_d_pad_but_not_while_heating() {
    let mut a = dry(Dry::always(23));
    read_through(&mut a, Stage::Desolder);
    tap(&mut a, Btn::Right);
    assert_eq!(bench(&a).selected(), 1);
    a.apply(Action::GbaDown(Btn::A));
    tap(&mut a, Btn::Left);
    assert_eq!(bench(&a).selected(), 1);
    a.apply(Action::GbaUp(Btn::A));
    tap(&mut a, Btn::Left);
    assert_eq!(bench(&a).selected(), 0);
}

#[test]
fn the_game_never_sees_the_soldering() {
    let a = dry(Dry::always(29));
    for btn in [Btn::A, Btn::B, Btn::Left, Btn::Start] {
        assert!(a.takes_from_the_game(Action::GbaDown(btn)));
        assert!(a.takes_from_the_game(Action::GbaUp(btn)));
    }
}

#[test]
fn a_dry_cart_can_still_be_ejected() {
    let mut a = dry(Dry::always(31));
    a.apply(Action::Eject);
    assert!(matches!(a.phase(), Phase::Ejecting { .. }));
    run(&mut a, 2.0);
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
}

#[test]
fn dust_comes_first_and_the_battery_can_still_be_dry_after_a_blow() {
    let mut a = app_with(Platform::Gba, &["Emerald", "Zelda"]);
    a.set_dust(Dust::always(1));
    a.set_dry(Dry::always(2));
    a.apply(Action::Insert);
    run(&mut a, 2.0);
    assert!(matches!(a.phase(), Phase::Dud { .. }), "{:?}", a.phase());
    a.set_dust(Dust::clean());
    a.apply(Action::GbaDown(Btn::B));
    run(&mut a, 2.5);
    a.apply(Action::GbaUp(Btn::B));
    run(&mut a, 2.0);
    assert!(matches!(a.phase(), Phase::Dry { .. }), "{:?}", a.phase());
}

#[test]
fn turning_dry_batteries_off_in_the_menu_keeps_the_clock_ticking() {
    let mut a = app_with(Platform::Gba, &["Emerald", "Zelda"]);
    a.set_dry(Dry::always(1));
    a.apply(Action::QuickMenu);
    a.apply(Action::GbaDown(Btn::Down));
    a.apply(Action::GbaDown(Btn::Down));
    a.apply(Action::GbaDown(Btn::A));
    a.apply(Action::GbaDown(Btn::Up));
    a.apply(Action::GbaDown(Btn::A));
    a.apply(Action::GbaDown(Btn::Up));
    assert_eq!(a.quick_menu(), Some(slot_ui::QuickRow::Dry));
    a.apply(Action::GbaDown(Btn::Right));
    assert_eq!(
        a.quick_value(slot_ui::QuickRow::Dry),
        Some(slot_ui::QuickValue::Off)
    );
    a.apply(Action::QuickMenu);
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
fn every_line_fits_its_box_on_every_screen() {
    let statuses = [
        "Cold joint! All dull.",
        "Hold A a bit longer.",
        "Too hot! The pad burnt.",
        "Let it cool, try again.",
        "A nice shiny joint!",
        "Now the other tab.",
        "Desolder the old cell.",
        "Hold A on each tab.",
        "Out with the old...",
        "...in with the new!",
        "Solder in the new cell.",
        "Let go in the green!",
        "Shiny! Nicely done.",
    ];
    for text in [NOTICE, FIXED].into_iter().chain(statuses) {
        assert!(
            text.chars().all(|c| c == '\n' || pixel_font::knows(c)),
            "the font cannot write {text:?}"
        );
    }
    for platform in Platform::ALL {
        let room = text_w(platform);
        for line in statuses {
            let w = pixel_font::width(line);
            assert!(
                w <= room,
                "{platform:?}: {line:?} is {w}px, room for {room}"
            );
        }
        for text in [NOTICE, FIXED] {
            for line in pixel_font::wrap(text, room) {
                assert!(pixel_font::width(&line) <= room, "{platform:?}: {line:?}");
            }
        }
    }
}

#[test]
fn the_zone_always_sits_inside_the_gauge() {
    for seed in 0..500 {
        let b = Bench::new(Platform::Gba, seed);
        for joint in 0..2 {
            let (lo, hi) = b.zone(joint);
            assert!(lo > 0.3 && hi < 0.95 && (hi - lo - ZONE_W).abs() < 1e-6);
        }
    }
}

#[test]
fn the_bench_paints_only_the_picture() {
    for platform in Platform::ALL {
        let mut a = dry_on(platform, Dry::always(37));
        let mut frames = Vec::new();
        for (name, until) in [("notice", Stage::Notice), ("desolder", Stage::Desolder)] {
            read_through(&mut a, until);
            run(&mut a, 1.0);
            let mut buf = Vec::new();
            paint(&mut buf, bench(&a));
            dump(&buf, platform, name);
            assert_eq!(buf.len(), (SRC_W * SRC_H * 4) as usize);
            let (w, h) = platform.picture();
            let (ox, oy) = ((SRC_W - w) / 2, (SRC_H - h) / 2);
            let px = |x: u32, y: u32| {
                let at = ((y * SRC_W + x) * 4) as usize;
                [buf[at], buf[at + 1], buf[at + 2]]
            };
            if ox > 0 {
                assert_eq!(px(0, SRC_H / 2), [0, 0, 0], "{platform:?} spills left");
                assert_ne!(px(ox, oy + h / 2), [0, 0, 0], "{platform:?} is blank");
            }
            frames.push(buf);
        }
        assert_ne!(
            frames[0], frames[1],
            "{platform:?}: the bench looks like the notice"
        );
        let mut b = Bench::new(platform, 3);
        let mut buf = Vec::new();
        for step in 0..12 {
            b.step(0.25);
            paint(&mut buf, &b);
            dump(&buf, platform, &format!("typing-{step}"));
        }
    }
}

#[test]
fn a_heated_joint_glows_and_smokes() {
    let mut a = dry(Dry::always(41));
    read_through(&mut a, Stage::Desolder);
    desolder(&mut a);
    let mut cold = Vec::new();
    paint(&mut cold, bench(&a));
    dump(&cold, Platform::Gba, "solder");
    a.apply(Action::GbaDown(Btn::A));
    for _ in 0..50 {
        a.update(DT);
    }
    let mut hot = Vec::new();
    paint(&mut hot, bench(&a));
    dump(&hot, Platform::Gba, "soldering");
    assert_ne!(cold, hot);
    for _ in 0..60 {
        a.update(DT);
    }
    let mut burnt = Vec::new();
    paint(&mut burnt, bench(&a));
    dump(&burnt, Platform::Gba, "burnt");
    a.apply(Action::GbaUp(Btn::A));
}

fn snap(a: &App, name: &str) {
    let mut buf = Vec::new();
    let b = bench(a);
    paint(&mut buf, b);
    dump(&buf, b.platform(), name);
}

fn dump(buf: &[u8], platform: Platform, name: &str) {
    let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") else {
        return;
    };
    const ZOOM: u32 = 3;
    let (w, h) = (SRC_W * ZOOM, SRC_H * ZOOM);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let at = (((y / ZOOM) * SRC_W + x / ZOOM) * 4) as usize;
            rgba.extend_from_slice(&[buf[at + 2], buf[at + 1], buf[at], 0xff]);
        }
    }
    let path = format!("{dir}/dry-{platform:?}-{name}.png").to_lowercase();
    let file = std::fs::File::create(&path).expect("create png");
    let mut e = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header()
        .expect("png header")
        .write_image_data(&rgba)
        .expect("png data");
}
