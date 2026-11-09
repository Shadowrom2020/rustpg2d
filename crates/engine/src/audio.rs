//! Sound effects and looping music: WAV or Ogg Vorbis files, decoded by macroquad (quad-snd)
//! and mixed at 44.1 kHz stereo - other rates are resampled roughly, so 44.1 kHz plays best.
//! A file is checked before it reaches the decoder, and a file in another format is reported
//! by name (Opus, FLAC, MP3) so the creator knows what to convert.
//!
//! **Gapless music.** quad-snd (0.2.8) leaves a short silence where a looped sound wraps unless
//! its length is a whole number of mixer buffers (see `rain_samples`). Music is therefore decoded
//! here (`loop_ready`): resampled to 44.1 kHz and stretched to whole buffers (tracks under 30 s
//! are repeated first, so the stretch stays below 0.32 %, about 5 cents), then handed to
//! macroquad as a WAV. Long sound files are prepared in a background thread when the project
//! loads; any other sound is prepared the first time it is played as music.

use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use macroquad::audio::{load_sound_from_bytes, play_sound, set_sound_volume, stop_sound, PlaySoundParams, Sound};

use crate::project::Project;

/// Polls a future once. macroquad's native sound loading completes immediately.
fn poll_now<F: Future>(f: F) -> Option<F::Output> {
    let mut cx = Context::from_waker(Waker::noop());
    match pin!(f).poll(&mut cx) {
        Poll::Ready(v) => Some(v),
        Poll::Pending => None,
    }
}

/// Checks that the bytes are a format the mixer decodes (WAV or Ogg Vorbis); the error names
/// a format it recognizes but can't play.
fn check_format(bytes: &[u8]) -> Result<(), String> {
    let has = |pat: &[u8]| bytes[..bytes.len().min(512)].windows(pat.len()).any(|w| w == pat);
    if bytes.len() < 12 {
        return Err("not a WAV or Ogg Vorbis file".into());
    }
    if &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WAVE" {
        return Ok(());
    }
    if &bytes[..4] == b"OggS" {
        return if has(b"\x01vorbis") {
            Ok(())
        } else if has(b"OpusHead") {
            Err("Ogg Opus isn't supported: convert it to Ogg Vorbis".into())
        } else if has(b"\x7fFLAC") {
            Err("Ogg FLAC isn't supported: convert it to Ogg Vorbis or WAV".into())
        } else {
            Err("an Ogg file without a Vorbis stream".into())
        };
    }
    if &bytes[..3] == b"ID3" || (bytes[0] == 0xFF && bytes[1] & 0xE0 == 0xE0) {
        return Err("MP3 isn't supported: convert it to Ogg Vorbis or WAV".into());
    }
    if &bytes[..4] == b"fLaC" {
        return Err("FLAC isn't supported: convert it to Ogg Vorbis or WAV".into());
    }
    Err("not a WAV or Ogg Vorbis file".into())
}

/// Gapless loops prepared in the background: sound id → WAV bytes (empty: the sound already is
/// one), or why it failed.
type Prepared = Arc<Mutex<HashMap<String, Result<Vec<u8>, String>>>>;

/// How a sound loops as music.
enum Loop {
    /// Its length is whole mixer buffers already: it loops cleanly as it is.
    Whole,
    /// Its gapless version (resampled, stretched).
    Stretched(Sound),
    /// No gapless loop could be made: plays as it is.
    Plain,
}

pub struct Audio {
    sounds: HashMap<String, (Sound, f32)>,
    /// Sound files by id (music is decoded again for its gapless loop).
    files: HashMap<String, PathBuf>,
    /// Gapless loops of sounds played as music, made on first use.
    loops: HashMap<String, Loop>,
    prepared: Prepared,
    music: Option<String>,
    /// The sound last started by [`Audio::play_preview`] (the editor's Play button).
    preview: Option<String>,
    pub errors: Vec<String>,
    pub enabled: bool,
    /// Player options (0..1): effects and music volume, master already applied.
    effects: f32,
    music_vol: f32,
    /// The built-in rain loop (made on first use) and its volume (0 = stopped).
    rain: Option<Sound>,
    rain_vol: f32,
    /// Player option (0..1): environmental sounds (rain), master applied.
    ambience: f32,
    /// The last sound effects started (also while muted): `status()` for automated tests.
    recent: std::cell::RefCell<std::collections::VecDeque<String>>,
}

