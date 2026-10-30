//! File access for both builds.
//!
//! Natively these are the file system and the system file dialogs, used
//! synchronously as before. In a browser there is no file system: a file
//! the user picks is kept in memory under a synthetic path
//! (`/upload/<name>`), so the path-based open and import code reads it with
//! [`read`] like any file, and [`write`] offers the bytes as a download
//! named after the path's file name.
//!
//! Dialogs come in three shapes:
//! - [`Dialog::pick_file`] runs a closure with the picked path: at once
//!   natively, on a later frame in a browser (see [`run_finished`]).
//! - [`Dialog::pick_into`] stores the picked path under a key that a
//!   dialog's own code collects with [`take_picked`] on the same frame
//!   (natively) or a later one (browser).
//! - [`Dialog::save_file`] runs a closure with the chosen path at once: a
//!   save dialog natively, `/download/<file name>` in a browser.

use crate::app::App;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

type Job = Box<dyn FnOnce(&mut App, PathBuf)>;

thread_local! {
    /// Dialogs finished in the background (browser), waiting for a frame.
    static FINISHED: RefCell<Vec<(Job, PathBuf)>> = const { RefCell::new(Vec::new()) };
    /// Picked paths waiting for the dialog that asked for them.
    static PICKED: RefCell<HashMap<String, PathBuf>> = RefCell::new(HashMap::new());
    /// Uploaded files by synthetic path (browser only).
    static STORE: RefCell<HashMap<PathBuf, Vec<u8>>> = RefCell::new(HashMap::new());
}

/// True in the browser build: no file system, downloads instead of saves.
pub const WEB: bool = cfg!(target_arch = "wasm32");

/// The path naming the same file however it was reached (links, `.`
/// and `..`); the absolute path when that fails (the file is gone);
/// `path` as it is in a browser.
pub fn canonical(path: &Path) -> PathBuf {
    if WEB {
        return path.to_path_buf();
    }
    std::fs::canonicalize(path)
        .or_else(|_| std::path::absolute(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

/// Two canonical paths name one file: on Windows without regard to the
/// letter case and to the `\\?\` prefix.
pub fn same_path(a: &Path, b: &Path) -> bool {
    if cfg!(windows) {
        let key = |p: &Path| {
            let s = p.to_string_lossy().to_lowercase();
            s.strip_prefix(r"\\?\").map(str::to_string).unwrap_or(s)
        };
        key(a) == key(b)
    } else {
        a == b
    }
}

/// A canonical path as people type it: on Windows without the `\\?\`
/// prefix (`\\?\C:\a` is `C:\a`, `\\?\UNC\server\share` is
/// `\\server\share`); other paths as they are.
pub fn plain_path(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    match s.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => path.to_path_buf(),
    }
}

/// Read a whole file (or an uploaded one in a browser).
pub fn read(path: impl AsRef<Path>) -> std::io::Result<Vec<u8>> {
    let path = path.as_ref();
    if WEB {
        return STORE
            .with(|s| s.borrow().get(path).cloned())
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("{} is not available in the browser", path.display()),
                )
            });
    }
    std::fs::read(path)
}

/// Read a whole file as UTF-8 text.
pub fn read_to_string(path: impl AsRef<Path>) -> std::io::Result<String> {
    let bytes = read(path)?;
    String::from_utf8(bytes)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))
}

/// Write a whole file; in a browser, download it under the path's name.
pub fn write(path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> std::io::Result<()> {
    let path = path.as_ref();
    if WEB {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "download".into());
        return download(&name, bytes.as_ref());
    }
    std::fs::write(path, bytes)
}

/// Write a file as `fill` makes it, without holding the whole file in
/// memory; in a browser, the bytes are gathered and downloaded.
pub fn write_with(
    path: impl AsRef<Path>,
    fill: impl FnOnce(&mut dyn std::io::Write) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let path = path.as_ref();
    if WEB {
        let mut bytes = Vec::new();
        fill(&mut bytes)?;
        return write(path, bytes);
    }
    let mut w = std::io::BufWriter::new(std::fs::File::create(path)?);
    fill(&mut w)?;
    std::io::Write::flush(&mut w)
}

