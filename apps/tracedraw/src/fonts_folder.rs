//! The fonts a phone keeps in a folder of its own.
//!
//! The font lists are built from the system's fonts and from the ones a
//! drawing carries. A phone installs no fonts, so a designer drops the
//! `.ttf`, `.otf` and `.ttc` files into `Fonts` in the folder the drawings
//! live in instead (Download, once the phone lets the app see it; the app's
//! own Documents before that) and they join the lists.
//!
//! They are read once at start and again when the Files sheet asks, which
//! is how a file added while TraceDraw runs gets in without a restart. A
//! file already read is not read again, so asking costs a folder listing.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;

/// The files already read, and the families they gave.
static LOADED: Mutex<Option<HashSet<PathBuf>>> = Mutex::new(None);

/// The folders a phone's own fonts are read from, in the order they are
/// read: `Fonts` beside the drawings, then the app's own folder.
pub fn folders() -> Vec<PathBuf> {
    #[cfg(target_os = "android")]
    {
        let mut out: Vec<PathBuf> = Vec::new();
        // The shared Download folder itself, which is where a phone puts
        // what a chat app saves, and what the drawings folder is once the
        // storage grant is in place.
        if let Some(phone) = crate::android_system::phone() {
            if crate::android_system::storage_granted(phone) {
                if let Some(root) = crate::android_system::storage_root(phone) {
                    out.push(root.join("Download").join("Fonts"));
                }
            }
        }
        // Then the folder the drawings live in, and the app's own, which is
        // where they live before the phone lets it see Download.
        out.push(crate::files::android_documents().join("Fonts"));
        out.push(crate::settings::android_documents().join("Fonts"));
        out.dedup();
        out
    }
    #[cfg(not(target_os = "android"))]
    {
        // A desktop reads the system's fonts already; its own live beside
        // the settings.
        crate::settings::config_dir()
            .map(|dir| vec![dir.join("Fonts")])
            .unwrap_or_default()
    }
}

/// Read the fonts in [`folders`] that were not read before; answers how
/// many families they added.
pub fn scan() -> usize {
    let Ok(mut loaded) = LOADED.lock() else {
        return 0;
    };
    let loaded = loaded.get_or_insert_with(HashSet::new);
    let mut files = Vec::new();
    for dir in folders() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !is_font(&path) || loaded.contains(&path) {
                continue;
            }
            let Ok(data) = std::fs::read(&path) else {
                continue;
            };
            // Read once, even if it turns out not to be a font.
            loaded.insert(path);
            files.push(data);
        }
    }
    if files.is_empty() {
        return 0;
    }
    tracedraw_text::fonts().add_fonts(files).len()
}

/// Whether the storage grant has been seen: the fonts in the shared
/// `Download` folder cannot be read until the phone has let the app see it,
/// and the scan at start runs before that.
static GRANTED_SEEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Read the fonts again once the phone has let the app see its storage.
///
/// The scan at start runs before the grant, so a designer's fonts in
/// `Download/Fonts` were not read until the Files sheet asked: the drawing
/// opened with a font missing, and only tapping "reload fonts" brought it
/// in. This is asked for on every frame, and answers at once once it has
/// been done, so the cost is one atomic load.
pub fn scan_when_storage_is_granted() {
    #[cfg(target_os = "android")]
    {
        if GRANTED_SEEN.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        let Some(phone) = crate::android_system::phone() else {
            return;
        };
        if !crate::android_system::storage_granted(phone) {
            return;
        }
        GRANTED_SEEN.store(true, std::sync::atomic::Ordering::Relaxed);
        scan();
    }
    #[cfg(not(target_os = "android"))]
    {
        GRANTED_SEEN.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// A name that ends like a font file: `.ttf`, `.otf` or a `.ttc`
/// collection, in any case.
fn is_font(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "ttf" | "otf" | "ttc"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_font_files_count() {
        for name in ["a.ttf", "b.OTF", "c.ttc", "d.Ttf"] {
            assert!(is_font(std::path::Path::new(name)), "{name}");
        }
        for name in ["a.txt", "b.ttf.zip", "font", "c.png"] {
            assert!(!is_font(std::path::Path::new(name)), "{name}");
        }
    }

    #[test]
    fn the_folders_name_fonts_inside_the_drawings_folder() {
        for folder in folders() {
            assert_eq!(folder.file_name().and_then(|n| n.to_str()), Some("Fonts"));
        }
    }
}