// The web game: browsers decode sounds asynchronously, so they are loaded (awaited) before the
// game starts and `Audio::load` takes them from here.
thread_local! {
    static PRELOADED: std::cell::RefCell<HashMap<String, Result<Sound, String>>> = std::cell::RefCell::new(HashMap::new());
    static PRELOADED_RAIN: std::cell::RefCell<Option<Sound>> = const { std::cell::RefCell::new(None) };
}

/// Loads the project's sounds and the rain loop ahead of [`Audio::load`], waiting for the
/// decoder (the web game; natively sounds load instantly and this isn't needed). `progress` is
/// called with (done, total).
pub async fn preload(project: &Project, mut progress: impl FnMut(usize, usize)) {
    let total = project.db.sounds.len() + 1;
    for (k, def) in project.db.sounds.iter().enumerate() {
        progress(k, total);
        let res = match crate::sys::read(&project.path(&def.file)).map_err(|e| e.to_string()).and_then(|b| check_format(&b).map(|_| b)) {
            Ok(bytes) => load_sound_from_bytes(&bytes).await.map_err(|e| e.to_string()),
            Err(e) => Err(e),
        };
        PRELOADED.with(|p| p.borrow_mut().insert(def.id.clone(), res));
    }
    progress(total - 1, total);
    let rain = load_sound_from_bytes(&wav_bytes(&rain_samples(), MIX_RATE)).await.ok();
    PRELOADED_RAIN.with(|r| *r.borrow_mut() = rain);
    progress(total, total);
}

