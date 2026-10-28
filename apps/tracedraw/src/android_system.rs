//! The phone around the interface: its system bars, its keyboard and its
//! language.
//!
//! A Material app draws edge to edge: the window covers the whole screen,
//! the status bar and the gesture bar included, and the app keeps its own
//! content out of them. Without that measurement the top bar sits under
//! the status bar and its buttons cannot be pressed.
//!
//! The window is the only place that knows, so this module asks it through
//! the activity: Android 11 and later report the insets and take the window
//! edge to edge (`setDecorFitsSystemWindows(false)`); older systems lay the
//! window out below the bars already, so nothing is added there. Every call
//! is best effort: a device that refuses leaves the interface as it was
//! rather than crashing.
//!
//! The keyboard is the second thing the window knows and `winit` does not
//! pass on: it can be shown (`show_soft_input`) but the text typed on it
//! never arrives as events, because the Android backend of `winit` 0.30
//! only forwards physical key presses. What the keyboard edits is the
//! activity's own text buffer, so this module reads that buffer and hands
//! the difference to the interface as typing and backspaces.
//!
//! The language is the third: a phone keeps it in the configuration, not
//! in `LANG`, which is what the desktop reads.

use jni::objects::{JObject, JString, JValue};
use jni::JavaVM;
use std::ffi::c_void;
use std::sync::OnceLock;
use winit::platform::android::activity::AndroidApp;

/// Space the system bars take, in pixels: the status bar at the top, the
/// gesture bar at the bottom (and, with the phone on its side, at the
/// left or the right).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Insets {
    pub top: i32,
    pub bottom: i32,
    pub left: i32,
    pub right: i32,
    pub status_bar_visible: bool,
    /// How much of `top` the camera cutout alone accounts for: the status
    /// bar adds nothing to it when it is hidden, so this is the room the
    /// interface still owes the notch with no bar on screen.
    pub cutout_top: i32,
}

/// Run `f` with the activity's JNI environment, on the calling thread.
fn with_env<R>(vm_ptr: *mut c_void, f: impl FnOnce(&mut jni::JNIEnv<'_>) -> Option<R>) -> Option<R> {
    static VM: OnceLock<JavaVM> = OnceLock::new();
    if vm_ptr.is_null() {
        return None;
    }
    let vm = match VM.get_or_init(|| unsafe {
        JavaVM::from_raw(vm_ptr as *mut jni::sys::JavaVM).expect("the Java VM the activity runs on")
    }) {
        vm => vm,
    };
    let mut env = vm.attach_current_thread().ok()?;
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
    }
    let res = f(&mut env);
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
    }
    res
}

/// The activity as a reference of our own: the one the system handed over
/// belongs to the activity and must not be deleted.
fn activity_ref<'a>(env: &mut jni::JNIEnv<'a>, activity: *mut c_void) -> Option<JObject<'a>> {
    if activity.is_null() {
        return None;
    }
    let raw = unsafe { JObject::from_raw(activity as jni::sys::jobject) };
    let local = env.new_local_ref(&raw).ok();
    std::mem::forget(raw);
    local
}

/// The activity's window.
fn window<'a>(env: &mut jni::JNIEnv<'a>, activity: &JObject<'a>) -> Option<JObject<'a>> {
    env.call_method(activity, "getWindow", "()Landroid/view/Window;", &[])
        .ok()?
        .l()
        .ok()
}

/// Ask the window to take the whole screen, so the interface may draw its
/// own background behind the system bars (once per start).
pub fn go_edge_to_edge(activity: *mut c_void, vm: *mut c_void, sdk: i32) {
    if sdk < 28 {
        return;
    }
    with_env(vm, |env| {
        let activity = activity_ref(env, activity)?;
        let window = window(env, &activity)?;
        if sdk >= 30 {
            // false: the decor lays nothing out around the system bars; the app
            // does it itself with the insets.
            let _ = env.call_method(
                &window,
                "setDecorFitsSystemWindows",
                "(Z)V",
                &[JValue::Bool(0)],
            );
            if env.exception_check().unwrap_or(false) {
                let _ = env.exception_clear();
            }
        }
        // Extend into the cutout areas (short edges / notch) in landscape,
        // eliminating the black letterbox bar on the camera edge.
        // Constant 1 is LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES across API 28+.
        const SHORT_EDGES: i32 = 1;
        if let Ok(val) = env.call_method(
            &window,
            "getAttributes",
            "()Landroid/view/WindowManager$LayoutParams;",
            &[],
        ) {
            if let Ok(params) = val.l() {
                if !params.is_null() {
                    let _ = env.set_field(&params, "layoutInDisplayCutoutMode", "I", JValue::Int(SHORT_EDGES));
                    let _ = env.call_method(
                        &window,
                        "setAttributes",
                        "(Landroid/view/WindowManager$LayoutParams;)V",
                        &[JValue::Object(&params)],
                    );
                }
            }
        }
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
        }
        Some(())
    });
}

/// The insets the window reports now, in pixels; `None` when the window is
/// not attached yet, or on a system older than Android 11.
pub fn read(activity: *mut c_void, vm: *mut c_void, sdk: i32) -> Option<Insets> {
    if sdk < 28 {
        return None;
    }
    with_env(vm, |env| {
        let activity = activity_ref(env, activity)?;
        let window = window(env, &activity)?;
        let decor = env
            .call_method(&window, "getDecorView", "()Landroid/view/View;", &[])
            .ok()?
            .l()
            .ok()?;
        let insets = env
            .call_method(
                &decor,
                "getRootWindowInsets",
                "()Landroid/view/WindowInsets;",
                &[],
            )
            .ok()?
            .l()
            .ok()?;
        if insets.is_null() {
            return None;
        }

        let mut top = 0;
        let mut bottom = 0;
        let mut left = 0;
        let mut right = 0;

        if sdk >= 30 {
            let sys_bars = env
                .call_static_method("android/view/WindowInsets$Type", "systemBars", "()I", &[])
                .map(|v| v.i().unwrap_or(7))
                .unwrap_or(7);
            let cutout = env
                .call_static_method("android/view/WindowInsets$Type", "displayCutout", "()I", &[])
                .map(|v| v.i().unwrap_or(128))
                .unwrap_or(128);
            if let Ok(val) = env.call_method(
                &insets,
                "getInsets",
                "(I)Landroid/graphics/Insets;",
                &[JValue::Int(sys_bars | cutout)],
            ) {
                if let Ok(bars) = val.l() {
                    if !bars.is_null() {
                        top = env.get_field(&bars, "top", "I").ok().and_then(|v| v.i().ok()).unwrap_or(0);
                        bottom = env.get_field(&bars, "bottom", "I").ok().and_then(|v| v.i().ok()).unwrap_or(0);
                        left = env.get_field(&bars, "left", "I").ok().and_then(|v| v.i().ok()).unwrap_or(0);
                        right = env.get_field(&bars, "right", "I").ok().and_then(|v| v.i().ok()).unwrap_or(0);
                    }
                }
            }
        }

        // Measure DisplayCutout safely across API 28+. The top of it is
        // kept apart: with the status bar hidden the notch may still reach
        // the top edge, and that is the only room the bar owes then.
        let mut cutout_top = 0;
        if let Ok(val) = env.call_method(&insets, "getDisplayCutout", "()Landroid/view/DisplayCutout;", &[]) {
            if let Ok(cutout_obj) = val.l() {
                if !cutout_obj.is_null() {
                    let cl = env.call_method(&cutout_obj, "getSafeInsetLeft", "()I", &[]).ok().and_then(|v| v.i().ok()).unwrap_or(0);
                    let ct = env.call_method(&cutout_obj, "getSafeInsetTop", "()I", &[]).ok().and_then(|v| v.i().ok()).unwrap_or(0);
                    let cr = env.call_method(&cutout_obj, "getSafeInsetRight", "()I", &[]).ok().and_then(|v| v.i().ok()).unwrap_or(0);
                    let cb = env.call_method(&cutout_obj, "getSafeInsetBottom", "()I", &[]).ok().and_then(|v| v.i().ok()).unwrap_or(0);
                    cutout_top = ct;
                    left = left.max(cl);
                    top = top.max(ct);
                    right = right.max(cr);
                    bottom = bottom.max(cb);
                }
            }
        }

        let mut status_bar_visible = top > 0;
        if sdk >= 30 {
            let status_bar_type = env
                .call_static_method("android/view/WindowInsets$Type", "statusBars", "()I", &[])
                .map(|v| v.i().unwrap_or(1))
                .unwrap_or(1);
            if let Ok(visible_val) = env.call_method(
                &insets,
                "isVisible",
                "(I)Z",
                &[JValue::Int(status_bar_type)],
            ) {
                if let Ok(vis) = visible_val.z() {
                    status_bar_visible = vis;
                }
            }
            if env.exception_check().unwrap_or(false) {
                let _ = env.exception_clear();
            }
        }

        Some(Insets {
            top,
            bottom,
            left,
            right,
            status_bar_visible,
            cutout_top,
        })
    })
}

