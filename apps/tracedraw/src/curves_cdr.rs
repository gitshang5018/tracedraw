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

/// A 转曲保存 on its way: the copy of the drawing the pages are converted
/// in, one page per frame, and the name the file takes.
pub struct CurvesSave {
    work: Box<App>,
    name: String,
    version: u16,
    pages: usize,
    done: usize,
}

impl App {
    /// Begin a 转曲保存: the copy is made now, and a page is converted per
    /// frame by [`App::curves_tick`], so the bar moves and the interface
    /// keeps drawing. Answers an error when there is nothing to save.
    pub fn start_curves_save(&mut self, version: u16) -> Result<(), String> {
        if !self.has_document() {
            return Err("no drawing is open".to_string());
        }
        let mut work = App::scratch();
        work.open_document(self.doc().clone(), None);
        let pages = work.doc().pages.len().max(1);
        self.curves_save = Some(CurvesSave {
            work: Box::new(work),
            name: file_name(&self.document_title(), &today(), version),
            version,
            pages,
            done: 0,
        });
        Ok(())
    }

    /// One frame of a 转曲保存: convert the next page, and once they are all
    /// converted write the file and offer it. `ctx` gets a frame while it
    /// runs, so the bar reaches the end before the file is written.
    pub fn curves_tick(&mut self, ctx: &egui::Context) {
        let Some(save) = self.curves_save.as_mut() else {
            return;
        };
        if save.done < save.pages {
            save.work.goto_page(save.done);
            save.work.select_all();
            save.work.convert_to_curves();
            save.done += 1;
            ctx.request_repaint();
            return;
        }
        let Some(save) = self.curves_save.take() else {
            return;
        };
        let bytes =
            tracedraw_cdr::write::document_to_cdr_version(save.work.doc(), save.version);
        let dir = self
            .file
            .as_ref()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()));
        match save_curves_file(dir, &save.name, &bytes) {
            Ok(true) => {
                self.status = crate::i18n::trf("status.saved_share", &[("name", &save.name)]);
            }
            Ok(false) => {
                self.status = crate::i18n::trf("status.saved", &[("p", &save.name)]);
            }
            Err(e) => self.status = e,
        }
    }

    /// How far a 转曲保存 has got: the fraction for the bar, and what it is
    /// doing, for the box drawn over the interface.
    pub fn curves_progress(&self) -> Option<(f32, String)> {
        let save = self.curves_save.as_ref()?;
        let fraction = (save.done as f32 / save.pages as f32).clamp(0.0, 1.0);
        Some((
            fraction,
            format!(
                "{}  {}/{}",
                crate::i18n::tr("mobile.save_curves"),
                save.done,
                save.pages
            ),
        ))
    }
}

/// The box a 转曲保存 shows while it runs: what it is doing, and a bar for
/// how far the pages have got.
pub fn progress_box(app: &App, ctx: &egui::Context) {
    let Some((fraction, text)) = app.curves_progress() else {
        return;
    };
    egui::Area::new(egui::Id::new("curves_save_progress"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            egui::Frame::popup(&ctx.global_style()).show(ui, |ui| {
                ui.set_min_width(240.0);
                ui.label(egui::RichText::new(text).size(15.0));
                ui.add(
                    egui::ProgressBar::new(fraction)
                        .desired_width(220.0)
                        .show_percentage(),
                );
            });
        });
}

/// Write `bytes` as `name`: into the phone's Download folder and on to its
/// share sheet, or beside the drawing on a desktop. Answers whether the
/// share sheet was opened.
fn save_curves_file(
    dir: Option<std::path::PathBuf>,
    name: &str,
    bytes: &[u8],
) -> Result<bool, String> {
    #[cfg(target_os = "android")]
    {
        let _ = dir;
        let phone = crate::android_system::phone().ok_or("no activity")?;
        crate::android_system::save_and_share(phone, name, "application/octet-stream", bytes)
    }
    #[cfg(not(target_os = "android"))]
    {
        let dir = dir.unwrap_or_else(crate::files::android_documents);
        crate::files::write_creating(dir.join(name), bytes).map_err(|e| e.to_string())?;
        Ok(false)
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
