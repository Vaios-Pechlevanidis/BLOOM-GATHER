//! Reusable UI widgets.

use bevy::prelude::*;

use super::theme::{font, palette, radius, space, TRANSITION_RATE};

pub struct WidgetsPlugin;

impl Plugin for WidgetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, animate_buttons);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    /// Main call to action (sage).
    Primary,
    /// Secondary actions (parchment).
    Secondary,
    /// Compact HUD button.
    Compact,
    /// Small inline button (lists, tiles).
    Small,
}

/// Colours a button eases between depending on its [`Interaction`].
#[derive(Component, Debug, Clone, Copy)]
pub struct ButtonPalette {
    pub normal: Color,
    pub hovered: Color,
    pub pressed: Color,
}

impl ButtonVariant {
    fn palette(self) -> (ButtonPalette, Color, Color) {
        match self {
            ButtonVariant::Primary => (
                ButtonPalette {
                    normal: palette::SAGE,
                    hovered: palette::SAGE_LIGHT,
                    pressed: palette::SAGE_DARK,
                },
                palette::SAGE_DARK,
                palette::CREAM,
            ),
            ButtonVariant::Secondary | ButtonVariant::Compact | ButtonVariant::Small => (
                ButtonPalette {
                    normal: palette::PARCHMENT,
                    hovered: palette::CREAM,
                    pressed: palette::BEIGE,
                },
                palette::BEIGE_DEEP,
                palette::INK,
            ),
        }
    }

    fn node(self) -> Node {
        let (width, height) = match self {
            ButtonVariant::Primary | ButtonVariant::Secondary => (Val::Px(240.0), Val::Px(52.0)),
            ButtonVariant::Compact => (Val::Auto, Val::Px(40.0)),
            ButtonVariant::Small => (Val::Auto, Val::Px(26.0)),
        };
        let pad = if self == ButtonVariant::Small {
            space::MD
        } else {
            space::LG
        };
        Node {
            width,
            height,
            padding: UiRect::horizontal(Val::Px(pad)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        }
    }
}

/// Spawn a button whose behaviour is defined by `action`, a component the
/// owning screen queries (e.g. `MenuAction::Quit`).
pub fn spawn_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    variant: ButtonVariant,
    action: impl Component,
) {
    let (colors, border, text_color) = variant.palette();
    let font_size = match variant {
        ButtonVariant::Primary | ButtonVariant::Secondary => font::BUTTON,
        ButtonVariant::Compact => font::BODY,
        ButtonVariant::Small => font::CAPTION,
    };
    parent
        .spawn((
            Button,
            variant.node(),
            BackgroundColor(colors.normal),
            BorderColor(border),
            BorderRadius::all(Val::Px(radius::PILL)),
            BoxShadow::new(
                palette::SHADOW,
                Val::Px(0.0),
                Val::Px(3.0),
                Val::Px(0.0),
                Val::Px(8.0),
            ),
            colors,
            action,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                TextLayout::new_with_no_wrap(),
                TextFont {
                    font_size,
                    ..default()
                },
                TextColor(text_color),
            ));
        });
}

/// Ease button backgrounds towards the colour for their current interaction.
/// Uses real time so menus stay responsive while the game is paused.
fn animate_buttons(
    time: Res<Time<Real>>,
    mut buttons: Query<(&Interaction, &ButtonPalette, &mut BackgroundColor)>,
) {
    let k = 1.0 - (-TRANSITION_RATE * time.delta_secs()).exp();
    for (interaction, colors, mut background) in &mut buttons {
        let target = match interaction {
            Interaction::Pressed => colors.pressed,
            Interaction::Hovered => colors.hovered,
            Interaction::None => colors.normal,
        };
        let current = background.0.to_srgba();
        let next = current.mix(&target.to_srgba(), k);
        if next != current {
            background.0 = next.into();
        }
    }
}