/// Dark icons on the system bars for a light interface, light ones for a
/// dark one.
pub fn set_light_bars(activity: *mut c_void, vm: *mut c_void, sdk: i32, light: bool) {
    if sdk < 30 {
        return;
    }
    with_env(vm, |env| {
        let activity = activity_ref(env, activity)?;
        let window = window(env, &activity)?;
        let decor = env
            .call_method(&window, "getDecorView", "()Landroid/view/View;", &[])
            .ok()?
            .l()
            .ok()?;
        let controller = env
            .call_method(
                &decor,
                "getWindowInsetsController",
                "()Landroid/view/WindowInsetsController;",
                &[],
            )
            .ok()?
            .l()
            .ok()?;
        if controller.is_null() {
            return None;
        }
        // APPEARANCE_LIGHT_STATUS_BARS (8) and APPEARANCE_LIGHT_NAVIGATION_BARS
        // (16), both set or both cleared.
        let mask = 8 | 16;
        let appearance = if light { mask } else { 0 };
        let _ = env.call_method(
            &controller,
            "setSystemBarsAppearance",
            "(II)V",
            &[JValue::Int(appearance), JValue::Int(mask)],
        );
        Some(())
    });
}

// ---------------------------------------------------------------------------
// The keyboard
// ---------------------------------------------------------------------------
//
// `winit` 0.30 has no text input on Android: its backend forwards physical
// key presses and nothing else, and `android-activity`'s native activity
// keeps no text buffer to read (its `text_input_state` is empty and
// `set_text_input_state` does nothing). A soft keyboard therefore has
// nothing to type into.
//
// So the app brings its own: a one pixel `EditText`, invisible, added to the
// window and focused while a field wants typing. The keyboard edits it as it
// would any field, and this module reads what it holds and hands the
// difference to the interface. It is removed as soon as the typing stops.

/// The activity, kept for the system bars, the keyboard and the language.
static PHONE: OnceLock<AndroidApp> = OnceLock::new();

/// Keep the activity for the calls in this module.
pub fn set_phone(app: AndroidApp) {
    let _ = PHONE.set(app);
}

/// The activity, once the app has started.
pub fn phone() -> Option<&'static AndroidApp> {
    PHONE.get()
}

/// What the keyboard did to its field since it was last read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Typing {
    /// Characters it added.
    Text(String),
    /// A character it removed.
    Backspace,
}

/// The field the keyboard edits, while it is up.
static FIELD: std::sync::Mutex<Option<jni::objects::GlobalRef>> = std::sync::Mutex::new(None);
/// What the field held when it was last read.
static TYPED: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
/// Whether the keyboard is up.
static KEYBOARD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// When the keyboard was last asked for, and how many times: a field that
/// was just added sometimes loses the first ask, so it is asked again
/// while nothing has been typed.
static ASKED: std::sync::Mutex<Option<(web_time::Instant, u8)>> = std::sync::Mutex::new(None);

/// Show or hide the phone's keyboard: the field is put in the window and
/// focused, or taken out again.
pub fn set_keyboard(app: &AndroidApp, show: bool) {
    use std::sync::atomic::Ordering;
    if KEYBOARD.swap(show, Ordering::Relaxed) == show {
        return;
    }
    lock(&TYPED).clear();
    if show {
        *lock(&ASKED) = Some((web_time::Instant::now(), 0));
        open_keyboard(app);
    } else {
        *lock(&ASKED) = None;
        close_keyboard(app);
    }
}

/// Put the field in the window and ask for the keyboard.
fn open_keyboard(app: &AndroidApp) {
    let vm = app.vm_as_ptr() as usize;
    let activity = app.activity_as_ptr() as usize;
    // A view belongs to the thread that owns the window.
    app.run_on_java_main_thread(Box::new(move || {
        let (vm, activity) = (vm as *mut c_void, activity as *mut c_void);
        if let Err(e) = open_field(vm, activity) {
            log::warn!("no keyboard: {e}");
        }
    }));
}

/// Ask the keyboard to come up over the field that is already there.
fn ask_again(app: &AndroidApp) {
    let vm = app.vm_as_ptr() as usize;
    let activity = app.activity_as_ptr() as usize;
    app.run_on_java_main_thread(Box::new(move || {
        let (vm, activity) = (vm as *mut c_void, activity as *mut c_void);
        let _ = with_env(vm, |env| {
            let field = lock(&FIELD);
            let field = field.as_ref()?;
            let activity = activity_ref(env, activity)?;
            let name: JObject = JObject::from(env.new_string("input_method").ok()?);
            let service = env
                .call_method(
                    &activity,
                    "getSystemService",
                    "(Ljava/lang/String;)Ljava/lang/Object;",
                    &[JValue::Object(&name)],
                )
                .ok()?
                .l()
                .ok()?;
            let _ = env.call_method(
                &service,
                "showSoftInput",
                "(Landroid/view/View;I)Z",
                &[JValue::Object(field.as_obj()), JValue::Int(0)],
            );
            Some(())
        });
    }));
}

/// Take the field out of the window.
fn close_keyboard(app: &AndroidApp) {
    let vm = app.vm_as_ptr() as usize;
    let activity = app.activity_as_ptr() as usize;
    app.run_on_java_main_thread(Box::new(move || {
        let (vm, activity) = (vm as *mut c_void, activity as *mut c_void);
        close_field(vm, activity);
    }));
}

/// The field in the window, focused, with the keyboard over it.
fn open_field(vm: *mut c_void, activity: *mut c_void) -> Result<(), String> {
    with_env(vm, |env| {
        let activity = activity_ref(env, activity)?;
        let window = window(env, &activity)?;
        let decor = env
            .call_method(&window, "getDecorView", "()Landroid/view/View;", &[])
            .ok()?
            .l()
            .ok()?;
        // One pixel, invisible, and a colour of its own: it is never seen,
        // only typed into.
        let field = env
            .new_object(
                "android/widget/EditText",
                "(Landroid/content/Context;)V",
                &[JValue::Object(&activity)],
            )
            .ok()?;
        let _ = env.call_method(&field, "setAlpha", "(F)V", &[JValue::Float(0.0)]);
        let _ = env.call_method(
            &field,
            "setBackgroundColor",
            "(I)V",
            &[JValue::Int(0x00FF_FFFF)],
        );
        let _ = env.call_method(&field, "setSingleLine", "(Z)V", &[JValue::Bool(0)]);
        let params = env
            .new_object(
                "android/view/ViewGroup$LayoutParams",
                "(II)V",
                &[JValue::Int(1), JValue::Int(1)],
            )
            .ok()?;
        let _ = env.call_method(
            &decor,
            "addView",
            "(Landroid/view/View;Landroid/view/ViewGroup$LayoutParams;)V",
            &[JValue::Object(&field), JValue::Object(&params)],
        );
        let _ = env.call_method(
            &field,
            "setFocusableInTouchMode",
            "(Z)V",
            &[JValue::Bool(1)],
        );
        let _ = env.call_method(&field, "requestFocus", "()Z", &[]);
        // The keyboard over it.
        let name: JObject = JObject::from(env.new_string("input_method").ok()?);
        let service = env
            .call_method(
                &activity,
                "getSystemService",
                "(Ljava/lang/String;)Ljava/lang/Object;",
                &[JValue::Object(&name)],
            )
            .ok()
            .and_then(|v| v.l().ok());
        if let Some(service) = service.filter(|s| !s.is_null()) {
            let _ = env.call_method(
                &service,
                "showSoftInput",
                "(Landroid/view/View;I)Z",
                &[JValue::Object(&field), JValue::Int(0)],
            );
        }
        let global = env.new_global_ref(&field).ok()?;
        *lock(&FIELD) = Some(global);
        Some(())
    })
    .ok_or_else(|| "the keyboard field could not be added".to_string())
}

