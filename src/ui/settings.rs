//! The settings screen, opened from the pause menu or the title screen.
//!
//! Four tabs: Controls (mouse, comfort), Video (quality, window, UI size,
//! minimap), Audio (master, effects, music, ambience) and Keys (rebind the
//! main actions; conflicts swap, menu keys stay fixed). Everything works
//! from the keyboard: 1-4 tabs, Up/Down rows, Left/Right change, Enter
//! toggles or rebinds, Esc closes. Changes apply at once and are saved.

use bevy::prelude::*;

use super::theme::{self, font, palette, radius, space};
use super::widgets::{spawn_button, ButtonVariant};
use crate::audio::{PlaySfx, Sfx};
use crate::game::{AppState, Quality, Settings};
use crate::input::{key_name, rebind, Action, Bindings, KeyOverrides, RebindError};

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SettingsScreen>()
            .add_systems(OnExit(AppState::InGame), close)
            .add_systems(OnExit(AppState::MainMenu), close)
            .add_systems(
                Update,
                (keys, buttons, sync_overlay, rebuild)
                    .chain()
                    .run_if(|s: Res<SettingsScreen>| s.open),
            );
    }
}

/// Whether the settings screen is open (other screens check this so their
/// keys don't fire underneath).
#[derive(Resource, Default)]
pub struct SettingsScreen {
    pub open: bool,
    tab: Tab,
    row: usize,
    /// Waiting for a key to bind to this action.
    waiting: Option<Action>,
    notice: Option<String>,
    stale: bool,
}

impl SettingsScreen {
    pub fn show(&mut self) {
        *self = Self {
            open: true,
            stale: true,
            ..default()
        };
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    Controls,
    Video,
    Audio,
    Keys,
}

impl Tab {
    const ALL: [Tab; 4] = [Tab::Controls, Tab::Video, Tab::Audio, Tab::Keys];

    fn label(self) -> &'static str {
        match self {
            Tab::Controls => "1  Controls",
            Tab::Video => "2  Video",
            Tab::Audio => "3  Audio",
            Tab::Keys => "4  Keys",
        }
    }

