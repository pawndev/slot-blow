const PAPER: [u8; 3] = [0xec, 0xe0, 0xc0];
const PAPER_BACK: [u8; 3] = [0xf3, 0xef, 0xe6];
const GRIME: [u8; 3] = [0x4a, 0x3c, 0x2c];
const FIBRE: [u8; 3] = [0xfa, 0xf7, 0xf0];

const SCRATCHES: usize = 14;
const CORNER_FROM: [f32; 3] = [0.25, 0.55, 0.85];
const SHOW_THROUGH: f32 = 0.22;

const HOUR: u64 = 3600;

const WEAR_TIERS: [(u64, f32); 9] = [
    (HOUR, 0.1),
    (3 * HOUR, 0.2),
    (5 * HOUR, 0.3),
    (10 * HOUR, 0.4),
    (20 * HOUR, 0.5),
    (35 * HOUR, 0.6),
    (50 * HOUR, 0.7),
    (75 * HOUR, 0.85),
    (100 * HOUR, 1.0),
];

pub fn wear_for(played_secs: u64) -> f32 {
    WEAR_TIERS
        .iter()
        .take_while(|(from, _)| played_secs >= *from)
        .last()
        .map_or(0.0, |(_, wear)| *wear)
}

pub fn wear_seed(stem: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in stem.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

pub fn wear_label(rgba: &mut [u8], w: u32, h: u32, amount: f32, seed: u64) {
    let amount = amount.clamp(0.0, 1.0);
    if amount <= 0.0 || w == 0 || h == 0 {
        return;
    }
    let mut label = Label { rgba, w, h };
    fade(&mut label, amount, seed);
    smudge(&mut label, amount, seed);
    scratch(&mut label, amount, seed);
    fray(&mut label, amount, seed);
    corners(&mut label, amount, seed);
}

struct Label<'a> {
    rgba: &'a mut [u8],
    w: u32,
    h: u32,
}

impl Label<'_> {
    fn at(&self, x: u32, y: u32) -> usize {
        ((y * self.w + x) * 4) as usize
    }

    fn mix(&mut self, x: u32, y: u32, to: [u8; 3], a: f32) {
        let i = self.at(x, y);
        let a = a.clamp(0.0, 1.0);
        for (px, to) in self.rgba[i..i + 3].iter_mut().zip(to) {
            let from = *px as f32;
            *px = (from + (to as f32 - from) * a).round() as u8;
        }
    }

    fn scale(&mut self, x: u32, y: u32, f: f32) {
        let i = self.at(x, y);
        for c in 0..3 {
            self.rgba[i + c] = (self.rgba[i + c] as f32 * f).clamp(0.0, 255.0) as u8;
        }
    }
}

fn fade(l: &mut Label, amount: f32, seed: u64) {
    let fade = amount.powf(0.8);
    for y in 0..l.h {
        for x in 0..l.w {
            let i = l.at(x, y);
            if l.rgba[i + 3] == 0 {
                continue;
            }
            let sun = fade * (0.7 + 0.6 * noise(x as f32 / 90.0, y as f32 / 90.0, seed));
            let px = [l.rgba[i], l.rgba[i + 1], l.rgba[i + 2]];
            let luma = 0.2126 * px[0] as f32 + 0.7152 * px[1] as f32 + 0.0722 * px[2] as f32;
            for c in 0..3 {
                let grey = px[c] as f32 + (luma - px[c] as f32) * 0.45 * sun;
                let washed = grey + (PAPER[c] as f32 - grey) * 0.3 * sun;
                l.rgba[i + c] = washed.clamp(0.0, 255.0) as u8;
            }
        }
    }
}

