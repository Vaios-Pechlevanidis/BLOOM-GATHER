//! Debug builds only: `BLOOM_VIEWS=<dir>` starts a fresh game in a private
//! save folder, stands at a few fixed spots at fixed times of day, saves a
//! screenshot of each into `<dir>` and quits. For checking the look of the
//! map and the day cycle without playing.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bloom_core::player::{CharacterBody, Environment};

use crate::game::{AppState, Content, RegionScene};
use crate::player::Player;
use crate::rendering::Daylight;

/// Point the save folder at `<views dir>/save` before anything reads it.
pub fn isolate_save_dir() {
    if let Some(dir) = std::env::var_os("BLOOM_VIEWS") {
        // Called at startup before any other thread exists.
        std::env::set_var("BLOOM_SAVE_DIR", PathBuf::from(dir).join("save"));
    }
}

pub struct ViewsPlugin;

impl Plugin for ViewsPlugin {
    fn build(&self, app: &mut App) {
        let Ok(dir) = std::env::var("BLOOM_VIEWS") else {
            return;
        };
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).expect("create views output dir");
        app.insert_resource(Views {
            dir,
            step: 0,
            wait: 3.0,
            enter: false,
        })
        .insert_resource(crate::game::Settings {
            pause_on_focus_loss: false,
            master_volume: 0.0,
            ..default()
        })
        .add_systems(PreUpdate, only_our_input.after(bevy::input::InputSystems))
        .add_systems(Update, drive);
    }
}

#[derive(Resource)]
struct Views {
    dir: PathBuf,
    step: usize,
    wait: f32,
    /// Press Enter next frame.
    enter: bool,
}

/// Whatever the person at the computer does meanwhile is ignored.
fn only_our_input(
    mut views: ResMut<Views>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut buttons: ResMut<ButtonInput<MouseButton>>,
    mut motion: ResMut<bevy::input::mouse::AccumulatedMouseMotion>,
) {
    keys.reset_all();
    buttons.reset_all();
    motion.delta = Vec2::ZERO;
    if std::mem::take(&mut views.enter) {
        keys.press(KeyCode::Enter);
    }
}

/// (name, hour, where to stand (x, z), where to look (x, z, height)).
type Shot = (&'static str, f32, (f32, f32), (f32, f32, f32));
const SHOTS: [Shot; 8] = [
    ("1_morning_track", 8.5, (0.0, 22.0), (0.0, -40.0, 0.6)),
    ("2_noon_furrow", 13.0, (4.55, 15.5), (4.55, -40.0, 0.2)),
    ("3_noon_across", 13.0, (-1.0, 4.0), (22.0, -6.0, 0.0)),
    ("4_afternoon_beds", 16.5, (9.0, 17.0), (-12.0, -30.0, 0.0)),
    ("5_golden_hour", 19.6, (0.0, 14.0), (-8.0, -40.0, 1.5)),
    ("6_sunset", 20.4, (-0.6, 15.0), (-30.0, -36.0, 2.5)),
    ("7_dusk", 21.2, (0.0, 10.0), (0.0, -40.0, 0.8)),
    ("8_night", 0.5, (0.0, 18.0), (2.0, -40.0, 1.5)),
];

#[allow(clippy::too_many_arguments)]
fn drive(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut views: ResMut<Views>,
    state: Res<State<AppState>>,
    mut next: ResMut<NextState<AppState>>,
    player: Option<ResMut<Player>>,
    scene: Option<Res<RegionScene>>,
    content: Res<Content>,
    mut daylight: ResMut<Daylight>,
    mut exit: MessageWriter<AppExit>,
    cameras: Query<&GlobalTransform, With<crate::rendering::MainCamera>>,
) {
    views.wait -= time.delta_secs();
    if views.wait > 0.0 {
        return;
    }
    if *state.get() == AppState::MainMenu {
        // "Begin" a new game.
        views.enter = true;
        views.wait = 2.0;
        return;
    }
    let (Some(mut player), Some(scene)) = (player, scene) else {
        views.wait = 0.5;
        return;
    };
    // Each shot: place and set the time, wait for the flowers and the light
    // to settle, then take the picture.
    let k = views.step / 2;
    if k >= SHOTS.len() {
        info!("VIEWS done: {}", views.dir.display());
        exit.write(AppExit::Success);
        next.set(AppState::MainMenu);
        views.wait = 100.0;
        return;
    }
    let (name, hour, stand, look) = SHOTS[k];
    if views.step % 2 == 0 {
        daylight.hour = hour;
        let layout = &scene.layout;
        let env = Environment {
            terrain: &layout.terrain,
            colliders: &layout.colliders,
        };
        let body = CharacterBody::standing_at(Vec2::new(stand.0, stand.1), &env);
        player.body = body;
        player.previous = body.position;
        let at = Vec2::new(look.0, look.1);
        let target = Vec3::new(at.x, layout.terrain.height_at(at) + look.2, at.y);
        let d = target - body.eye(&content.player);
        player.yaw = (-d.x).atan2(-d.z);
        player.pitch = (d.y / d.length()).asin();
        views.wait = 2.5;
    } else {
        let path = views.dir.join(format!("{name}.png"));
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
        let cam = cameras
            .iter()
            .next()
            .map(|c| (c.translation(), c.forward()));
        info!(
            "VIEWS shot {name} at {} (body {:?}, yaw {:.2}, camera {cam:?})",
            daylight.clock(),
            player.body.position,
            player.yaw
        );
        views.wait = 0.6;
    }
    views.step += 1;
}