    fn rows(self) -> Vec<Row> {
        match self {
            Tab::Controls => vec![
                Row::Sensitivity,
                Row::InvertY,
                Row::Fov,
                Row::HeadBob,
                Row::FocusPause,
            ],
            Tab::Video => vec![
                Row::Quality,
                Row::Fullscreen,
                Row::Vsync,
                Row::UiScale,
                Row::Minimap,
            ],
            Tab::Audio => vec![Row::Master, Row::Effects, Row::Music, Row::Ambience],
            Tab::Keys => Action::ALL
                .into_iter()
                .map(Row::Key)
                .chain([Row::ResetKeys])
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Row {
    Sensitivity,
    InvertY,
    Fov,
    HeadBob,
    FocusPause,
    Quality,
    Fullscreen,
    Vsync,
    UiScale,
    Minimap,
    Master,
    Effects,
    Music,
    Ambience,
    Key(Action),
    ResetKeys,
}

/// How a row is changed.
#[derive(PartialEq, Eq)]
enum Control {
    /// Left/Right (or -/+) steps a value.
    Stepper,
    /// Enter (or the button) flips it.
    Toggle,
    /// Enter (or the button) does something.
    Press(&'static str),
}

impl Row {
    fn label(self) -> &'static str {
        match self {
            Row::Sensitivity => "Mouse sensitivity",
            Row::InvertY => "Invert look",
            Row::Fov => "Field of view",
            Row::HeadBob => "Head movement",
            Row::FocusPause => "Pause when the window loses focus",
            Row::Quality => "Graphics quality",
            Row::Fullscreen => "Fullscreen",
            Row::Vsync => "VSync",
            Row::UiScale => "Interface size",
            Row::Minimap => "Minimap",
            Row::Master => "Master volume",
            Row::Effects => "Effects",
            Row::Music => "Music",
            Row::Ambience => "Ambience",
            Row::Key(a) => a.label(),
            Row::ResetKeys => "Reset all keys to defaults",
        }
    }

    fn control(self) -> Control {
        match self {
            Row::InvertY | Row::HeadBob | Row::FocusPause | Row::Fullscreen | Row::Vsync => {
                Control::Toggle
            }
            Row::Minimap => Control::Toggle,
            Row::Key(_) => Control::Press("Change"),
            Row::ResetKeys => Control::Press("Reset"),
            _ => Control::Stepper,
        }
    }

    fn value(self, s: &Settings, b: &Bindings) -> String {
        let on = |v: bool| if v { "On" } else { "Off" }.to_string();
        let pct = |v: f32| format!("{:.0}%", v * 100.0);
        match self {
            Row::Sensitivity => format!("{:.1}", s.mouse_sensitivity),
            Row::InvertY => on(s.invert_y),
            Row::Fov => format!("{:.0} deg", s.fov_degrees),
            Row::HeadBob => on(s.head_bob),
            Row::FocusPause => on(s.pause_on_focus_loss),
            Row::Quality => s.quality.label().to_string(),
            Row::Fullscreen => on(s.fullscreen),
            Row::Vsync => on(s.vsync),
            Row::UiScale => pct(s.ui_scale),
            Row::Minimap => on(s.show_minimap),
            Row::Master => pct(s.master_volume),
            Row::Effects => pct(s.sound_volume),
            Row::Music => pct(s.music_volume),
            Row::Ambience => pct(s.ambience_volume),
            Row::Key(a) => key_name(b.primary(a)).unwrap_or("?").to_string(),
            Row::ResetKeys => String::new(),
        }
    }

    /// What graphics quality means, shown under the row.
    fn hint(self, s: &Settings) -> Option<&'static str> {
        match self {
            Row::Quality => Some(match s.quality {
                Quality::Low => "No shadows or glow, less grass, shorter view of it.",
                Quality::Medium => "Shadows and glow, most of the grass.",
                Quality::High => "Everything: sharp shadows, soft glow, all the grass.",
            }),
            _ => None,
        }
    }

    fn step(self, s: &mut Settings, steps: f32) {
        match self {
            Row::Sensitivity => s.adjust_sensitivity(steps),
            Row::Fov => s.adjust_fov(steps),
            Row::Quality => s.cycle_quality(steps),
            Row::UiScale => s.adjust_ui_scale(steps),
            Row::Master => Settings::step_volume(&mut s.master_volume, steps),
            Row::Effects => Settings::step_volume(&mut s.sound_volume, steps),
            Row::Music => Settings::step_volume(&mut s.music_volume, steps),
            Row::Ambience => Settings::step_volume(&mut s.ambience_volume, steps),
            _ => {}
        }
    }

    fn toggle(self, s: &mut Settings) {
        match self {
            Row::InvertY => s.invert_y = !s.invert_y,
            Row::HeadBob => s.head_bob = !s.head_bob,
            Row::FocusPause => s.pause_on_focus_loss = !s.pause_on_focus_loss,
            Row::Fullscreen => s.fullscreen = !s.fullscreen,
            Row::Vsync => s.vsync = !s.vsync,
            Row::Minimap => s.show_minimap = !s.show_minimap,
            _ => {}
        }
    }
}

#[derive(Component)]
struct Root;

#[derive(Component)]
struct Body;

#[derive(Component, Clone, Copy)]
enum Act {
    Tab(Tab),
    Step(usize, f32),
    Activate(usize),
    Close,
}

fn close(mut screen: ResMut<SettingsScreen>) {
    screen.open = false;
    screen.waiting = None;
}

/// Activate a row: toggle, start rebinding or reset keys.
fn activate(screen: &mut SettingsScreen, settings: &mut Settings, row: Row) {
    match row.control() {
        Control::Toggle => row.toggle(settings),
        Control::Press(_) => match row {
            Row::Key(action) => {
                screen.waiting = Some(action);
                screen.notice = Some(format!("Press a key for {} (Esc cancels)", action.label()));
            }
            Row::ResetKeys => {
                settings.keys = KeyOverrides::new();
                screen.notice = Some("All keys are back to their defaults.".into());
            }
            _ => {}
        },
        Control::Stepper => row.step(settings, 1.0),
    }
    screen.stale = true;
}

fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut screen: ResMut<SettingsScreen>,
    mut settings: ResMut<Settings>,
    mut sfx: EventWriter<PlaySfx>,
) {
    if let Some(action) = screen.waiting {
        let Some(&key) = keys.get_just_pressed().next() else {
            return;
        };
        screen.waiting = None;
        screen.stale = true;
        if key == KeyCode::Escape {
            screen.notice = None;
            return;
        }
        let mut overrides = settings.keys.clone();
        match rebind(&mut overrides, action, key) {
            Ok(swapped) => {
                settings.keys = overrides;
                let name = key_name(key).unwrap_or("?");
                screen.notice = Some(match swapped {
                    Some(other) => format!(
                        "{} is now {name}; {} took its old key.",
                        action.label(),
                        other.label()
                    ),
                    None => format!("{} is now {name}.", action.label()),
                });
                sfx.write(PlaySfx::new(Sfx::Click));
            }
            Err(RebindError::Reserved) => {
                screen.notice =
                    Some("That key is kept for menus (Esc, Enter, F3, arrows, 1-4).".into());
                sfx.write(PlaySfx::new(Sfx::Deny));
            }
        }
        return;
    }
    let rows = screen.tab.rows();
    let mut moved = false;
    for (i, digit) in [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
    ]
    .into_iter()
    .enumerate()
    {
        if keys.just_pressed(digit) && screen.tab != Tab::ALL[i] {
            screen.tab = Tab::ALL[i];
            screen.row = 0;
            screen.notice = None;
            moved = true;
        }
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        screen.row = (screen.row + 1).min(rows.len() - 1);
        moved = true;
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        screen.row = screen.row.saturating_sub(1);
        moved = true;
    }
    let row = screen.tab.rows()[screen.row.min(screen.tab.rows().len() - 1)];
    for (key, steps) in [(KeyCode::ArrowLeft, -1.0), (KeyCode::ArrowRight, 1.0)] {
        if keys.just_pressed(key) && row.control() == Control::Stepper {
            row.step(&mut settings, steps);
            moved = true;
        }
    }
    if keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter]) {
        activate(&mut screen, &mut settings, row);
        sfx.write(PlaySfx::new(Sfx::Click));
    }
    if keys.just_pressed(KeyCode::Escape) {
        screen.open = false;
        sfx.write(PlaySfx::new(Sfx::Click));
    }
    if moved {
        screen.stale = true;
    }
}

