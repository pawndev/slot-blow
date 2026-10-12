use slot_gfx::{SRC_H, SRC_W};
use slot_input::Millis;
use slot_store::Platform;

pub const ODDS: u64 = 10;

pub const PUFF_TAP: f32 = 0.16;
pub const PUFF_HOLD_S: f32 = 0.45;
pub const PUFF_FADE_S: f32 = 0.25;
pub const PUFF_SOUND_MS: Millis = 300;

const TEAR_MS: Millis = 90;
const TILES: usize = 12;

pub struct Glitch {
    pub platform: Platform,
    pub seed: u64,
    pub at: Millis,
    pub puff: f32,
}

pub struct Dust {
    rng: Option<Rng>,
    odds: u64,
}

impl Dust {
    pub fn clean() -> Self {
        Dust {
            rng: None,
            odds: ODDS,
        }
    }

    pub fn seeded(seed: u64) -> Self {
        Dust {
            rng: Some(Rng(seed)),
            odds: ODDS,
        }
    }

    pub fn always(seed: u64) -> Self {
        Dust {
            rng: Some(Rng(seed)),
            odds: 1,
        }
    }

    pub fn from_env(seed: u64) -> Self {
        match std::env::var("SLOT_DUST").as_deref() {
            Ok("always") => Dust::always(seed),
            Ok("off") => Dust::clean(),
            _ => Dust::seeded(seed),
        }
    }

    pub fn shaken() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64);
        Dust::from_env(seed)
    }

    pub fn roll(&mut self) -> Option<u64> {
        let rng = self.rng.as_mut()?;
        let dud = rng.next() % self.odds == 0;
        dud.then(|| rng.next())
    }
}

pub fn puffed(puff: f32, held: bool, dt: f32) -> f32 {
    let step = if held { PUFF_HOLD_S } else { -PUFF_FADE_S };
    (puff + step * dt).clamp(0.0, 1.0)
}

pub fn glitch(buf: &mut Vec<u8>, platform: Platform, seed: u64, at: Millis, puff: f32) {
    let (sw, sh) = (SRC_W as usize, SRC_H as usize);
    buf.clear();
    buf.resize(sw * sh * 4, 0);
    let (w, h) = platform.picture();
    let (w, h) = (w as usize, h as usize);
    let (ox, oy) = ((sw - w) / 2, (sh - h) / 2);

    let mut rng = Rng(seed);
    let palette = palette(platform, &mut rng);
    let mut tiles: Vec<[u8; 64]> = (0..TILES).map(|_| tile(&mut rng, palette.len())).collect();
    tiles[0] = [0; 64];
    let (cols, rows) = (w / 8, h / 8);
    let map = tile_map(&mut rng, cols, rows);

    let tick = at / TEAR_MS;
    let mut shake = Rng(seed ^ tick.wrapping_mul(0x2545_f491_4f6c_dd1d));
    let roll = if shake.chance(0.12 + 0.2 * puff) {
        shake.below(5) as usize
    } else {
        0
    };
    let mut tears = Vec::new();
    for _ in 0..1 + shake.below(3) {
        if shake.chance(0.55 + 0.4 * puff) {
            let y = shake.below(h as u64) as usize;
            let tall = 2 + shake.below(10) as usize;
            let dx = shake.below(w as u64) as usize;
            tears.push((y, tall, dx));
        }
    }

    for y in 0..h {
        let sy = (y + roll) % h;
        let dx = tears
            .iter()
            .find(|(top, tall, _)| (*top..top + tall).contains(&y))
            .map_or(0, |t| t.2);
        let row = ((oy + y) * sw + ox) * 4;
        for x in 0..w {
            let sx = (x + dx) % w;
            let t = map[(sy / 8) * cols + sx / 8] as usize;
            let c = palette[tiles[t][(sy % 8) * 8 + sx % 8] as usize];
            let at = row + x * 4;
            buf[at] = c[2];
            buf[at + 1] = c[1];
            buf[at + 2] = c[0];
        }
    }
}

fn tile_map(rng: &mut Rng, cols: usize, rows: usize) -> Vec<u8> {
    let mut map = vec![0u8; cols * rows];
    match rng.below(3) {
        0 => {
            let mut run = rng.below(TILES as u64) as u8;
            for cell in &mut map {
                if !rng.chance(0.7) {
                    run = rng.below(TILES as u64) as u8;
                }
                *cell = run;
            }
        }
        1 => {
            let (lw, lh) = (cols * 3 / 5, 2);
            let (lx, ly) = ((cols - lw) / 2, rows / 2 - 1);
            for r in ly..ly + lh {
                for c in lx..lx + lw {
                    map[r * cols + c] = 1 + rng.below(TILES as u64 - 1) as u8;
                }
            }
            for _ in 0..rng.below(4) {
                let r = rng.below(rows as u64) as usize;
                let tile = rng.below(TILES as u64) as u8;
                map[r * cols..(r + 1) * cols].fill(tile);
            }
        }
        _ => {
            let stripes: Vec<u8> = (0..cols).map(|_| rng.below(TILES as u64) as u8).collect();
            for (i, cell) in map.iter_mut().enumerate() {
                *cell = if rng.chance(0.08) {
                    rng.below(TILES as u64) as u8
                } else {
                    stripes[i % cols]
                };
            }
        }
    }
    map
}

fn tile(rng: &mut Rng, colours: usize) -> [u8; 64] {
    let n = colours as u64;
    let (a, b) = (rng.below(n) as u8, rng.below(n) as u8);
    let mut t = [0u8; 64];
    let kind = rng.below(6);
    let lines: Vec<u8> = (0..8).map(|_| rng.below(n) as u8).collect();
    for y in 0..8 {
        for x in 0..8 {
            t[y * 8 + x] = match kind {
                0 => rng.below(n) as u8,
                1 => lines[y],
                2 => lines[x],
                3 => [a, b][(x ^ y) & 1],
                4 => a,
                _ => [a, b][((x + y) / 2) & 1],
            };
        }
    }
    t
}

fn palette(platform: Platform, rng: &mut Rng) -> Vec<[u8; 3]> {
    match platform {
        Platform::Gb => vec![[0xff; 3], [0xaa; 3], [0x55; 3], [0x00; 3]],
        Platform::Gbc => {
            let hue = rng.unit();
            vec![
                [0xf8, 0xf8, 0xf0],
                hsv(hue, 0.75, 0.95),
                hsv(hue + 0.38, 0.9, 0.6),
                [0x10, 0x08, 0x18],
            ]
        }
        Platform::Gba => {
            let mut p = vec![[0xf8, 0xf8, 0xf8]];
            p.extend((1..16).map(|_| hsv(rng.unit(), 0.5 + 0.5 * rng.unit(), rng.unit())));
            p
        }
    }
}

pub(crate) fn hsv(h: f32, s: f32, v: f32) -> [u8; 3] {
    let h = h.rem_euclid(1.0) * 6.0;
    let f = h.fract();
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    let (r, g, b) = match h as u32 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    [r, g, b].map(|c| (c * 255.0).round() as u8)
}

pub(crate) struct Rng(pub(crate) u64);

impl Rng {
    pub(crate) fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    pub(crate) fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }

    pub(crate) fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub(crate) fn chance(&mut self, p: f32) -> bool {
        self.unit() < p
    }
}
