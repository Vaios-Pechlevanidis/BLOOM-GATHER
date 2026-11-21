//! Sound effects.
//!
//! Until recorded/composed audio exists (polish phase), every effect is
//! synthesised at startup into an in-memory WAV: soft, short and quiet, in
//! keeping with the cozy tone. Nothing here needs a licence.

use std::f32::consts::TAU;
use std::sync::Arc;

use bevy::audio::{AudioSinkPlayback, Volume};
use bevy::prelude::*;

use crate::game::Settings;

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<PlaySfx>()
            .add_systems(Startup, (create_sounds, start_ambience).chain())
            .add_systems(PostUpdate, (play_sounds, ambience_volume));
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
    meadow: Handle<AudioSource>,
}

/// The looping background ambience.
#[derive(Component)]
struct Ambience;

/// Background ambience sits well below the effects.
const AMBIENCE_GAIN: f32 = 0.35;

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
        meadow: add(meadow_ambience()),
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

fn start_ambience(mut commands: Commands, sounds: Res<Sounds>, settings: Res<Settings>) {
    commands.spawn((
        Ambience,
        AudioPlayer(sounds.meadow.clone()),
        PlaybackSettings::LOOP.with_volume(Volume::Linear(settings.sound_volume * AMBIENCE_GAIN)),
    ));
}

/// Follow the volume setting live.
fn ambience_volume(settings: Res<Settings>, mut sinks: Query<&mut AudioSink, With<Ambience>>) {
    if !settings.is_changed() {
        return;
    }
    for mut sink in &mut sinks {
        sink.set_volume(Volume::Linear(settings.sound_volume * AMBIENCE_GAIN));
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

/// 24 s seamless loop: a breeze that swells and settles, with a few
/// birds singing short phrases at different pitches.
fn meadow_ambience() -> Vec<f32> {
    const LOOP_SECS: f32 = 24.0;
    const FADE_SECS: f32 = 1.0;
    let mut noise = Noise(0x2545_F491);
    let (mut low, mut lower) = (0.0f32, 0.0f32);
    let mut out = samples(LOOP_SECS + FADE_SECS, |t| {
        // Brown-ish noise, double low-passed: a soft rush of air.
        low += (noise.next() - low) * 0.02;
        lower += (low - lower) * 0.05;
        let swell = 0.55 + 0.45 * ((t * 0.21).sin() * 0.6 + (t * 0.083 + 1.0).sin() * 0.4);
        lower * 2.4 * swell
    });

    // Bird phrases: (start, base pitch, notes, note length).
    let mut rng = Noise(0x0BAD_5EED);
    let mut start = 1.2;
    while start < LOOP_SECS - 2.0 {
        let base = 2600.0 + (rng.next() * 0.5 + 0.5) * 1800.0;
        let notes = 3 + ((rng.next() * 0.5 + 0.5) * 4.0) as usize;
        let len = 0.07 + (rng.next() * 0.5 + 0.5) * 0.06;
        let gain = 0.05 + (rng.next() * 0.5 + 0.5) * 0.05;
        for n in 0..notes {
            let t0 = start + n as f32 * (len * 1.35);
            let up = if n % 2 == 0 { 1.0 } else { -1.0 };
            let f0 = base * (1.0 + 0.08 * (n as f32 * 1.7).sin());
            let i0 = (t0 * RATE as f32) as usize;
            let count = (len * RATE as f32) as usize;
            let mut phase = 0.0f32;
            for i in 0..count {
                let local = i as f32 / RATE as f32;
                let k = local / len;
                let f = f0 * (1.0 + up * 0.25 * k) + (local * 90.0).sin() * 60.0;
                phase += TAU * f / RATE as f32;
                let env = (k * std::f32::consts::PI).sin().powf(1.5);
                if let Some(s) = out.get_mut(i0 + i) {
                    *s += phase.sin() * env * gain;
                }
            }
        }
        start += 2.0 + (rng.next() * 0.5 + 0.5) * 3.5;
    }

    // Crossfade the extra tail into the head so the loop has no seam.
    let loop_len = (LOOP_SECS * RATE as f32) as usize;
    let fade = (FADE_SECS * RATE as f32) as usize;
    for i in 0..fade {
        let k = i as f32 / fade as f32;
        out[i] = out[i] * k + out[loop_len + i] * (1.0 - k);
    }
    out.truncate(loop_len);
    out
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

    #[test]
    fn ambience_loops_seamlessly_and_stays_quiet() {
        let a = meadow_ambience();
        assert_eq!(a.len(), (24.0 * RATE as f32) as usize);
        let peak = a.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(peak < 0.9, "peak {peak}");
        // The jump across the loop point is no bigger than a normal step.
        let seam = (a[0] - a[a.len() - 1]).abs();
        let typical = a
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0f32, f32::max);
        assert!(seam <= typical, "seam {seam} vs max step {typical}");
    }
}