/// Take the field out of the window, keyboard first.
fn close_field(vm: *mut c_void, activity: *mut c_void) {
    with_env(vm, |env| {
        let field = lock(&FIELD).take()?;
        let field = field.as_obj();
        let activity = activity_ref(env, activity)?;
        let window = window(env, &activity)?;
        let decor = env
            .call_method(&window, "getDecorView", "()Landroid/view/View;", &[])
            .ok()?
            .l()
            .ok()?;
        let _ = env.call_method(
            &decor,
            "removeView",
            "(Landroid/view/View;)V",
            &[JValue::Object(field)],
        );
        Some(())
    });
    lock(&TYPED).clear();
}

/// What the keyboard typed since the last frame.
pub fn typed(vm: *mut c_void) -> Vec<Typing> {
    if !KEYBOARD.load(std::sync::atomic::Ordering::Relaxed) {
        return Vec::new();
    }
    let now = field_text(vm);
    let Some(now) = now else {
        return Vec::new();
    };
    let mut last = lock(&TYPED);
    if now == *last {
        return Vec::new();
    }
    let old = std::mem::replace(&mut *last, now.clone());
    let mut out = Vec::new();
    if let Some(added) = now.strip_prefix(&old) {
        if !added.is_empty() {
            out.push(Typing::Text(added.to_string()));
        }
    } else if let Some(removed) = old.strip_prefix(&now) {
        out.extend(std::iter::repeat(Typing::Backspace).take(removed.chars().count()));
    } else {
        // The keyboard replaced what was there (a suggestion, or a
        // selection typed over): clear it, then type the new text.
        out.extend(std::iter::repeat(Typing::Backspace).take(old.chars().count()));
        if !now.is_empty() {
            out.push(Typing::Text(now));
        }
    }
    out
}

/// The text the field holds now.
fn field_text(vm: *mut c_void) -> Option<String> {
    with_env(vm, |env| {
        let field = lock(&FIELD);
        let field = field.as_ref()?;
        let editable = env
            .call_method(field.as_obj(), "getText", "()Landroid/text/Editable;", &[])
            .ok()?
            .l()
            .ok()?;
        if editable.is_null() {
            return None;
        }
        let text = env
            .call_method(&editable, "toString", "()Ljava/lang/String;", &[])
            .ok()?
            .l()
            .ok()?;
        let text = JString::from(text);
        let text: String = env.get_string(&text).ok()?.into();
        Some(text)
    })
}

/// Hand what the keyboard typed to the frame being drawn, and put the
/// keyboard up while `wants` says a field has the focus.
pub fn pump_typing(ctx: &egui::Context, wants: bool) {
    let Some(phone) = phone() else {
        return;
    };
    set_keyboard(phone, wants);
    for edit in typed(phone.vm_as_ptr()) {
        match edit {
            Typing::Text(text) => {
                ctx.input_mut(|i| i.events.push(egui::Event::Text(text)));
            }
            Typing::Backspace => {
                for pressed in [true, false] {
                    ctx.input_mut(|i| {
                        i.events.push(egui::Event::Key {
                            key: egui::Key::Backspace,
                            physical_key: None,
                            pressed,
                            repeat: false,
                            modifiers: egui::Modifiers::NONE,
                        })
                    });
                }
            }
        }
    }
    if wants {
        // Nothing typed yet and the field has been there a moment: the
        // keyboard may have missed its first ask.
        // The locks are taken one at a time, in the order every other
        // path takes them, so two of them never wait on each other.
        let nothing_yet = lock(&TYPED).is_empty();
        let retry = if nothing_yet {
            let mut asked = lock(&ASKED);
            match *asked {
                Some((when, times))
                    if times < 3 && when.elapsed() >= std::time::Duration::from_millis(400) =>
                {
                    *asked = Some((web_time::Instant::now(), times + 1));
                    true
                }
                _ => false,
            }
        } else {
            false
        };
        if retry {
            ask_again(phone);
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(80));
    }
}

fn lock<T>(m: &'static std::sync::Mutex<T>) -> std::sync::MutexGuard<'static, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

// ---------------------------------------------------------------------------
// The language
// ---------------------------------------------------------------------------

/// The language of the phone, as a tag such as `zh-CN` or `pt-BR`: a phone
/// keeps it in its configuration, not in `LANG`.
pub fn system_locale(vm: *mut c_void) -> Option<String> {
    with_env(vm, |env| {
        let locale = env
            .call_static_method(
                "java/util/Locale",
                "getDefault",
                "()Ljava/util/Locale;",
                &[],
            )
            .ok()?
            .l()
            .ok()?;
        let tag = env
            .call_method(&locale, "toLanguageTag", "()Ljava/lang/String;", &[])
            .ok()?
            .l()
            .ok()?;
        if tag.is_null() {
            return None;
        }
        let tag = JString::from(tag);
        let text: String = env.get_string(&tag).ok()?.into();
        (!text.is_empty()).then_some(text)
    })
}

// ---------------------------------------------------------------------------
// The files the user can see
// ---------------------------------------------------------------------------
//
// A phone keeps an app's own folder out of sight: since Android 11 a file
// manager does not even show `Android/data`, so drawings saved there can
// never be opened again by hand. The app asks for "all files access"
// instead, which is the permission a drawing app needs, and then reads and
// writes the folders the user knows: Documents, Download, Pictures.

/// Whether the app may read and write the shared storage.
pub fn storage_granted(app: &AndroidApp) -> bool {
    if AndroidApp::sdk_version() < 30 {
        // Before Android 11 the storage permission is granted at install;
        // a read that fails tells the rest.
        return true;
    }
    let is_manager = with_env(app.vm_as_ptr(), |env| {
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
        }
        let val = env.call_static_method(
            "android/os/Environment",
            "isExternalStorageManager",
            "()Z",
            &[],
        );
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
        }
        val.ok()?.z().ok()
    })
    .unwrap_or(false);

    // The system's own answer is the answer. A listing of the shared
    // storage is not a second opinion: a phone without the grant still
    // answers one, empty, which is how a fresh install came to believe it
    // had the permission, so the Files sheet offered no way to give it and
    // listed nothing. The screen that grants it is what the app offers
    // instead, and this is read again when that screen closes.
    log::info!("all files access: {is_manager}");

    is_manager
}

/// The folder the phone keeps the user's files in (`/storage/emulated/0`).
pub fn storage_root(app: &AndroidApp) -> Option<std::path::PathBuf> {
    let from_jni = with_env(app.vm_as_ptr(), |env| {
        let dir = env
            .call_static_method(
                "android/os/Environment",
                "getExternalStorageDirectory",
                "()Ljava/io/File;",
                &[],
            );
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
        }
        let dir = dir.ok()?.l().ok()?;
        if dir.is_null() {
            return None;
        }
        let path = env.call_method(&dir, "getAbsolutePath", "()Ljava/lang/String;", &[]);
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
        }
        let path = path.ok()?.l().ok()?;
        let path = JString::from(path);
        let path: String = env.get_string(&path).ok()?.into();
        (!path.is_empty()).then(|| std::path::PathBuf::from(path))
    });
    from_jni.or_else(|| {
        let fallback = std::path::PathBuf::from("/storage/emulated/0");
        fallback.is_dir().then_some(fallback)
    })
}

