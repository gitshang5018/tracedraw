// No console window in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! TraceDraw desktop and browser entry point: the application itself lives
//! in `lib.rs`, shared with the Android build.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    tracedraw::desktop_main()
}
