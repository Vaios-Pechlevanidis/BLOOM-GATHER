//! Sound effects, footsteps, each region's ambience and calm music.
//!
//! Until recorded/composed audio exists (polish phase), every effect is
//! synthesised at startup into an in-memory WAV: soft, short and quiet, in
//! keeping with the cozy tone. Nothing here needs a licence. The ambience
//! loop follows the region you are in (meadow, valley stream, forest,
//! night crickets). Two gentle music loops (day and night) are composed by
//! a seeded generator on a background thread at startup and cross-fade as
//! you travel. Effects, music and ambience have their own volume.

use std::f32::consts::TAU;
use std::sync::Arc;

use bevy::audio::{AudioSinkPlayback, Volume};
use bevy::prelude::*;
use bloom_core::data::Ambience;

use crate::game::{ActiveRegion, Content, Settings};

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlaySfx>()
            .add_systems(Startup, (create_sounds, compose_music))
            .add_systems(Update, footsteps)
            .add_systems(
                PostUpdate,
                (play_sounds, receive_music, follow_region, fade_loops).chain(),
            );
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
    /// A footstep on grass, the gravel path, wood (bridges) or in water.
    StepGrass,
    StepGravel,
    StepWood,
    StepWater,
    /// Landing after a jump or a drop.
    Land,
    /// The soft rush of travelling to another region.
    Whoosh,
}

/// Request a sound. `pitch` varies playback speed (1 = normal).
#[derive(Message, Debug, Clone, Copy)]
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
    step_grass: Handle<AudioSource>,
    step_gravel: Handle<AudioSource>,
    step_wood: Handle<AudioSource>,
    step_water: Handle<AudioSource>,
    land: Handle<AudioSource>,
    whoosh: Handle<AudioSource>,
    meadow: Handle<AudioSource>,
    valley: Handle<AudioSource>,
    forest: Handle<AudioSource>,
    night: Handle<AudioSource>,
}

impl Sounds {
    fn ambience(&self, kind: Ambience) -> &Handle<AudioSource> {
        match kind {
            Ambience::Meadow => &self.meadow,
            Ambience::Valley => &self.valley,
            Ambience::Forest => &self.forest,
            Ambience::Night => &self.night,
        }
    }
}

/// A looping background track that fades in, and out before it stops.
#[derive(Component)]
struct LoopTrack {
    kind: Track,
    /// 0–1 fade level (times the channel volume).
    level: f32,
    leaving: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Track {
    Ambience(Ambience),
    Music { night: bool },
}

/// The two music loops, once composed.
#[derive(Resource)]
struct Music {
    day: Handle<AudioSource>,
    night: Handle<AudioSource>,
}

/// Music being composed on a background thread.
#[derive(Resource)]
struct MusicJob(std::sync::Mutex<std::sync::mpsc::Receiver<(Vec<u8>, Vec<u8>)>>);

/// Seconds for a loop to fade in or out.
const FADE_SECS: f32 = 2.5;
/// Music sits below everything else.
const MUSIC_GAIN: f32 = 0.45;

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
        step_grass: add(step(StepKind::Grass)),
        step_gravel: add(step(StepKind::Gravel)),
        step_wood: add(step(StepKind::Wood)),
        step_water: add(step(StepKind::Water)),
        land: add(land()),
        whoosh: add(whoosh()),
        meadow: add(ambience(Ambience::Meadow)),
        valley: add(ambience(Ambience::Valley)),
        forest: add(ambience(Ambience::Forest)),
        night: add(ambience(Ambience::Night)),
    });
}