/// Open the system screen where the user grants that access. It is a
/// screen of the phone's, so nothing comes back: the app reads the answer
/// when it is shown again.
pub fn ask_storage(app: &AndroidApp) {
    if AndroidApp::sdk_version() < 30 {
        return;
    }
    let vm = app.vm_as_ptr();
    let activity = app.activity_as_ptr();
    with_env(vm, |env| {
        let activity = activity_ref(env, activity)?;
        let name = env.call_method(&activity, "getPackageName", "()Ljava/lang/String;", &[]);
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
        }
        let name = name.ok()?.l().ok()?;
        let name = JString::from(name);
        let name: String = env.get_string(&name).ok()?.into();

        let mut opened = false;

        // 1. Try ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION with package URI
        if let Ok(text) = env.new_string(format!("package:{name}")) {
            let text = JObject::from(text);
            if let Ok(uri_val) = env.call_static_method(
                "android/net/Uri",
                "parse",
                "(Ljava/lang/String;)Landroid/net/Uri;",
                &[JValue::Object(&text)],
            ) {
                if let Ok(uri) = uri_val.l() {
                    if let Ok(action) = env.new_string("android.settings.MANAGE_APP_ALL_FILES_ACCESS_PERMISSION") {
                        let action = JObject::from(action);
                        if let Ok(intent) = env.new_object(
                            "android/content/Intent",
                            "(Ljava/lang/String;Landroid/net/Uri;)V",
                            &[JValue::Object(&action), JValue::Object(&uri)],
                        ) {
                            if env.call_method(
                                &activity,
                                "startActivity",
                                "(Landroid/content/Intent;)V",
                                &[JValue::Object(&intent)],
                            ).is_ok() && !env.exception_check().unwrap_or(false) {
                                opened = true;
                            }
                        }
                    }
                }
            }
        }
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
        }

        // 2. If direct package screen failed, try general ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION
        if !opened {
            if let Ok(action) = env.new_string("android.settings.MANAGE_ALL_FILES_ACCESS_PERMISSION") {
                let action = JObject::from(action);
                if let Ok(intent) = env.new_object(
                    "android/content/Intent",
                    "(Ljava/lang/String;)V",
                    &[JValue::Object(&action)],
                ) {
                    if env.call_method(
                        &activity,
                        "startActivity",
                        "(Landroid/content/Intent;)V",
                        &[JValue::Object(&intent)],
                    ).is_ok() && !env.exception_check().unwrap_or(false) {
                        opened = true;
                    }
                }
            }
            if env.exception_check().unwrap_or(false) {
                let _ = env.exception_clear();
            }
        }

        // 3. Fallback: ACTION_APPLICATION_DETAILS_SETTINGS with package URI
        if !opened {
            if let Ok(text) = env.new_string(format!("package:{name}")) {
                let text = JObject::from(text);
                if let Ok(uri_val) = env.call_static_method(
                    "android/net/Uri",
                    "parse",
                    "(Ljava/lang/String;)Landroid/net/Uri;",
                    &[JValue::Object(&text)],
                ) {
                    if let Ok(uri) = uri_val.l() {
                        if let Ok(action) = env.new_string("android.settings.APPLICATION_DETAILS_SETTINGS") {
                            let action = JObject::from(action);
                            if let Ok(intent) = env.new_object(
                                "android/content/Intent",
                                "(Ljava/lang/String;Landroid/net/Uri;)V",
                                &[JValue::Object(&action), JValue::Object(&uri)],
                            ) {
                                let _ = env.call_method(
                                    &activity,
                                    "startActivity",
                                    "(Landroid/content/Intent;)V",
                                    &[JValue::Object(&intent)],
                                );
                            }
                        }
                    }
                }
            }
            if env.exception_check().unwrap_or(false) {
                let _ = env.exception_clear();
            }
        }

        Some(())
    });
}

// ---------------------------------------------------------------------------
// The drawing the system handed over
// ---------------------------------------------------------------------------
//
// Tapping a `.cdr` or a `.tdraw` in a file manager starts TraceDraw with
// that file as the intent's data: a `file://` path on older systems, a
// `content://` URI since Android 7. The URI is not a path — the bytes come
// through the content resolver — so the drawing is copied into the app's
// own Documents folder and opened from there.

/// The name the file should take here: the URI's last segment, with the
/// extension the type names when the name has none.
fn shared_name(last: &str, mime: &str) -> String {
    let safe: String = last
        .chars()
        .map(|c| if "/\\:*?\"<>|".contains(c) { '_' } else { c })
        .collect();
    let safe = if safe.trim().is_empty() {
        "Shared".to_string()
    } else {
        safe
    };
    let lower = safe.to_ascii_lowercase();
    if lower.ends_with(".cdr") || lower.ends_with(".tdraw") {
        return safe;
    }
    let ext = if mime.contains("corel") || mime.contains("cdr") {
        ".cdr"
    } else {
        ".tdraw"
    };
    format!("{safe}{ext}")
}

/// A Java string as Rust text, if there is one.
fn jstring_text(env: &mut jni::JNIEnv<'_>, obj: JObject<'_>) -> Option<String> {
    if obj.is_null() {
        return None;
    }
    let text = JString::from(obj);
    let out: String = env.get_string(&text).ok()?.into();
    (!out.is_empty()).then_some(out)
}

/// The `content://` URI the activity was started with, its last path
/// segment and the type the system gives it.
fn intent_data(app: &AndroidApp) -> Option<(String, String, String)> {
    with_env(app.vm_as_ptr(), |env| {
        let activity = activity_ref(env, app.activity_as_ptr())?;
        let intent = env
            .call_method(&activity, "getIntent", "()Landroid/content/Intent;", &[])
            .ok()?
            .l()
            .ok()?;
        if intent.is_null() {
            return None;
        }
        let uri = env
            .call_method(&intent, "getData", "()Landroid/net/Uri;", &[])
            .ok()?
            .l()
            .ok()?;
        if uri.is_null() {
            return None;
        }
        let uri_text = env
            .call_method(&uri, "toString", "()Ljava/lang/String;", &[])
            .ok()?
            .l()
            .ok()?;
        let uri_text = jstring_text(env, uri_text)?;
        let last = env
            .call_method(&uri, "getLastPathSegment", "()Ljava/lang/String;", &[])
            .ok()
            .and_then(|v| v.l().ok())
            .and_then(|o| jstring_text(env, o))
            .unwrap_or_default();
        let mime = env
            .call_method(&intent, "getType", "()Ljava/lang/String;", &[])
            .ok()
            .and_then(|v| v.l().ok())
            .and_then(|o| jstring_text(env, o))
            .unwrap_or_default();
        Some((uri_text, last, mime))
    })
}

/// Copy the drawing the app was started to open into `dir`, and give back
/// its path there. Nothing to open (a start from the launcher) gives none.
pub fn shared_drawing(app: &AndroidApp, dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let (uri, last, mime) = intent_data(app)?;
    let name = shared_name(&last, &mime);
    let _ = std::fs::create_dir_all(dir);
    let path = dir.join(&name);
    // A drawing of that name is already there: keep both.
    let path = if path.exists() {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Shared".into());
        let ext = path
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();
        let mut n = 2;
        loop {
            let candidate = dir.join(format!("{stem} ({n}){ext}"));
            if !candidate.exists() {
                break candidate;
            }
            n += 1;
        }
    } else {
        path
    };
    // A `file://` URI is a path already; a `content://` one is read.
    let bytes = if uri.starts_with("file:") {
        let file_path = with_env(app.vm_as_ptr(), |env| {
            let activity = activity_ref(env, app.activity_as_ptr())?;
            let intent = env
                .call_method(&activity, "getIntent", "()Landroid/content/Intent;", &[])
                .ok()?
                .l()
                .ok()?;
            let uri = env
                .call_method(&intent, "getData", "()Landroid/net/Uri;", &[])
                .ok()?
                .l()
                .ok()?;
            let path = env
                .call_method(&uri, "getPath", "()Ljava/lang/String;", &[])
                .ok()?
                .l()
                .ok()?;
            jstring_text(env, path)
        });
        file_path.and_then(|p| std::fs::read(p).ok())
    } else {
        read_uri(app, &uri)
    }?;
    std::fs::write(&path, bytes).ok()?;
    Some(path)
}