fn smudge(l: &mut Label, amount: f32, seed: u64) {
    let grime = ((amount - 0.2) / 0.8).max(0.0);
    if grime <= 0.0 {
        return;
    }
    let mut rng = Rng(seed ^ 0x005e_ed0f_7b0b);
    let cx = l.w as f32 * rng.range(0.35, 0.65);
    let cy = l.h as f32 * rng.range(0.7, 0.85);
    let (rx, ry) = (l.w as f32 * 0.2, l.h as f32 * 0.16);
    for y in 0..l.h {
        for x in 0..l.w {
            let (dx, dy) = ((x as f32 - cx) / rx, (y as f32 - cy) / ry);
            let d = dx * dx + dy * dy;
            if d >= 1.0 {
                continue;
            }
            let ridges = noise(x as f32 / 3.0, y as f32 / 3.0, seed ^ 0xf1);
            let k = (1.0 - d).powi(2) * (0.5 + 0.5 * ridges) * 0.2 * grime;
            l.mix(x, y, GRIME, k);
        }
    }
}

struct Stroke {
    from: (f32, f32),
    angle: f32,
    bend: f32,
    len: f32,
    strength: f32,
}

fn scratch(l: &mut Label, amount: f32, seed: u64) {
    let mut rng = Rng(seed ^ 0x5c7a_7c4e);
    let strokes: Vec<Stroke> = (0..SCRATCHES)
        .map(|_| Stroke {
            from: (rng.range(0.0, l.w as f32), rng.range(0.0, l.h as f32)),
            angle: rng.range(-0.45, 0.45)
                + if rng.next().is_multiple_of(4) {
                    1.4
                } else {
                    0.0
                },
            bend: rng.range(-0.004, 0.004),
            len: rng.range(0.15, 0.6) * l.w as f32,
            strength: rng.range(0.3, 0.65),
        })
        .collect();
    let shown = ((amount - 0.08).max(0.0) / 0.92 * SCRATCHES as f32).ceil() as usize;

    let mut mask = vec![0f32; (l.w * l.h) as usize];
    for (n, s) in strokes.iter().take(shown).enumerate() {
        let (mut x, mut y) = s.from;
        let mut a = s.angle;
        let steps = (s.len * 2.0) as u32;
        for t in 0..steps {
            let gap = noise(t as f32 / 14.0, n as f32 * 7.0, seed ^ 0x9a9);
            if gap > 0.35 {
                let taper = (t.min(steps - t) as f32 / 10.0).min(1.0);
                splat(&mut mask, l.w, l.h, x, y, s.strength * taper);
            }
            x += a.cos() * 0.5;
            y += a.sin() * 0.5;
            a += s.bend;
        }
    }
    for y in 0..l.h {
        for x in 0..l.w {
            let m = mask[(y * l.w + x) as usize];
            if m > 0.0 && l.rgba[l.at(x, y) + 3] > 0 {
                l.mix(x, y, FIBRE, m);
            }
        }
    }
}

fn splat(mask: &mut [f32], w: u32, h: u32, x: f32, y: f32, strength: f32) {
    let (x0, y0) = (x.floor() as i32, y.floor() as i32);
    for yy in y0 - 1..=y0 + 1 {
        for xx in x0 - 1..=x0 + 1 {
            if xx < 0 || yy < 0 || xx >= w as i32 || yy >= h as i32 {
                continue;
            }
            let d = ((xx as f32 + 0.5 - x).powi(2) + (yy as f32 + 0.5 - y).powi(2)).sqrt();
            let cover = (1.0 - d / 0.9).max(0.0) * strength;
            let m = &mut mask[(yy as u32 * w + xx as u32) as usize];
            *m = m.max(cover);
        }
    }
}

