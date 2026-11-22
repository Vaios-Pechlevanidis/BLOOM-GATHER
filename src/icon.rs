//! The window icon on Windows and Linux (macOS shows the app bundle's icon).
//! The image is the 64 x 64 RGBA rendering of the app icon made by
//! `scripts/make_icon.py`, compiled into the binary.

use bevy::prelude::*;
use bevy::winit::WinitWindows;

const ICON: &[u8] = include_bytes!("../packaging/icon/icon-64.rgba");

pub struct IconPlugin;

impl Plugin for IconPlugin {
    fn build(&self, app: &mut App) {
        if !cfg!(target_os = "macos") {
            app.add_systems(Update, set_icon);
        }
    }
}

/// Once the window exists, give it the icon (then stop trying).
fn set_icon(windows: NonSend<WinitWindows>, mut done: Local<bool>) {
    if *done {
        return;
    }
    let Ok(icon) = winit::window::Icon::from_rgba(ICON.to_vec(), 64, 64) else {
        *done = true;
        return;
    };
    for window in windows.windows.values() {
        window.set_window_icon(Some(icon.clone()));
        *done = true;
    }
}
