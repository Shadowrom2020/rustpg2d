//! The example's music: eight looping tracks - *Market Day* (the village), *Whisperwood* (the
//! forest), *The Copper Kettle* (the tavern), *Steel in the Woods* (battles), *The Old Mine*
//! (dungeons and caves), *Lanterns at Night* (towns at night), *The Open Road* (travel, the
//! wilds) and *Grumbold's Wrath* (a boss fight) - written in a small note notation and rendered by a tiny synthesizer: plucked
//! strings (Karplus-Strong: lute, harp), a recorder, a bowed drone, a tabor, a shaker and
//! bells, mixed in stereo through a Schroeder reverb, then encoded as Ogg Vorbis.
//!
//! Every track loops seamlessly: it is rendered with a tail that is folded back onto its start.

use std::num::{NonZeroU32, NonZeroU8};

use vorbis_rs::{VorbisBitrateManagementStrategy, VorbisEncoderBuilder};

pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
    pub fn range(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

pub const SR: u32 = 44100;
const SRF: f32 = SR as f32;

/// The tracks: (sound id, title).
pub const TRACKS: [(&str, &str); 8] = [
    ("music_village", "Market Day"),
    ("music_forest", "Whisperwood"),
    ("music_tavern", "The Copper Kettle"),
    ("music_battle", "Steel in the Woods"),
    ("music_dungeon", "The Old Mine"),
    ("music_night", "Lanterns at Night"),
    ("music_travel", "The Open Road"),
    ("music_boss", "Grumbold's Wrath"),
];

/// Renders a track (stereo samples, -1..1).
pub fn render(id: &str) -> (Vec<f32>, Vec<f32>) {
    match id {
        "music_village" => village(),
        "music_forest" => forest(),
        "music_tavern" => tavern(),
        "music_battle" => battle(),
        "music_dungeon" => dungeon(),
        "music_night" => night(),
        "music_travel" => travel(),
        "music_boss" => boss(),
        _ => panic!("no track {id}"),
    }
}

/// Encodes stereo samples as Ogg Vorbis (quality 0.3 ~ 100 kbit/s). The Ogg stream serial is
/// fixed (from the id), so regenerating gives byte-identical files.
pub fn ogg(id: &str, l: &[f32], r: &[f32]) -> Vec<u8> {
    let serial = id.bytes().fold(0x811c9dc5u32, |h, b| (h ^ b as u32).wrapping_mul(0x01000193)) as i32;
    let mut out = Vec::new();
    let mut enc = VorbisEncoderBuilder::new_with_serial(NonZeroU32::new(SR).unwrap(), NonZeroU8::new(2).unwrap(), &mut out, serial)
        .bitrate_management_strategy(VorbisBitrateManagementStrategy::QualityVbr { target_quality: 0.3 })
        .build()
        .unwrap();
    enc.encode_audio_block([l, r]).unwrap();
    enc.finish().unwrap();
    out
}

// ------------------------------------------------------------------ notation

fn hz(midi: i32) -> f32 {
    440.0 * 2f32.powf((midi - 69) as f32 / 12.0)
}

/// Notes from "D5/2 F#5 r/3": a name with octave (sharps `#`, flats `b`) or `r` for a rest,
/// `/length` in beats (default 1).
fn notes(s: &str) -> Vec<(Option<i32>, f32)> {
    s.split_whitespace()
        .map(|tok| {
            let (name, len) = tok.split_once('/').unwrap_or((tok, "1"));
            let len: f32 = len.parse().unwrap_or_else(|_| panic!("bad length in {tok}"));
            if name == "r" {
                return (None, len);
            }
            let mut c = name.chars();
            let mut semi = match c.next() {
                Some('C') => 0,
                Some('D') => 2,
                Some('E') => 4,
                Some('F') => 5,
                Some('G') => 7,
                Some('A') => 9,
                Some('B') => 11,
                _ => panic!("bad note {tok}"),
            };
            let rest: String = c.collect();
            let oct = rest.trim_start_matches(['#', 'b']);
            semi += rest.matches('#').count() as i32 - rest.matches('b').count() as i32;
            let oct: i32 = oct.parse().unwrap_or_else(|_| panic!("bad octave in {tok}"));
            (Some((oct + 1) * 12 + semi), len)
        })
        .collect()
}

/// Root (the first pitch at or above `low`) and third (3 = minor, 4 = major) of a chord name
/// like "G", "Am".
fn chord(name: &str, low: i32) -> (i32, i32) {
    let (root, minor) = match name.strip_suffix('m') {
        Some(r) => (r, true),
        None => (name, false),
    };
    let (root, acc) = match (root.strip_suffix('b'), root.strip_suffix('#')) {
        (Some(r), _) if !r.is_empty() => (r, -1),
        (_, Some(r)) => (r, 1),
        _ => (root, 0),
    };
    let pc = acc + match root {
        "C" => 0,
        "D" => 2,
        "E" => 4,
        "F" => 5,
        "G" => 7,
        "A" => 9,
        "B" => 11,
        _ => panic!("bad chord {name}"),
    };
    (low + (pc - low).rem_euclid(12), if minor { 3 } else { 4 })
}

/// The scale note `steps` degrees below `midi` (a harmony voice in thirds).
fn below(midi: i32, scale: &[i32], steps: usize) -> i32 {
    let mut n = midi;
    let mut left = steps;
    while left > 0 {
        n -= 1;
        if scale.contains(&n.rem_euclid(12)) {
            left -= 1;
        }
    }
    n
}

// ------------------------------------------------------------------ instruments

fn env_ar(t: f32, len: f32, attack: f32, release: f32) -> f32 {
    let a = (t / attack).min(1.0);
    let r = ((len - t) / release).clamp(0.0, 1.0);
    a * a * (3.0 - 2.0 * a) * r
}

/// A plucked string (Karplus-Strong): `decay` is the amplitude kept per period (0.99 =
/// lute, 0.998 = harp), `soft` how dull the pluck is.
fn pluck(rng: &mut Rng, f: f32, secs: f32, decay: f32, soft: u32) -> Vec<f32> {
    let d = SRF / f - 0.5; // the two-point average adds half a sample of delay
    let n = d.ceil() as usize + 2;
    let total = (secs * SRF) as usize + n;
    let mut h: Vec<f32> = (0..n).map(|_| rng.range(2001) as f32 / 1000.0 - 1.0).collect();
    for _ in 0..soft {
        for i in 1..n {
            h[i] = 0.5 * (h[i] + h[i - 1]);
        }
    }
    let mean = h.iter().sum::<f32>() / n as f32;
    h.iter_mut().for_each(|v| *v -= mean);
    h.reserve(total);
    for i in n..total {
        let pos = i as f32 - d;
        let k = pos.floor() as usize;
        let fr = pos - k as f32;
        let a = h[k] * (1.0 - fr) + h[k + 1] * fr;
        let b = h[k - 1] * (1.0 - fr) + h[k] * fr;
        h.push(decay * 0.5 * (a + b));
    }
    let out = &h[n..];
    let fade = (0.04 * SRF) as usize;
    out.iter().enumerate().map(|(i, &v)| v * ((out.len() - i) as f32 / fade as f32).min(1.0)).collect()
}

/// A recorder: a soft sine-ish tone with a breathy onset, delayed vibrato and a little air.
fn recorder(rng: &mut Rng, f: f32, secs: f32) -> Vec<f32> {
    let n = (secs * SRF) as usize;
    let mut phase = 0.0f32;
    let (mut air, mut air2) = (0.0f32, 0.0f32);
    (0..n)
        .map(|i| {
            let t = i as f32 / SRF;
            let vib = 1.0 + 0.005 * (t * 5.3 * std::f32::consts::TAU).sin() * ((t - 0.18) / 0.3).clamp(0.0, 1.0);
            let bend = 1.0 - 0.012 * (-t / 0.025).exp();
            phase = (phase + f * vib * bend / SRF).fract();
            let p = phase * std::f32::consts::TAU;
            let env = env_ar(t, secs, 0.05, 0.09);
            let tone = p.sin() + 0.28 * env * (2.0 * p).sin() + 0.1 * env * (3.0 * p).sin() + 0.04 * (4.0 * p).sin();
            // Breath: noise through two low-passes (the hiss stays well under the tone).
            air = air * 0.96 + (rng.range(2001) as f32 / 1000.0 - 1.0) * 0.04;
            air2 = air2 * 0.96 + air * 0.04;
            (tone * 0.8 + air2 * 0.9) * env
        })
        .collect()
}

/// A bowed string / hurdy-gurdy drone: two detuned saws through a low-pass with a slow swell.
fn bowed(f: f32, secs: f32, attack: f32, release: f32) -> Vec<f32> {
    let n = (secs * SRF) as usize;
    let (mut p1, mut p2, mut lp) = (0.0f32, 0.0f32, 0.0f32);
    let alpha = 1.0 - (-std::f32::consts::TAU * 2.5 * f / SRF).exp();
    (0..n)
        .map(|i| {
            let t = i as f32 / SRF;
            p1 = (p1 + f * 1.003 / SRF).fract();
            p2 = (p2 + f * 0.997 / SRF).fract();
            let saw = (p1 - 0.5) + (p2 - 0.5);
            lp += alpha * (saw - lp);
            let trem = 1.0 - 0.06 * (t * 4.1 * std::f32::consts::TAU).sin();
            lp * trem * env_ar(t, secs, attack, release)
        })
        .collect()
}

/// A frame drum: a falling sine thump with a little skin noise.
fn tabor(rng: &mut Rng) -> Vec<f32> {
    let n = (0.3 * SRF) as usize;
    let mut phase = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / SRF;
            let f = 55.0 + 90.0 * (-t / 0.04).exp();
            phase = (phase + f / SRF).fract();
            let noise = (rng.range(2001) as f32 / 1000.0 - 1.0) * (-t / 0.012).exp() * 0.5;
            ((phase * std::f32::consts::TAU).sin() + noise) * (-t * 9.0).exp() * (t / 0.002).min(1.0)
        })
        .collect()
}