impl Audio {
    /// Loads all sounds of the project. Needs a macroquad window.
    pub fn load(project: &Project) -> Audio {
        let mut sounds = HashMap::new();
        let mut errors = Vec::new();
        for def in &project.db.sounds {
            let path = project.path(&def.file);
            if let Some(pre) = PRELOADED.with(|p| p.borrow_mut().remove(&def.id)) {
                match pre {
                    Ok(s) => {
                        sounds.insert(def.id.clone(), (s, def.volume.clamp(0.0, 1.0)));
                    }
                    Err(e) => errors.push(format!("{}: {e}", path.display())),
                }
                continue;
            }
            let res = crate::sys::read(&path).map_err(|e| e.to_string()).and_then(|bytes| {
                check_format(&bytes)?;
                // The decoder panics on some malformed files; don't take the game down.
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| poll_now(load_sound_from_bytes(&bytes))))
                    .map_err(|_| "could not decode".to_string())?
                    .ok_or("loading did not complete".to_string())?
                    .map_err(|e| e.to_string())
            });
            match res {
                Ok(s) => {
                    sounds.insert(def.id.clone(), (s, def.volume.clamp(0.0, 1.0)));
                }
                Err(e) => errors.push(format!("{}: {e}", path.display())),
            }
        }
        for e in &errors {
            eprintln!("[audio] {e}");
        }
        let files: HashMap<String, PathBuf> = project.db.sounds.iter().filter(|d| sounds.contains_key(&d.id)).map(|d| (d.id.clone(), project.path(&d.file))).collect();
        let enabled = std::env::var_os("RUSTPG2D_MUTE").is_none();
        let prepared = Prepared::default();
        // (No threads in the web game, and the browser's audio loops without a gap anyway.)
        #[cfg(not(target_arch = "wasm32"))]
        if enabled {
            // Long files are likely music: make their loops in the background.
            let long: Vec<(String, PathBuf)> = files.iter().filter(|(_, p)| likely_music(p)).map(|(id, p)| (id.clone(), p.clone())).collect();
            if !long.is_empty() {
                let out = prepared.clone();
                std::thread::spawn(move || {
                    for (id, path) in long {
                        let res = std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| loop_ready(&b));
                        out.lock().unwrap().insert(id, res);
                    }
                });
            }
        }
        Audio {
            sounds,
            files,
            loops: HashMap::new(),
            prepared,
            music: None,
            preview: None,
            errors,
            enabled,
            effects: 1.0,
            music_vol: 1.0,
            rain: None,
            rain_vol: 0.0,
            ambience: 1.0,
            recent: Default::default(),
        }
    }

    /// The gapless loop of a sound, made (or taken from the background thread) on first use.
    fn gapless(&mut self, id: &str) -> &Loop {
        if cfg!(target_arch = "wasm32") {
            // The browser loops a sound seamlessly (an AudioBufferSourceNode with `loop`).
            self.loops.entry(id.to_string()).or_insert(Loop::Whole);
        }
        if !self.loops.contains_key(id) {
            let ready = self.prepared.lock().unwrap().remove(id);
            let wav = match ready {
                Some(r) => r,
                None => self.files.get(id).ok_or_else(|| "no file".to_string()).and_then(|p| crate::sys::read(p).map_err(|e| e.to_string())).and_then(|b| loop_ready(&b)),
            };
            let made = match wav {
                Ok(w) if w.is_empty() => Loop::Whole,
                Ok(w) => poll_now(load_sound_from_bytes(&w)).and_then(|r| r.ok()).map_or(Loop::Plain, Loop::Stretched),
                Err(e) => {
                    eprintln!("[audio] {id}: no gapless loop ({e}); it loops as it is");
                    Loop::Plain
                }
            };
            self.loops.insert(id.to_string(), made);
        }
        &self.loops[id]
    }

    /// The sound playing as music: its gapless loop if one was made.
    fn music_sound(&self) -> Option<(&Sound, f32)> {
        let id = self.music.as_ref()?;
        let (orig, vol) = self.sounds.get(id)?;
        let s = match self.loops.get(id) {
            Some(Loop::Stretched(s)) => s,
            _ => orig,
        };
        Some((s, *vol))
    }

    /// `status()`: how the music loops: `whole` (its length already is whole mixer buffers),
    /// `stretched` (a gapless loop was made), `plain` (it couldn't be: it may have a gap), `-`.
    pub fn music_loop_status(&self) -> &'static str {
        match self.music.as_ref().and_then(|id| self.loops.get(id)) {
            None => "-",
            Some(Loop::Whole) => "whole",
            Some(Loop::Stretched(_)) => "stretched",
            Some(Loop::Plain) => "plain",
        }
    }

    pub fn has(&self, id: &str) -> bool {
        self.sounds.contains_key(id)
    }

    /// Plays a sound effect once (unknown or empty ids are ignored).
    pub fn play(&self, id: &str) {
        if !id.is_empty() {
            let mut r = self.recent.borrow_mut();
            if r.len() >= 6 {
                r.pop_front();
            }
            r.push_back(id.to_string());
        }
        if !self.enabled {
            return;
        }
        if let Some((s, vol)) = self.sounds.get(id) {
            play_sound(s, PlaySoundParams { looped: false, volume: *vol * self.effects });
        }
    }

    /// The last sound effects started, oldest first (`status()` prints them as `sounds=`).
    pub fn recent(&self) -> Vec<String> {
        self.recent.borrow().iter().cloned().collect()
    }

    /// Plays a sound once as a preview (the editor): the previous preview stops first.
    pub fn play_preview(&mut self, id: &str) {
        self.stop_preview();
        if self.sounds.contains_key(id) {
            self.play(id);
            self.preview = Some(id.to_string());
        }
    }

    /// Stops the preview (and the music, if it is the same sound).
    pub fn stop_preview(&mut self) {
        if let Some(id) = self.preview.take() {
            self.stop(&id);
        }
    }

    /// Stops every playback of a sound, as preview, effect or music.
    pub fn stop(&mut self, id: &str) {
        if let Some((s, _)) = self.sounds.get(id) {
            stop_sound(s);
        }
        if self.music.as_deref() == Some(id) {
            self.stop_music();
        }
        if self.preview.as_deref() == Some(id) {
            self.preview = None;
        }
    }

    /// The sound started as a preview, if one was (a short effect may have ended by itself).
    pub fn previewing(&self) -> Option<&str> {
        self.preview.as_deref()
    }

    /// Applies the player's volume options (also to the music that is playing).
    pub fn set_volumes(&mut self, effects: f32, music: f32) {
        self.effects = effects.clamp(0.0, 1.0);
        self.music_vol = music.clamp(0.0, 1.0);
        if let Some((s, vol)) = self.music_sound() {
            set_sound_volume(s, vol * self.music_vol);
        }
    }

    /// Switches the looping music; `""` stops it. Re-requesting the current track keeps it playing.
    pub fn play_music(&mut self, id: &str) {
        if self.music.as_deref() == Some(id) {
            return;
        }
        self.stop_music();
        if id.is_empty() || !self.enabled {
            return;
        }
        let Some(&(_, vol)) = self.sounds.get(id) else { return };
        self.gapless(id);
        self.music = Some(id.to_string());
        if let Some((s, _)) = self.music_sound() {
            play_sound(s, PlaySoundParams { looped: true, volume: vol * self.music_vol });
        }
    }

    pub fn stop_music(&mut self) {
        if let Some((s, _)) = self.music_sound() {
            stop_sound(s);
        }
        self.music = None;
    }

    pub fn current_music(&self) -> Option<&str> {
        self.music.as_deref()
    }

    /// The player's ambience volume (Options › Ambience volume), also for the playing rain.
    pub fn set_ambience_volume(&mut self, v: f32) {
        self.ambience = v.clamp(0.0, 1.0);
        if let (Some(s), true) = (&self.rain, self.rain_vol > 0.0) {
            set_sound_volume(s, self.rain_vol * self.ambience);
        }
    }

    /// The built-in rain loop at `volume` (0..1, before the ambience volume; 0 stops it).
    pub fn set_rain(&mut self, volume: f32) {
        let v = if self.enabled { volume.clamp(0.0, 1.0) } else { 0.0 };
        if (v - self.rain_vol).abs() < 0.01 {
            return;
        }
        if v <= 0.0 {
            if let Some(s) = &self.rain {
                stop_sound(s);
            }
            self.rain_vol = 0.0;
            return;
        }
        if self.rain.is_none() {
            self.rain = PRELOADED_RAIN.with(|r| r.borrow_mut().take());
        }
        if self.rain.is_none() {
            let bytes = wav_bytes(&rain_samples(), MIX_RATE);
            self.rain = poll_now(load_sound_from_bytes(&bytes)).and_then(|r| r.ok());
        }
        let Some(s) = &self.rain else { return };
        if self.rain_vol <= 0.0 {
            play_sound(s, PlaySoundParams { looped: true, volume: v * self.ambience });
        } else {
            set_sound_volume(s, v * self.ambience);
        }
        self.rain_vol = v;
    }

    /// Volume of the rain loop (0 = not playing).
    pub fn rain_volume(&self) -> f32 {
        self.rain_vol
    }
}

