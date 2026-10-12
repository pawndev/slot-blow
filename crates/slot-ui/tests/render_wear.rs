use slot_store::{scan, Cart};
use slot_ui::{cart_face_worn, label_size, CartFace};
use tempfile::TempDir;

const STEPS: [f32; 7] = [0.0, 0.15, 0.3, 0.5, 0.7, 0.85, 1.0];
const CELL_W: u32 = 380;
const CELL_H: u32 = 360;

const GROUND: [u8; 3] = [0x25, 0x27, 0x2e];

fn write_rom(d: &TempDir, dir: &str, name: &str, cgb: u8) {
    let mut rom = vec![0u8; 0x150];
    rom[0x143] = cgb;
    let games = d.path().join("Games").join(dir);
    std::fs::create_dir_all(&games).expect("create games dir");
    std::fs::write(games.join(name), rom).expect("write rom");
}

fn write_gba_rom(d: &TempDir, name: &str, code: &str) {
    let mut rom = vec![0u8; 0x100];
    rom[0xac..0xac + code.len()].copy_from_slice(code.as_bytes());
    let games = d.path().join("Games/GBA");
    std::fs::create_dir_all(&games).expect("create games dir");
    std::fs::write(games.join(name), rom).expect("write rom");
}

fn box_art(w: u32, h: u32) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    let (sun_x, sun_y, sun_r) = (w as f32 * 0.7, h as f32 * 0.42, h as f32 * 0.16);
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
            let hill = 0.72 + 0.08 * (fx * 9.0).sin() + 0.04 * (fx * 23.0).cos();
            let c = if fy < 0.2 {
                if (x / 6 + y / 6) % 2 == 0 {
                    [0xd8, 0x1e, 0x2a]
                } else {
                    [0xb8, 0x14, 0x22]
                }
            } else if fy > hill {
                [0x1f, (0x8a as f32 - fy * 60.0) as u8, 0x3c]
            } else if (x as f32 - sun_x).hypot(y as f32 - sun_y) < sun_r {
                [0xff, 0xc8, 0x2a]
            } else {
                let t = (fy - 0.2) / 0.6;
                [
                    (0x2a as f32 + t * 0x90 as f32) as u8,
                    (0x6c as f32 + t * 0x70 as f32) as u8,
                    0xe8,
                ]
            };
            rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    rgba
}

fn paste(frame: &mut [u8], sheet_w: u32, face: &CartFace, left: u32, top: u32) {
    for y in 0..face.h {
        for x in 0..face.w {
            let s = ((y * face.w + x) * 4) as usize;
            let a = face.rgba[s + 3] as u32;
            if a == 0 {
                continue;
            }
            let d = (((y + top) * sheet_w + x + left) * 3) as usize;
            for c in 0..3 {
                frame[d + c] =
                    ((face.rgba[s + c] as u32 * a + frame[d + c] as u32 * (255 - a) + 127) / 255)
                        as u8;
            }
        }
    }
}

fn find<'a>(carts: &'a [Cart], stem: &str) -> &'a Cart {
    carts.iter().find(|c| c.stem == stem).expect("cart")
}

#[test]
fn render_wear() {
    let Ok(out) = std::env::var("SCRATCH_PNG") else {
        return;
    };
    let d = tempfile::tempdir().expect("tempdir");
    write_gba_rom(&d, "Metroid Fusion.gba", "AMTE");
    write_gba_rom(&d, "Golden Sun.gba", "AGSE");
    write_rom(&d, "GB", "Tetris.gb", 0x00);
    write_rom(&d, "GBC", "Tetris Chromatic.gbc", 0xc0);
    let carts = scan(d.path()).expect("scan");

    let rows: [(&str, bool); 4] = [
        ("Metroid Fusion", false),
        ("Golden Sun", true),
        ("Tetris", false),
        ("Tetris Chromatic", true),
    ];
    let sheet_w = CELL_W * STEPS.len() as u32;
    let sheet_h = CELL_H * rows.len() as u32;
    let mut frame: Vec<u8> = GROUND
        .iter()
        .copied()
        .cycle()
        .take((sheet_w * sheet_h * 3) as usize)
        .collect();

    for (r, (stem, art)) in rows.iter().enumerate() {
        let cart = find(&carts, stem);
        for (c, worn) in STEPS.iter().enumerate() {
            let (lw, lh) = label_size(cart);
            let face = cart_face_worn(cart, art.then(|| box_art(lw, lh)), *worn);
            let left = c as u32 * CELL_W + (CELL_W - face.w) / 2;
            let top = r as u32 * CELL_H + (CELL_H - face.h) / 2;
            paste(&mut frame, sheet_w, &face, left, top);
        }
    }

    let f = std::fs::File::create(&out).expect("create png");
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), sheet_w, sheet_h);
    e.set_color(png::ColorType::Rgb);
    e.set_depth(png::BitDepth::Eight);
    e.write_header()
        .expect("png header")
        .write_image_data(&frame)
        .expect("png data");
    println!("wrote {out} (columns: wear {STEPS:?})");
}