/// A shaker: a short burst of bright noise.
fn shaker(rng: &mut Rng) -> Vec<f32> {
    let n = (0.08 * SRF) as usize;
    let mut last = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / SRF;
            let v = rng.range(2001) as f32 / 1000.0 - 1.0;
            let hp = v - last;
            last = v;
            hp * 0.5 * (-t * 45.0).exp() * (t / 0.004).min(1.0)
        })
        .collect()
}

/// A small bell: inharmonic partials with their own decays.
fn bell(f: f32, secs: f32) -> Vec<f32> {
    const PARTIALS: [(f32, f32, f32); 5] = [(1.0, 1.0, 1.1), (2.0, 0.5, 1.6), (2.4, 0.35, 2.2), (3.01, 0.2, 2.8), (4.5, 0.1, 4.0)];
    let n = (secs * SRF) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / SRF;
            let v: f32 = PARTIALS.iter().map(|(r, a, d)| a * (t * f * r * std::f32::consts::TAU).sin() * (-t * d).exp()).sum();
            v * 0.45 * (t / 0.003).min(1.0) * ((secs - t) / 0.3).clamp(0.0, 1.0)
        })
        .collect()
}

// ------------------------------------------------------------------ mixing

/// Frames macroquad's mixer fills per pass (`audio::MIX_BUFFER_FRAMES`).
const MIX_BUFFER: usize = 4096;

/// The beat length nearest `beat` (rounded up) that makes `beats` beats a whole number of mixer
/// buffers: the engine then plays the loop as it is, without stretching it.
fn whole_buffers(beat: f32, beats: usize) -> f32 {
    let frames = (beat as f64 * beats as f64 * SR as f64 / MIX_BUFFER as f64).ceil() * MIX_BUFFER as f64;
    (frames / SR as f64 / beats as f64) as f32
}

/// A stereo mix of a loop: the loop plus a tail that is folded onto its start at the end.
struct Mix {
    len: usize,
    l: Vec<f32>,
    r: Vec<f32>,
    /// Reverb input.
    send: Vec<f32>,
    rng: Rng,
}