fn buttons(
    pressed: Query<(&Interaction, &Act), Changed<Interaction>>,
    mut screen: ResMut<SettingsScreen>,
    mut settings: ResMut<Settings>,
    mut sfx: EventWriter<PlaySfx>,
) {
    for (interaction, act) in &pressed {
        if *interaction != Interaction::Pressed {
            continue;
        }
        sfx.write(PlaySfx::new(Sfx::Click));
        let rows = screen.tab.rows();
        match *act {
            Act::Tab(t) => {
                screen.tab = t;
                screen.row = 0;
                screen.notice = None;
            }
            Act::Step(i, steps) => {
                screen.row = i;
                rows[i].step(&mut settings, steps);
            }
            Act::Activate(i) => {
                screen.row = i;
                activate(&mut screen, &mut settings, rows[i]);
            }
            Act::Close => screen.open = false,
        }
        screen.stale = true;
    }
}

/// Spawn the overlay when opened; remove it when closed.
fn sync_overlay(
    mut commands: Commands,
    screen: Res<SettingsScreen>,
    roots: Query<Entity, With<Root>>,
) {
    let exists = !roots.is_empty();
    if screen.open && !exists {
        commands
            .spawn((
                Root,
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(palette::SCRIM),
                GlobalZIndex(30),
            ))
            .with_children(|root| {
                root.spawn((
                    Body,
                    theme::card(Node {
                        width: Val::Px(920.0),
                        max_width: Val::Percent(96.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(space::XL)),
                        row_gap: Val::Px(space::SM),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    }),
                ));
            });
    }
}

