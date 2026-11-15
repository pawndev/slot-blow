use slot_gfx::{SRC_H, SRC_W};
use slot_input::Btn;
use slot_store::Platform;

use crate::dust::Rng;
use crate::pixel_font as font;

pub const ODDS: u64 = 10;

pub const HEAT_S: f32 = 1.5;
pub const COOL_S: f32 = 0.9;
pub const MELT: f32 = 0.6;
pub const ZONE_W: f32 = 0.22;
pub const TOUCH: f32 = 0.12;
pub const BURN_COOL_S: f32 = 1.0;
pub const SWAP_S: f32 = 1.4;
pub const SETTLE_S: f32 = 1.0;

const TYPE_CPS: f32 = 45.0;
const IRON_SPEED: f32 = 5.0;
const SIZZLE_S: f32 = 0.3;

const BOX_H: i32 = 48;
const BOX_MARGIN: i32 = 2;
const TEXT_INSET: i32 = 8;
const LINE_PITCH: i32 = 16;
const ARROW_ROOM: i32 = 10;

pub const NOTICE: &str = "The internal battery has run dry.\n\
                          The game can be played.\n\
                          However, clock-based events will\n\
                          no longer occur.";
pub const FIXED: &str = "The new battery is soldered in.\n\
                         The game can be played.\n\
                         And clock-based events will\n\
                         occur again!";

pub struct Dry {
    rng: Option<Rng>,
    odds: u64,
}

impl Dry {
    pub fn clean() -> Self {
        Dry {
            rng: None,
            odds: ODDS,
        }
    }

    pub fn seeded(seed: u64) -> Self {
        Dry {
            rng: Some(Rng(seed)),
            odds: ODDS,
        }
    }

    pub fn always(seed: u64) -> Self {
        Dry {
            rng: Some(Rng(seed)),
            odds: 1,
        }
    }

    pub fn from_env(seed: u64) -> Self {
        match std::env::var("SLOT_DRY").as_deref() {
            Ok("always") => Dry::always(seed),
            Ok("off") => Dry::clean(),
            _ => Dry::seeded(seed),
        }
    }