impl Mix {
    fn new(loop_secs: f32, tail_secs: f32) -> Mix {
        // Whole mixer buffers (`whole_buffers` makes the loop that long up to rounding).
        let len = ((loop_secs * SRF / MIX_BUFFER as f32).round() as usize).max(1) * MIX_BUFFER;
        let total = len + (tail_secs * SRF) as usize;
        Mix { len, l: vec![0.0; total], r: vec![0.0; total], send: vec![0.0; total], rng: Rng(0x5eed) }
    }

    /// Adds a sound at `at` seconds: `pan` -1 (left) .. 1 (right), `verb` = reverb send.
    fn add(&mut self, at: f32, s: &[f32], vol: f32, pan: f32, verb: f32) {
        let start = (at.max(0.0) * SRF) as usize;
        let a = (pan + 1.0) * std::f32::consts::FRAC_PI_4;
        let (gl, gr) = (a.cos() * vol, a.sin() * vol);
        for (i, &v) in s.iter().enumerate() {
            if let Some(k) = start.checked_add(i).filter(|k| *k < self.l.len()) {
                self.l[k] += v * gl;
                self.r[k] += v * gr;
                self.send[k] += v * vol * verb;
            }
        }
    }

    /// ±`ms` of timing looseness.
    fn loose(&mut self, ms: f32) -> f32 {
        (self.rng.range(2001) as f32 / 1000.0 - 1.0) * ms / 1000.0
    }

    /// Reverb, fold the tail back, normalize.
    fn finish(mut self, wet: f32, decay: f32) -> (Vec<f32>, Vec<f32>) {
        let (wl, wr) = (reverb(&self.send, decay, 0), reverb(&self.send, decay, 29));
        for i in 0..self.l.len() {
            self.l[i] += wet * wl[i];
            self.r[i] += wet * wr[i];
        }
        let len = self.len;
        for i in 0..self.l.len() - len {
            self.l[i] += self.l[len + i];
            self.r[i] += self.r[len + i];
        }
        self.l.truncate(len);
        self.r.truncate(len);
        let peak = self.l.iter().chain(&self.r).fold(0.0f32, |m, v| m.max(v.abs())).max(1e-6);
        let g = 0.9 / peak;
        self.l.iter_mut().chain(&mut self.r).for_each(|v| *v *= g);
        (self.l, self.r)
    }
}

/// Schroeder reverb: four damped combs in parallel, two all-passes in series. `shift` detunes
/// the delays (a different value per channel gives width).
fn reverb(input: &[f32], decay: f32, shift: usize) -> Vec<f32> {
    const COMBS: [usize; 4] = [1557, 1617, 1491, 1422];
    const ALLPASS: [usize; 2] = [225, 556];
    let mut out = vec![0.0f32; input.len()];
    for d in COMBS {
        let d = d + shift;
        let mut buf = vec![0.0f32; d];
        let mut lp = 0.0f32;
        for (i, x) in input.iter().enumerate() {
            let k = i % d;
            let y = buf[k];
            lp = lp * 0.35 + y * 0.65;
            buf[k] = x + lp * decay;
            out[i] += y * 0.25;
        }
    }
    for d in ALLPASS {
        let mut buf = vec![0.0f32; d];
        for (i, o) in out.iter_mut().enumerate() {
            let k = i % d;
            let v = *o + buf[k] * 0.5;
            *o = buf[k] - 0.5 * v;
            buf[k] = v;
        }
    }
    out
}

// ------------------------------------------------------------------ the tracks

/// *Market Day*: a lilting 6/8 dance in D Mixolydian - lute, recorder, tabor and shaker.
/// Intro (lute) - A - A - B (harmony in thirds, strums) - A.
fn village() -> (Vec<f32>, Vec<f32>) {
    const SCALE: [i32; 7] = [2, 4, 6, 7, 9, 11, 0]; // D Mixolydian
    let a_chords = ["D", "D", "G", "D", "D", "C", "G", "D"];
    let b_chords = ["Bm", "G", "D", "Am", "Bm", "G", "C", "D"];
    let chords: Vec<&str> = [&["D", "D"][..], &a_chords, &a_chords, &b_chords, &a_chords].concat();
    let bars = chords.len();
    // An eighth: dotted quarter = 100 (a hair slower, so the loop fills whole mixer buffers).
    let unit = whole_buffers(0.2, bars * 6);
    let mut m = Mix::new(bars as f32 * 6.0 * unit, 3.0);
    let bar_at = |bar: usize| bar as f32 * 6.0 * unit;

    // Lute: a rolling pattern, every eighth, with the first and fourth beats stressed.
    let pattern = |t: i32| [0, 7, 12, 12 + t, 19, 12 + t];
    let stress = [1.0, 0.5, 0.65, 0.85, 0.5, 0.65];
    for (bar, c) in chords.iter().enumerate() {
        let (r, t) = chord(c, 43);
        for (k, iv) in pattern(t).iter().enumerate() {
            let at = bar_at(bar) + k as f32 * unit + m.loose(4.0);
            let s = pluck(&mut m.rng, hz(r + iv), 0.8, 0.994, 2);
            m.add(at, &s, 0.42 * stress[k], -0.35, 0.3);
        }
    }
    // Strums (a brighter instrument) on the first and fourth beats of the B part.
    for (bar, c) in chords.iter().enumerate().take(26).skip(18) {
        let (r, t) = chord(c, 43);
        for beat in [0.0, 3.0] {
            for (k, iv) in [12, 12 + t, 19, 24].iter().enumerate() {
                let s = pluck(&mut m.rng, hz(r + iv), 1.0, 0.997, 1);
                m.add(bar_at(bar) + beat * unit + k as f32 * 0.014, &s, 0.2, 0.4, 0.45);
            }
        }
    }
    // Recorder: the tune, A twice (the second leading into B), B, A to the end.
    let a = ["A4 D5 F#5 A5/2 G5", "F#5/2 E5 D5/3", "G5 A5 B5 D6/2 B5", "A5/2 F#5 D5/3", "A4 D5 F#5 A5/2 B5", "C6/2 B5 G5/3", "A5 G5 F#5 G5/2 E5"];
    let b = ["F#5/2 D5 B5/3", "B5 A5 G5 B5/2 D6", "A5/2 F#5 D5/3", "E5 C6 E6 C6/2 A5", "F#5/2 B5 D6/3", "B5 D6 G5 B5/2 A5", "G5/2 E5 C6/3", "A5/3 F#5/2 E5"];
    let tune = [a.join(" "), "D5/5 r".into(), a.join(" "), "D5/3 E5 F#5 A5".into(), b.join(" "), a.join(" "), "D5/4 r/2".into()].join(" ");
    let mut at = bar_at(2);
    let mut beats = 0.0;
    for (n, len) in notes(&tune) {
        if let Some(n) = n {
            let secs = len * unit - 0.03;
            let s = recorder(&mut m.rng, hz(n), secs);
            m.add(at, &s, 0.26, 0.15, 0.5);
            // The B part gets a second recorder a third below.
            if (18.0 * 6.0..26.0 * 6.0).contains(&beats) {
                let s = recorder(&mut m.rng, hz(below(n, &SCALE, 2)), secs);
                m.add(at + 0.01, &s, 0.15, -0.1, 0.5);
            }
        }
        at += len * unit;
        beats += len;
    }
    assert_eq!(beats, (bars - 2) as f32 * 6.0, "the tune must fill the bars after the intro");
    // Tabor from the second A on (strong on 1, lighter on 4); shaker from the B part on.
    for bar in 10..bars {
        for (beat, vol) in [(0.0, 0.4), (3.0, 0.24)] {
            let s = tabor(&mut m.rng);
            m.add(bar_at(bar) + beat * unit, &s, vol, 0.0, 0.15);
        }
        if bar >= 18 {
            for beat in [2.0, 5.0] {
                let s = shaker(&mut m.rng);
                m.add(bar_at(bar) + beat * unit, &s, 0.12, 0.5, 0.2);
            }
        }
    }
    m.finish(0.3, 0.74)
}