/// Write a file as `fill` makes it, creating its folder first
/// (auto-backups). It is written under a `.part` name and renamed when
/// complete, so a write cut short never leaves half a file under the
/// real name. Nothing happens in a browser.
pub fn write_creating_with(
    path: impl AsRef<Path>,
    fill: impl FnOnce(&mut dyn std::io::Write) -> std::io::Result<()>,
) -> std::io::Result<()> {
    if WEB {
        return Ok(());
    }
    let path = path.as_ref();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut part = path.as_os_str().to_owned();
    part.push(".part");
    let part = PathBuf::from(part);
    let written = std::fs::File::create(&part).and_then(|f| {
        let mut w = std::io::BufWriter::new(f);
        fill(&mut w)?;
        std::io::Write::flush(&mut w)
    });
    match written.and_then(|()| std::fs::rename(&part, path)) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&part);
            Err(e)
        }
    }
}

/// Copy a file, creating the target folder (backups). Nothing happens in
/// a browser, which has no files to copy.
pub fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> std::io::Result<()> {
    if WEB {
        return Ok(());
    }
    if let Some(dir) = to.as_ref().parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::copy(from, to).map(|_| ())
}

/// True when a file exists on disk (never in a browser).
pub fn exists(path: impl AsRef<Path>) -> bool {
    !WEB && path.as_ref().is_file()
}

/// Write a file, creating its folder first (auto-backups).
pub fn write_creating(path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> std::io::Result<()> {
    if WEB {
        return Ok(());
    }
    if let Some(dir) = path.as_ref().parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, bytes)
}

/// Delete a file (nothing in a browser).
pub fn remove(path: impl AsRef<Path>) -> std::io::Result<()> {
    if WEB {
        return Ok(());
    }
    std::fs::remove_file(path)
}

/// Choose a folder (desktop only; `None` when cancelled or in a browser).
pub fn pick_folder() -> Option<PathBuf> {
    // A phone has no folder chooser without a system dialog: a folder
    // feature gets the documents folder, the one it may read.
    #[cfg(target_os = "android")]
    {
        Some(android_documents())
    }
    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
    {
        rfd::FileDialog::new().pick_folder()
    }
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

/// The files directly inside a folder (not its subfolders), in no
/// particular order; empty when it cannot be read or in a browser.
pub fn list_dir(path: impl AsRef<Path>) -> Vec<PathBuf> {
    if WEB {
        return Vec::new();
    }
    match std::fs::read_dir(path) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// One entry of a folder listing (the Save Drawing dialog).
#[derive(Debug, Clone, PartialEq)]
pub struct DirEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    /// Bytes (files).
    pub size: u64,
    /// Seconds since 1970, when known.
    pub modified: Option<i64>,
}

/// The folders and files directly inside `dir`: folders first, then by
/// name ignoring case; hidden ones (a leading dot) left out. Empty in a
/// browser or when the folder cannot be read.
pub fn list_entries(dir: impl AsRef<Path>) -> Vec<DirEntry> {
    if WEB {
        return Vec::new();
    }
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<DirEntry> = read
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                return None;
            }
            let path = e.path();
            // Follow links: a link to a folder lists as a folder.
            let meta = std::fs::metadata(&path).ok();
            let is_dir = meta.as_ref().is_some_and(|m| m.is_dir());
            let size = meta.as_ref().map_or(0, |m| m.len());
            let modified = meta
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64);
            Some(DirEntry {
                name,
                path,
                is_dir,
                size,
                modified,
            })
        })
        .collect();
    out.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    out
}

/// True for a folder on disk (never in a browser).
pub fn is_dir(path: impl AsRef<Path>) -> bool {
    !WEB && path.as_ref().is_dir()
}