    pub fn shaken() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64);
        Dry::from_env(seed.rotate_left(17))
    }

    pub fn roll(&mut self) -> Option<u64> {
        let rng = self.rng.as_mut()?;
        let dry = rng.next() % self.odds == 0;
        dry.then(|| rng.next())
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Stage {
    Notice,
    Desolder,
    Swap,
    Solder,
    Settle,
    Fixed,
    Done,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Joint {
    Old,
    Lifted,
    Bare,
    Cold,
    Burnt,
    Good,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Note {
    Cold,
    Burnt,
    Good,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Cue {
    Blip,
    Sizzle,
    Ding,
}

#[derive(Debug)]
pub struct Bench {
    platform: Platform,
    stage: Stage,
    joints: [Joint; 2],
    zones: [f32; 2],
    sel: usize,
    iron: f32,
    heat: f32,
    heating: bool,
    lockout: f32,
    note: Option<Note>,
    lines: Vec<String>,
    page: usize,
    shown: f32,
    t: f32,
    clock: f32,
    sizzled: f32,
    cue: Option<Cue>,
}

impl Bench {
    pub fn new(platform: Platform, seed: u64) -> Self {
        let mut rng = Rng(seed);
        let zones = [(); 2].map(|_| 0.5 + 0.18 * rng.unit());
        Bench {
            platform,
            stage: Stage::Notice,
            joints: [Joint::Old; 2],
            zones,
            sel: 0,
            iron: 0.0,
            heat: 0.0,
            heating: false,
            lockout: 0.0,
            note: None,
            lines: font::wrap(NOTICE, text_w(platform)),
            page: 0,
            shown: 0.0,
            t: 0.0,
            clock: 0.0,
            sizzled: 0.0,
            cue: None,
        }
    }

    pub fn platform(&self) -> Platform {
        self.platform
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }

    pub fn joints(&self) -> [Joint; 2] {
        self.joints
    }

    pub fn selected(&self) -> usize {
        self.sel
    }

    pub fn heat(&self) -> f32 {
        self.heat
    }

    pub fn zone(&self, joint: usize) -> (f32, f32) {
        let lo = self.zones[joint];
        (lo, lo + ZONE_W)
    }

    pub fn note(&self) -> Option<Note> {
        self.note
    }

    pub fn done(&self) -> bool {
        self.stage == Stage::Done
    }

    pub fn take_cue(&mut self) -> Option<Cue> {
        self.cue.take()
    }

    pub fn page(&self) -> &[String] {
        let from = (self.page * 2).min(self.lines.len());
        &self.lines[from..(from + 2).min(self.lines.len())]
    }

    fn pages(&self) -> usize {
        self.lines.len().div_ceil(2)
    }

    fn page_chars(&self) -> usize {
        self.page().iter().map(|l| l.chars().count()).sum()
    }

    fn typed(&self) -> bool {
        self.shown >= self.page_chars() as f32
    }

    fn arrived(&self) -> bool {
        (self.iron - self.sel as f32).abs() < 0.02
    }

    fn tell(&mut self, text: &str) {
        self.lines = font::wrap(text, text_w(self.platform));
        self.page = 0;
        self.shown = 0.0;
    }

    fn enter(&mut self, stage: Stage) {
        self.stage = stage;
        self.t = 0.0;
        self.heat = 0.0;
        self.heating = false;
        self.note = None;
    }

    pub fn press(&mut self, btn: Btn) {
        match (self.stage, btn) {
            (Stage::Notice | Stage::Fixed, Btn::A) => self.read_on(),
            (Stage::Desolder | Stage::Solder, Btn::A) => {
                let finished = match self.stage {
                    Stage::Desolder => Joint::Lifted,
                    _ => Joint::Good,
                };
                if self.lockout <= 0.0 && self.joints[self.sel] != finished {
                    self.heating = true;
                    self.note = None;
                }
            }
            (Stage::Desolder | Stage::Solder, Btn::Left) if !self.heating => self.sel = 0,
            (Stage::Desolder | Stage::Solder, Btn::Right) if !self.heating => self.sel = 1,
            _ => {}
        }
    }

    pub fn release(&mut self, btn: Btn) {
        if btn != Btn::A || !self.heating {
            return;
        }
        self.heating = false;
        if self.stage != Stage::Solder || self.heat < TOUCH {
            return;
        }
        let (lo, hi) = self.zone(self.sel);
        if self.heat < lo {
            self.joints[self.sel] = Joint::Cold;
            self.note = Some(Note::Cold);
            self.cue = Some(Cue::Blip);
        } else if self.heat <= hi {
            self.joints[self.sel] = Joint::Good;
            self.note = Some(Note::Good);
            self.cue = Some(Cue::Ding);
            self.next_joint(Joint::Good);
        } else {
            self.burn();
        }
    }

    fn burn(&mut self) {
        self.joints[self.sel] = Joint::Burnt;
        self.note = Some(Note::Burnt);
        self.heating = false;
        self.lockout = BURN_COOL_S;
        self.cue = Some(Cue::Sizzle);
    }

    fn next_joint(&mut self, finished: Joint) {
        if self.joints.iter().all(|j| *j == finished) {
            self.enter(match finished {
                Joint::Lifted => Stage::Swap,
                _ => Stage::Settle,
            });
            return;
        }
        let other = 1 - self.sel;
        if self.joints[other] != finished {
            self.sel = other;
        }
    }

    fn read_on(&mut self) {
        if !self.typed() {
            self.shown = self.page_chars() as f32;
            return;
        }
        self.cue = Some(Cue::Blip);
        if self.page + 1 < self.pages() {
            self.page += 1;
            self.shown = 0.0;
            return;
        }
        self.enter(match self.stage {
            Stage::Notice => Stage::Desolder,
            _ => Stage::Done,
        });
    }

    pub fn step(&mut self, dt: f32) {
        self.clock += dt;
        self.t += dt;
        match self.stage {
            Stage::Notice | Stage::Fixed => self.shown += dt * TYPE_CPS,
            Stage::Desolder | Stage::Solder => self.work(dt),
            Stage::Swap if self.t >= SWAP_S => {
                self.joints = [Joint::Bare; 2];
                self.sel = 0;
                self.enter(Stage::Solder);
            }
            Stage::Settle if self.t >= SETTLE_S => {
                self.tell(FIXED);
                self.enter(Stage::Fixed);
            }
            _ => {}
        }
    }

    fn work(&mut self, dt: f32) {
        let goal = self.sel as f32;
        let step = IRON_SPEED * dt;
        self.iron += (goal - self.iron).clamp(-step, step);
        self.lockout = (self.lockout - dt).max(0.0);
        let hot = self.heating && self.arrived();
        self.heat = match hot {
            true => self.heat + dt / HEAT_S,
            false => self.heat - dt / COOL_S,
        }
        .clamp(0.0, 1.0);
        self.sizzled += dt;
        if hot && self.heat > 0.25 && self.sizzled >= SIZZLE_S {
            self.sizzled = 0.0;
            self.cue = Some(Cue::Sizzle);
        }
        if !hot {
            return;
        }
        match self.stage {
            Stage::Desolder if self.heat >= MELT => {
                self.joints[self.sel] = Joint::Lifted;
                self.heating = false;
                self.cue = Some(Cue::Ding);
                self.next_joint(Joint::Lifted);
            }
            Stage::Solder if self.heat >= 1.0 => self.burn(),
            _ => {}
        }
    }

    pub fn status(&self) -> [&'static str; 2] {
        match (self.stage, self.note) {
            (_, Some(Note::Cold)) => ["Cold joint! All dull.", "Hold A a bit longer."],
            (_, Some(Note::Burnt)) => ["Too hot! The pad burnt.", "Let it cool, try again."],
            (_, Some(Note::Good)) => ["A nice shiny joint!", "Now the other tab."],
            (Stage::Desolder, _) => ["Desolder the old cell.", "Hold A on each tab."],
            (Stage::Swap, _) => ["Out with the old...", "...in with the new!"],
            (Stage::Solder, _) => ["Solder in the new cell.", "Let go in the green!"],
            (Stage::Settle, _) => ["Shiny! Nicely done.", ""],
            _ => ["", ""],
        }
    }
}

pub fn text_w(platform: Platform) -> usize {
    let (w, _) = platform.picture();
    (w as i32 - 2 * BOX_MARGIN - 2 * TEXT_INSET - ARROW_ROOM) as usize
}

type Rgb = [u8; 3];

const INK: Rgb = [0x60, 0x60, 0x60];
const INK_SHADOW: Rgb = [0xd0, 0xd0, 0xc8];
const PAPER: Rgb = [0xf8, 0xf8, 0xf8];
const ARROW: Rgb = [0xe0, 0x38, 0x38];
const ARROW_SHADOW: Rgb = [0xf8, 0xb0, 0xa8];
const SKY_TOP: Rgb = [0x78, 0xa0, 0xf8];
const SKY_BOTTOM: Rgb = [0x28, 0x50, 0xd8];
const GB_INK: Rgb = [0x10, 0x10, 0x10];

const BOARD: Rgb = [0x2a, 0x7a, 0x4a];
const TRACE: Rgb = [0x3a, 0x96, 0x5e];
const SILK: Rgb = [0xe8, 0xf0, 0xe8];
const GOLD: Rgb = [0xe8, 0xb8, 0x48];
const GOLD_DARK: Rgb = [0xa8, 0x78, 0x20];
const CHIP: Rgb = [0x22, 0x22, 0x26];
const PIN: Rgb = [0xb8, 0xb8, 0xb8];
const COPPER: Rgb = [0xd8, 0x98, 0x48];
const SCORCH: Rgb = [0x58, 0x38, 0x20];
const TAB: Rgb = [0xc0, 0xc0, 0xb8];
const TAB_EDGE: Rgb = [0x70, 0x70, 0x68];
const SOLDER: Rgb = [0xd8, 0xdc, 0xe4];
const SOLDER_RIM: Rgb = [0x90, 0x94, 0xa0];
const DULL: Rgb = [0x98, 0x98, 0x90];
const DULL_RIM: Rgb = [0x68, 0x68, 0x60];
const SOOT: Rgb = [0x48, 0x40, 0x38];
const OUTLINE: Rgb = [0x18, 0x18, 0x1c];
const TIP: Rgb = [0xa0, 0xa0, 0xa8];
const GLOW: Rgb = [0xff, 0x70, 0x20];
const STEEL: Rgb = [0xc8, 0xc8, 0xd0];
const COLLAR: Rgb = [0x50, 0x50, 0x58];
const GRIP: Rgb = [0xd8, 0x40, 0x30];
const GRIP_RIDGE: Rgb = [0x98, 0x28, 0x20];
const SMOKE: Rgb = [0xe8, 0xe8, 0xe8];
const SOOTY_SMOKE: Rgb = [0x88, 0x88, 0x88];
const GAUGE_TRACK: Rgb = [0x18, 0x30, 0x20];
const GAUGE_ZONE: Rgb = [0x50, 0xe0, 0x70];

struct Cell {
    base: Rgb,
    rim: Rgb,
    shine: Rgb,
    spots: bool,
}

const OLD_CELL: Cell = Cell {
    base: [0x9c, 0x98, 0x8c],
    rim: [0x6c, 0x68, 0x5c],
    shine: [0xbc, 0xb8, 0xac],
    spots: true,
};

const NEW_CELL: Cell = Cell {
    base: [0xc8, 0xcc, 0xd4],
    rim: [0x84, 0x88, 0x94],
    shine: [0xf4, 0xf6, 0xfa],
    spots: false,
};

struct Canvas<'a> {
    buf: &'a mut [u8],
    ox: i32,
    oy: i32,
    w: i32,
    h: i32,
}

impl Canvas<'_> {
    fn px(&mut self, x: i32, y: i32, c: Rgb) {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return;
        }
        let at = (((self.oy + y) * SRC_W as i32 + self.ox + x) * 4) as usize;
        self.buf[at] = c[2];
        self.buf[at + 1] = c[1];
        self.buf[at + 2] = c[0];
    }

    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgb) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.px(xx, yy, c);
            }
        }
    }

    fn disc(&mut self, cx: f32, cy: f32, r: f32, c: Rgb) {
        let reach = r.ceil() as i32 + 1;
        let (bx, by) = (cx.floor() as i32, cy.floor() as i32);
        for y in by - reach..=by + reach {
            for x in bx - reach..=bx + reach {
                let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                if dx * dx + dy * dy <= r * r {
                    self.px(x, y, c);
                }
            }
        }
    }

    fn text(&mut self, text: &str, x: i32, y: i32, ink: Rgb, shadow: Option<Rgb>) {
        if let Some(s) = shadow {
            for (dx, dy) in [(1, 0), (0, 1), (1, 1)] {
                font::draw(text, x + dx, y + dy, |px, py| self.px(px, py, s));
            }
        }
        font::draw(text, x, y, |px, py| self.px(px, py, ink));
    }
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    [0, 1, 2].map(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round() as u8)
}