/// *Whisperwood*: slow and airy in A minor - a bowed drone and bass, a rolling harp, a far
/// recorder and bells. Intro - A - B - outro.
fn forest() -> (Vec<f32>, Vec<f32>) {
    let a_chords = ["Am", "Am", "F", "G", "Am", "Em", "F", "Am"];
    let b_chords = ["Dm", "Am", "F", "G", "Dm", "Em", "F", "G"];
    let chords: Vec<&str> = [&["Am", "Am"][..], &a_chords, &b_chords, &["Am", "Am"]].concat();
    let bars = chords.len();
    // A quarter at 70 (a hair slower, so the loop fills whole mixer buffers).
    let unit = whole_buffers(60.0 / 70.0, bars * 4);
    let loop_secs = bars as f32 * 4.0 * unit;
    let mut m = Mix::new(loop_secs, 4.0);
    let bar_at = |bar: usize| bar as f32 * 4.0 * unit;

    // The drone (A, all the way round: faded in at the start and out in the tail, so the fold
    // crossfades it) and a bowed bass on the chord roots.
    let drone = bowed(hz(45), loop_secs + 2.0, 2.0, 2.0);
    m.add(0.0, &drone, 0.07, -0.2, 0.3);
    let drone = bowed(hz(52), loop_secs + 2.0, 2.0, 2.0);
    m.add(0.0, &drone, 0.04, 0.2, 0.3);
    for (bar, c) in chords.iter().enumerate() {
        let (r, _) = chord(c, 38);
        let s = bowed(hz(r), 4.0 * unit + 0.3, 0.35, 0.4);
        m.add(bar_at(bar) - 0.1, &s, 0.11, 0.0, 0.35);
    }
    // Harp: rolling eighths.
    let stress = [0.8, 0.45, 0.55, 0.65, 0.5, 0.6, 0.5, 0.45];
    for (bar, c) in chords.iter().enumerate() {
        let (r, t) = chord(c, 52);
        for (k, iv) in [0, 7, 12, 12 + t, 19, 12 + t, 12, 7].iter().enumerate() {
            let at = bar_at(bar) + k as f32 * unit / 2.0 + m.loose(6.0);
            let s = pluck(&mut m.rng, hz(r + iv), 1.6, 0.998, 1);
            m.add(at, &s, 0.36 * stress[k], -0.3 + 0.08 * k as f32, 0.45);
        }
    }
    // Recorder: A and B, nothing in the intro and outro.
    let a = ["E5/3 D5", "C5/2 B4 A4", "C5/2 D5 E5", "D5/3 B4", "C5/2 E5/2", "B4/3 G4", "A4/2 C5 B4", "A4/4"];
    let b = ["F5/2 E5 D5", "E5/3 C5", "F5 E5 C5/2", "D5/2 G5/2", "A5/3 F5", "E5/2 G5 E5", "C5/2 F5 A5", "G5/2 B4 D5"];
    let tune = [a.join(" "), b.join(" ")].join(" ");
    let mut at = bar_at(2);
    let mut beats = 0.0;
    for (n, len) in notes(&tune) {
        if let Some(n) = n {
            let s = recorder(&mut m.rng, hz(n), len * unit - 0.06);
            m.add(at, &s, 0.17, 0.25, 0.6);
        }
        at += len * unit;
        beats += len;
    }
    assert_eq!(beats, 16.0 * 4.0, "the tune must fill the A and B parts");
    // Bells: on the chord root, every other bar of the B part and in the outro.
    for bar in (10..18).step_by(2).chain([18, 19]) {
        let (r, _) = chord(chords[bar], 52);
        let s = bell(hz(r + 24), 4.0);
        m.add(bar_at(bar) + if bar == 19 { 2.0 * unit } else { 0.0 }, &s, 0.13, if bar % 4 == 2 { 0.5 } else { -0.5 }, 0.7);
    }
    m.finish(0.5, 0.84)
}