fn fray(l: &mut Label, amount: f32, seed: u64) {
    let reach = 1.0 + amount * 5.0;
    for y in 0..l.h {
        for x in 0..l.w {
            let sides = [x, l.w - 1 - x, y, l.h - 1 - y];
            let (side, edge) = sides
                .iter()
                .enumerate()
                .min_by_key(|(_, d)| **d)
                .map(|(i, d)| (i, *d as f32))
                .unwrap_or((0, 0.0));
            if edge >= reach {
                continue;
            }
            let along = if side < 2 { y } else { x } as f32 + side as f32 * 517.0;
            let coarse = noise(along / 18.0, 0.0, seed ^ 0xed6e);
            let fibre = noise(along / 2.5, edge / 1.5, seed ^ 0xf1b2);
            let worn = reach * (0.15 + 0.85 * coarse) * (0.6 + 0.4 * fibre);
            if edge < 1.0 && coarse > 1.0 - amount * 0.3 && fibre > 0.45 {
                let i = l.at(x, y);
                l.rgba[i + 3] = 0;
            } else if edge < worn {
                l.mix(
                    x,
                    y,
                    FIBRE,
                    (1.0 - edge / worn).powf(1.5) * 0.7 * amount.sqrt(),
                );
            }
        }
    }
}

#[derive(Copy, Clone)]
enum Corner {
    DogEar,
    Torn,
}

fn corners(l: &mut Label, amount: f32, seed: u64) {
    let mut rng = Rng(seed ^ 0xc0e2_4e25);
    let mut order = [0u8, 1, 2, 3];
    for i in (1..4).rev() {
        order.swap(i, (rng.next() % (i as u64 + 1)) as usize);
    }
    let count = 2 + (rng.next() % 2) as usize;
    let short = l.w.min(l.h) as f32;
    for (k, which) in order.into_iter().take(count).enumerate() {
        let kind = if k == 0 || rng.next() % 10 < 7 {
            Corner::DogEar
        } else {
            Corner::Torn
        };
        let ratio = rng.range(0.6, 1.5);
        let start = CORNER_FROM[k];
        if amount <= start {
            continue;
        }
        let grown = ((amount - start) / (1.0 - start)).sqrt();
        let leg = short * (0.07 + 0.17 * grown) * if k == 0 { 1.0 } else { 0.75 };
        let (a, b) = (leg * ratio.sqrt(), leg / ratio.sqrt());
        match kind {
            Corner::DogEar => dog_ear(l, which, a, b, amount),
            Corner::Torn => torn(l, which, a, b, seed ^ (k as u64 * 0x1d)),
        }
    }
}

fn local(l: &Label, which: u8, x: f32, y: f32) -> (f32, f32) {
    let (w, h) = (l.w as f32, l.h as f32);
    match which {
        0 => (x, y),
        1 => (w - x, y),
        2 => (x, h - y),
        _ => (w - x, h - y),
    }
}

enum Fold {
    Gone,
    Flap { depth: f32, from: (f32, f32) },
    Kept,
}

fn fold(u: f32, v: f32, a: f32, b: f32) -> Fold {
    let n = (a * a + b * b).sqrt();
    let s = (u * b + v * a - a * b) / n;
    if s < 0.0 {
        return Fold::Gone;
    }
    let (ru, rv) = (u - 2.0 * s * b / n, v - 2.0 * s * a / n);
    if ru >= 0.0 && rv >= 0.0 {
        Fold::Flap {
            depth: s / (a * b / n),
            from: (ru, rv),
        }
    } else {
        Fold::Kept
    }
}

const SUB: [(f32, f32); 4] = [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)];
const SHADOW: (f32, f32) = (2.0, 3.0);

