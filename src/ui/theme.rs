//! Design tokens. Every colour, spacing step, radius and font size used by the
//! UI comes from here so screens stay visually consistent.

#![allow(dead_code)] // The token set is complete on purpose; later phases use more of it.

use bevy::prelude::*;

/// Cottagecore palette: soft, low-saturation naturals.
pub mod palette {
    use bevy::prelude::Color;

    pub const CREAM: Color = Color::srgb_u8(247, 241, 224);
    pub const PARCHMENT: Color = Color::srgb_u8(240, 231, 210);
    pub const BEIGE: Color = Color::srgb_u8(226, 211, 182);
    pub const BEIGE_DEEP: Color = Color::srgb_u8(208, 188, 154);
    pub const SAGE: Color = Color::srgb_u8(156, 178, 138);
    pub const SAGE_LIGHT: Color = Color::srgb_u8(178, 198, 158);
    pub const SAGE_DARK: Color = Color::srgb_u8(110, 136, 98);
    pub const SAGE_DEEP: Color = Color::srgb_u8(88, 112, 80);
    pub const PASTEL_GREEN: Color = Color::srgb_u8(204, 224, 180);
    pub const SOFT_PINK: Color = Color::srgb_u8(238, 190, 190);
    pub const LAVENDER: Color = Color::srgb_u8(190, 172, 214);
    pub const MUTED_YELLOW: Color = Color::srgb_u8(232, 212, 140);
    pub const BROWN: Color = Color::srgb_u8(122, 94, 70);
    /// Primary text: warm dark brown rather than black.
    pub const INK: Color = Color::srgb_u8(74, 60, 48);
    pub const INK_SOFT: Color = Color::srgb_u8(122, 104, 86);
    pub const SHADOW: Color = Color::srgba(0.29, 0.22, 0.16, 0.22);
    pub const SCRIM: Color = Color::srgba(0.22, 0.18, 0.14, 0.38);
}

pub mod space {
    pub const XS: f32 = 4.0;
    pub const SM: f32 = 8.0;
    pub const MD: f32 = 12.0;
    pub const LG: f32 = 20.0;
    pub const XL: f32 = 32.0;
    pub const XXL: f32 = 48.0;
}

pub mod radius {
    pub const SM: f32 = 8.0;
    pub const MD: f32 = 14.0;
    pub const LG: f32 = 22.0;
    pub const PILL: f32 = 999.0;
}

pub mod font {
    pub const CAPTION: f32 = 13.0;
    pub const BODY: f32 = 16.0;
    pub const BUTTON: f32 = 18.0;
    pub const TITLE: f32 = 24.0;
    pub const HEADING: f32 = 34.0;
    pub const HERO: f32 = 60.0;
}

/// How quickly hover/press colours ease towards their target (per second).
pub const TRANSITION_RATE: f32 = 14.0;

/// A raised, rounded parchment card.
pub fn card(node: Node) -> impl Bundle {
    (
        node,
        BackgroundColor(palette::CREAM),
        BorderColor(palette::BEIGE),
        BorderRadius::all(Val::Px(radius::LG)),
        BoxShadow::new(
            palette::SHADOW,
            Val::Px(0.0),
            Val::Px(6.0),
            Val::Px(0.0),
            Val::Px(18.0),
        ),
    )
}

pub fn text(value: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(value),
        TextFont {
            font_size: size,
            ..default()
        },
        TextColor(color),
    )
}