/// Make a folder (nothing in a browser).
pub fn create_dir(path: impl AsRef<Path>) -> std::io::Result<()> {
    if WEB {
        return Ok(());
    }
    std::fs::create_dir(path)
}

/// Rename a file or folder; one already called `to` is never replaced
/// (nothing in a browser).
pub fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> std::io::Result<()> {
    if WEB {
        return Ok(());
    }
    if to.as_ref().exists() {
        return Err(std::io::Error::from(std::io::ErrorKind::AlreadyExists));
    }
    std::fs::rename(from, to)
}

/// A folder worth a shortcut in the Save Drawing dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Home,
    Desktop,
    Documents,
    Downloads,
    Pictures,
    /// A drive or the file system's root.
    Root,
}

/// The user's home folder.
pub fn home_dir() -> Option<PathBuf> {
    if WEB {
        return None;
    }
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
}

/// The shortcuts that exist on this computer: home, desktop, documents,
/// downloads and pictures (the names the system gives them, read from
/// the user's folder settings on Linux), then the drives (Windows) or
/// the root. Empty in a browser.
pub fn places() -> Vec<(Place, PathBuf)> {
    let mut out = Vec::new();
    let Some(home) = home_dir() else {
        return out;
    };
    out.push((Place::Home, home.clone()));
    let xdg = xdg_user_dirs(&home);
    for (place, key, default) in [
        (Place::Desktop, "XDG_DESKTOP_DIR", "Desktop"),
        (Place::Documents, "XDG_DOCUMENTS_DIR", "Documents"),
        (Place::Downloads, "XDG_DOWNLOAD_DIR", "Downloads"),
        (Place::Pictures, "XDG_PICTURES_DIR", "Pictures"),
    ] {
        let dir = xdg
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, p)| p.clone())
            .unwrap_or_else(|| home.join(default));
        if dir.is_dir() && dir != home {
            out.push((place, dir));
        }
    }
    if cfg!(windows) {
        for letter in b'C'..=b'Z' {
            let root = PathBuf::from(format!("{}:\\", letter as char));
            if root.is_dir() {
                out.push((Place::Root, root));
            }
        }
    } else {
        out.push((Place::Root, PathBuf::from("/")));
    }
    out
}

/// The `XDG_*_DIR` folders of `~/.config/user-dirs.dirs` (Linux), with
/// `$HOME` expanded.
fn xdg_user_dirs(home: &Path) -> Vec<(String, PathBuf)> {
    let Ok(text) = std::fs::read_to_string(home.join(".config/user-dirs.dirs")) else {
        return Vec::new();
    };
    parse_user_dirs(&text, home)
}

/// Read `KEY="$HOME/Folder"` lines.
pub(crate) fn parse_user_dirs(text: &str, home: &Path) -> Vec<(String, PathBuf)> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.starts_with('#') {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            let value = value.trim().trim_matches('"');
            let path = match value.strip_prefix("$HOME") {
                Some(rest) => home.join(rest.trim_start_matches('/')),
                None => PathBuf::from(value),
            };
            Some((key.trim().to_string(), path))
        })
        .collect()
}

/// Decode an image file (any format the image crate was built with).
pub fn open_image(path: impl AsRef<Path>) -> Result<image::DynamicImage, String> {
    let bytes = read(path).map_err(|e| e.to_string())?;
    image::load_from_memory(&bytes).map_err(|e| e.to_string())
}

/// Keep bytes under a synthetic path so [`read`] finds them (browser).
pub fn store(path: &Path, bytes: Vec<u8>) {
    STORE.with(|s| {
        s.borrow_mut().insert(path.to_path_buf(), bytes);
    });
}

/// Keep an uploaded file past the dialog that picked it (a profile that
/// is read again later): returns the path to read it from. Natively the
/// file stays where it is.
pub fn keep(path: &Path) -> PathBuf {
    if !WEB {
        return path.to_path_buf();
    }
    let kept = PathBuf::from("/kept").join(path.file_name().unwrap_or_default());
    if let Ok(bytes) = read(path) {
        store(&kept, bytes);
    }
    kept
}