/// Sample rate of macroquad's mixer (sounds at other rates are resampled when loaded).
pub const MIX_RATE: u32 = 44_100;
/// Frames macroquad's mixer (quad-snd) fills per pass on Linux.
pub const MIX_BUFFER_FRAMES: usize = 4096;

/// About six seconds of rain that loop seamlessly: filtered noise (the hiss) with soft drop
/// ticks, at the mixer's own rate.
///
/// - The length is a whole number of mixer buffers: quad-snd rewinds a looped sound that ends
///   inside a buffer but writes the rest at the buffer's start again, leaving the end of that
///   buffer silent — a short gap at every loop. Sounds start at a buffer boundary, so a length
///   of whole buffers always ends on one (and smaller power-of-two buffers divide it too).
/// - The extra tail fades into the start with an equal-power crossfade: the two noises are
///   unrelated, so a linear fade would dip in loudness at every loop.
pub fn rain_samples() -> Vec<f32> {
    let rate = MIX_RATE;
    let n = MIX_BUFFER_FRAMES * 65;
    let fade = rate as usize;
    let mut seed = 0x1234_5678u32;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
    };
    let (mut low, mut high, mut drop, mut drop_env) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let mut raw = Vec::with_capacity(n + fade);
    for _ in 0..n + fade {
        let w = rnd();
        low += (w - low) * 0.18; // body of the hiss
        high = 0.8 * (high + w - low) * 0.5; // a little brightness
        if rnd() > 0.9975 {
            drop_env = 0.6 + 0.4 * rnd().abs();
        }
        drop = drop * 0.6 + rnd() * drop_env;
        drop_env *= 0.993;
        raw.push(low * 0.55 + high * 0.25 + drop * 0.12);
    }
    // Crossfade the extra tail into the beginning so the loop has no seam.
    let mut out: Vec<f32> = raw[..n].to_vec();
    for i in 0..fade {
        let t = i as f32 / fade as f32 * std::f32::consts::FRAC_PI_2;
        out[i] = out[i] * t.sin() + raw[n + i] * t.cos();
    }
    let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs())).max(1e-6);
    out.iter().map(|v| v / peak * 0.6).collect()
}

/// Encodes mono 16-bit PCM samples (-1..1) as a WAV file.
pub fn wav_bytes(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    wav_bytes_channels(samples, sample_rate, 1)
}