fn rebuild(
    mut commands: Commands,
    mut screen: ResMut<SettingsScreen>,
    settings: Res<Settings>,
    bindings: Res<Bindings>,
    body: Query<(Entity, Option<&Children>), With<Body>>,
    roots: Query<Entity, With<Root>>,
) {
    if !screen.open {
        for e in &roots {
            commands.entity(e).despawn();
        }
        return;
    }
    let Ok((body, children)) = body.single() else {
        return;
    };
    // Settings applied this frame (keys) show next frame too.
    if !screen.stale && !settings.is_changed() {
        return;
    }
    screen.stale = false;
    if let Some(children) = children {
        for c in children.iter() {
            commands.entity(c).despawn();
        }
    }
    let rows = screen.tab.rows();
    let selected = screen.row.min(rows.len() - 1);
    // Bindings follow the settings one frame later; show the settings' view.
    let shown = if bindings.as_ref() == &Bindings::from_overrides(&settings.keys) {
        bindings.clone()
    } else {
        Bindings::from_overrides(&settings.keys)
    };
    commands.entity(body).with_children(|card| {
        card.spawn(Node {
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|h| {
            h.spawn(theme::text("Settings", font::HEADING, palette::INK));
            h.spawn(theme::text(
                "Up/Down choose   Left/Right change   Enter toggle",
                font::CAPTION,
                palette::INK_SOFT,
            ));
        });
        card.spawn(Node {
            column_gap: Val::Px(space::SM),
            margin: UiRect::bottom(Val::Px(space::SM)),
            ..default()
        })
        .with_children(|tabs| {
            for t in Tab::ALL {
                let label = if t == screen.tab {
                    format!("> {}", t.label())
                } else {
                    t.label().to_string()
                };
                spawn_button(tabs, &label, ButtonVariant::Compact, Act::Tab(t));
            }
        });
        // Keys fit in two columns; other tabs are one list.
        let two_columns = screen.tab == Tab::Keys;
        card.spawn(Node {
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(space::SM),
            row_gap: Val::Px(2.0),
            ..default()
        })
        .with_children(|card| {
            for (i, row) in rows.iter().enumerate() {
                let is_selected = i == selected;
                card.spawn((
                    Node {
                        width: if two_columns {
                            Val::Percent(49.0)
                        } else {
                            Val::Percent(100.0)
                        },
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        padding: UiRect::axes(Val::Px(space::MD), Val::Px(4.0)),
                        border: UiRect::all(Val::Px(1.5)),
                        ..default()
                    },
                    BackgroundColor(if is_selected {
                        palette::PASTEL_GREEN.with_alpha(0.55)
                    } else {
                        Color::NONE
                    }),
                    BorderColor(if is_selected {
                        palette::SAGE
                    } else {
                        Color::NONE
                    }),
                    BorderRadius::all(Val::Px(radius::SM)),
                ))
                .with_children(|r| {
                    r.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        ..default()
                    })
                    .with_children(|left| {
                        left.spawn(theme::text(row.label(), font::BODY, palette::INK));
                        if let Some(hint) = row.hint(&settings) {
                            left.spawn(theme::text(hint, font::CAPTION, palette::INK_SOFT));
                        }
                    });
                    r.spawn(Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(space::SM),
                        ..default()
                    })
                    .with_children(|c| {
                        let waiting = screen.waiting.is_some_and(|a| Row::Key(a) == *row);
                        let value = if waiting {
                            "...".to_string()
                        } else {
                            row.value(&settings, &shown)
                        };
                        match row.control() {
                            Control::Stepper => {
                                spawn_button(c, "-", ButtonVariant::Small, Act::Step(i, -1.0));
                                c.spawn((
                                    Node {
                                        width: Val::Px(90.0),
                                        justify_content: JustifyContent::Center,
                                        ..default()
                                    },
                                    children![theme::text(value, font::BODY, palette::INK)],
                                ));
                                spawn_button(c, "+", ButtonVariant::Small, Act::Step(i, 1.0));
                            }
                            Control::Toggle => {
                                c.spawn((
                                    Node {
                                        width: Val::Px(60.0),
                                        justify_content: JustifyContent::Center,
                                        ..default()
                                    },
                                    children![theme::text(value, font::BODY, palette::INK)],
                                ));
                                spawn_button(c, "Toggle", ButtonVariant::Small, Act::Activate(i));
                            }
                            Control::Press(label) => {
                                if !value.is_empty() {
                                    c.spawn((
                                        Node {
                                            width: Val::Px(100.0),
                                            justify_content: JustifyContent::Center,
                                            ..default()
                                        },
                                        children![theme::text(value, font::BODY, palette::INK)],
                                    ));
                                }
                                spawn_button(c, label, ButtonVariant::Small, Act::Activate(i));
                            }
                        }
                    });
                });
            }
        });
        card.spawn(theme::text(
            screen.notice.clone().unwrap_or_else(|| {
                if screen.tab == Tab::Keys {
                    "Esc, Enter, F3, the arrow keys and 1-4 stay fixed so menus always work.".into()
                } else {
                    "Changes apply at once and are saved.".into()
                }
            }),
            font::CAPTION,
            palette::SAGE_DEEP,
        ));
        card.spawn(Node {
            justify_content: JustifyContent::FlexEnd,
            ..default()
        })
        .with_children(|f| {
            spawn_button(f, "Done (Esc)", ButtonVariant::Compact, Act::Close);
        });
    });
}