/// *The Copper Kettle*: a cheerful 4/4 jig-reel for the tavern in G major - lute "oom-pah",
/// recorder, a fiddle for the middle part, tabor and shaker, a bell at every part.
/// Intro (lute) - A - A - B (fiddle) - A.
fn tavern() -> (Vec<f32>, Vec<f32>) {
    let a_chords = ["G", "G", "C", "G", "Em", "C", "D", "G"];
    let b_chords = ["C", "D", "G", "Em", "C", "D", "G", "D"];
    let chords: Vec<&str> = [&["G", "D"][..], &a_chords, &a_chords, &b_chords, &a_chords].concat();
    let bars = chords.len();
    // A quarter: about 132 per minute (made to fill whole mixer buffers).
    let beat = whole_buffers(0.455, bars * 4);
    let mut m = Mix::new(bars as f32 * 4.0 * beat, 3.0);
    let bar_at = |bar: usize| bar as f32 * 4.0 * beat;

    // Lute: the root on 1 and 3 (the fifth on 3), a quick strum of the chord on 2 and 4.
    for (bar, c) in chords.iter().enumerate() {
        let (r, t) = chord(c, 43);
        for (b, low) in [(0.0, r), (2.0, r + 7)] {
            let s = pluck(&mut m.rng, hz(low), 0.7, 0.993, 2);
            let at = bar_at(bar) + b * beat + m.loose(3.0);
            m.add(at, &s, 0.5, -0.3, 0.25);
        }
        for b in [1.0, 3.0] {
            for (k, iv) in [12, 12 + t, 19].iter().enumerate() {
                let s = pluck(&mut m.rng, hz(r + iv), 0.45, 0.99, 1);
                m.add(bar_at(bar) + b * beat + k as f32 * 0.012, &s, 0.22, -0.1, 0.3);
            }
        }
    }
    // The tune: the recorder plays A, the fiddle B.
    let a = "D5 G5 A5 B5 D6/2 B5 G5 C6 B5 A5 G5 B5/2 G5/2 E5 G5 B5 G5 A5 G5 E5 C5 D5 F#5 A5 C6 B5 A5 G5/2";
    let b = "E6/2 D6 C6 D6 C6 B5 A5 B5/2 D6/2 E6 D6 B5 G5 C6/2 E6 C6 A5 B5 C6 A5 B5 G5 D6 B5 A5/3 F#5";
    let parts = [(a, false), (a, false), (b, true), (a, false)];
    let mut at = bar_at(2);
    for (k, (tune, fiddle)) in parts.iter().enumerate() {
        let start = at;
        let mut beats = 0.0;
        for (n, len) in notes(tune) {
            if let Some(n) = n {
                let secs = len * beat - 0.04;
                if *fiddle {
                    let s = bowed(hz(n), secs, 0.03, 0.08);
                    m.add(at, &s, 0.22, 0.25, 0.45);
                } else {
                    let s = recorder(&mut m.rng, hz(n), secs);
                    m.add(at, &s, 0.24, 0.15, 0.45);
                }
            }
            at += len * beat;
            beats += len;
        }
        assert_eq!(beats, 32.0, "part {k} of the tavern tune must fill eight bars");
        let s = bell(hz(79), 2.0);
        m.add(start, &s, 0.12, 0.45, 0.6);
    }
    // Tabor on 1 and 3 from the second A on; a shaker on the off-beats from the B part.
    for bar in 10..bars {
        for (b, vol) in [(0.0, 0.42), (2.0, 0.28)] {
            let s = tabor(&mut m.rng);
            m.add(bar_at(bar) + b * beat, &s, vol, 0.0, 0.15);
        }
        if bar >= 18 {
            for b in [0.5, 1.5, 2.5, 3.5] {
                let s = shaker(&mut m.rng);
                m.add(bar_at(bar) + b * beat, &s, 0.1, 0.5, 0.2);
            }
        }
    }
    m.finish(0.3, 0.72)
}

/// A deep war drum: a slow falling thump, longer and lower than the tabor.
fn war_drum(rng: &mut Rng) -> Vec<f32> {
    let n = (0.6 * SRF) as usize;
    let mut phase = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / SRF;
            let f = 42.0 + 70.0 * (-t / 0.05).exp();
            phase = (phase + f / SRF).fract();
            let noise = (rng.range(2001) as f32 / 1000.0 - 1.0) * (-t / 0.02).exp() * 0.35;
            ((phase * std::f32::consts::TAU).sin() + noise) * (-t * 5.0).exp() * (t / 0.003).min(1.0)
        })
        .collect()
}