/// Encodes interleaved 16-bit PCM samples (-1..1) with `channels` channels as a WAV file.
pub fn wav_bytes_channels(samples: &[f32], sample_rate: u32, channels: u16) -> Vec<u8> {
    let data_len = samples.len() as u32 * 2;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * 2 * channels as u32).to_le_bytes());
    out.extend_from_slice(&(2 * channels).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes());
    }
    out
}

/// Decoded audio: interleaved samples (-1..1).
#[derive(Clone, Debug, PartialEq)]
pub struct Pcm {
    pub rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

impl Pcm {
    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels.max(1) as usize
    }
}

/// Whether a sound file is long enough to be music (prepared in the background at load):
/// Ogg Vorbis over 64 KiB (several seconds), WAV over 1 MiB (about six seconds of stereo).
#[cfg(not(target_arch = "wasm32"))]
fn likely_music(path: &std::path::Path) -> bool {
    let len = std::fs::metadata(path).map_or(0, |m| m.len());
    match path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("ogg") => len > 64 * 1024,
        Some("wav") => len > 1024 * 1024,
        _ => false,
    }
}

/// Decodes WAV (8 / 16 / 24 / 32-bit PCM, 32-bit float) or Ogg Vorbis.
pub fn decode(bytes: &[u8]) -> Result<Pcm, String> {
    check_format(bytes)?;
    if &bytes[..4] == b"OggS" {
        let mut r = lewton::inside_ogg::OggStreamReader::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
        let (rate, channels) = (r.ident_hdr.audio_sample_rate, r.ident_hdr.audio_channels as u16);
        let mut samples = Vec::new();
        while let Some(p) = r.read_dec_packet_itl().map_err(|e| e.to_string())? {
            samples.extend(p.iter().map(|&v| v as f32 / 32768.0));
        }
        return Ok(Pcm { rate, channels, samples });
    }
    // WAV: walk the chunks for "fmt " and "data".
    let (mut fmt, mut data) = (None, None);
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let len = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
        // (Checked: a corrupt length must not overflow on 32-bit targets like the web.)
        let Some(end) = at.checked_add(8).and_then(|s| s.checked_add(len)) else { break };
        let body = &bytes[at + 8..end.min(bytes.len())];
        match id {
            b"fmt " => fmt = Some(body),
            b"data" => data = Some(body),
            _ => {}
        }
        let Some(next) = end.checked_add(len % 2) else { break };
        at = next;
    }
    let (fmt, data) = (fmt.ok_or("WAV without a fmt chunk")?, data.ok_or("WAV without a data chunk")?);
    if fmt.len() < 16 {
        return Err("bad WAV fmt chunk".into());
    }
    let u16_at = |i: usize| u16::from_le_bytes([fmt[i], fmt[i + 1]]);
    let mut format = u16_at(0);
    if format == 0xFFFE && fmt.len() >= 26 {
        format = u16_at(24); // WAVE_FORMAT_EXTENSIBLE: the sub-format's first two bytes
    }
    let (channels, rate, bits) = (u16_at(2).max(1), u32::from_le_bytes(fmt[4..8].try_into().unwrap()), u16_at(14));
    let samples: Vec<f32> = match (format, bits) {
        (1, 8) => data.iter().map(|&b| (b as f32 - 128.0) / 128.0).collect(),
        (1, 16) => data.as_chunks::<2>().0.iter().map(|c| i16::from_le_bytes(*c) as f32 / 32768.0).collect(),
        (1, 24) => data.as_chunks::<3>().0.iter().map(|c| (i32::from_le_bytes([0, c[0], c[1], c[2]]) >> 8) as f32 / 8_388_608.0).collect(),
        (1, 32) => data.as_chunks::<4>().0.iter().map(|c| i32::from_le_bytes(*c) as f32 / 2_147_483_648.0).collect(),
        (3, 32) => data.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect(),
        _ => return Err(format!("WAV format {format} with {bits} bits isn't supported")),
    };
    Ok(Pcm { rate, channels, samples })
}