fn play_sounds(
    mut commands: Commands,
    mut requests: MessageReader<PlaySfx>,
    sounds: Option<Res<Sounds>>,
    settings: Res<Settings>,
) {
    let Some(sounds) = sounds else { return };
    for req in requests.read() {
        if settings.effects() <= 0.0 {
            continue;
        }
        let (handle, gain) = match req.sfx {
            Sfx::Pick => (&sounds.pick, 0.55),
            Sfx::Discover => (&sounds.discover, 0.5),
            Sfx::LevelUp => (&sounds.level, 0.5),
            Sfx::Coins => (&sounds.coins, 0.45),
            Sfx::Deny => (&sounds.deny, 0.5),
            Sfx::Click => (&sounds.click, 0.35),
            Sfx::StepGrass => (&sounds.step_grass, 0.22),
            Sfx::StepGravel => (&sounds.step_gravel, 0.24),
            Sfx::StepWood => (&sounds.step_wood, 0.3),
            Sfx::StepWater => (&sounds.step_water, 0.3),
            Sfx::Land => (&sounds.land, 0.35),
            Sfx::Whoosh => (&sounds.whoosh, 0.35),
        };
        commands.spawn((
            AudioPlayer(handle.clone()),
            PlaybackSettings::DESPAWN
                .with_volume(Volume::Linear(settings.effects() * gain))
                .with_speed(req.pitch.clamp(0.5, 2.0)),
        ));
    }
}

fn compose_music(mut commands: Commands) {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send((wav(&music(false)), wav(&music(true))));
    });
    commands.insert_resource(MusicJob(std::sync::Mutex::new(rx)));
}

fn receive_music(
    mut commands: Commands,
    job: Option<Res<MusicJob>>,
    mut sources: ResMut<Assets<AudioSource>>,
) {
    let Some(job) = job else { return };
    let Ok(rx) = job.0.lock() else { return };
    if let Ok((day, night)) = rx.try_recv() {
        commands.insert_resource(Music {
            day: sources.add(AudioSource {
                bytes: Arc::from(day),
            }),
            night: sources.add(AudioSource {
                bytes: Arc::from(night),
            }),
        });
        commands.remove_resource::<MusicJob>();
    }
}

/// Play the active region's ambience and music (day or night), cross-fading
/// when you travel.
fn follow_region(
    mut commands: Commands,
    sounds: Option<Res<Sounds>>,
    music: Option<Res<Music>>,
    content: Option<Res<Content>>,
    active: Option<Res<ActiveRegion>>,
    daylight: Option<Res<crate::rendering::Daylight>>,
    mut playing: Query<&mut LoopTrack>,
) {
    let (Some(sounds), Some(content), Some(active)) = (sounds, content, active) else {
        return;
    };
    let scenery = &content.regions[active.0].scenery;
    // With a day cycle, crickets and the night tune take over after dark.
    let night = match (&daylight, scenery.day_cycle) {
        (Some(d), Some(_)) => d.night > 0.5,
        _ => scenery.atmosphere.night,
    };
    let ambience = if night && scenery.day_cycle.is_some() {
        Ambience::Night
    } else {
        scenery.ambience
    };
    let mut wanted = vec![(Track::Ambience(ambience), sounds.ambience(ambience).clone())];
    if let Some(music) = &music {
        let handle = if night { &music.night } else { &music.day };
        wanted.push((Track::Music { night }, handle.clone()));
    }
    for mut track in &mut playing {
        let keep = wanted.iter().any(|(k, _)| *k == track.kind);
        if !keep && !track.leaving {
            track.leaving = true;
        }
    }
    for (kind, handle) in wanted {
        if playing.iter().any(|t| t.kind == kind && !t.leaving) {
            continue;
        }
        commands.spawn((
            LoopTrack {
                kind,
                level: 0.0,
                leaving: false,
            },
            AudioPlayer(handle),
            PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
        ));
    }
}

/// Fade loops in and out and follow the volume settings live.
fn fade_loops(
    mut commands: Commands,
    time: Res<Time<Real>>,
    settings: Res<Settings>,
    mut tracks: Query<(Entity, &mut LoopTrack, Option<&mut AudioSink>)>,
) {
    let step = time.delta_secs() / FADE_SECS;
    for (e, mut track, sink) in &mut tracks {
        track.level = if track.leaving {
            (track.level - step).max(0.0)
        } else {
            (track.level + step).min(1.0)
        };
        if track.leaving && track.level <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        let channel = match track.kind {
            Track::Ambience(_) => settings.ambience() * AMBIENCE_GAIN,
            Track::Music { .. } => settings.music() * MUSIC_GAIN,
        };
        if let Some(mut sink) = sink {
            sink.set_volume(Volume::Linear(channel * track.level));
        }
    }
}