fn modern(platform: Platform) -> bool {
    platform == Platform::Gba
}

pub fn paint(buf: &mut Vec<u8>, bench: &Bench) {
    let (sw, sh) = (SRC_W as usize, SRC_H as usize);
    buf.clear();
    buf.resize(sw * sh * 4, 0);
    let (w, h) = bench.platform.picture();
    let (w, h) = (w as i32, h as i32);
    let mut c = Canvas {
        buf: buf.as_mut_slice(),
        ox: (sw as i32 - w) / 2,
        oy: (sh as i32 - h) / 2,
        w,
        h,
    };
    match bench.stage {
        Stage::Notice | Stage::Fixed | Stage::Done => {
            backdrop(&mut c, bench.platform);
            let lines = bench.page();
            let shown = match bench.stage {
                Stage::Done => usize::MAX,
                _ => bench.shown as usize,
            };
            let arrow = bench.typed().then_some(bench.clock);
            text_box(&mut c, bench.platform, lines, shown, arrow);
        }
        _ => {
            workbench(&mut c, bench);
            let lines = bench.status().map(str::to_string);
            text_box(&mut c, bench.platform, &lines, usize::MAX, None);
        }
    }
    if bench.platform == Platform::Gb {
        greyed(buf, (sw as i32 - w) / 2, (sh as i32 - h) / 2, w, h);
    }
}

