#![cfg(target_os = "macos")]

use slot::app::{App, Phase};
use slot::dust::{glitch, Dust};
use slot_gfx::{Compositor, HeadlessSurface, OUT_H, OUT_W};
use slot_input::{Action, Btn};
use slot_store::{Cart, Platform};
use slot_ui::hint_face;

const DT: f32 = 1.0 / 60.0;

fn dusty(platform: Platform) -> App {
    let mut a = App::new(
        ["Tetris", "Zelda"]
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
    a.set_dust(Dust::always(37));
    a.apply(Action::Insert);
    for _ in 0..180 {
        a.on_core_ready();
        a.update(DT);
    }
    a
}

fn composed(a: &App, c: &mut Compositor, glitched: &mut Vec<u8>, name: &str) -> Vec<u8> {
    c.set_picture(a.picture_rect());
    c.set_game_source_rect(a.source_rect());
    c.set_screen_power(a.screen_power());
    c.set_frame_lift(a.frame_lift());
    c.begin_frame();
    let g = a.glitch().expect("no glitch on a dusty cart");
    glitch(glitched, g.platform, g.seed, g.at, g.puff);
    c.upload_game(glitched);
    let mut draws = Vec::new();
    a.draw(&mut draws);
    c.draw_list(&draws);
    let px = c.read_frame();
    if let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") {
        let path = format!("{dir}/dust-{name}.png");
        let file = std::fs::File::create(&path).expect("create png");
        let mut e = png::Encoder::new(std::io::BufWriter::new(file), OUT_W, OUT_H);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header()
            .expect("png header")
            .write_image_data(&px)
            .expect("png data");
        println!("wrote {path}");
    }
    px
}

fn at(px: &[u8], x: usize, y: usize) -> [u8; 3] {
    let o = (y * OUT_W as usize + x) * 4;
    [px[o], px[o + 1], px[o + 2]]
}

#[test]
fn a_half_blown_cart_shows_its_breath_over_the_glitch() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let mut glitched = Vec::new();
    for platform in [Platform::Gb, Platform::Gbc, Platform::Gba] {
        let mut a = dusty(platform);
        let face = hint_face("B", "Blow");
        let tex = c.create_texture(face.w, face.h, &face.rgba);
        a.set_blow_face(tex, face.w);
        assert!(matches!(a.phase(), Phase::Dud { .. }), "{:?}", a.phase());

        let name = format!("{platform:?}-glitch").to_lowercase();
        composed(&a, &mut c, &mut glitched, &name);

        a.apply(Action::GbaDown(Btn::B));
        for _ in 0..50 {
            a.update(DT);
        }
        let name = format!("{platform:?}-blowing").to_lowercase();
        let px = composed(&a, &mut c, &mut glitched, &name);
        let Phase::Dud { puff, .. } = a.phase() else {
            panic!("{:?}", a.phase());
        };
        assert!((0.4..0.9).contains(puff), "puff {puff}");

        let bar_y = OUT_H as usize - 24 - 20;
        let lit: Vec<usize> = (0..OUT_W as usize)
            .filter(|&x| at(&px, x, bar_y) == [0xf5, 0xf2, 0xf0])
            .collect();
        assert!(
            lit.len() > 40,
            "{platform:?}: no breath on the meter ({} px)",
            lit.len()
        );
    }
}