fn dog_ear(l: &mut Label, which: u8, a: f32, b: f32, amount: f32) {
    let reach = (a.max(b) * 1.2 + 4.0) as u32;
    let print = l.rgba.to_vec();
    let aged = [0, 1, 2].map(|c| {
        let (fresh, old) = (PAPER_BACK[c] as f32, PAPER[c] as f32);
        fresh + (old - fresh) * 0.5 * amount
    });
    let flap_at = |l: &Label, x: f32, y: f32| {
        let (u, v) = local(l, which, x, y);
        fold(u, v, a, b)
    };
    for (x, y) in corner_box(l, which, reach) {
        let (mut gone, mut flap, mut depth, mut shade) = (0f32, 0f32, 0f32, 0f32);
        let mut through = [0f32; 3];
        for (sx, sy) in SUB {
            let (px, py) = (x as f32 + sx, y as f32 + sy);
            match flap_at(l, px, py) {
                Fold::Gone => gone += 0.25,
                Fold::Flap { depth: d, from } => {
                    flap += 0.25;
                    depth += d * 0.25;
                    let (fx, fy) = local(l, which, from.0, from.1);
                    let fx = (fx.max(0.0) as u32).min(l.w - 1);
                    let fy = (fy.max(0.0) as u32).min(l.h - 1);
                    let j = l.at(fx, fy);
                    for c in 0..3 {
                        through[c] += print[j + c] as f32 * 0.25;
                    }
                }
                Fold::Kept => {
                    if let Fold::Flap { .. } = flap_at(l, px - SHADOW.0, py - SHADOW.1) {
                        shade += 0.25;
                    }
                }
            }
        }
        let i = l.at(x, y);
        if shade > 0.0 {
            l.scale(x, y, 1.0 - 0.35 * shade);
        }
        if flap > 0.0 {
            let lit = 0.86 + 0.12 * (depth / flap).min(1.0);
            let ink = through.map(|c| c / flap);
            let luma = 0.2126 * ink[0] + 0.7152 * ink[1] + 0.0722 * ink[2];
            let back = [0, 1, 2].map(|c| {
                let ink = ink[c] + (luma - ink[c]) * 0.5;
                ((aged[c] + (ink - aged[c]) * SHOW_THROUGH) * lit) as u8
            });
            l.mix(x, y, back, flap / (1.0 - gone).max(0.25));
            l.rgba[i + 3] = l.rgba[i + 3].max((flap * 255.0) as u8);
        }
        if gone > 0.0 {
            l.rgba[i + 3] = (l.rgba[i + 3] as f32 * (1.0 - gone)) as u8;
        }
    }
}

fn torn(l: &mut Label, which: u8, a: f32, b: f32, seed: u64) {
    let reach = (a.max(b) * 1.5 + 4.0) as u32;
    for (x, y) in corner_box(l, which, reach) {
        let (u, v) = local(l, which, x as f32 + 0.5, y as f32 + 0.5);
        let jag = 1.0 + 0.35 * (noise((u - v) / 3.0, 0.0, seed) - 0.5);
        let inside = u / a + v / b;
        if inside < jag {
            let i = l.at(x, y);
            l.rgba[i + 3] = 0;
        } else if inside < jag + 2.5 / a.min(b) {
            l.mix(x, y, FIBRE, 0.8);
        }
    }
}

fn corner_box(l: &Label, which: u8, reach: u32) -> Vec<(u32, u32)> {
    let (rw, rh) = (reach.min(l.w), reach.min(l.h));
    let xs = if which.is_multiple_of(2) {
        0..rw
    } else {
        l.w - rw..l.w
    };
    let ys = if which < 2 { 0..rh } else { l.h - rh..l.h };
    ys.flat_map(|y| xs.clone().map(move |x| (x, y))).collect()
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (self.next() >> 40) as f32 / (1u64 << 24) as f32 * (hi - lo)
    }
}

fn lattice(x: i32, y: i32, seed: u64) -> f32 {
    let mut h = (x as u32 as u64) << 32 | y as u32 as u64;
    h ^= seed;
    h = (h ^ (h >> 33)).wrapping_mul(0xff51_afd7_ed55_8ccd);
    h = (h ^ (h >> 33)).wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    h ^= h >> 33;
    (h >> 40) as f32 / (1u64 << 24) as f32
}

fn noise(x: f32, y: f32, seed: u64) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let (xi, yi) = (x0 as i32, y0 as i32);
    let top = lattice(xi, yi, seed) * (1.0 - sx) + lattice(xi + 1, yi, seed) * sx;
    let bottom = lattice(xi, yi + 1, seed) * (1.0 - sx) + lattice(xi + 1, yi + 1, seed) * sx;
    top * (1.0 - sy) + bottom * sy
}
