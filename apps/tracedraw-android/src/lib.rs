//! TraceDraw for Android: the editor in a native activity.
//!
//! `android_main` is the entry point the activity calls on a thread of its
//! own; the interface itself is `tracedraw`'s, re-arranged for a phone
//! screen by `tracedraw::ui::mobile`. The crate is empty on every other
//! platform, so a workspace build of the desktop apps still passes.

#![cfg(target_os = "android")]

use winit::platform::android::activity::AndroidApp;

#[no_mangle]
pub extern "C" fn android_main(app: AndroidApp) {
    if let Err(e) = tracedraw::android_run(app) {
        log::error!("TraceDraw could not open its window: {e}");
    }
}