fn greyed(buf: &mut [u8], ox: i32, oy: i32, w: i32, h: i32) {
    const SHADES: [u8; 4] = [0x00, 0x55, 0xaa, 0xff];
    for y in oy..oy + h {
        for x in ox..ox + w {
            let at = ((y * SRC_W as i32 + x) * 4) as usize;
            let (b, g, r) = (buf[at] as u32, buf[at + 1] as u32, buf[at + 2] as u32);
            let luma = (r * 30 + g * 59 + b * 11) / 100;
            let s = SHADES[((luma * 4) / 256).min(3) as usize];
            buf[at..at + 3].fill(s);
        }
    }
}

fn backdrop(c: &mut Canvas, platform: Platform) {
    if !modern(platform) {
        c.rect(0, 0, c.w, c.h, PAPER);
        return;
    }
    for y in 0..c.h {
        let band = (y / 8 * 8) as f32 / c.h as f32;
        let colour = mix(SKY_TOP, SKY_BOTTOM, band);
        c.rect(0, y, c.w, 1, colour);
    }
}

fn text_box(
    c: &mut Canvas,
    platform: Platform,
    lines: &[String],
    shown: usize,
    arrow: Option<f32>,
) {
    let (x, y) = (BOX_MARGIN, c.h - BOX_H - BOX_MARGIN);
    let (w, h) = (c.w - 2 * BOX_MARGIN, BOX_H);
    let modern = modern(platform);
    if modern {
        let rings: [(i32, Rgb); 4] = [
            (0, [0x40, 0x48, 0x60]),
            (1, [0xa8, 0xb8, 0xc8]),
            (3, [0x60, 0x70, 0x88]),
            (4, PAPER),
        ];
        for (inset, colour) in rings {
            c.rect(x + inset, y + inset, w - 2 * inset, h - 2 * inset, colour);
        }
        for (cx, cy) in [
            (x, y),
            (x + w - 1, y),
            (x, y + h - 1),
            (x + w - 1, y + h - 1),
        ] {
            c.px(cx, cy, [0, 0, 0]);
        }
    } else {
        c.rect(x, y, w, h, GB_INK);
        c.rect(x + 1, y + 1, w - 2, h - 2, PAPER);
        c.rect(x + 2, y + 2, w - 4, h - 4, GB_INK);
        c.rect(x + 3, y + 3, w - 6, h - 6, PAPER);
    }
    let (ink, shadow) = match modern {
        true => (INK, Some(INK_SHADOW)),
        false => (GB_INK, None),
    };
    let mut left = shown;
    let mut end = (x + TEXT_INSET, y + TEXT_INSET);
    for (i, line) in lines.iter().enumerate() {
        let typed: String = line.chars().take(left).collect();
        left = left.saturating_sub(line.chars().count());
        let ly = y + TEXT_INSET + LINE_PITCH * i as i32;
        c.text(&typed, x + TEXT_INSET, ly, ink, shadow);
        end = (x + TEXT_INSET + font::width(&typed) as i32, ly);
    }
    if let Some(clock) = arrow {
        let bob = [0, 1, 2, 1][(clock * 6.0) as usize % 4];
        let (colour, shade) = match modern {
            true => (ARROW, Some(ARROW_SHADOW)),
            false => (GB_INK, None),
        };
        let (ax, ay) = (end.0 + 3, end.1 + 2 + bob);
        for (row, span) in [(0, 0..7), (1, 1..6), (2, 2..5), (3, 3..4)] {
            for dx in span {
                if let Some(s) = shade {
                    c.px(ax + dx + 1, ay + row + 1, s);
                }
                c.px(ax + dx, ay + row, colour);
            }
        }
    }
}