/// The bytes behind a `content://` URI, through the content resolver.
fn read_uri(app: &AndroidApp, uri: &str) -> Option<Vec<u8>> {
    with_env(app.vm_as_ptr(), |env| {
        let activity = activity_ref(env, app.activity_as_ptr())?;
        let text: JObject = JObject::from(env.new_string(uri).ok()?);
        let parsed = env
            .call_static_method(
                "android/net/Uri",
                "parse",
                "(Ljava/lang/String;)Landroid/net/Uri;",
                &[JValue::Object(&text)],
            )
            .ok()?
            .l()
            .ok()?;
        let resolver = env
            .call_method(
                &activity,
                "getContentResolver",
                "()Landroid/content/ContentResolver;",
                &[],
            )
            .ok()?
            .l()
            .ok()?;
        let stream = env
            .call_method(
                &resolver,
                "openInputStream",
                "(Landroid/net/Uri;)Ljava/io/InputStream;",
                &[JValue::Object(&parsed)],
            )
            .ok()?
            .l()
            .ok()?;
        if stream.is_null() {
            return None;
        }
        let buffer = env.new_byte_array(16 * 1024).ok()?;
        let out = env
            .new_object("java/io/ByteArrayOutputStream", "()V", &[])
            .ok()?;
        loop {
            let n = env
                .call_method(&stream, "read", "([B)I", &[JValue::Object(&buffer)])
                .ok()?
                .i()
                .ok()?;
            if n <= 0 {
                break;
            }
            env.call_method(
                &out,
                "write",
                "([BII)V",
                &[JValue::Object(&buffer), JValue::Int(0), JValue::Int(n)],
            )
            .ok()?;
        }
        let _ = env.call_method(&stream, "close", "()V", &[]);
        let bytes = env
            .call_method(&out, "toByteArray", "()[B", &[])
            .ok()?
            .l()
            .ok()?;
        let bytes = jni::objects::JByteArray::from(bytes);
        env.convert_byte_array(&bytes).ok()
    })
}

/// Show or hide the system bars: a phone lying on its side is all drawing,
/// and the status bar over the top of it takes a row of the screen.
///
/// Android 11 and later hide them through the window's insets controller,
/// and bring them back with a swipe from the edge; older systems use the
/// system UI flags of the view.
pub fn set_fullscreen(app: &AndroidApp, hidden: bool) {
    let vm = app.vm_as_ptr() as usize;
    let activity = app.activity_as_ptr() as usize;
    let sdk = AndroidApp::sdk_version();
    app.run_on_java_main_thread(Box::new(move || {
        let (vm, activity) = (vm as *mut c_void, activity as *mut c_void);
        let _ = with_env(vm, |env| {
        let activity = activity_ref(env, activity)?;
        let window = window(env, &activity)?;
        if sdk >= 28 {
            const SHORT_EDGES: i32 = 1;
            if let Ok(val) = env.call_method(
                &window,
                "getAttributes",
                "()Landroid/view/WindowManager$LayoutParams;",
                &[],
            ) {
                if let Ok(params) = val.l() {
                    if !params.is_null() {
                        let _ = env.set_field(&params, "layoutInDisplayCutoutMode", "I", JValue::Int(SHORT_EDGES));
                        let _ = env.call_method(
                            &window,
                            "setAttributes",
                            "(Landroid/view/WindowManager$LayoutParams;)V",
                            &[JValue::Object(&params)],
                        );
                    }
                }
            }
            if env.exception_check().unwrap_or(false) {
                let _ = env.exception_clear();
            }
        }
        let decor = env
            .call_method(&window, "getDecorView", "()Landroid/view/View;", &[])
            .ok()?
            .l()
            .ok()?;
        if sdk >= 30 {
            let controller = env
                .call_method(
                    &decor,
                    "getWindowInsetsController",
                    "()Landroid/view/WindowInsetsController;",
                    &[],
                )
                .ok()?
                .l()
                .ok()?;
            if controller.is_null() {
                return None;
            }
            let bars = env
                .call_static_method("android/view/WindowInsets$Type", "systemBars", "()I", &[])
                .ok()?
                .i()
                .ok()?;
            if hidden {
                // BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE: a swipe from the
                // edge shows them for a moment, and they go again.
                let _ = env.call_method(
                    &controller,
                    "setSystemBarsBehavior",
                    "(I)V",
                    &[JValue::Int(2)],
                );
                let _ = env.call_method(&controller, "hide", "(I)V", &[JValue::Int(bars)]);
            } else {
                let _ = env.call_method(&controller, "show", "(I)V", &[JValue::Int(bars)]);
            }
        } else {
            // LAYOUT_STABLE (256) | LAYOUT_HIDE_NAVIGATION (512) | LAYOUT_FULLSCREEN (1024),
            // and with them FULLSCREEN (4) | HIDE_NAVIGATION (2) | IMMERSIVE_STICKY (4096).
            const LAYOUT: i32 = 256 | 512 | 1024;
            const HIDE: i32 = 4 | 2 | 4096;
            let flags = if hidden { LAYOUT | HIDE } else { LAYOUT };
            let _ = env.call_method(
                &decor,
                "setSystemUiVisibility",
                "(I)V",
                &[JValue::Int(flags)],
            );
        }
        Some(())
    });
    }));
}

// ---------------------------------------------------------------------------
// Updating the app
// ---------------------------------------------------------------------------
//
// A new build is an APK, and only the system's installer may put one in
// place: the app writes the download into its own cache and hands the file
// to `PackageInstaller`, which asks the user to update TraceDraw. Android
// 8 and later also want the app to be allowed to install packages at all,
// which the update dialog asks for before it downloads anything.

/// The app's cache folder: a download of its own goes there, where the
/// system's installer can read it and nothing else can see it.
pub fn cache_dir(app: &AndroidApp) -> Option<std::path::PathBuf> {
    let vm = app.vm_as_ptr();
    let activity = app.activity_as_ptr();
    let path = with_env(vm, |env| {
        let activity = activity_ref(env, activity)?;
        let dir = env
            .call_method(&activity, "getCacheDir", "()Ljava/io/File;", &[])
            .ok()?
            .l()
            .ok()?;
        if dir.is_null() {
            return None;
        }
        let text = env
            .call_method(&dir, "getAbsolutePath", "()Ljava/lang/String;", &[])
            .ok()?
            .l()
            .ok()?;
        jstring_text(env, text)
    })?;
    Some(std::path::PathBuf::from(path))
}

/// Whether the system lets this app install packages: asked of the package
/// manager, which says no until the user allows it for TraceDraw.
pub fn can_install_packages(app: &AndroidApp) -> bool {
    // Before Android 8 any app could hand an APK to the installer.
    if AndroidApp::sdk_version() < 26 {
        return true;
    }
    let vm = app.vm_as_ptr();
    let activity = app.activity_as_ptr();
    with_env(vm, |env| {
        let activity = activity_ref(env, activity)?;
        let manager = env
            .call_method(
                &activity,
                "getPackageManager",
                "()Landroid/content/pm/PackageManager;",
                &[],
            )
            .ok()?
            .l()
            .ok()?;
        if manager.is_null() {
            return None;
        }
        let allowed = env
            .call_method(&manager, "canRequestPackageInstalls", "()Z", &[])
            .ok()?
            .z()
            .ok()?;
        Some(allowed)
    })
    .unwrap_or(false)
}