/// Rate, channels and length in frames read from the headers without decoding: a WAV's data
/// size, an Ogg Vorbis stream's last granule position.
fn header_frames(bytes: &[u8]) -> Option<(u32, u16, u64)> {
    if bytes.len() < 64 {
        return None;
    }
    if &bytes[..4] == b"OggS" {
        let id = bytes.windows(7).position(|w| w == b"\x01vorbis")?;
        let h = bytes.get(id + 7..id + 16)?;
        let (channels, rate) = (h[4] as u16, u32::from_le_bytes(h[5..9].try_into().ok()?));
        // The last page: the final granule position is the stream's length in frames.
        let last = bytes.windows(4).rposition(|w| w == b"OggS")?;
        let granule = u64::from_le_bytes(bytes.get(last + 6..last + 14)?.try_into().ok()?);
        return Some((rate, channels, granule));
    }
    let pcm = decode(bytes).ok()?;
    Some((pcm.rate, pcm.channels, pcm.frames() as u64))
}

/// Tracks shorter than this are repeated before stretching (a smaller pitch change).
const MIN_LOOP_SECS: f32 = 30.0;

/// A loop that quad-snd plays without a gap: at the mixer's rate, a whole number of mixer
/// buffers long (at most two channels). Short loops are repeated to at least `MIN_LOOP_SECS`
/// first. The stretch is a linear resampling that wraps around, so the loop point stays seamless.
pub fn gapless_loop(pcm: &Pcm) -> Pcm {
    let ch = pcm.channels.clamp(1, 2) as usize;
    let src_ch = pcm.channels.max(1) as usize;
    let n = pcm.frames();
    if n == 0 || pcm.rate == 0 {
        return Pcm { rate: MIX_RATE, channels: ch as u16, samples: Vec::new() };
    }
    let secs = n as f64 / pcm.rate as f64;
    let repeats = if secs < MIN_LOOP_SECS as f64 { (MIN_LOOP_SECS as f64 / secs).ceil() as usize } else { 1 };
    let total = n * repeats;
    let target = ((total as f64 * MIX_RATE as f64 / pcm.rate as f64 / MIX_BUFFER_FRAMES as f64).ceil() as usize).max(1) * MIX_BUFFER_FRAMES;
    let at = |frame: usize, c: usize| pcm.samples[(frame % n) * src_ch + c];
    let mut samples = Vec::with_capacity(target * ch);
    let step = total as f64 / target as f64;
    for i in 0..target {
        let pos = i as f64 * step;
        let (j, f) = (pos.floor() as usize, (pos - pos.floor()) as f32);
        for c in 0..ch {
            samples.push(at(j, c) * (1.0 - f) + at(j + 1, c) * f);
        }
    }
    Pcm { rate: MIX_RATE, channels: ch as u16, samples }
}

