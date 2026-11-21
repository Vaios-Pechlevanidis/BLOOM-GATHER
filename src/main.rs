//! Bloom & Gather — native desktop entry point.
//!
//! The binary is the *presentation layer*: windowing, rendering, UI and input
//! on top of Bevy. All gameplay rules live in the engine-independent
//! `bloom_core` crate.

// Hide the console window on Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
#[cfg(debug_assertions)]
mod dev;
mod game;
mod input;
mod player;
mod rendering;
mod ui;

fn main() -> bevy::app::AppExit {
    app::build().run()
}