/// Forget an uploaded file once nothing needs it any more.
pub fn forget(path: &Path) {
    STORE.with(|s| {
        s.borrow_mut().remove(path);
    });
}

/// Run the dialogs that finished since the last frame (browser). Called
/// once per frame before the interface is drawn.
pub fn run_finished(app: &mut App) {
    let jobs: Vec<(Job, PathBuf)> = FINISHED.with(|f| std::mem::take(&mut *f.borrow_mut()));
    for (job, path) in jobs {
        job(app, path.clone());
        forget(&path);
    }
}

/// Files dropped on the window this frame: `then` runs with each one's
/// path, at once natively, on a later frame in a browser (where the bytes
/// are read first and kept under `/upload/<name>` like a picked file).
pub fn take_dropped(
    ctx: &egui::Context,
    app: &mut App,
    then: impl FnOnce(&mut App, PathBuf) + Clone + 'static,
) {
    let dropped = ctx.input(|i| i.raw.dropped_files.clone());
    for file in dropped {
        #[cfg(not(target_arch = "wasm32"))]
        then.clone()(app, file.path().to_path_buf());
        #[cfg(target_arch = "wasm32")]
        {
            let _ = &app;
            let then = then.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match file.bytes_async().await {
                    Ok(bytes) => {
                        let name = file.path().file_name().unwrap_or_default();
                        let path = PathBuf::from("/upload").join(name);
                        store(&path, bytes);
                        let job: Job = Box::new(then);
                        FINISHED.with(|f| f.borrow_mut().push((job, path)));
                        crate::web::request_repaint();
                    }
                    Err(e) => log::warn!("{}: {e}", file.path().display()),
                }
            });
        }
    }
}

/// The path picked for `key` by [`Dialog::pick_into`], once.
pub fn take_picked(key: &str) -> Option<PathBuf> {
    PICKED.with(|p| p.borrow_mut().remove(key))
}

// ---------------------------------------------------------------------------
// A phone has no file dialogs of the system's: a dialog becomes a request
// the interface answers with its own list (see `ui::mobile::files_sheet`).
// The asking code is the same as on the desktop: it hands over a closure
// that runs with the path, and gets it a frame or two later.
// ---------------------------------------------------------------------------

/// What the interface shows for a file dialog asked on a phone.
#[cfg(target_os = "android")]
#[derive(Debug, Clone)]
pub struct AskInfo {
    /// The extensions the dialog offers, lower case, empty for any file.
    pub exts: Vec<String>,
    /// Whether more than one file may be picked.
    pub multiple: bool,
    /// Whether the answer is kept for [`take_picked`] rather than run.
    pub into: bool,
}

/// A file dialog waiting for the interface's answer.
#[cfg(target_os = "android")]
enum Ask {
    Open {
        exts: Vec<String>,
        then: Job,
    },
    Many {
        exts: Vec<String>,
        then: Box<dyn FnOnce(&mut App, Vec<PathBuf>)>,
    },
    Into {
        exts: Vec<String>,
        key: String,
    },
}