/// A sound file's gapless loop as WAV bytes, or empty bytes when the file already is one (a WAV
/// or Ogg at the mixer's rate, whole buffers long, at least `MIN_LOOP_SECS`).
pub fn loop_ready(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if header_frames(bytes).is_some_and(|(rate, channels, frames)| rate == MIX_RATE && (1..=2).contains(&channels) && frames > 0 && frames % MIX_BUFFER_FRAMES as u64 == 0) {
        return Ok(Vec::new());
    }
    let pcm = decode(bytes)?;
    if pcm.rate == MIX_RATE && pcm.channels <= 2 && pcm.frames() % MIX_BUFFER_FRAMES == 0 && pcm.frames() > 0 {
        return Ok(Vec::new());
    }
    let l = gapless_loop(&pcm);
    if l.samples.is_empty() {
        return Err("no audio".into());
    }
    Ok(wav_bytes_channels(&l.samples, l.rate, l.channels))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// quad-snd's mixing of one looped sound, as it is (0.2.8): buffers of `MIX_BUFFER_FRAMES`
    /// stereo frames; after a rewind the rest is written from the buffer's start again.
    fn quad_snd_mix(data: &[f32], buffers: usize) -> Vec<f32> {
        let mut out = Vec::new();
        let mut pos = 0usize;
        for _ in 0..buffers {
            let mut buffer = vec![0.0f32; MIX_BUFFER_FRAMES * 2];
            let mut remainder = buffer.len();
            loop {
                let chunk = &data[pos.min(data.len())..];
                let chunk = &chunk[..remainder.min(chunk.len())];
                pos += remainder;
                for (b, s) in buffer.iter_mut().zip(chunk) {
                    *b += s;
                }
                remainder -= chunk.len();
                if remainder > 0 {
                    pos = 0;
                    continue;
                }
                break;
            }
            out.extend(buffer);
        }
        out
    }

    #[test]
    fn rain_loop_survives_quad_snds_mixer() {
        let stereo = |m: &[f32]| m.iter().flat_map(|&v| [v, v]).collect::<Vec<f32>>();
        let rain = stereo(&rain_samples());
        let n = rain.len() / (MIX_BUFFER_FRAMES * 2);
        let mixed = quad_snd_mix(&rain, 2 * n + 3);
        let ideal: Vec<f32> = rain.iter().cycle().take(mixed.len()).copied().collect();
        assert_eq!(mixed, ideal, "the rain loops exactly through the mixer");
        // A loop that ends inside a buffer (like the old 6 s at 22 050 Hz, resampled) gets a gap.
        let odd = &rain[..rain.len() - 3000];
        let mixed = quad_snd_mix(odd, n + 2);
        let silent = mixed.chunks(2).skip(odd.len() / 2).take(MIX_BUFFER_FRAMES).filter(|f| f[0] == 0.0).count();
        assert!(silent > 1000, "the mixer bug this avoids: {silent} silent frames after the loop point");
    }

    #[test]
    fn rain_loops_without_a_click() {
        let s = rain_samples();
        assert_eq!(s.len() % MIX_BUFFER_FRAMES, 0, "whole mixer buffers (quad-snd's loop gap)");
        assert!((5.9..6.2).contains(&(s.len() as f32 / MIX_RATE as f32)));
        // At the mixer's rate: loading doesn't resample (which could change the length).
        let wav = wav_bytes(&s, MIX_RATE);
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 44_100);
        let peak = s.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!((0.59..=0.61).contains(&peak), "normalized: {peak}");
        // Loudness stays even across the loop point (no dip after it): RMS of 50 ms windows over
        // two passes.
        let two: Vec<f32> = s.iter().chain(s.iter()).copied().collect();
        let rms: Vec<f32> = two.chunks(1102).map(|c| (c.iter().map(|v| v * v).sum::<f32>() / c.len() as f32).sqrt()).collect();
        let avg = rms.iter().sum::<f32>() / rms.len() as f32;
        let low = rms.iter().cloned().fold(f32::MAX, f32::min);
        assert!(low > avg * 0.8, "no loudness dip: lowest {low} vs average {avg}");
        // The end flows into the start like any two neighbouring samples do.
        let step = |a: f32, b: f32| (a - b).abs();
        let typical = s.windows(2).map(|w| step(w[0], w[1])).sum::<f32>() / s.len() as f32;
        assert!(step(s[s.len() - 1], s[0]) < typical * 6.0, "seam {} vs typical {typical}", step(s[s.len() - 1], s[0]));
    }

    /// A stereo test tone: `secs` long at `rate`, left and right different.
    fn tone(rate: u32, secs: f32) -> Pcm {
        let n = (rate as f32 * secs) as usize;
        let samples = (0..n).flat_map(|i| {
            let t = i as f32 / rate as f32;
            [(t * 440.0 * std::f32::consts::TAU).sin() * 0.5, (t * 220.0 * std::f32::consts::TAU).sin() * 0.25]
        });
        Pcm { rate, channels: 2, samples: samples.collect() }
    }

    #[test]
    fn music_loops_fill_whole_buffers() {
        // 61.3 s at 44.1 kHz: stretched by less than one buffer.
        let long = tone(MIX_RATE, 61.3);
        let l = gapless_loop(&long);
        assert_eq!((l.rate, l.channels, l.frames() % MIX_BUFFER_FRAMES), (MIX_RATE, 2, 0));
        let stretch = l.frames() as f32 / long.frames() as f32;
        assert!((1.0..1.0016).contains(&stretch), "stretch {stretch}");
        // 22.05 kHz, 7 s: resampled and repeated to at least 30 s (5 times).
        let short = tone(22_050, 7.0);
        let l = gapless_loop(&short);
        assert_eq!(l.frames() % MIX_BUFFER_FRAMES, 0);
        let secs = l.frames() as f32 / MIX_RATE as f32;
        assert!((35.0..35.1).contains(&secs), "{secs} s");
        // Mono stays mono; already whole buffers at 44.1 kHz needs nothing.
        let mono = Pcm { rate: MIX_RATE, channels: 1, samples: vec![0.1; MIX_BUFFER_FRAMES * 800] };
        assert!(loop_ready(&wav_bytes(&mono.samples, MIX_RATE)).unwrap().is_empty());
        let odd = wav_bytes(&mono.samples[..MIX_BUFFER_FRAMES * 800 - 77], MIX_RATE);
        let wav = loop_ready(&odd).unwrap();
        let back = decode(&wav).unwrap();
        assert_eq!((back.channels, back.frames()), (1, MIX_BUFFER_FRAMES * 800));
    }

    #[test]
    fn music_loops_play_gapless_through_quad_snds_mixer() {
        // A 41.7 s track (not whole buffers): the mixer leaves a gap; its gapless loop has none.
        let pcm = tone(MIX_RATE, 41.7);
        let mixed = quad_snd_mix(&pcm.samples, pcm.frames() / MIX_BUFFER_FRAMES + 2);
        let silent = mixed.chunks(2).skip(pcm.frames()).take(MIX_BUFFER_FRAMES).filter(|f| f[0] == 0.0 && f[1] == 0.0).count();
        // (It ends 3962 frames into a buffer: the buffer's last 134 frames stay silent.)
        assert!(silent >= 134, "the gap: {silent} silent frames");
        let l = gapless_loop(&pcm);
        let bufs = l.frames() / MIX_BUFFER_FRAMES;
        let mixed = quad_snd_mix(&l.samples, 2 * bufs + 3);
        let ideal: Vec<f32> = l.samples.iter().cycle().take(mixed.len()).copied().collect();
        assert_eq!(mixed, ideal, "the loop repeats exactly");
        // The seam is as smooth as the rest: the step from the last frame to the first.
        let steps: Vec<f32> = l.samples.chunks(2).map(|f| f[0]).collect::<Vec<_>>().windows(2).map(|w| (w[1] - w[0]).abs()).collect();
        let max = steps.iter().cloned().fold(0.0f32, f32::max);
        assert!((l.samples[0] - l.samples[l.samples.len() - 2]).abs() <= max * 1.01);
    }

    #[test]
    fn decodes_wav_formats_and_ogg() {
        let mono = wav_bytes(&[0.0, 0.5, -0.5], 22_050);
        let p = decode(&mono).unwrap();
        assert_eq!((p.rate, p.channels, p.samples.len()), (22_050, 1, 3));
        assert!((p.samples[1] - 0.5).abs() < 1e-3);
        let st = wav_bytes_channels(&[0.25, -0.25, 0.5, -0.5], 48_000, 2);
        assert_eq!(decode(&st).unwrap().frames(), 2);
        assert!(decode(b"RIFF\0\0\0\0WAVEnope").is_err());
        // The example's music (Ogg Vorbis) decodes and is already whole buffers long.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../example_project/sounds/music_village.ogg");
        if let Ok(b) = std::fs::read(root) {
            let p = decode(&b).unwrap();
            assert_eq!((p.rate, p.channels, p.frames() % MIX_BUFFER_FRAMES), (MIX_RATE, 2, 0));
            assert!(loop_ready(&b).unwrap().is_empty());
            assert_eq!(header_frames(&b), Some((MIX_RATE, 2, p.frames() as u64)), "the Ogg header tells the length");
        }
    }

    #[test]
    fn wav_header() {
        let w = wav_bytes(&[0.0, 1.0, -1.0], 22050);
        assert_eq!(check_format(&w), Ok(()));
        assert_eq!(w.len(), 44 + 6);
        assert_eq!(&w[46..48], &i16::MAX.to_le_bytes());
    }

    #[test]
    fn formats_are_told_apart() {
        assert!(check_format(b"definitely not audio").is_err());
        let ogg = |body: &[u8]| [b"OggS\0\x02", &[0u8; 22][..], b"\x01\x1e", &[0u8; 30][..], body].concat();
        assert_eq!(check_format(&ogg(b"\x01vorbis\0\0\0\0")), Ok(()));
        assert!(check_format(&ogg(b"OpusHead\x01")).unwrap_err().contains("Opus"));
        assert!(check_format(&ogg(b"\x7fFLAC\x01")).unwrap_err().contains("FLAC"));
        assert!(check_format(&ogg(b"\x80theora")).unwrap_err().contains("without a Vorbis"));
        assert!(check_format(b"ID3\x04\0\0\0\0\0\0\0\0").unwrap_err().contains("MP3"));
        assert!(check_format(&[&[0xFF, 0xFB, 0x90, 0x00][..], &[0u8; 20][..]].concat()).unwrap_err().contains("MP3"));
        assert!(check_format(b"fLaC\0\0\0\x22\0\0\0\0\0").unwrap_err().contains("FLAC"));
    }
}
