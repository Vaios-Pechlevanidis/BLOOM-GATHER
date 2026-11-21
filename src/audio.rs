//! Sound effects.
//!
//! Until recorded/composed audio exists (polish phase), every effect is
//! synthesised at startup into an in-memory WAV: soft, short and quiet, in
//! keeping with the cozy tone. Nothing here needs a licence.

use std::f32::consts::TAU;
use std::sync::Arc;

use bevy::audio::Volume;
use bevy::prelude::*;

use crate::game::Settings;

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<PlaySfx>()
            .add_systems(Startup, create_sounds)
            .add_systems(PostUpdate, play_sounds);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sfx {
    /// A flower is picked.
    Pick,
    /// A new species enters the collection.
    Discover,
    LevelUp,
    /// Coins from a sale.
    Coins,
    /// An action isn't possible right now.
    Deny,
    /// Menu/UI confirmation.
    Click,
}

/// Request a sound. `pitch` varies playback speed (1 = normal).
#[derive(Event, Debug, Clone, Copy)]
pub struct PlaySfx {
    pub sfx: Sfx,
    pub pitch: f32,
}

impl PlaySfx {
    pub fn new(sfx: Sfx) -> Self {
        Self { sfx, pitch: 1.0 }
    }
}

#[derive(Resource)]
struct Sounds {
    pick: Handle<AudioSource>,
    discover: Handle<AudioSource>,
    level: Handle<AudioSource>,
    coins: Handle<AudioSource>,
    deny: Handle<AudioSource>,
    click: Handle<AudioSource>,
}

const RATE: u32 = 44_100;

fn create_sounds(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    let mut add = |samples: Vec<f32>| {
        sources.add(AudioSource {
            bytes: Arc::from(wav(&samples)),
        })
    };
    commands.insert_resource(Sounds {
        pick: add(pick()),
        discover: add(chime(&[1318.5, 1760.0], 0.11, 0.9)),
        level: add(chime(&[523.25, 659.25, 783.99, 1046.5], 0.085, 1.0)),
        coins: add(coins()),
        deny: add(deny()),
        click: add(click()),
    });
}

fn play_sounds(
    mut commands: Commands,
    mut requests: EventReader<PlaySfx>,
    sounds: Option<Res<Sounds>>,
    settings: Res<Settings>,
) {
    let Some(sounds) = sounds else { return };
    for req in requests.read() {
        if settings.sound_volume <= 0.0 {
            continue;
        }
        let (handle, gain) = match req.sfx {
            Sfx::Pick => (&sounds.pick, 0.55),
            Sfx::Discover => (&sounds.discover, 0.5),
            Sfx::LevelUp => (&sounds.level, 0.5),
            Sfx::Coins => (&sounds.coins, 0.45),
            Sfx::Deny => (&sounds.deny, 0.5),
            Sfx::Click => (&sounds.click, 0.35),
        };
        commands.spawn((
            AudioPlayer(handle.clone()),
            PlaybackSettings::DESPAWN
                .with_volume(Volume::Linear(settings.sound_volume * gain))
                .with_speed(req.pitch.clamp(0.5, 2.0)),
        ));
    }
}

// ---------------------------------------------------------------------------
// Synthesis
// ---------------------------------------------------------------------------

fn samples(secs: f32, mut f: impl FnMut(f32) -> f32) -> Vec<f32> {
    let n = (secs * RATE as f32) as usize;
    (0..n).map(|i| f(i as f32 / RATE as f32)).collect()
}

/// Small deterministic noise source.
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0 as f32 / u32::MAX as f32 * 2.0 - 1.0
    }
}

/// A soft stem "snap" plus a short leafy rustle.
fn pick() -> Vec<f32> {
    let mut noise = Noise(0x9E37_79B9);
    let mut low = 0.0;
    let mut phase = 0.0;
    samples(0.22, |t| {
        let freq = 420.0 + 380.0 * (-t * 40.0).exp();
        phase += TAU * freq / RATE as f32;
        let pop = phase.sin() * (-t * 32.0).exp() * (t * 900.0).min(1.0);
        low += (noise.next() - low) * 0.08; // gentle low-pass
        let rustle = low * (-t * 22.0).exp() * 0.9;
        (pop * 0.7 + rustle) * 0.8
    })
}

/// Bell-like notes played one after another.
fn chime(notes: &[f32], gap: f32, decay_secs: f32) -> Vec<f32> {
    let total = gap * notes.len() as f32 + decay_secs;
    samples(total, |t| {
        let mut v = 0.0;
        for (i, &f) in notes.iter().enumerate() {
            let local = t - i as f32 * gap;
            if local < 0.0 {
                continue;
            }
            let env = (local * 400.0).min(1.0) * (-local * 4.5 / decay_secs).exp();
            v += env
                * ((TAU * f * local).sin() * 0.6
                    + (TAU * f * 2.01 * local).sin() * 0.25
                    + (TAU * f * 3.0 * local).sin() * 0.1);
        }
        v * 0.45
    })
}

fn coins() -> Vec<f32> {
    samples(0.4, |t| {
        let hit = |start: f32, f: f32| {
            let l = t - start;
            if l < 0.0 {
                return 0.0;
            }
            (l * 600.0).min(1.0)
                * (-l * 14.0).exp()
                * ((TAU * f * l).sin() + (TAU * f * 1.5 * l).sin() * 0.5)
        };
        (hit(0.0, 2093.0) + hit(0.07, 2637.0)) * 0.3
    })
}

fn deny() -> Vec<f32> {
    samples(0.16, |t| {
        let f = 190.0 - t * 300.0;
        (TAU * f * t).sin() * (-t * 22.0).exp() * (t * 500.0).min(1.0) * 0.6
    })
}

fn click() -> Vec<f32> {
    samples(0.05, |t| {
        (TAU * 1400.0 * t).sin() * (-t * 90.0).exp() * 0.35
    })
}

/// 16-bit mono PCM WAV.
fn wav(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_is_valid() {
        let w = wav(&[0.0, 0.5, -0.5]);
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(&w[8..12], b"WAVE");
        assert_eq!(u32::from_le_bytes(w[40..44].try_into().unwrap()), 6);
        assert_eq!(w.len(), 44 + 6);
    }

    #[test]
    fn sounds_are_short_and_never_clip() {
        for s in [
            pick(),
            chime(&[1318.5, 1760.0], 0.11, 0.9),
            coins(),
            deny(),
            click(),
        ] {
            assert!(s.len() < RATE as usize * 2);
            assert!(s.iter().all(|v| v.abs() <= 1.0));
        }
    }
}