struct Layout {
    scene_h: i32,
    cx: f32,
    cy: f32,
    r: f32,
    pads: [(f32, f32); 2],
}

fn layout(c: &Canvas) -> Layout {
    let scene_h = c.h - BOX_H - BOX_MARGIN;
    let r = (scene_h as f32 * 0.24).round();
    let cx = (c.w as f32 * 0.6).round();
    let cy = (scene_h as f32 * 0.48).round();
    let reach = r + 14.0;
    Layout {
        scene_h,
        cx,
        cy,
        r,
        pads: [(cx - reach, cy), (cx + reach, cy)],
    }
}

fn workbench(c: &mut Canvas, bench: &Bench) {
    let l = layout(c);
    board(c, &l);
    let (fresh, dy) = cell_motion(bench, &l);
    for (i, &(px, py)) in l.pads.iter().enumerate() {
        if bench.joints[i] == Joint::Burnt {
            c.disc(px, py, 8.0, [0x3c, 0x4a, 0x2c]);
            c.disc(px, py, 6.5, SCORCH);
        }
        c.rect(px as i32 - 4, py as i32 - 4, 8, 8, COPPER);
    }
    cell(c, &l, fresh, dy);
    for i in 0..2 {
        let heated = i == bench.sel && bench.heat > 0.0;
        joint(c, bench, i, l.pads[i], heated, dy);
    }
    let lifted = matches!(bench.stage, Stage::Swap | Stage::Settle);
    let lift = if lifted { 26.0 } else { 0.0 };
    let (lx, ly) = l.pads[0];
    let (rx, _) = l.pads[1];
    let tip = (lx + (rx - lx) * bench.iron + 3.0, ly - 4.0 - lift);
    smoke(c, bench, tip);
    iron(c, tip, bench.heat);
    if matches!(bench.stage, Stage::Desolder | Stage::Solder) {
        gauge(c, bench, &l);
    }
}