/// The system's screen for allowing TraceDraw to install packages.
pub fn ask_install_permission(app: &AndroidApp) {
    let vm = app.vm_as_ptr() as usize;
    let activity = app.activity_as_ptr() as usize;
    app.run_on_java_main_thread(Box::new(move || {
        let (vm, activity) = (vm as *mut c_void, activity as *mut c_void);
        let _ = with_env(vm, |env| {
            let activity = activity_ref(env, activity)?;
            let package = env
                .call_method(&activity, "getPackageName", "()Ljava/lang/String;", &[])
                .ok()?
                .l()
                .ok()?;
            let name = jstring_text(env, package)?;
            let text = env.new_string(format!("package:{name}")).ok()?;
            let uri = env
                .call_static_method(
                    "android/net/Uri",
                    "parse",
                    "(Ljava/lang/String;)Landroid/net/Uri;",
                    &[JValue::Object(&JObject::from(text))],
                )
                .ok()?
                .l()
                .ok()?;
            let action = env.new_string("android.settings.MANAGE_UNKNOWN_APP_SOURCES").ok()?;
            let intent = env
                .new_object(
                    "android/content/Intent",
                    "(Ljava/lang/String;Landroid/net/Uri;)V",
                    &[
                        JValue::Object(&JObject::from(action)),
                        JValue::Object(&uri),
                    ],
                )
                .ok()?;
            let _ = env.call_method(
                &activity,
                "startActivity",
                "(Landroid/content/Intent;)V",
                &[JValue::Object(&intent)],
            );
            if env.exception_check().unwrap_or(false) {
                let _ = env.exception_clear();
            }
            Some(())
        });
    }));
}

/// Hand `path` to the system's package installer: it shows its own prompt
/// and updates TraceDraw when the user agrees. On Android 10 and later the
/// file is put in Download and opened with a VIEW intent, the way a file
/// manager opens a tapped APK; older phones get it through a session.
pub fn install_package(path: &std::path::Path) -> Result<(), String> {
    let app = phone().ok_or_else(|| "no activity".to_string())?;
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if bytes.is_empty() {
        return Err("the download is empty".to_string());
    }
    // The log is what says how far the hand-over got when the installer
    // shows nothing: the steps below are logged as they happen.
    log::info!(
        "handing {} bytes to the system's installer, allowed to install: {}",
        bytes.len(),
        can_install_packages(&app)
    );
    // Android 10 and later: the file goes into Download through the
    // system's own provider and is opened with the intent a file manager
    // sends, which is the one every phone answers. A session, committed by
    // the app itself, is accepted and then quietly dropped on some phones:
    // nothing is shown and nothing is installed.
    if AndroidApp::sdk_version() >= 29 {
        match install_through_view(&app, &bytes) {
            Ok(()) => return Ok(()),
            Err(e) => log::warn!(
                "opening the update with a VIEW intent failed ({e}); handing over a session instead"
            ),
        }
    }
    let vm = app.vm_as_ptr();
    let activity = app.activity_as_ptr();
    // A call that throws answers with the bare "Java exception was thrown"
    // and leaves the exception pending: what it said is read here, so the
    // box names the reason the system gave instead of that sentence.
    let answer = with_env(vm, |env| {
        Some(match install_session(env, activity, &bytes) {
            Err(e) => Err(java_error(env, "the installer").unwrap_or(e)),
            ok => ok,
        })
    });
    match answer {
        Some(Ok(())) => Ok(()),
        Some(Err(e)) => Err(e),
        None => Err("the installer did not answer".to_string()),
    }
}

