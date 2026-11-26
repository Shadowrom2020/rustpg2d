//! Sounds: load a WAV or Ogg Vorbis file's bytes, play it (once or looped), change its loudness,
//! stop it. A sound can play several times at once; stopping and loudness reach all of them
//! (the behaviour of macroquad's mixer, which the engine's `audio.rs` was written for).
//!
//! The mixer is kira's. The sound device is opened with the first sound that is loaded; without
//! one (or when it can't be opened) sounds load and stay silent.

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};
use kira::sound::PlaybackState;
use kira::{AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Tween};

use crate::Error;

struct Entry {
    data: StaticSoundData,
    playing: Vec<StaticSoundHandle>,
}

struct Mixer {
    /// `None`: no sound device (the web: not yet, see [`user_gesture`]).
    manager: Option<AudioManager<DefaultBackend>>,
    sounds: HashMap<u64, Entry>,
    /// The web: loops started before the browser allowed sound (the music), to start with it.
    waiting: Vec<(u64, PlaySoundParams)>,
}

static MIXER: Mutex<Option<Mixer>> = Mutex::new(None);
static NEXT: AtomicU64 = AtomicU64::new(1);

fn with_mixer<R>(f: impl FnOnce(&mut Mixer) -> R) -> R {
    let mut m = MIXER.lock().unwrap_or_else(|e| e.into_inner());
    let mixer = m.get_or_insert_with(|| Mixer { manager: if cfg!(target_arch = "wasm32") { None } else { open_device() }, sounds: HashMap::new(), waiting: Vec::new() });
    f(mixer)
}

/// Opens the sound device. (One that panics while it is opened counts as none.)
fn open_device() -> Option<AudioManager<DefaultBackend>> {
    std::panic::catch_unwind(|| AudioManager::<DefaultBackend>::new(AudioManagerSettings::default())).ok().and_then(|r| r.map_err(|e| crate::logging::write("warn", &format!("[audio] no sound device: {e}"))).ok())
}

/// The player pressed a key or a button. Browsers allow sound only from then on: the web opens
/// its sound device now and starts the loops that were asked for before. Nothing elsewhere.
pub fn user_gesture() {
    #[cfg(target_arch = "wasm32")]
    {
        let waiting = with_mixer(|m| {
            if m.manager.is_some() {
                return Vec::new();
            }
            m.manager = open_device();
            std::mem::take(&mut m.waiting)
        });
        for (id, params) in waiting {
            play_id(id, params);
        }
    }
}

struct Handle(u64);

impl Drop for Handle {
    fn drop(&mut self) {
        if let Ok(mut m) = MIXER.lock() {
            if let Some(mixer) = m.as_mut() {
                if let Some(mut e) = mixer.sounds.remove(&self.0) {
                    for h in &mut e.playing {
                        h.stop(Tween::default());
                    }
                }
            }
        }
    }
}

/// A loaded sound. Clones are the same sound; it is freed (and stops) with the last one.
#[derive(Clone)]
pub struct Sound(Arc<Handle>);

impl std::fmt::Debug for Sound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Sound(#{})", self.0 .0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PlaySoundParams {
    pub looped: bool,
    /// 0 = silent, 1 = as recorded.
    pub volume: f32,
}

impl Default for PlaySoundParams {
    fn default() -> PlaySoundParams {
        PlaySoundParams { looped: false, volume: 1.0 }
    }
}

/// Loudness as kira counts it: decibels, with everything below a thousandth silent.
fn decibels(volume: f32) -> Decibels {
    if volume <= 0.001 {
        Decibels::SILENCE
    } else {
        Decibels(20.0 * volume.log10())
    }
}

/// Decodes a sound file's bytes (WAV or Ogg Vorbis). Ready at once; a future because the engine
/// awaits it.
pub async fn load_sound_from_bytes(data: &[u8]) -> Result<Sound, Error> {
    let decoded = StaticSoundData::from_cursor(Cursor::new(data.to_vec())).map_err(|e| Error::File(format!("sound: {e}")))?;
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    with_mixer(|m| m.sounds.insert(id, Entry { data: decoded, playing: Vec::new() }));
    Ok(Sound(Arc::new(Handle(id))))
}

/// Starts the sound (again, beside the times it already plays).
pub fn play_sound(sound: &Sound, params: PlaySoundParams) {
    play_id(sound.0 .0, params);
}

fn play_id(id: u64, params: PlaySoundParams) {
    with_mixer(|m| {
        if m.manager.is_none() && cfg!(target_arch = "wasm32") && params.looped {
            m.waiting.push((id, params));
        }
        let (Some(manager), Some(e)) = (m.manager.as_mut(), m.sounds.get_mut(&id)) else { return };
        e.playing.retain(|h| h.state() != PlaybackState::Stopped);
        let mut data = e.data.volume(decibels(params.volume));
        if params.looped {
            data = data.loop_region(..);
        }
        match manager.play(data) {
            Ok(h) => e.playing.push(h),
            Err(err) => crate::logging::write("warn", &format!("[audio] {err}")),
        }
    });
}

/// Stops every playing of the sound.
pub fn stop_sound(sound: &Sound) {
    with_mixer(|m| {
        m.waiting.retain(|(id, _)| *id != sound.0 .0);
        if let Some(e) = m.sounds.get_mut(&sound.0 .0) {
            for mut h in e.playing.drain(..) {
                h.stop(Tween::default());
            }
        }
    });
}

/// Sets the loudness of every playing of the sound.
pub fn set_sound_volume(sound: &Sound, volume: f32) {
    with_mixer(|m| {
        for (id, params) in &mut m.waiting {
            if *id == sound.0 .0 {
                params.volume = volume;
            }
        }
        if let Some(e) = m.sounds.get_mut(&sound.0 .0) {
            for h in &mut e.playing {
                h.set_volume(decibels(volume), Tween::default());
            }
        }
    });
}