/// Footsteps by surface while walking, and a soft thump on landing.
#[allow(clippy::too_many_arguments)]
fn footsteps(
    time: Res<Time>,
    state: Option<Res<State<crate::game::PlayState>>>,
    player: Option<Res<crate::player::Player>>,
    scene: Option<Res<crate::game::RegionScene>>,
    mut walked: Local<f32>,
    mut airborne: Local<f32>,
    mut last: Local<Option<Vec3>>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    let (Some(player), Some(scene)) = (player, scene) else {
        return;
    };
    let playing = state.is_some_and(|s| *s.get() == crate::game::PlayState::Playing);
    let pos = player.body.position;
    let moved = last.map_or(0.0, |l| (pos - l).with_y(0.0).length());
    *last = Some(pos);
    if !playing || moved > 3.0 {
        // Paused, or teleported (travel): no steps.
        *walked = 0.0;
        return;
    }
    if !player.body.grounded {
        *airborne += time.delta_secs();
        return;
    }
    let pitch = 0.9 + (pos.x * 7.3 + pos.z * 3.1).sin().abs() * 0.2;
    if *airborne > 0.3 {
        sfx.write(PlaySfx {
            sfx: Sfx::Land,
            pitch,
        });
        *walked = 0.0;
    }
    *airborne = 0.0;
    *walked += moved;
    // About one step every 0.8 m.
    if *walked < 0.8 {
        return;
    }
    *walked = 0.0;
    let layout = &scene.layout;
    let at = Vec2::new(pos.x, pos.z);
    let ground = layout.terrain.height_at(at);
    let surface = if layout
        .water_level_at(at)
        .is_some_and(|level| pos.y < level + 0.05)
    {
        Sfx::StepWater
    } else if pos.y > ground + 0.1
        && layout
            .bridges
            .iter()
            .any(|b| b.center.distance(at) < b.half_length + 0.3)
    {
        Sfx::StepWood
    } else if layout.distance_to_path(at) < layout.path_half_width {
        Sfx::StepGravel
    } else {
        Sfx::StepGrass
    };
    sfx.write(PlaySfx {
        sfx: surface,
        pitch,
    });
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
fn ambience(kind: Ambience) -> Vec<f32> {
    const LOOP_SECS: f32 = 24.0;
    const FADE_SECS: f32 = 1.0;
    // Wind: louder in the open, deeper in the forest, faint at night.
    let (wind_gain, wind_cut) = match kind {
        Ambience::Meadow | Ambience::Valley => (2.4, 0.05),
        Ambience::Forest => (2.8, 0.03),
        Ambience::Night => (1.3, 0.04),
    };
    let mut noise = Noise(0x2545_F491);
    let (mut low, mut lower) = (0.0f32, 0.0f32);
    let mut out = samples(LOOP_SECS + FADE_SECS, |t| {
        // Brown-ish noise, double low-passed: a soft rush of air.
        low += (noise.next() - low) * 0.02;
        lower += (low - lower) * wind_cut;
        let swell = 0.55 + 0.45 * ((t * 0.21).sin() * 0.6 + (t * 0.083 + 1.0).sin() * 0.4);
        lower * wind_gain * swell
    });
    match kind {
        Ambience::Valley => add_stream(&mut out),
        Ambience::Night => add_crickets(&mut out, LOOP_SECS),
        _ => {}
    }
    // Birds by day; fewer and lower-pitched in the forest; none at night.
    let (low_pitch, spread, gap) = match kind {
        Ambience::Meadow | Ambience::Valley => (2600.0, 1800.0, 3.5),
        Ambience::Forest => (1500.0, 1100.0, 5.5),
        Ambience::Night => (0.0, 0.0, f32::INFINITY),
    };

    // Bird phrases: (start, base pitch, notes, note length).
    let mut rng = Noise(0x0BAD_5EED);
    let mut start = 1.2;
    while start < LOOP_SECS - 2.0 {
        let base = low_pitch + (rng.next() * 0.5 + 0.5) * spread;
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
        start += 2.0 + (rng.next() * 0.5 + 0.5) * gap;
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

/// A babbling stream: band-passed noise that gurgles in little bursts.
fn add_stream(out: &mut [f32]) {
    let mut noise = Noise(0x57EA_3A11);
    let (mut a, mut b, mut bubble, mut level) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / RATE as f32;
        a += (noise.next() - a) * 0.3;
        b += (a - b) * 0.04;
        let band = a - b;
        // Random bubbles: a quick swell now and then.
        if noise.next() > 0.9993 {
            bubble = 1.0;
        }
        bubble *= 0.9992;
        level += ((0.55 + 0.45 * (t * 1.7).sin() * (t * 0.6).cos() + bubble) - level) * 0.001;
        *s += band * 0.16 * level;
    }
}

/// Crickets: short trains of high pulses from a few insects at their own
/// pace.
fn add_crickets(out: &mut [f32], loop_secs: f32) {
    let mut rng = Noise(0xC41C_7E75);
    for k in 0..4 {
        let pitch = 4200.0 + k as f32 * 260.0 + rng.next() * 120.0;
        let period = 0.55 + (rng.next() * 0.5 + 0.5) * 0.5;
        let gain = 0.018 + (rng.next() * 0.5 + 0.5) * 0.014;
        let mut start = rng.next().abs() * period;
        while start < loop_secs - 0.2 {
            for pulse in 0..3 {
                let t0 = start + pulse as f32 * 0.035;
                let i0 = (t0 * RATE as f32) as usize;
                let count = (0.022 * RATE as f32) as usize;
                for i in 0..count {
                    let local = i as f32 / RATE as f32;
                    let env = (local / 0.022 * std::f32::consts::PI).sin();
                    if let Some(s) = out.get_mut(i0 + i) {
                        *s += (TAU * pitch * local).sin() * env * gain;
                    }
                }
            }
            // Every so often a cricket pauses.
            start += period * if rng.next() > 0.85 { 3.0 } else { 1.0 };
        }
    }
}

#[derive(Clone, Copy)]
enum StepKind {
    Grass,
    Gravel,
    Wood,
    Water,
}

/// A soft footstep: a filtered noise crunch, with a knock on wood and a
/// splash in water.
fn step(kind: StepKind) -> Vec<f32> {
    let mut noise = Noise(match kind {
        StepKind::Grass => 0x51E9,
        StepKind::Gravel => 0x6A7E,
        StepKind::Wood => 0x3D0D,
        StepKind::Water => 0x5EA5,
    });
    let (secs, cut, decay) = match kind {
        StepKind::Grass => (0.12, 0.12, 38.0),
        StepKind::Gravel => (0.14, 0.35, 30.0),
        StepKind::Wood => (0.16, 0.25, 45.0),
        StepKind::Water => (0.32, 0.18, 11.0),
    };
    let (mut low, mut lower) = (0.0f32, 0.0f32);
    samples(secs, |t| {
        low += (noise.next() - low) * cut;
        lower += (low - lower) * 0.3;
        let band = low - lower * 0.6;
        let env = (t * 300.0).min(1.0) * (-t * decay).exp();
        let mut v = band * env * 1.6;
        match kind {
            StepKind::Gravel => {
                // Little crackles of stones.
                if noise.next() > 0.985 {
                    v += noise.next() * 0.5 * (-t * 20.0).exp();
                }
            }
            StepKind::Wood => {
                let knock = (TAU * 150.0 * t).sin() * 0.7 + (TAU * 310.0 * t).sin() * 0.3;
                v = v * 0.4 + knock * (-t * 28.0).exp() * 0.6;
            }
            StepKind::Water => {
                // A splash that brightens then settles.
                let sweep = (t * 9.0).sin().abs();
                v *= 0.6 + sweep * 0.6;
            }
            StepKind::Grass => {}
        }
        v * 0.6
    })
}

/// A low, soft thump.
fn land() -> Vec<f32> {
    let mut noise = Noise(0x7A4D);
    let mut low = 0.0f32;
    samples(0.2, |t| {
        low += (noise.next() - low) * 0.06;
        let thump = (TAU * (90.0 - 40.0 * t) * t).sin() * (-t * 22.0).exp();
        (thump * 0.6 + low * (-t * 18.0).exp() * 1.4) * 0.6
    })
}

/// A soft rush of air that swells and fades.
fn whoosh() -> Vec<f32> {
    let mut noise = Noise(0x0F15_7E11);
    let (mut a, mut b) = (0.0f32, 0.0f32);
    samples(0.9, |t| {
        let k = t / 0.9;
        let cut = 0.03 + 0.12 * (k * std::f32::consts::PI).sin();
        a += (noise.next() - a) * cut;
        b += (a - b) * 0.05;
        let env = (k * std::f32::consts::PI).sin().powf(1.5);
        (a - b) * env * 1.2
    })
}

fn midi(note: f32) -> f32 {
    440.0 * 2f32.powf((note - 69.0) / 12.0)
}

/// A calm, looping piece: a soft pad on a four-chord progression, a low
/// bass note per half bar, and a sparse kalimba melody on the pentatonic
/// scale, with a gentle echo. Day is brighter (F major); night is slower
/// and lower (D minor). Deterministic, and seamless at the loop point.
fn music(night: bool) -> Vec<f32> {
    let bpm = if night { 54.0 } else { 66.0 };
    let beat = 60.0 / bpm;
    let bar = beat * 4.0;
    let bars = 16;
    let loop_secs = bar * bars as f32;
    let tail = 5.0;
    let n = ((loop_secs + tail) * RATE as f32) as usize;
    let loop_len = (loop_secs * RATE as f32) as usize;
    let mut out = vec![0.0f32; n];

    // Chord voicings (MIDI) and the melody scale.
    let chords: [[f32; 4]; 4] = if night {
        [
            [50.0, 53.0, 57.0, 60.0],
            [46.0, 50.0, 53.0, 57.0],
            [43.0, 46.0, 50.0, 53.0],
            [45.0, 49.0, 52.0, 57.0],
        ]
    } else {
        [
            [53.0, 57.0, 60.0, 64.0],
            [50.0, 53.0, 57.0, 60.0],
            [46.0, 50.0, 53.0, 57.0],
            [48.0, 52.0, 55.0, 60.0],
        ]
    };
    let scale: Vec<f32> = if night {
        // D minor pentatonic, D4..C6
        vec![62.0, 65.0, 67.0, 69.0, 72.0, 74.0, 77.0, 79.0, 81.0, 84.0]
    } else {
        // F major pentatonic, F4..D6
        vec![65.0, 67.0, 69.0, 72.0, 74.0, 77.0, 79.0, 81.0, 84.0, 86.0]
    };
    let mut add = |start: f32, secs: f32, mut voice: Box<dyn FnMut(f32) -> f32>| {
        let i0 = (start * RATE as f32) as usize;
        let count = (secs * RATE as f32) as usize;
        for i in 0..count {
            if let Some(s) = out.get_mut(i0 + i) {
                *s += voice(i as f32 / RATE as f32);
            }
        }
    };

    let mut rng = Noise(if night { 0x4E16_47A1 } else { 0xDA7_5EED });
    let mut idx: i32 = 4;
    let patterns: [&[u32]; 5] = [
        &[0, 3, 4, 6],
        &[0, 2, 5],
        &[1, 4, 6, 7],
        &[0, 4],
        &[0, 3, 6],
    ];
    for b in 0..bars {
        let start = b as f32 * bar;
        let chord = chords[b % 4];
        // Pad: slow swell, long release, slightly detuned pairs.
        for (k, &note) in chord.iter().enumerate() {
            let f = midi(note + 12.0 * (k == 0) as i32 as f32);
            let span = bar + 2.0;
            add(
                start,
                span,
                Box::new(move |t| {
                    let env = (t / 1.0).min(1.0) * ((span - t) / 2.0).clamp(0.0, 1.0);
                    let wob = 1.0 + 0.0015 * (t * 0.7 + k as f32).sin();
                    ((TAU * f * t * wob).sin() + (TAU * f * 1.003 * t).sin() * 0.6) * env * 0.022
                }),
            );
        }
        // Bass on beats 1 and 3.
        for half in 0..2 {
            let f = midi(chord[0] - 12.0);
            add(
                start + half as f32 * beat * 2.0,
                2.2,
                Box::new(move |t| {
                    let env = (t * 60.0).min(1.0) * (-t * 1.6).exp();
                    ((TAU * f * t).sin() + (TAU * f * 2.0 * t).sin() * 0.15) * env * 0.11
                }),
            );
        }
        // Melody: a sparse rhythm; a gentle random walk on the scale that
        // lands on a chord tone at the start of each bar. Every fourth bar
        // rests, so the music breathes.
        if b % 4 == 3 {
            continue;
        }
        let pattern =
            patterns[((rng.next() * 0.5 + 0.5) * patterns.len() as f32) as usize % patterns.len()];
        for (j, &eighth) in pattern.iter().enumerate() {
            if j == 0 {
                // Nearest scale note sharing a pitch class with the chord.
                let pcs: Vec<i32> = chord.iter().map(|c| *c as i32 % 12).collect();
                if let Some(best) = (0..scale.len() as i32)
                    .filter(|&i| pcs.contains(&(scale[i as usize] as i32 % 12)))
                    .min_by_key(|&i| (i - idx).abs())
                {
                    idx = best;
                }
            } else {
                let step =
                    if rng.next() > 0.0 { 1 } else { -1 } * if rng.next() > 0.6 { 2 } else { 1 };
                idx = (idx + step).clamp(1, scale.len() as i32 - 2);
            }
            let note = scale[idx as usize] - if night { 12.0 } else { 0.0 };
            let f = midi(note);
            let gain = if night { 0.075 } else { 0.085 } * (0.8 + 0.2 * (rng.next() * 0.5 + 0.5));
            let decay = if night { 1.6 } else { 2.4 };
            add(
                start + eighth as f32 * beat / 2.0,
                2.8,
                Box::new(move |t| {
                    let env = (t * 250.0).min(1.0) * (-t * decay).exp();
                    let body =
                        (TAU * f * t).sin() + (TAU * f * 2.0 * t).sin() * 0.25 * (-t * 6.0).exp();
                    // A faint inharmonic shimmer, like a kalimba tine.
                    let tine = (TAU * f * 5.4 * t).sin() * 0.06 * (-t * 14.0).exp();
                    (body + tine) * env * gain
                }),
            );
        }
    }

    // A soft echo for space.
    let (delay, feedback) = if night { (0.55, 0.42) } else { (0.42, 0.32) };
    let d = (delay * RATE as f32) as usize;
    for i in d..n {
        out[i] += out[i - d] * feedback;
    }
    // Wrap the tail into the head so the loop is seamless, then normalise.
    for i in 0..(n - loop_len) {
        out[i] += out[loop_len + i];
    }
    out.truncate(loop_len);
    let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs())).max(1e-6);
    let scale_to = 0.6 / peak;
    for v in &mut out {
        *v *= scale_to;
    }
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
    fn footsteps_and_whoosh_are_short_and_never_clip() {
        for s in [
            step(StepKind::Grass),
            step(StepKind::Gravel),
            step(StepKind::Wood),
            step(StepKind::Water),
            land(),
            whoosh(),
        ] {
            assert!(s.len() < RATE as usize);
            assert!(s.iter().all(|v| v.abs() <= 1.0));
            assert!(s.iter().any(|v| v.abs() > 0.01), "audible");
        }
    }

    #[test]
    fn music_loops_seamlessly_without_clipping() {
        for night in [false, true] {
            let m = music(night);
            let bpm = if night { 54.0 } else { 66.0 };
            let secs = 16.0 * 4.0 * 60.0 / bpm;
            assert_eq!(m.len(), (secs * RATE as f32) as usize);
            let peak = m.iter().fold(0.0f32, |a, v| a.max(v.abs()));
            assert!((peak - 0.6).abs() < 1e-3, "{peak}");
            let seam = (m[0] - m[m.len() - 1]).abs();
            let typical = m
                .windows(2)
                .map(|w| (w[1] - w[0]).abs())
                .fold(0.0f32, f32::max);
            assert!(seam <= typical, "seam {seam} vs {typical}");
            // Deterministic.
            assert_eq!(m, music(night));
        }
    }

    #[test]
    fn ambience_loops_seamlessly_and_stays_quiet() {
        for kind in [
            Ambience::Meadow,
            Ambience::Valley,
            Ambience::Forest,
            Ambience::Night,
        ] {
            check_loop(&ambience(kind));
        }
    }

    fn check_loop(a: &[f32]) {
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