/// `values.put(key, value)`: a `ContentValues` holds strings here.
fn content_values_put(
    env: &mut jni::JNIEnv<'_>,
    values: &JObject<'_>,
    key: &str,
    value: &str,
) -> Result<(), String> {
    let key = env.new_string(key).map_err(|e| e.to_string())?;
    let value = env.new_string(value).map_err(|e| e.to_string())?;
    env.call_method(
        values,
        "put",
        "(Ljava/lang/String;Ljava/lang/String;)V",
        &[
            JValue::Object(&JObject::from(key)),
            JValue::Object(&JObject::from(value)),
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// The APK into the phone's Download folder through `MediaStore`, then the
/// intent a file manager sends for a tapped file: the system's installer
/// answers it, asks the user, and updates TraceDraw. The row is the app's
/// own, so nothing has to be granted for it.
fn install_through_view(app: &AndroidApp, bytes: &[u8]) -> Result<(), String> {
    let uri = with_env(app.vm_as_ptr(), |env| {
        let activity = activity_ref(env, app.activity_as_ptr())?;
        let resolver = env
            .call_method(
                &activity,
                "getContentResolver",
                "()Landroid/content/ContentResolver;",
                &[],
            )
            .ok()?
            .l()
            .ok()?;
        if resolver.is_null() {
            return None;
        }
        let values = env
            .new_object("android/content/ContentValues", "()V", &[])
            .ok()?;
        content_values_put(env, &values, "_display_name", "tracedraw-update.apk").ok()?;
        content_values_put(
            env,
            &values,
            "mime_type",
            "application/vnd.android.package-archive",
        )
        .ok()?;
        content_values_put(env, &values, "relative_path", "Download/").ok()?;
        let collection = env
            .get_static_field(
                "android/provider/MediaStore$Downloads",
                "EXTERNAL_CONTENT_URI",
                "Landroid/net/Uri;",
            )
            .ok()?
            .l()
            .ok()?;
        let row = env
            .call_method(
                &resolver,
                "insert",
                "(Landroid/net/Uri;Landroid/content/ContentValues;)Landroid/net/Uri;",
                &[JValue::Object(&collection), JValue::Object(&values)],
            )
            .ok()?
            .l()
            .ok()?;
        if row.is_null() {
            return None;
        }
        let out = env
            .call_method(
                &resolver,
                "openOutputStream",
                "(Landroid/net/Uri;)Ljava/io/OutputStream;",
                &[JValue::Object(&row)],
            )
            .ok()?
            .l()
            .ok()?;
        if out.is_null() {
            return None;
        }
        write_session(env, &out, bytes).ok()?;
        env.call_method(&out, "close", "()V", &[]).ok()?;
        let text = env
            .call_method(&row, "toString", "()Ljava/lang/String;", &[])
            .ok()?
            .l()
            .ok()?;
        jstring_text(env, text)
    })
    .ok_or_else(|| "the phone would not take the file".to_string())?;
    // The screen is opened on the Java main thread, where a window may be
    // started, and the answer comes back through a channel since the thread
    // does not wait for it.
    let (tx, rx) = std::sync::mpsc::channel();
    let vm = app.vm_as_ptr() as usize;
    let activity = app.activity_as_ptr() as usize;
    app.run_on_java_main_thread(Box::new(move || {
        let (vm, activity) = (vm as *mut c_void, activity as *mut c_void);
        let opened = with_env(vm, |env| {
            let activity = activity_ref(env, activity)?;
            let text = env.new_string(&uri).ok()?;
            let row = env
                .call_static_method(
                    "android/net/Uri",
                    "parse",
                    "(Ljava/lang/String;)Landroid/net/Uri;",
                    &[JValue::Object(&JObject::from(text))],
                )
                .ok()?
                .l()
                .ok()?;
            let action = env.new_string("android.intent.action.VIEW").ok()?;
            let intent = env
                .new_object(
                    "android/content/Intent",
                    "(Ljava/lang/String;)V",
                    &[JValue::Object(&JObject::from(action))],
                )
                .ok()?;
            let mime = env
                .new_string("application/vnd.android.package-archive")
                .ok()?;
            env.call_method(
                &intent,
                "setDataAndType",
                "(Landroid/net/Uri;Ljava/lang/String;)Landroid/content/Intent;",
                &[JValue::Object(&row), JValue::Object(&JObject::from(mime))],
            )
            .ok()?;
            // FLAG_GRANT_READ_URI_PERMISSION | FLAG_ACTIVITY_NEW_TASK
            env.call_method(
                &intent,
                "addFlags",
                "(I)Landroid/content/Intent;",
                &[JValue::Int(0x0000_0001 | 0x1000_0000)],
            )
            .ok()?;
            env.call_method(
                &activity,
                "startActivity",
                "(Landroid/content/Intent;)V",
                &[JValue::Object(&intent)],
            )
            .ok()?;
            Some(())
        });
        let _ = tx.send(opened.is_some());
    }));
    match rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(true) => Ok(()),
        Ok(false) => Err("the phone would not open the installer".to_string()),
        Err(_) => Err("the installer did not answer".to_string()),
    }
}

/// The `PackageInstaller` session: open, written, committed. A session
/// that cannot be committed is abandoned, so the system holds nothing.
fn install_session(
    env: &mut jni::JNIEnv<'_>,
    activity: *mut c_void,
    bytes: &[u8],
) -> Result<(), String> {
    let activity = activity_ref(env, activity).ok_or("no activity")?;
    // The installer comes from the package manager, which every Android
    // has: asking `getSystemService("package")` for it answers with the
    // package manager itself, or with nothing at all.
    let manager = env
        .call_method(
            &activity,
            "getPackageManager",
            "()Landroid/content/pm/PackageManager;",
            &[],
        )
        .map_err(|e| e.to_string())?
        .l()
        .map_err(|e| e.to_string())?;
    if let Some(e) = java_error(env, "the package manager") {
        return Err(e);
    }
    if manager.is_null() {
        return Err("the system has no package manager".to_string());
    }
    let installer = env
        .call_method(
            &manager,
            "getPackageInstaller",
            "()Landroid/content/pm/PackageInstaller;",
            &[],
        )
        .map_err(|e| e.to_string())?
        .l()
        .map_err(|e| e.to_string())?;
    if let Some(e) = java_error(env, "the package installer") {
        return Err(e);
    }
    if installer.is_null() {
        return Err("the system has no package installer".to_string());
    }
    // PackageInstaller.SessionParams(MODE_FULL_INSTALL).
    let params = env
        .new_object(
            "android/content/pm/PackageInstaller$SessionParams",
            "(I)V",
            &[JValue::Int(1)],
        )
        .map_err(|e| e.to_string())?;
    let id = env
        .call_method(
            &installer,
            "createSession",
            "(Landroid/content/pm/PackageInstaller$SessionParams;)I",
            &[JValue::Object(&params)],
        )
        .map_err(|e| e.to_string())?
        .i()
        .map_err(|e| e.to_string())?;
    if let Some(e) = java_error(env, "the installer session") {
        return Err(e);
    }
    log::info!("installer session {id} created");
    let session = env
        .call_method(
            &installer,
            "openSession",
            "(I)Landroid/content/pm/PackageInstaller$Session;",
            &[JValue::Int(id)],
        )
        .map_err(|e| e.to_string())?
        .l()
        .map_err(|e| e.to_string())?;
    if let Some(e) = java_error(env, "opening the session") {
        return Err(e);
    }
    if session.is_null() {
        return Err("the installer opened no session".to_string());
    }
    let name = env.new_string("base.apk").map_err(|e| e.to_string())?;
    let out = env
        .call_method(
            &session,
            "openWrite",
            "(Ljava/lang/String;JJ)Ljava/io/OutputStream;",
            &[
                JValue::Object(&JObject::from(name)),
                JValue::Long(0),
                JValue::Long(-1),
            ],
        )
        .map_err(|e| e.to_string())?
        .l()
        .map_err(|e| e.to_string())?;
    let written = write_session(env, &out, bytes).and_then(|()| {
        env.call_method(
            &session,
            "fsync",
            "(Ljava/io/OutputStream;)V",
            &[JValue::Object(&out)],
        )
        .map_err(|e| e.to_string())?;
        env.call_method(&out, "close", "()V", &[])
            .map_err(|e| e.to_string())?;
        Ok(())
    });
    if let Err(e) = written {
        let _ = env.call_method(&installer, "abandonSession", "(I)V", &[JValue::Int(id)]);
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
        }
        return Err(e);
    }
    // FLAG_UPDATE_CURRENT | FLAG_MUTABLE: the installer fills the result
    // in, and the app does not read it.
    const FLAGS: i32 = 0x0800_0000 | 0x0200_0000;
    let action = env
        .new_string("dev.tracedraw.app.UPDATED")
        .map_err(|e| e.to_string())?;
    let intent = env
        .new_object(
            "android/content/Intent",
            "(Ljava/lang/String;)V",
            &[JValue::Object(&JObject::from(action))],
        )
        .map_err(|e| e.to_string())?;
    // Android 14 refuses a mutable PendingIntent whose intent names neither
    // a component nor a package, and an action alone is an implicit one, so
    // the call throws. Naming this app's package makes it explicit.
    let package = env
        .call_method(&activity, "getPackageName", "()Ljava/lang/String;", &[])
        .map_err(|e| e.to_string())?
        .l()
        .map_err(|e| e.to_string())?;
    env.call_method(
        &intent,
        "setPackage",
        "(Ljava/lang/String;)Landroid/content/Intent;",
        &[JValue::Object(&package)],
    )
    .map_err(|e| e.to_string())?;
    let pending = env
        .call_static_method(
            "android/app/PendingIntent",
            "getBroadcast",
            "(Landroid/content/Context;ILandroid/content/Intent;I)Landroid/app/PendingIntent;",
            &[
                JValue::Object(&activity),
                JValue::Int(0),
                JValue::Object(&intent),
                JValue::Int(FLAGS),
            ],
        )
        .map_err(|e| e.to_string())?
        .l()
        .map_err(|e| e.to_string())?;
    let sender = env
        .call_method(
            &pending,
            "getIntentSender",
            "()Landroid/content/IntentSender;",
            &[],
        )
        .map_err(|e| e.to_string())?
        .l()
        .map_err(|e| e.to_string())?;
    let committed = env.call_method(
        &session,
        "commit",
        "(Landroid/content/IntentSender;)V",
        &[JValue::Object(&sender)],
    );
    if env.exception_check().unwrap_or(false) {
        // What the system said, before the exception is cleared: the bare
        // "refused the update" told nobody why.
        let refused = java_error(env, "the installer")
            .unwrap_or_else(|| "the installer refused the update".to_string());
        log::warn!("the installer refused session {id}: {refused}");
        let _ = env.call_method(&installer, "abandonSession", "(I)V", &[JValue::Int(id)]);
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
        }
        return Err(refused);
    }
    // `commit` answers nothing: the result arrives at the PendingIntent.
    committed.map(|_| ()).map_err(|e| e.to_string())?;
    log::info!("the installer took session {id}; its own prompt follows");
    Ok(())
}

/// The message of a Java exception a call left pending, cleared so the
/// calls after it work; `None` when none was thrown. A call that throws
/// answers with a null object, so a failure says what the exception said
/// rather than that something was missing.
fn java_error(env: &mut jni::JNIEnv<'_>, what: &str) -> Option<String> {
    if !env.exception_check().unwrap_or(false) {
        return None;
    }
    let mut text = String::new();
    if let Ok(thrown) = env.exception_occurred() {
        if let Ok(value) = env.call_method(&thrown, "toString", "()Ljava/lang/String;", &[]) {
            if let Ok(object) = value.l() {
                if let Some(message) = jstring_text(env, object) {
                    text = message;
                }
            }
        }
    }
    let _ = env.exception_clear();
    Some(if text.is_empty() {
        format!("{what} failed")
    } else {
        format!("{what}: {text}")
    })
}

