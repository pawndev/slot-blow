const INSERT: &[u8] = include_bytes!("../../assets/insert.pcm");
const EJECT: &[u8] = include_bytes!("../../assets/eject.pcm");
#[cfg(feature = "nds")]
const DS_INSERT: &[u8] = include_bytes!("../../assets/ds_insert.pcm");
#[cfg(feature = "nds")]
const DS_EJECT: &[u8] = include_bytes!("../../assets/ds_eject.pcm");
const PRINT_BUZZ: &[u8] = include_bytes!("../../../../assets/server/buzz.pcm");
const TEAR: &[u8] = include_bytes!("../../../../assets/server/tear.pcm");
const ASSET_HZ: f32 = 48_000.0;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Sfx {
    Insert,
    Eject,
    #[cfg(feature = "nds")]
    DsInsert,
    #[cfg(feature = "nds")]
    DsEject,
    PrintBuzz,
    Tear,
    Blow,
    Blip,
    Sizzle,
    Ding,
}

const BLOW_S: f32 = 0.32;
const BLOW_GAIN: f32 = 9000.0;

const BLIP: &[(f32, f32)] = &[(1760.0, 0.05)];
const DING: &[(f32, f32)] = &[(1318.5, 0.07), (1760.0, 0.2)];
const TONE_GAIN: f32 = 4200.0;
const SIZZLE_S: f32 = 0.28;
const SIZZLE_GAIN: f32 = 7000.0;

impl Sfx {
    pub fn lead(self) -> f32 {
        match self {
            Sfx::Insert => 0.097,
            Sfx::Eject => 0.021,
            #[cfg(feature = "nds")]
            Sfx::DsInsert => 0.541,
            #[cfg(feature = "nds")]
            Sfx::DsEject => 0.011,
            Sfx::PrintBuzz | Sfx::Tear | Sfx::Blow | Sfx::Blip | Sfx::Sizzle | Sfx::Ding => 0.0,
        }
    }

    fn pcm(self) -> &'static [u8] {
        match self {
            Sfx::Insert => INSERT,
            Sfx::Eject => EJECT,
            #[cfg(feature = "nds")]
            Sfx::DsInsert => DS_INSERT,
            #[cfg(feature = "nds")]
            Sfx::DsEject => DS_EJECT,
            Sfx::PrintBuzz => PRINT_BUZZ,
            Sfx::Tear => TEAR,
            Sfx::Blow | Sfx::Blip | Sfx::Sizzle | Sfx::Ding => &[],
        }
    }

    fn level(self) -> f32 {
        match self {
            Sfx::PrintBuzz => 0.40,
            Sfx::Tear => 0.45,
            _ => 1.0,
        }
    }

    pub fn tail(self) -> f32 {
        match self {
            Sfx::Blow => BLOW_S,
            Sfx::Blip => length(BLIP),
            Sfx::Sizzle => SIZZLE_S,
            Sfx::Ding => length(DING),
            _ => self.pcm().len() as f32 / 2.0 / ASSET_HZ - self.lead(),
        }
    }

    pub fn render(self, sample_rate: u32) -> Vec<i16> {
        match self {
            Sfx::Blow => return breath(sample_rate),
            Sfx::Blip => return tones(sample_rate, BLIP),
            Sfx::Sizzle => return sizzle(sample_rate),
            Sfx::Ding => return tones(sample_rate, DING),
            _ => {}
        }
        let pcm = self.pcm();
        let level = self.level();
        let mut out = Vec::with_capacity(pcm.len());
        for v in resampled(pcm, sample_rate) {
            let s = (v * level).clamp(-32768.0, 32767.0) as i16;
            out.push(s);
            out.push(s);
        }
        out
    }
}

fn breath(sample_rate: u32) -> Vec<i16> {
    let rate = sample_rate as f32;
    let n = (BLOW_S * rate) as usize;
    let mut out = Vec::with_capacity(n * 2);
    let mut seed: u32 = 0x1234_5678;
    let (mut low, mut slow) = (0.0f32, 0.0f32);
    let k = 1.0 - (-std::f32::consts::TAU * 2200.0 / rate).exp();
    let k_slow = 1.0 - (-std::f32::consts::TAU * 180.0 / rate).exp();
    for i in 0..n {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let white = seed as f32 / u32::MAX as f32 * 2.0 - 1.0;
        low += k * (white - low);
        slow += k_slow * (low - slow);
        let t = i as f32 / rate;
        let env = (t / 0.04).min(1.0) * ((BLOW_S - t) / 0.12).clamp(0.0, 1.0);
        let s = ((low - slow) * BLOW_GAIN * env) as i16;
        out.push(s);
        out.push(s);
    }
    out
}

fn length(notes: &[(f32, f32)]) -> f32 {
    notes.iter().map(|(_, s)| s).sum()
}

fn tones(sample_rate: u32, notes: &[(f32, f32)]) -> Vec<i16> {
    let rate = sample_rate as f32;
    let mut out = Vec::with_capacity((length(notes) * rate) as usize * 2);
    for &(hz, secs) in notes {
        let n = (secs * rate) as usize;
        for i in 0..n {
            let t = i as f32 / rate;
            let square = if (t * hz).fract() < 0.5 { 1.0 } else { -1.0 };
            let env = (t / 0.004).min(1.0) * (1.0 - t / secs).powi(2);
            let s = (square * TONE_GAIN * env) as i16;
            out.push(s);
            out.push(s);
        }
    }
    out
}

fn sizzle(sample_rate: u32) -> Vec<i16> {
    let rate = sample_rate as f32;
    let n = (SIZZLE_S * rate) as usize;
    let mut out = Vec::with_capacity(n * 2);
    let mut seed: u32 = 0x9e37_79b9;
    let mut low = 0.0f32;
    let mut crackle = 0.0f32;
    let k = 1.0 - (-std::f32::consts::TAU * 3500.0 / rate).exp();
    let decay = (-1.0 / (0.004 * rate)).exp();
    for i in 0..n {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let white = seed as f32 / u32::MAX as f32 * 2.0 - 1.0;
        low += k * (white - low);
        if seed.is_multiple_of(900) {
            crackle = 1.0;
        }
        crackle *= decay;
        let t = i as f32 / rate;
        let env = (t / 0.02).min(1.0) * ((SIZZLE_S - t) / 0.1).clamp(0.0, 1.0);
        let s = ((white - low) * (0.35 + 0.65 * crackle) * SIZZLE_GAIN * env) as i16;
        out.push(s);
        out.push(s);
    }
    out
}

fn resampled(pcm: &[u8], sample_rate: u32) -> Vec<f32> {
    let src: Vec<f32> = pcm
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32)
        .collect();
    if sample_rate == ASSET_HZ as u32 || src.is_empty() {
        return src;
    }
    let ratio = sample_rate as f32 / ASSET_HZ;
    let n = (src.len() as f32 * ratio) as usize;
    (0..n)
        .map(|i| {
            let x = i as f32 / ratio;
            let a = x as usize;
            let f = x - a as f32;
            let b = (a + 1).min(src.len() - 1);
            src[a] * (1.0 - f) + src[b] * f
        })
        .collect()
}