/// *Steel in the Woods*: the battle music, driving 4/4 in D minor at 150 - war drums, a chugging
/// low lute, a bowed bass, the fiddle with the tune (a second fiddle a third below in the B part),
/// a shaker and bells. Intro (drums) - A - B.
fn battle() -> (Vec<f32>, Vec<f32>) {
    const SCALE: [i32; 7] = [2, 4, 5, 7, 9, 10, 0]; // D natural minor
    let a_chords = ["Dm", "Dm", "Bb", "C", "Dm", "Dm", "Bb", "A"];
    let b_chords = ["Gm", "Gm", "Dm", "Dm", "Bb", "C", "A", "A"];
    let chords: Vec<&str> = [&["Dm", "Dm"][..], &a_chords, &b_chords].concat();
    let bars = chords.len();
    let beat = whole_buffers(0.4, bars * 4);
    let loop_secs = bars as f32 * 4.0 * beat;
    let mut m = Mix::new(loop_secs, 3.0);
    let bar_at = |bar: usize| bar as f32 * 4.0 * beat;

    // A low drone on D all the way round (crossfaded by the fold).
    let drone = bowed(hz(38), loop_secs + 2.0, 1.5, 2.0);
    m.add(0.0, &drone, 0.06, 0.0, 0.25);
    // War drums: the deep one on 1 and the "and" of 2, the tabor answering; fuller in B.
    for bar in 0..bars {
        let b_part = bar >= 10;
        for (b, vol) in [(0.0, 0.55), (1.5, 0.4), (2.0, 0.35), (3.5, 0.3)] {
            let s = war_drum(&mut m.rng);
            m.add(bar_at(bar) + b * beat, &s, vol, -0.1, 0.12);
        }
        for (b, vol) in [(1.0, 0.3), (3.0, 0.32), (3.75, 0.18)] {
            let s = tabor(&mut m.rng);
            m.add(bar_at(bar) + b * beat, &s, vol, 0.15, 0.15);
        }
        if b_part || bar == 9 {
            for k in 0..8 {
                let s = shaker(&mut m.rng);
                m.add(bar_at(bar) + k as f32 * beat / 2.0, &s, if k % 2 == 1 { 0.13 } else { 0.07 }, 0.5, 0.15);
            }
        }
    }
    // From the A part on: a chugging low lute on the chord (eighths, accents on 1 and the "and"
    // of 2) and a bowed bass on the root.
    for (bar, c) in chords.iter().enumerate().skip(2) {
        let (r, t) = chord(c, 38);
        for k in 0..8 {
            let iv = [0, 0, 7, 0, 0, 7, 12, 7][k];
            let acc = if matches!(k, 0 | 3 | 6) { 1.0 } else { 0.6 };
            let s = pluck(&mut m.rng, hz(r + iv), 0.32, 0.985, 3);
            let at = bar_at(bar) + k as f32 * beat / 2.0 + m.loose(2.0);
            m.add(at, &s, 0.42 * acc, -0.35, 0.2);
        }
        // A quick strum of the full chord on 1 (higher up).
        for (k, iv) in [12, 12 + t, 19, 24].iter().enumerate() {
            let s = pluck(&mut m.rng, hz(r + iv), 0.6, 0.993, 1);
            m.add(bar_at(bar) + k as f32 * 0.012, &s, 0.16, 0.35, 0.3);
        }
        let s = bowed(hz(r - 12), 4.0 * beat + 0.2, 0.08, 0.25);
        m.add(bar_at(bar), &s, 0.16, 0.0, 0.25);
    }
    // The fiddle: the tune in A and B; B with a second fiddle a third below.
    let a = "D5/1.5 E5/0.5 F5 A5 G5/0.5 F5/0.5 E5 D5/2 F5/1.5 G5/0.5 A5 Bb5 A5/0.5 G5/0.5 F5 E5/2 D5/1.5 E5/0.5 F5 A5 D6/2 C6 A5 Bb5/0.5 A5/0.5 G5 F5 G5 A5/3 r";
    let b = "G5/1.5 A5/0.5 Bb5 D6 C6/0.5 Bb5/0.5 A5 G5/2 F5/1.5 G5/0.5 A5 D6 E6/2 D6/2 D6/1.5 C6/0.5 Bb5 A5 G5 A5 Bb5 C6 A5/2 C#6/2 E6/3 r";
    for (k, (tune, second)) in [(a, false), (b, true)].iter().enumerate() {
        let start = bar_at(2 + k * 8);
        let mut at = start;
        let mut beats = 0.0;
        for (n, len) in notes(tune) {
            if let Some(n) = n {
                let secs = len * beat - 0.03;
                let s = bowed(hz(n), secs, 0.02, 0.06);
                m.add(at, &s, 0.3, 0.2, 0.4);
                if *second {
                    let s = bowed(hz(below(n, &SCALE, 2)), secs, 0.02, 0.06);
                    m.add(at + 0.008, &s, 0.18, -0.2, 0.4);
                }
            }
            at += len * beat;
            beats += len;
        }
        assert_eq!(beats, 32.0, "part {k} of the battle tune must fill eight bars");
        let s = bell(hz(74), 2.5);
        m.add(start, &s, 0.14, -0.45, 0.6);
    }
    m.finish(0.28, 0.7)
}

/// Plays a tune given bar by bar (each bar's notes must fill `beats_per_bar`) from bar `first`;
/// `voice` renders a note (pitch, seconds) and adds it. Returns the bars played.
fn tune_bars(bars: &[&str], beats_per_bar: f32, first: usize, unit: f32, mut voice: impl FnMut(f32, i32, f32)) -> usize {
    for (k, bar) in bars.iter().enumerate() {
        let mut at = (first + k) as f32 * beats_per_bar * unit;
        let mut beats = 0.0;
        for (n, len) in notes(bar) {
            if let Some(n) = n {
                voice(at, n, len * unit);
            }
            at += len * unit;
            beats += len;
        }
        assert_eq!(beats, beats_per_bar, "bar {k} ({bar}) must fill {beats_per_bar} beats");
    }
    bars.len()
}

/// *The Old Mine*: dark and slow in D minor (a flat second for the depths) - low drones, a
/// bowed bass, sparse harp, far recorder, a deep bell and drops of high plucks like water in the
/// dark. Intro - A - B - outro.
fn dungeon() -> (Vec<f32>, Vec<f32>) {
    const SCALE: [i32; 7] = [2, 4, 5, 7, 9, 10, 0];
    let a_chords = ["Dm", "Dm", "Bb", "A", "Dm", "Gm", "Eb", "A"];
    let b_chords = ["Gm", "Dm", "Eb", "Dm", "Gm", "Bb", "A", "A"];
    let chords: Vec<&str> = [&["Dm", "Dm"][..], &a_chords, &b_chords, &["Dm", "Dm"]].concat();
    let bars = chords.len();
    let unit = whole_buffers(60.0 / 58.0, bars * 4);
    let loop_secs = bars as f32 * 4.0 * unit;
    let mut m = Mix::new(loop_secs, 5.0);
    let bar_at = |bar: usize| bar as f32 * 4.0 * unit;
    // Drones on D and A all the way round, and a bowed bass on the roots.
    for (f, vol, pan) in [(38, 0.08, -0.25), (45, 0.045, 0.25)] {
        let s = bowed(hz(f), loop_secs + 3.0, 3.0, 3.0);
        m.add(0.0, &s, vol, pan, 0.4);
    }
    for (bar, c) in chords.iter().enumerate() {
        let (r, _) = chord(c, 38);
        let s = bowed(hz(r), 4.0 * unit + 0.6, 0.8, 0.8);
        m.add(bar_at(bar) - 0.2, &s, 0.1, 0.0, 0.45);
        // Harp: root and fifth on 1 and 3, the third on the "and" of 4.
        let (h, t) = chord(c, 50);
        for (b, iv, vol) in [(0.0, 0, 0.32), (2.0, 7, 0.22), (3.5, t, 0.14)] {
            let s = pluck(&mut m.rng, hz(h + iv), 2.4, 0.998, 2);
            let at = bar_at(bar) + b * unit + m.loose(8.0);
            m.add(at, &s, vol, -0.3 + 0.2 * b / 3.5, 0.6);
        }
    }
    // Drops: high plucks of the scale at loose times, here and there.
    let mut t = 1.3;
    while t < loop_secs - 0.5 {
        let deg = SCALE[m.rng.range(7) as usize];
        let n = 74 + (deg - 2).rem_euclid(12) + if m.rng.range(3) == 0 { 12 } else { 0 };
        let s = pluck(&mut m.rng, hz(n), 1.2, 0.996, 0);
        let pan = m.rng.range(2001) as f32 / 1000.0 - 1.0;
        m.add(t, &s, 0.06, pan, 0.8);
        t += 1.2 + m.rng.range(2600) as f32 / 1000.0;
    }
    // The far recorder: A and B.
    let a = ["A4/3 G4", "F4/2 E4/2", "D4/4", "F4/2 G4/2", "A4/3 Bb4", "A4/4", "D5/2 C5 Bb4", "A4/4"];
    let b = ["Bb4/3 A4", "F4/2 D4/2", "Eb4/2 G4/2", "F4/4", "G4/2 Bb4 D5", "C5/2 Bb4/2", "A4/2 C#5/2", "A4/4"];
    let mut rng = Rng(0xd1e);
    let mut adds = Vec::new();
    tune_bars(&[&a[..], &b[..]].concat(), 4.0, 2, unit, |at, n, secs| adds.push((at, recorder(&mut rng, hz(n), secs - 0.08))));
    for (at, s) in adds {
        m.add(at, &s, 0.13, 0.3, 0.75);
    }
    // A deep bell at each part and in the outro.
    for bar in [2, 10, 18] {
        let s = bell(hz(50), 6.0);
        m.add(bar_at(bar), &s, 0.12, -0.4, 0.8);
    }
    m.finish(0.55, 0.86)
}