#[cfg(target_os = "android")]
thread_local! {
    static ASK: RefCell<Option<Ask>> = const { RefCell::new(None) };
    /// How many dialogs have been asked since the app started: a new
    /// number means a new dialog, which the interface opens its list for
    /// once (a number that stayed the same is the one already shown, and
    /// the list must be free to close over it).
    static ASK_SEQ: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    /// The folder the Android build keeps its drawings in.
    static DOCUMENTS: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

/// A number that changes with every file dialog asked on a phone.
#[cfg(target_os = "android")]
pub fn ask_generation() -> u64 {
    ASK_SEQ.with(|s| s.get())
}

/// Keep the dialog asked, and count it as a new one.
#[cfg(target_os = "android")]
fn set_ask(ask: Ask) {
    ASK.with(|a| *a.borrow_mut() = Some(ask));
    ASK_SEQ.with(|s| s.set(s.get().wrapping_add(1)));
}

/// The folder drawings are read from and written to on a phone, set once
/// as the app starts.
#[cfg(target_os = "android")]
pub fn set_android_documents(dir: PathBuf) {
    DOCUMENTS.with(|d| *d.borrow_mut() = Some(dir));
}

/// Look again for the folder the drawings live in: the shared Download
/// folder once the phone lets the app see the storage, the app's own
/// before that. Called as the Files sheet opens, since the user may have
/// just granted the access in the system's screen.
#[cfg(target_os = "android")]
pub fn refresh_android_documents() {
    let shared = crate::android_system::phone().and_then(|app| {
        if !crate::android_system::storage_granted(app) {
            return None;
        }
        let root = crate::android_system::storage_root(app)?;
        let downloads = root.join("Download");
        if !downloads.is_dir() {
            let _ = std::fs::create_dir_all(&downloads);
        }
        if downloads.is_dir() {
            Some(downloads)
        } else {
            Some(root)
        }
    });
    let dir = shared.unwrap_or_else(crate::settings::android_documents);
    let _ = std::fs::create_dir_all(&dir);
    set_android_documents(dir);
}

/// The highest folder the phone's file list goes up to: the root of the
/// shared storage, so the way up stops at `emulated/0` instead of walking
/// into the system's own folders. Without the storage grant it is the
/// folder the app keeps its drawings in.
#[cfg(target_os = "android")]
pub fn android_root() -> PathBuf {
    if let Some(phone) = crate::android_system::phone() {
        if crate::android_system::storage_granted(phone) {
            if let Some(root) = crate::android_system::storage_root(phone) {
                if root.is_dir() {
                    return root;
                }
            }
        }
    }
    android_documents()
}

#[cfg(not(target_os = "android"))]
pub fn android_root() -> PathBuf {
    android_documents()
}

/// The folder drawings are read from and written to on a phone.
#[cfg(target_os = "android")]
pub fn android_documents() -> PathBuf {
    DOCUMENTS
        .with(|d| d.borrow().clone())
        .unwrap_or_else(crate::settings::android_documents)
}

/// The folder the phone layout shows its file list from, in a build that
/// is not the phone's (the visual checks, `TRACEDRAW_MOBILE=1`): the
/// folder named by `TRACEDRAW_MOBILE_DIR`, else the working folder.
#[cfg(not(target_os = "android"))]
pub fn android_documents() -> PathBuf {
    std::env::var_os("TRACEDRAW_MOBILE_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default()
}

/// The file dialog waiting for an answer, for the interface to show.
#[cfg(target_os = "android")]
pub fn ask() -> Option<AskInfo> {
    ASK.with(|a| {
        a.borrow().as_ref().map(|ask| match ask {
            Ask::Open { exts, .. } => AskInfo {
                exts: exts.clone(),
                multiple: false,
                into: false,
            },
            Ask::Many { exts, .. } => AskInfo {
                exts: exts.clone(),
                multiple: true,
                into: false,
            },
            Ask::Into { exts, .. } => AskInfo {
                exts: exts.clone(),
                multiple: false,
                into: true,
            },
        })
    })
}

/// Answer the waiting dialog with `paths`; false when none was waiting.
#[cfg(target_os = "android")]
pub fn answer(app: &mut App, paths: Vec<PathBuf>) -> bool {
    let Some(ask) = ASK.with(|a| a.borrow_mut().take()) else {
        return false;
    };
    match ask {
        Ask::Open { then, .. } => {
            if let Some(path) = paths.into_iter().next() {
                then(app, path);
            }
        }
        Ask::Many { then, .. } => then(app, paths),
        Ask::Into { key, .. } => {
            if let Some(path) = paths.into_iter().next() {
                PICKED.with(|p| {
                    p.borrow_mut().insert(key, path);
                });
            }
        }
    }
    true
}

/// Drop the waiting dialog, as cancelling it does.
#[cfg(target_os = "android")]
pub fn cancel_ask() {
    ASK.with(|a| *a.borrow_mut() = None);
}

/// The extensions of a dialog's filters, for the list to sift by.
#[cfg(target_os = "android")]
fn ask_exts(filters: &[(String, Vec<String>)]) -> Vec<String> {
    filters
        .iter()
        .flat_map(|(_, e)| e.iter())
        .map(|e| e.to_ascii_lowercase())
        .collect()
}

/// A path in the documents folder for a name, made safe and not taken:
/// `Drawing (2).tdraw` when the name is already there.
#[cfg(target_os = "android")]
fn android_save_path(name: &str) -> PathBuf {
    let dir = android_documents();
    let _ = std::fs::create_dir_all(&dir);
    let safe: String = name
        .chars()
        .map(|c| if "/\\:*?\"<>|".contains(c) { '_' } else { c })
        .collect();
    let safe = if safe.trim().is_empty() {
        "Drawing".to_string()
    } else {
        safe
    };
    let path = PathBuf::from(&safe);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Drawing".into());
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let mut candidate = dir.join(&safe);
    let mut n = 2;
    while candidate.exists() {
        candidate = dir.join(format!("{stem} ({n}){ext}"));
        n += 1;
    }
    candidate
}

/// A file dialog description, the same in both builds.
#[derive(Debug, Clone, Default)]
pub struct Dialog {
    filters: Vec<(String, Vec<String>)>,
    file_name: Option<String>,
}

impl Dialog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_filter(mut self, name: impl Into<String>, extensions: &[&str]) -> Self {
        self.filters.push((
            name.into(),
            extensions.iter().map(|e| e.to_string()).collect(),
        ));
        self
    }

    pub fn set_file_name(mut self, name: impl Into<String>) -> Self {
        self.file_name = Some(name.into());
        self
    }

    /// Pick a file and run `then` with its path: at once natively, on a
    /// later frame in a browser or on a phone.
    pub fn pick_file(self, app: &mut App, then: impl FnOnce(&mut App, PathBuf) + 'static) {
        #[cfg(target_os = "android")]
        {
            let _ = app;
            set_ask(Ask::Open {
                exts: ask_exts(&self.filters),
                then: Box::new(then),
            });
        }
        #[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
        if let Some(path) = self.native_pick() {
            then(app, path);
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = app;
            self.web_pick(Box::new(then));
        }
    }

    /// Pick one or more files and run `then` with their paths (one file in
    /// a browser).
    pub fn pick_files(self, app: &mut App, then: impl FnOnce(&mut App, Vec<PathBuf>) + 'static) {
        #[cfg(target_os = "android")]
        {
            let _ = app;
            set_ask(Ask::Many {
                exts: ask_exts(&self.filters),
                then: Box::new(then),
            });
        }
        #[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
        {
            let mut d = rfd::FileDialog::new();
            for (name, exts) in &self.filters {
                let exts: Vec<&str> = exts.iter().map(|e| e.as_str()).collect();
                d = d.add_filter(name, &exts);
            }
            if let Some(paths) = d.pick_files() {
                then(app, paths);
            }
        }
        #[cfg(target_arch = "wasm32")]
        self.pick_file(app, move |app, p| then(app, vec![p]));
    }

    /// Pick a file and keep its path for [`take_picked`]`(key)`.
    pub fn pick_into(self, key: &str) {
        let key = key.to_string();
        #[cfg(target_os = "android")]
        set_ask(Ask::Into {
            exts: ask_exts(&self.filters),
            key,
        });
        #[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
        if let Some(path) = self.native_pick() {
            PICKED.with(|p| {
                p.borrow_mut().insert(key, path);
            });
        }
        // The upload outlives this job: the dialog reads it on a later frame.
        #[cfg(target_arch = "wasm32")]
        self.web_pick(Box::new(move |_app, path| {
            let kept = keep(&path);
            PICKED.with(|p| {
                p.borrow_mut().insert(key, kept);
            });
        }));
    }

    /// Choose where to save and run `then` with that path; `then` writes
    /// with [`write`]. In a browser there is no dialog: the path is
    /// `/download/<file name>` and the write becomes a download.
    pub fn save_file(self, app: &mut App, then: impl FnOnce(&mut App, PathBuf)) {
        if let Some(path) = self.save_path() {
            then(app, path);
        }
    }

    /// The path to save to, or `None` when the user cancels: a save
    /// dialog natively, `/download/<file name>` in a browser (write it with
    /// [`write`] to download it), a name in the documents folder on a
    /// phone.
    pub fn save_path(self) -> Option<PathBuf> {
        if WEB {
            let name = self.file_name.unwrap_or_else(|| "download".into());
            return Some(PathBuf::from("/download").join(name));
        }
        #[cfg(target_os = "android")]
        {
            let name = self.file_name.unwrap_or_else(|| "Drawing".into());
            return Some(android_save_path(&name));
        }
        #[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
        {
            let mut d = rfd::FileDialog::new();
            for (name, exts) in &self.filters {
                let exts: Vec<&str> = exts.iter().map(|e| e.as_str()).collect();
                d = d.add_filter(name, &exts);
            }
            if let Some(n) = &self.file_name {
                d = d.set_file_name(n);
            }
            return d.save_file();
        }
        #[allow(unreachable_code)]
        None
    }

    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
    fn native_pick(&self) -> Option<PathBuf> {
        let mut d = rfd::FileDialog::new();
        for (name, exts) in &self.filters {
            let exts: Vec<&str> = exts.iter().map(|e| e.as_str()).collect();
            d = d.add_filter(name, &exts);
        }
        d.pick_file()
    }

    #[cfg(target_arch = "wasm32")]
    fn web_pick(self, then: Job) {
        let mut d = rfd::AsyncFileDialog::new();
        for (name, exts) in &self.filters {
            let exts: Vec<&str> = exts.iter().map(|e| e.as_str()).collect();
            d = d.add_filter(name, &exts);
        }
        wasm_bindgen_futures::spawn_local(async move {
            let Some(handle) = d.pick_file().await else {
                return;
            };
            let name = handle.file_name();
            let bytes = handle.read().await;
            let path = PathBuf::from("/upload").join(name);
            store(&path, bytes);
            FINISHED.with(|f| f.borrow_mut().push((then, path)));
            crate::web::request_repaint();
        });
    }
}

/// Offer bytes as a browser download (no-op natively).
#[cfg(target_arch = "wasm32")]
fn download(name: &str, bytes: &[u8]) -> std::io::Result<()> {
    crate::web::download(name, bytes).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
}

#[cfg(not(target_arch = "wasm32"))]
fn download(_name: &str, _bytes: &[u8]) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picked_paths_are_taken_once() {
        PICKED.with(|p| {
            p.borrow_mut().insert("k".into(), PathBuf::from("/x/a.csv"));
        });
        assert_eq!(take_picked("k"), Some(PathBuf::from("/x/a.csv")));
        assert_eq!(take_picked("k"), None);
    }

    #[test]
    fn finished_jobs_run_once_and_free_their_upload() {
        let mut app = App::headless();
        let path = PathBuf::from("/upload/t.txt");
        store(&path, b"hello".to_vec());
        FINISHED.with(|f| {
            f.borrow_mut().push((
                Box::new(|app: &mut App, p: PathBuf| {
                    app.status = format!("got {}", p.display());
                }),
                path.clone(),
            ))
        });
        run_finished(&mut app);
        assert_eq!(app.status, "got /upload/t.txt");
        run_finished(&mut app);
        assert_eq!(app.status, "got /upload/t.txt");
        assert!(STORE.with(|s| s.borrow().get(&path).is_none()));
    }

    #[test]
    fn native_read_and_write_use_the_file_system() {
        let dir = std::env::temp_dir().join(format!("tracedraw-files-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("a.bin");
        write(&p, [1u8, 2, 3]).unwrap();
        assert_eq!(read(&p).unwrap(), vec![1, 2, 3]);
        assert!(read(dir.join("missing")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