fn board(c: &mut Canvas, l: &Layout) {
    c.rect(0, 0, c.w, l.scene_h, BOARD);
    let mut x = 6;
    while x + 4 < c.w - 4 {
        c.rect(x, 1, 4, 7, GOLD);
        c.rect(x, 7, 4, 1, GOLD_DARK);
        x += 6;
    }
    let chip_w = ((l.cx - l.r - 30.0) as i32 - 10).max(20);
    let chip = (10, 18, chip_w, 30);
    for i in 0..chip_w / 6 {
        let tx = chip.0 + 3 + i * 6;
        c.rect(tx, 8, 2, chip.1 - 8, TRACE);
    }
    let trace_y = (l.cy + l.r * 0.5) as i32 + 8;
    c.rect(
        chip.0 + chip_w,
        trace_y,
        (l.pads[0].0 as i32) - chip.0 - chip_w,
        2,
        TRACE,
    );
    c.rect(
        chip.0 + chip_w / 2,
        chip.1 + chip.3,
        2,
        trace_y - chip.1 - chip.3,
        TRACE,
    );
    c.rect(chip.0 + chip_w / 2, trace_y, chip_w / 2, 2, TRACE);
    c.rect(
        (l.pads[1].0) as i32,
        trace_y,
        2,
        l.scene_h - trace_y - 12,
        TRACE,
    );
    for i in 0..chip_w / 4 {
        let pin = chip.0 + 1 + i * 4;
        c.rect(pin, chip.1 - 2, 2, 2, PIN);
        c.rect(pin, chip.1 + chip.3, 2, 2, PIN);
    }
    c.rect(chip.0, chip.1, chip_w, chip.3, CHIP);
    c.disc(
        chip.0 as f32 + 4.0,
        chip.1 as f32 + 4.0,
        1.5,
        [0x40, 0x40, 0x48],
    );
    c.text("U1", chip.0, chip.1 + chip.3 + 4, SILK, None);
    let (bx, by) = (l.cx as i32 - 7, (l.cy - l.r) as i32 - 13);
    c.text("BT1", bx, by, SILK, None);
    let (lx, ly) = l.pads[0];
    let (rx, ry) = l.pads[1];
    c.text("+", lx as i32 - 2, ly as i32 + 7, SILK, None);
    c.text("-", rx as i32 - 1, ry as i32 + 8, SILK, None);
}

