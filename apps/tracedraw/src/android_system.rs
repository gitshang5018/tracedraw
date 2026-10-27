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
    f(&mut env)
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
        }
        // Extend into the cutout areas (short edges / notch) in landscape,
        // eliminating the black letterbox bar on the camera edge.
        let mode = if sdk >= 30 { 3 } else { 1 };
        if let Ok(val) = env.call_method(
            &window,
            "getAttributes",
            "()Landroid/view/WindowManager$LayoutParams;",
            &[],
        ) {
            if let Ok(params) = val.l() {
                if !params.is_null() {
                    let _ = env.set_field(&params, "layoutInDisplayCutoutMode", "I", JValue::Int(mode));
                    let _ = env.call_method(
                        &window,
                        "setAttributes",
                        "(Landroid/view/WindowManager$LayoutParams;)V",
                        &[JValue::Object(&params)],
                    );
                }
            }
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

        // Measure DisplayCutout safely across API 28+
        if let Ok(val) = env.call_method(&insets, "getDisplayCutout", "()Landroid/view/DisplayCutout;", &[]) {
            if let Ok(cutout_obj) = val.l() {
                if !cutout_obj.is_null() {
                    let cl = env.call_method(&cutout_obj, "getSafeInsetLeft", "()I", &[]).ok().and_then(|v| v.i().ok()).unwrap_or(0);
                    let ct = env.call_method(&cutout_obj, "getSafeInsetTop", "()I", &[]).ok().and_then(|v| v.i().ok()).unwrap_or(0);
                    let cr = env.call_method(&cutout_obj, "getSafeInsetRight", "()I", &[]).ok().and_then(|v| v.i().ok()).unwrap_or(0);
                    let cb = env.call_method(&cutout_obj, "getSafeInsetBottom", "()I", &[]).ok().and_then(|v| v.i().ok()).unwrap_or(0);
                    left = left.max(cl);
                    top = top.max(ct);
                    right = right.max(cr);
                    bottom = bottom.max(cb);
                }
            }
        }

        Some(Insets {
            top,
            bottom,
            left,
            right,
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
static ASKED: std::sync::Mutex<Option<(std::time::Instant, u8)>> = std::sync::Mutex::new(None);

/// Show or hide the phone's keyboard: the field is put in the window and
/// focused, or taken out again.
pub fn set_keyboard(app: &AndroidApp, show: bool) {
    use std::sync::atomic::Ordering;
    if KEYBOARD.swap(show, Ordering::Relaxed) == show {
        return;
    }
    lock(&TYPED).clear();
    if show {
        *lock(&ASKED) = Some((std::time::Instant::now(), 0));
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
                    *asked = Some((std::time::Instant::now(), times + 1));
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
    with_env(app.vm_as_ptr(), |env| {
        env.call_static_method(
            "android/os/Environment",
            "isExternalStorageManager",
            "()Z",
            &[],
        )
        .ok()?
        .z()
        .ok()
    })
    .unwrap_or(false)
}

/// The folder the phone keeps the user's files in (`/storage/emulated/0`).
pub fn storage_root(app: &AndroidApp) -> Option<std::path::PathBuf> {
    with_env(app.vm_as_ptr(), |env| {
        let dir = env
            .call_static_method(
                "android/os/Environment",
                "getExternalStorageDirectory",
                "()Ljava/io/File;",
                &[],
            )
            .ok()?
            .l()
            .ok()?;
        if dir.is_null() {
            return None;
        }
        let path = env
            .call_method(&dir, "getAbsolutePath", "()Ljava/lang/String;", &[])
            .ok()?
            .l()
            .ok()?;
        let path = JString::from(path);
        let path: String = env.get_string(&path).ok()?.into();
        (!path.is_empty()).then(|| std::path::PathBuf::from(path))
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
        let name = env
            .call_method(&activity, "getPackageName", "()Ljava/lang/String;", &[])
            .ok()?
            .l()
            .ok()?;
        let name = JString::from(name);
        let name: String = env.get_string(&name).ok()?.into();
        let text: JObject = JObject::from(env.new_string(format!("package:{name}")).ok()?);
        let uri = env
            .call_static_method(
                "android/net/Uri",
                "parse",
                "(Ljava/lang/String;)Landroid/net/Uri;",
                &[JValue::Object(&text)],
            )
            .ok()?
            .l()
            .ok()?;
        let action: JObject = JObject::from(
            env.new_string("android.settings.MANAGE_APP_ALL_FILES_ACCESS_PERMISSION")
                .ok()?,
        );
        let intent = env
            .new_object(
                "android/content/Intent",
                "(Ljava/lang/String;Landroid/net/Uri;)V",
                &[JValue::Object(&action), JValue::Object(&uri)],
            )
            .ok()?;
        let _ = env.call_method(
            &activity,
            "startActivity",
            "(Landroid/content/Intent;)V",
            &[JValue::Object(&intent)],
        );
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
    let vm = app.vm_as_ptr();
    let activity = app.activity_as_ptr();
    with_env(vm, |env| {
        let activity = activity_ref(env, activity)?;
        let window = window(env, &activity)?;
        let decor = env
            .call_method(&window, "getDecorView", "()Landroid/view/View;", &[])
            .ok()?
            .l()
            .ok()?;
        if AndroidApp::sdk_version() >= 30 {
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
            // LAYOUT_STABLE | LAYOUT_HIDE_NAVIGATION | LAYOUT_FULLSCREEN,
            // and with them FULLSCREEN | HIDE_NAVIGATION | IMMERSIVE_STICKY.
            const LAYOUT: i32 = 1 | 512 | 1024;
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
}