/// *Lanterns at Night*: a quiet 3/4 lullaby in F major - harp, a soft recorder, a low pad and
/// bells far off. Intro - A - B - outro.
fn night() -> (Vec<f32>, Vec<f32>) {
    let a_chords = ["F", "Dm", "Bb", "C", "F", "Am", "Bb", "C"];
    let b_chords = ["Dm", "Bb", "F", "C", "Dm", "Gm", "Bb", "C"];
    let chords: Vec<&str> = [&["F", "F"][..], &a_chords, &b_chords, &["F", "F"]].concat();
    let bars = chords.len();
    let unit = whole_buffers(60.0 / 66.0, bars * 3);
    let loop_secs = bars as f32 * 3.0 * unit;
    let mut m = Mix::new(loop_secs, 4.0);
    let bar_at = |bar: usize| bar as f32 * 3.0 * unit;
    for (bar, c) in chords.iter().enumerate() {
        // The pad: the root and the fifth, bowed softly.
        let (r, _) = chord(c, 41);
        for (iv, vol) in [(0, 0.06), (7, 0.035)] {
            let s = bowed(hz(r + iv), 3.0 * unit + 0.6, 0.6, 0.7);
            m.add(bar_at(bar) - 0.15, &s, vol, if iv == 0 { -0.2 } else { 0.2 }, 0.5);
        }
        // Harp: a rolling arpeggio of eighths.
        let (h, t) = chord(c, 53);
        for (k, iv) in [0, 7, 12, 12 + t, 19, 12 + t].iter().enumerate() {
            let at = bar_at(bar) + k as f32 * unit / 2.0 + m.loose(6.0);
            let s = pluck(&mut m.rng, hz(h + iv), 1.8, 0.998, 2);
            m.add(at, &s, if k == 0 { 0.3 } else { 0.18 }, -0.35 + 0.12 * k as f32, 0.55);
        }
    }
    let a = ["C5/2 A4", "D5/3", "F5/2 D5", "E5/3", "F5/2 A5", "E5/2 C5", "D5 F5 D5", "C5/3"];
    let b = ["A5/2 F5", "D5/2 Bb4", "C5 F5 A5", "G5/3", "F5/2 D5", "Bb4/2 D5", "F5 D5 Bb4", "C5/3"];
    let mut rng = Rng(0x1a7e);
    let mut adds = Vec::new();
    tune_bars(&[&a[..], &b[..]].concat(), 3.0, 2, unit, |at, n, secs| adds.push((at, recorder(&mut rng, hz(n), secs - 0.06))));
    for (at, s) in adds {
        m.add(at, &s, 0.14, 0.2, 0.6);
    }
    for bar in [2, 6, 10, 14, 18] {
        let (r, _) = chord(chords[bar], 65);
        let s = bell(hz(r + 12), 4.0);
        m.add(bar_at(bar) + 1.0 * unit, &s, 0.08, if bar % 8 == 2 { 0.5 } else { -0.5 }, 0.8);
    }
    m.finish(0.5, 0.84)
}