fn cell_motion(bench: &Bench, l: &Layout) -> (bool, f32) {
    let away = l.cy + l.r + 12.0;
    match bench.stage {
        Stage::Notice | Stage::Desolder => (false, 0.0),
        Stage::Swap => {
            let t = bench.t;
            if t < 0.6 {
                (false, -(t / 0.6).powi(2) * away)
            } else if t < 0.8 {
                (true, -away)
            } else {
                let k = 1.0 - ((t - 0.8) / (SWAP_S - 0.8)).min(1.0);
                (true, -k * k * away)
            }
        }
        _ => (true, 0.0),
    }
}

fn cell(c: &mut Canvas, l: &Layout, fresh: bool, dy: f32) {
    let look = if fresh { &NEW_CELL } else { &OLD_CELL };
    let (cx, cy, r) = (l.cx, l.cy + dy, l.r);
    let tab_h = 6;
    let (lx, _) = l.pads[0];
    let (rx, _) = l.pads[1];
    let right = ((cx + r * 0.4) as i32, (rx + 3.0) as i32);
    c.rect(
        right.0,
        cy as i32 - tab_h / 2 - 1,
        right.1 - right.0 + 1,
        tab_h + 2,
        TAB_EDGE,
    );
    c.rect(
        right.0,
        cy as i32 - tab_h / 2,
        right.1 - right.0,
        tab_h,
        TAB,
    );
    c.disc(cx, cy, r, look.rim);
    c.disc(cx, cy, r - 1.5, look.base);
    let reach = r as i32;
    for y in -reach..=reach {
        for x in -reach..=reach {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let d = (fx * fx + fy * fy).sqrt();
            if d > r * 0.55 && d < r * 0.72 && fx + fy < -r * 0.5 {
                c.px(cx as i32 + x, cy as i32 + y, look.shine);
            }
        }
    }
    if look.spots {
        for (sx, sy) in [(0.3, 0.35), (0.42, 0.1), (-0.2, 0.55), (0.05, 0.62)] {
            c.disc(cx + sx * r, cy + sy * r, 1.2, [0x8a, 0x5c, 0x30]);
        }
    }
    let label = "CR2025";
    let tw = font::width(label) as f32;
    c.text(label, (cx - tw / 2.0) as i32, cy as i32 + 5, look.rim, None);
    let left = ((lx - 3.0) as i32, (cx - r * 0.45) as i32);
    c.rect(
        left.0 - 1,
        cy as i32 - tab_h / 2 - 1,
        left.1 - left.0 + 2,
        tab_h + 2,
        TAB_EDGE,
    );
    c.rect(left.0, cy as i32 - tab_h / 2, left.1 - left.0, tab_h, TAB);
}

fn joint(c: &mut Canvas, bench: &Bench, i: usize, (px, py): (f32, f32), heated: bool, dy: f32) {
    let state = bench.joints[i];
    let melting = heated && bench.heating;
    match (state, bench.stage) {
        (Joint::Old, _) if dy == 0.0 => {
            let shine = if heated { bench.heat / MELT } else { 0.0 };
            c.disc(px, py, 3.8, mix(DULL_RIM, SOLDER_RIM, shine));
            c.disc(px, py, 2.8, mix(DULL, SOLDER, shine));
        }
        (Joint::Bare | Joint::Cold | Joint::Burnt, Stage::Solder) if melting => {
            let (lo, _) = bench.zone(i);
            let r = 1.0 + 2.8 * (bench.heat / lo).min(1.0);
            c.disc(px, py, r + 1.0, SOLDER_RIM);
            c.disc(px, py, r, SOLDER);
            c.px(px as i32 - 1, py as i32 - 1, [0xff; 3]);
        }
        (Joint::Cold, _) => {
            c.disc(px, py, 3.8, DULL_RIM);
            c.disc(px, py, 2.8, DULL);
            for (dx, dy) in [(-1, 0), (1, 1), (0, -2), (2, -1), (-2, 1)] {
                c.px(px as i32 + dx, py as i32 + dy, DULL_RIM);
            }
        }
        (Joint::Burnt, _) => {
            c.disc(px, py, 3.2, SOOT);
            c.px(px as i32, py as i32 - 1, [0x70, 0x60, 0x50]);
        }
        (Joint::Good, _) => {
            c.disc(px, py, 3.8, SOLDER_RIM);
            c.disc(px, py, 2.8, SOLDER);
            c.px(px as i32 - 1, py as i32 - 1, [0xff; 3]);
        }
        _ => {}
    }
}