/// The APK's bytes into the session's stream, a block at a time.
fn write_session(
    env: &mut jni::JNIEnv<'_>,
    out: &JObject<'_>,
    bytes: &[u8],
) -> Result<(), String> {
    for chunk in bytes.chunks(256 * 1024) {
        let array = env
            .byte_array_from_slice(chunk)
            .map_err(|e| e.to_string())?;
        env.call_method(
            out,
            "write",
            "([B)V",
            &[JValue::Object(&JObject::from(array))],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// A web page in the phone's browser.
pub fn open_url(app: &AndroidApp, url: &str) {
    let vm = app.vm_as_ptr() as usize;
    let activity = app.activity_as_ptr() as usize;
    let url = url.to_string();
    app.run_on_java_main_thread(Box::new(move || {
        let (vm, activity) = (vm as *mut c_void, activity as *mut c_void);
        let _ = with_env(vm, |env| {
            let activity = activity_ref(env, activity)?;
            let text = env.new_string(url.as_str()).ok()?;
            let uri = env
                .call_static_method(
                    "android/net/Uri",
                    "parse",
                    "(Ljava/lang/String;)Landroid/net/Uri;",
                    &[JValue::Object(&JObject::from(text))],
                )
                .ok()?
                .l()
                .ok()?;
            let action = env.new_string("android.intent.action.VIEW").ok()?;
            let intent = env
                .new_object(
                    "android/content/Intent",
                    "(Ljava/lang/String;Landroid/net/Uri;)V",
                    &[
                        JValue::Object(&JObject::from(action)),
                        JValue::Object(&uri),
                    ],
                )
                .ok()?;
            let _ = env.call_method(
                &activity,
                "startActivity",
                "(Landroid/content/Intent;)V",
                &[JValue::Object(&intent)],
            );
            if env.exception_check().unwrap_or(false) {
                let _ = env.exception_clear();
            }
            Some(())
        });
    }));
}

// ---------------------------------------------------------------------------
// A file of the drawing, out to other apps
// ---------------------------------------------------------------------------

/// Write `bytes` into the phone's Download folder as `name` and offer the
/// file to the system's share sheet, where WeChat, QQ and the rest of the
/// apps take it. Answers whether the sheet was opened: from Android 10 the
/// file goes in through `MediaStore`, which is what gives it an address
/// other apps may read (a plain path cannot leave an app since Android 7),
/// and before that the file is written where the app may write and nothing
/// can be shared.
pub fn save_and_share(
    app: &AndroidApp,
    name: &str,
    mime: &str,
    bytes: &[u8],
) -> Result<bool, String> {
    if AndroidApp::sdk_version() < 29 {
        let path = crate::files::android_documents().join(name);
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
        return Ok(false);
    }
    let vm = app.vm_as_ptr();
    let activity = app.activity_as_ptr();
    with_env(vm, |env| {
        Some(share_from_downloads(env, activity, name, mime, bytes))
    })
    .unwrap_or_else(|| Err("no Java environment".to_string()))
}

/// The `MediaStore` half of [`save_and_share`]: insert a row for the file,
/// write the bytes through it, then hand its `content://` address to the
/// share sheet.
fn share_from_downloads(
    env: &mut jni::JNIEnv<'_>,
    activity_ptr: *mut c_void,
    name: &str,
    mime: &str,
    bytes: &[u8],
) -> Result<bool, String> {
    let activity = activity_ref(env, activity_ptr).ok_or("no activity")?;
    let resolver = env
        .call_method(
            &activity,
            "getContentResolver",
            "()Landroid/content/ContentResolver;",
            &[],
        )
        .map_err(|e| e.to_string())?
        .l()
        .map_err(|e| e.to_string())?;

    // The row the file gets in the Download collection: its name, its type
    // and the folder it belongs to.
    let values = env
        .new_object("android/content/ContentValues", "()V", &[])
        .map_err(|e| e.to_string())?;
    for (key, value) in [
        ("_display_name", name),
        ("mime_type", mime),
        ("relative_path", "Download/"),
    ] {
        let key = env.new_string(key).map_err(|e| e.to_string())?;
        let value = env.new_string(value).map_err(|e| e.to_string())?;
        env.call_method(
            &values,
            "put",
            "(Ljava/lang/String;Ljava/lang/String;)V",
            &[
                JValue::Object(&JObject::from(key)),
                JValue::Object(&JObject::from(value)),
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    let volume = env
        .new_string("external_primary")
        .map_err(|e| e.to_string())?;
    let collection = env
        .call_static_method(
            "android/provider/MediaStore$Downloads",
            "getContentUri",
            "(Ljava/lang/String;)Landroid/net/Uri;",
            &[JValue::Object(&JObject::from(volume))],
        )
        .map_err(|e| e.to_string())?
        .l()
        .map_err(|e| e.to_string())?;
    let uri = env
        .call_method(
            &resolver,
            "insert",
            "(Landroid/net/Uri;Landroid/content/ContentValues;)Landroid/net/Uri;",
            &[JValue::Object(&collection), JValue::Object(&values)],
        )
        .map_err(|e| e.to_string())?
        .l()
        .map_err(|e| e.to_string())?;
    if uri.is_null() {
        return Err("the Download folder refused the file".to_string());
    }

    let out = env
        .call_method(
            &resolver,
            "openOutputStream",
            "(Landroid/net/Uri;)Ljava/io/OutputStream;",
            &[JValue::Object(&uri)],
        )
        .map_err(|e| e.to_string())?
        .l()
        .map_err(|e| e.to_string())?;
    if out.is_null() {
        return Err("the Download folder could not be written".to_string());
    }
    let written = write_session(env, &out, bytes).and_then(|()| {
        env.call_method(&out, "close", "()V", &[])
            .map_err(|e| e.to_string())
    });
    if let Err(e) = written {
        // A half-written row would sit in the Download folder for good.
        let _ = env.call_method(
            &resolver,
            "delete",
            "(Landroid/net/Uri;Ljava/lang/String;[Ljava/lang/String;)I",
            &[
                JValue::Object(&uri),
                JValue::Object(&JObject::null()),
                JValue::Object(&JObject::null()),
            ],
        );
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
        }
        return Err(e);
    }

    // The share sheet: ACTION_SEND with the file's address, readable by
    // whichever app is chosen.
    let action = env
        .new_string("android.intent.action.SEND")
        .map_err(|e| e.to_string())?;
    let intent = env
        .new_object(
            "android/content/Intent",
            "(Ljava/lang/String;)V",
            &[JValue::Object(&JObject::from(action))],
        )
        .map_err(|e| e.to_string())?;
    let kind = env.new_string(mime).map_err(|e| e.to_string())?;
    env.call_method(
        &intent,
        "setType",
        "(Ljava/lang/String;)Landroid/content/Intent;",
        &[JValue::Object(&JObject::from(kind))],
    )
    .map_err(|e| e.to_string())?;
    let extra = env
        .new_string("android.intent.extra.STREAM")
        .map_err(|e| e.to_string())?;
    env.call_method(
        &intent,
        "putExtra",
        "(Ljava/lang/String;Landroid/os/Parcelable;)Landroid/content/Intent;",
        &[JValue::Object(&JObject::from(extra)), JValue::Object(&uri)],
    )
    .map_err(|e| e.to_string())?;
    // FLAG_GRANT_READ_URI_PERMISSION: the app chosen may read the file.
    env.call_method(
        &intent,
        "addFlags",
        "(I)Landroid/content/Intent;",
        &[JValue::Int(1)],
    )
    .map_err(|e| e.to_string())?;
    let title = env.new_string("分享").map_err(|e| e.to_string())?;
    let chooser = env
        .call_static_method(
            "android/content/Intent",
            "createChooser",
            "(Landroid/content/Intent;Ljava/lang/CharSequence;)Landroid/content/Intent;",
            &[
                JValue::Object(&intent),
                JValue::Object(&JObject::from(title)),
            ],
        )
        .map_err(|e| e.to_string())?
        .l()
        .map_err(|e| e.to_string())?;
    let started = env.call_method(
        &activity,
        "startActivity",
        "(Landroid/content/Intent;)V",
        &[JValue::Object(&chooser)],
    );
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
        // The file is saved and in the Download folder all the same.
        return Ok(false);
    }
    started.map_err(|e| e.to_string())?;
    Ok(true)
}