/// *The Open Road*: a striding 4/4 in G major for the road and the wilds - lute, recorder for the
/// A part, a fiddle for B, a bowed bass and a light tabor. Intro - A - A - B - A.
fn travel() -> (Vec<f32>, Vec<f32>) {
    let a_chords = ["G", "C", "G", "D", "G", "C", "D", "G"];
    let b_chords = ["Em", "C", "G", "D", "Em", "C", "D", "D"];
    let chords: Vec<&str> = [&["G", "D"][..], &a_chords, &a_chords, &b_chords, &a_chords].concat();
    let bars = chords.len();
    let beat = whole_buffers(60.0 / 104.0, bars * 4);
    let mut m = Mix::new(bars as f32 * 4.0 * beat, 3.0);
    let bar_at = |bar: usize| bar as f32 * 4.0 * beat;
    for (bar, c) in chords.iter().enumerate() {
        let (r, t) = chord(c, 43);
        // Lute: eighths, the root low on 1 and 3.
        for (k, iv) in [0, 12, 7, 12 + t, 0, 12, 7, 12 + t].iter().enumerate() {
            let low = k % 4 == 0;
            let s = pluck(&mut m.rng, hz(r + iv), if low { 0.9 } else { 0.5 }, 0.993, 2);
            let at = bar_at(bar) + k as f32 * beat / 2.0 + m.loose(3.0);
            m.add(at, &s, if low { 0.42 } else { 0.24 }, -0.3, 0.3);
        }
        if bar >= 2 {
            let s = bowed(hz(r - 12), 4.0 * beat + 0.15, 0.06, 0.2);
            m.add(bar_at(bar), &s, 0.12, 0.0, 0.3);
        }
        if bar >= 10 {
            for (b, vol) in [(0.0, 0.36), (2.0, 0.24), (2.5, 0.14)] {
                let s = tabor(&mut m.rng);
                m.add(bar_at(bar) + b * beat, &s, vol, 0.05, 0.15);
            }
        }
    }
    let a = ["G4 B4 D5 G5", "E5/2 C5/2", "D5 B4 G4 B4", "A4/4", "G4 B4 D5 G5", "E5 G5 E5 C5", "D5/2 F#5 A5", "G5/4"];
    let b = ["B4 E5 G5 B5", "A5/2 G5 E5", "D5/2 B4 D5", "A4/2 F#4/2", "G4 B4 E5 G5", "E5/2 C5 E5", "D5 C5 B4 A4", "D5/4"];
    let mut rng = Rng(0x70ad);
    let mut adds: Vec<(f32, Vec<f32>, bool)> = Vec::new();
    for (first, part, fiddle) in [(2, &a, false), (10, &a, false), (18, &b, true), (26, &a, false)] {
        tune_bars(&part[..], 4.0, first, beat, |at, n, secs| {
            let s = if fiddle { bowed(hz(n), secs - 0.04, 0.03, 0.08) } else { recorder(&mut rng, hz(n), secs - 0.04) };
            adds.push((at, s, fiddle));
        });
    }
    for (at, s, fiddle) in adds {
        m.add(at, &s, if fiddle { 0.22 } else { 0.24 }, 0.2, 0.45);
    }
    for bar in [2, 18] {
        let s = bell(hz(79), 2.5);
        m.add(bar_at(bar), &s, 0.1, 0.45, 0.6);
    }
    m.finish(0.32, 0.74)
}

/// *Grumbold's Wrath*: a boss fight in E minor at 160 - war drums on every beat, a chugging low
/// lute and bass, the fiddle with the tune (harmonic minor), a second fiddle in B, bells.
/// Intro (drums) - A - B - A.
fn boss() -> (Vec<f32>, Vec<f32>) {
    const SCALE: [i32; 7] = [4, 6, 7, 9, 11, 0, 3]; // E harmonic minor
    let a_chords = ["Em", "Em", "C", "B", "Em", "Em", "C", "B"];
    let b_chords = ["Am", "Am", "Em", "Em", "C", "D", "B", "B"];
    let chords: Vec<&str> = [&["Em", "Em"][..], &a_chords, &b_chords, &a_chords].concat();
    let bars = chords.len();
    let beat = whole_buffers(60.0 / 160.0, bars * 4);
    let loop_secs = bars as f32 * 4.0 * beat;
    let mut m = Mix::new(loop_secs, 3.0);
    let bar_at = |bar: usize| bar as f32 * 4.0 * beat;
    let drone = bowed(hz(40), loop_secs + 2.0, 1.0, 2.0);
    m.add(0.0, &drone, 0.07, 0.0, 0.25);
    for (bar, c) in chords.iter().enumerate() {
        for b in 0..4 {
            let s = war_drum(&mut m.rng);
            m.add(bar_at(bar) + b as f32 * beat, &s, if b % 2 == 0 { 0.55 } else { 0.4 }, -0.1, 0.12);
        }
        for (b, vol) in [(1.5, 0.28), (3.5, 0.3), (3.75, 0.2)] {
            let s = tabor(&mut m.rng);
            m.add(bar_at(bar) + b * beat, &s, vol, 0.2, 0.15);
        }
        if bar < 2 {
            continue;
        }
        let (r, t) = chord(c, 40);
        for k in 0..8 {
            let iv = [0, 0, 12, 0, 7, 0, 12, 7][k];
            let s = pluck(&mut m.rng, hz(r + iv), 0.25, 0.985, 3);
            let at = bar_at(bar) + k as f32 * beat / 2.0 + m.loose(2.0);
            m.add(at, &s, if k % 4 == 0 { 0.45 } else { 0.3 }, -0.35, 0.2);
        }
        for (k, iv) in [12, 12 + t, 19, 24].iter().enumerate() {
            let s = pluck(&mut m.rng, hz(r + iv), 0.5, 0.993, 1);
            m.add(bar_at(bar) + k as f32 * 0.01, &s, 0.15, 0.35, 0.3);
        }
        let s = bowed(hz(r - 12), 4.0 * beat + 0.1, 0.05, 0.2);
        m.add(bar_at(bar), &s, 0.17, 0.0, 0.25);
        for k in 0..8 {
            let s = shaker(&mut m.rng);
            m.add(bar_at(bar) + k as f32 * beat / 2.0, &s, if k % 2 == 1 { 0.12 } else { 0.06 }, 0.5, 0.15);
        }
    }
    let a = ["E5/1.5 F#5/0.5 G5 B5", "A5 G5 F#5 E5", "C6/2 B5 A5", "B5/2 D#5/2", "E5/1.5 F#5/0.5 G5 B5", "E6/2 D6 B5", "C6 B5 A5 G5", "F#5/2 B5/2"];
    let b = ["A5/1.5 B5/0.5 C6 E6", "D6/2 C6 B5", "B5/1.5 A5/0.5 G5 E5", "G5/2 B5/2", "C6 E6 C6 G5", "D6/2 F#5/2", "D#6/2 B5 F#5", "B5/4"];
    let mut adds: Vec<(f32, Vec<f32>, f32, f32)> = Vec::new();
    for (first, part, second) in [(2, &a, false), (10, &b, true), (18, &a, false)] {
        tune_bars(&part[..], 4.0, first, beat, |at, n, secs| {
            adds.push((at, bowed(hz(n), secs - 0.03, 0.02, 0.06), 0.3, 0.2));
            if second {
                adds.push((at + 0.008, bowed(hz(below(n, &SCALE, 2)), secs - 0.03, 0.02, 0.06), 0.18, -0.2));
            }
        });
    }
    for (at, s, vol, pan) in adds {
        m.add(at, &s, vol, pan, 0.4);
    }
    for bar in [2, 10, 18] {
        let s = bell(hz(76), 2.5);
        m.add(bar_at(bar), &s, 0.14, -0.45, 0.6);
    }
    m.finish(0.26, 0.7)
}