fn iron(c: &mut Canvas, (tx, ty): (f32, f32), heat: f32) {
    let (dx, dy) = (0.55f32, -0.83f32);
    let parts: [(f32, f32, f32, Rgb); 4] = [
        (0.0, 6.0, 1.0, mix(TIP, GLOW, heat)),
        (6.0, 16.0, 1.5, STEEL),
        (16.0, 21.0, 2.5, COLLAR),
        (21.0, 90.0, 3.5, GRIP),
    ];
    for outline in [true, false] {
        for &(from, to, r, colour) in &parts {
            let mut s = from;
            while s <= to {
                let (x, y) = (tx + dx * s, ty + dy * s);
                match outline {
                    true => c.disc(x, y, r + 1.0, OUTLINE),
                    false => {
                        let ridge = from >= 21.0 && (s as i32 / 4) % 2 == 1;
                        c.disc(x, y, r, if ridge { GRIP_RIDGE } else { colour });
                    }
                }
                s += 0.5;
            }
        }
    }
}

fn smoke(c: &mut Canvas, bench: &Bench, (tx, ty): (f32, f32)) {
    let sooty = bench.lockout > 0.0;
    let fuming = (bench.heating && bench.heat > 0.3) || sooty;
    if !fuming {
        return;
    }
    let colour = if sooty { SOOTY_SMOKE } else { SMOKE };
    let puffs = if sooty { 7 } else { 4 };
    for i in 0..puffs {
        let phase = (bench.clock * 0.9 + i as f32 / puffs as f32).fract();
        let x = tx - 4.0 + (phase * 6.0 + i as f32).sin() * 3.0 - phase * 8.0;
        let y = ty - 3.0 - phase * 34.0;
        let r = 1.5 + phase * 3.5;
        let reach = r.ceil() as i32;
        for yy in -reach..=reach {
            for xx in -reach..=reach {
                let (px, py) = (x as i32 + xx, y as i32 + yy);
                let thin = phase > 0.55 && (px + py) % 2 == 0;
                if (xx * xx + yy * yy) as f32 <= r * r && !thin {
                    c.px(px, py, colour);
                }
            }
        }
    }
}

fn gauge(c: &mut Canvas, bench: &Bench, l: &Layout) {
    let (x, y, w, h) = (8, l.scene_h - 10, c.w - 16, 6);
    c.rect(x - 1, y - 1, w + 2, h + 2, OUTLINE);
    c.rect(x, y, w, h, GAUGE_TRACK);
    let (lo, hi) = match bench.stage {
        Stage::Desolder => (MELT, 1.0),
        _ => bench.zone(bench.sel),
    };
    let at = |v: f32| x + (v * w as f32).round() as i32;
    c.rect(at(lo), y, at(hi) - at(lo), h, GAUGE_ZONE);
    let fill = at(bench.heat) - x;
    let hot = mix([0xf8, 0xe0, 0x40], [0xf0, 0x30, 0x20], bench.heat);
    c.rect(x, y + 1, fill, h - 2, hot);
    if fill > 0 {
        c.rect(x + fill - 1, y - 2, 2, h + 4, [0xff; 3]);
    }
}
