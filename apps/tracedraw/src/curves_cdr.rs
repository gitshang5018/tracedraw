//! 转曲保存: the drawing with every text, table, dimension and symbol
//! instance as curves, written as a `.cdr` of a chosen version (18 is
//! CorelDRAW X8) into the phone's Download folder, then handed to the
//! system's share sheet so it can go to WeChat, QQ or anywhere else.
//!
//! The drawing that is open is never touched: the conversion runs on a copy
//! of it, in an app of its own. A file that does not come out whole leaves
//! nothing behind, and nothing to undo.
//!
//! The name follows the plugin this comes from: `<drawing>_<yyyy-M-d>_x8曲.cdr`,
//! with an older date suffix of that shape dropped from the drawing's name
//! first, so saving the same drawing again on the same day does not stack
//! `_2026-10-28_x8曲_2026-10-28_x8曲`.

use crate::app::App;

/// The version CorelDRAW X8 writes and reads.
pub const X8: u16 = 18;

/// The name a 转曲保存 file takes: `stem` without an older date suffix,
/// then the day and the version's mark.
pub fn file_name(stem: &str, date: &str, version: u16) -> String {
    let mut base = stem.trim().trim_end_matches('*').trim().to_string();
    // An older `<date>_x4曲` or `<date>_x8曲` suffix of a file saved before
    // goes, so the name stays the drawing's own.
    for mark in ["_x4曲", "_x8曲"] {
        if let Some(at) = base.rfind(mark) {
            let before = &base[..at];
            if let Some(dash) = before.rfind('_') {
                let date_like = &before[dash + 1..];
                let parts: Vec<&str> = date_like.split('-').collect();
                if parts.len() == 3 && parts.iter().all(|p| p.parse::<u32>().is_ok()) {
                    base.truncate(dash);
                }
            }
        }
    }
    if base.trim().is_empty() {
        base = "未命名".to_string();
    }
    let mark = if version >= X8 { "x8曲" } else { "x4曲" };
    format!("{base}_{date}_{mark}.cdr")
}

/// Today, as `2026-10-28`, in the phone's own time zone.
pub fn today() -> String {
    let now = jiff::Zoned::now();
    format!("{}-{}-{}", now.year(), now.month(), now.day())
}

impl App {
    /// The drawing as a `.cdr` of `version`, with everything Convert to
    /// Curves changes turned into curves. The drawing that is open is left
    /// as it was, undo history and all.
    pub fn cdr_curves(&self, version: u16) -> Vec<u8> {
        let mut work = App::scratch();
        work.open_document(self.doc().clone(), None);
        let pages = work.doc().pages.len();
        for i in 0..pages {
            work.goto_page(i);
            work.select_all();
            work.convert_to_curves();
        }
        tracedraw_cdr::write::document_to_cdr_version(work.doc(), version)
    }

    /// 转曲保存: write the drawing as curves, as a `.cdr` of `version`,
    /// into the phone's Download folder, and offer it to the share sheet.
    /// Answers the file's name.
    pub fn save_curves_cdr(&mut self, version: u16) -> Result<String, String> {
        if !self.has_document() {
            return Err("no drawing is open".to_string());
        }
        let bytes = self.cdr_curves(version);
        let name = file_name(&self.document_title(), &today(), version);
        #[cfg(target_os = "android")]
        {
            let phone = crate::android_system::phone().ok_or("no activity")?;
            let shared = crate::android_system::save_and_share(
                phone,
                &name,
                "application/octet-stream",
                &bytes,
            )?;
            self.status = if shared {
                crate::i18n::trf("status.saved_share", &[("name", &name)])
            } else {
                crate::i18n::trf("status.saved", &[("p", &name)])
            };
            return Ok(name);
        }
        #[cfg(not(target_os = "android"))]
        {
            // A desktop has folders of its own: the drawing's folder when it
            // has one, else the one the phone layout shows.
            let dir = self
                .file
                .as_ref()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                .unwrap_or_else(crate::files::android_documents);
            let path = dir.join(&name);
            crate::files::write_creating(&path, &bytes).map_err(|e| e.to_string())?;
            let shown = path.display().to_string();
            self.status = crate::i18n::trf("status.saved", &[("p", &shown)]);
            Ok(name)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_carries_the_day_and_the_version() {
        assert_eq!(
            file_name("未命名-1", "2026-10-28", X8),
            "未命名-1_2026-10-28_x8曲.cdr"
        );
        // The dirty mark a title carries is not part of the name.
        assert_eq!(
            file_name("海报*", "2026-10-28", X8),
            "海报_2026-10-28_x8曲.cdr"
        );
        // Saving again the same day does not stack the suffix.
        let once = file_name("海报", "2026-10-28", X8);
        let twice = file_name(once.trim_end_matches(".cdr"), "2026-10-28", X8);
        assert_eq!(twice, "海报_2026-10-28_x8曲.cdr");
        // Nor does an older day's, and the other version's either.
        assert_eq!(
            file_name("海报_2026-10-27_x8曲", "2026-10-28", X8),
            "海报_2026-10-28_x8曲.cdr"
        );
        assert_eq!(
            file_name("海报_2026-10-27_x4曲", "2026-10-28", 14),
            "海报_2026-10-28_x4曲.cdr"
        );
        // A drawing with no name of its own still gets one.
        assert_eq!(file_name("", "2026-10-28", X8), "未命名_2026-10-28_x8曲.cdr");
        // A name that only ends like the suffix keeps it.
        assert_eq!(
            file_name("曲_x8曲稿", "2026-10-28", X8),
            "曲_x8曲稿_2026-10-28_x8曲.cdr"
        );
    }

    #[test]
    fn the_day_is_written_the_way_the_name_wants_it() {
        let day = today();
        let parts: Vec<&str> = day.split('-').collect();
        assert_eq!(parts.len(), 3, "{day}");
        assert_eq!(parts[0].len(), 4, "{day}");
    }
}
