//! The three image tools the phone's left chip opens: matting (taking the
//! background out of a picture), upscaling (a model that makes a picture
//! four times its size) and variable data (one drawing filled from a
//! table, a copy a row).
//!
//! Their models are not in the app: they are in the project's own bucket,
//! under `models/`, and are fetched once into the app's folder. Nothing
//! here runs a model; this module knows which models there are, where
//! their files go, and what a download is doing.

use egui::Id;
use std::path::PathBuf;

/// A model the tools can use: the files it is made of, and what it is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Model {
    /// Its name in the bucket, without an extension.
    pub key: &'static str,
    /// The files it is made of, in the bucket's `models/` folder.
    pub files: &'static [&'static str],
    /// The i18n key of its name.
    pub title: &'static str,
}

/// The matting models: one for people, one for anything.
pub const MATTING: &[Model] = &[
    Model {
        key: "modnet",
        files: &["modnet.onnx"],
        title: "ai.matting_portrait",
    },
    Model {
        key: "rmbg-1.4",
        files: &["rmbg-1.4.onnx"],
        title: "ai.matting_generic",
    },
];

/// What the user picked in the chip's three panels, kept across frames and
/// panels being closed.
#[derive(Debug, Clone, PartialEq)]
pub struct AiTools {
    /// The index into [`MATTING`].
    pub matting: usize,
    /// The matting settings, the same the desktop dialog uses.
    pub matting_settings: crate::bg_remove::BgSettings,
    /// The variable data: the header and the rows read from a table, the
    /// field each column fills, and how the copies are laid out.
    pub table: Option<Table>,
    /// The column bound to each field of the drawing, by field index.
    pub bindings: Vec<usize>,
    /// Copies across and down a page, the room between them in mm, and
    /// whether the page gets crop marks.
    pub columns: usize,
    pub rows_per_page: usize,
    pub gap_mm: f64,
    pub crop_marks: bool,
    /// The row the preview shows.
    pub preview_row: usize,
    /// The contour's own numbers: how far it grows past the subject, in mm,
    /// and the detail and smoothing a trace would use.
    pub contour_expand_mm: f64,
    pub contour_detail: f64,
    pub contour_smooth: f64,
    /// The contour filled white under the picture, and the picture put
    /// inside the contour as a container: what a cutting outline is made
    /// with when the picture is to be printed on its own white.
    pub contour_white_backing: bool,
    pub contour_power_clip: bool,
    /// The cut-out picture split into its separate subjects, each an
    /// object of its own.
    pub matting_split: bool,
    /// Each piece cut to what it fills, with no transparent room around it.
    pub matting_trim: bool,
    /// The identification photo made of a cut-out person, and the way it
    /// is made.
    pub id_photo_on: bool,
    pub id_photo: crate::matting::IdPhoto,
    /// The smart layout's numbers: the sheet the parts are packed onto, in
    /// millimetres, the least space between two of them, and whether a part
    /// may be turned, or turned to any angle and settled by its own outline.
    pub arrange_width_mm: f64,
    pub arrange_height_mm: f64,
    pub arrange_gap_mm: f64,
    pub arrange_rotate: bool,
    pub arrange_irregular: bool,
    /// The groups in the selection opened before the packing, so each
    /// member is placed on its own rather than as one block.
    pub arrange_split: bool,
}

impl Default for AiTools {
    fn default() -> Self {
        AiTools {
            matting: 0,
            matting_settings: crate::bg_remove::BgSettings::default(),
            table: None,
            bindings: Vec::new(),
            columns: 5,
            rows_per_page: 0,
            gap_mm: 5.0,
            crop_marks: false,
            preview_row: 0,
            contour_expand_mm: 0.0,
            contour_detail: 60.0,
            contour_smooth: 40.0,
            contour_white_backing: false,
            contour_power_clip: false,
            matting_split: true,
            matting_trim: true,
            id_photo_on: false,
            id_photo: crate::matting::IdPhoto::default(),
            arrange_width_mm: 1200.0,
            arrange_height_mm: 2400.0,
            arrange_gap_mm: 10.0,
            arrange_rotate: true,
            arrange_irregular: false,
            arrange_split: false,
        }
    }
}

/// A table read from a CSV or TSV file: its header and its rows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Table {
    /// The file it came from, for the panel to name.
    pub path: String,
    pub header: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl Table {
    /// The table in `text`, the separator taken from the first line (a
    /// comma or a tab). A quoted field may hold the separator, and two
    /// quotes inside one stand for a quote.
    pub fn parse(text: &str, path: &str) -> Option<Table> {
        let first = text.lines().next()?;
        let sep = if first.contains('\t') { '\t' } else { ',' };
        let mut lines = split_rows(text, sep).into_iter();
        let header = lines.next()?;
        if header.is_empty() {
            return None;
        }
        let rows: Vec<Vec<String>> = lines.filter(|r| r.iter().any(|c| !c.trim().is_empty())).collect();
        Some(Table {
            path: path.to_string(),
            header,
            rows,
        })
    }

    /// How many columns the header has.
    pub fn columns(&self) -> usize {
        self.header.len()
    }
}

/// The fields of the table, one line of text at a time, honouring quotes.
fn split_rows(text: &str, sep: char) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    quoted = false;
                }
            } else {
                field.push(c);
            }
            continue;
        }
        match c {
            '"' => quoted = true,
            '\r' => {}
            '\n' => {
                row.push(field.clone());
                field.clear();
                rows.push(std::mem::take(&mut row));
            }
            c if c == sep => {
                row.push(field.clone());
                field.clear();
            }
            c => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

/// Where the models live: `Download/models`, where a file manager can drop
/// one as easily as the app can, so a model copied by hand is found like a
/// downloaded one. A phone without the storage grant has only its own
/// folder, and that is the way out.
pub fn dir() -> Option<PathBuf> {
    #[cfg(target_os = "android")]
    let dir = {
        let phone = crate::android_system::phone()?;
        match crate::android_system::storage_root(phone) {
            Some(root) if root.is_dir() => root.join("Download").join("models"),
            _ => crate::android_system::files_dir(phone)?.join("models"),
        }
    };
    #[cfg(not(target_os = "android"))]
    let dir = crate::files::home_dir()?.join("Download").join("models");
    if !dir.is_dir() {
        let _ = crate::files::create_dir(&dir);
    }
    Some(dir)
}

/// The file of a model in [`dir`].
pub fn path(file: &str) -> Option<PathBuf> {
    Some(dir()?.join(file))
}

/// Whether every file of `model` is there.
pub fn ready(model: &Model) -> bool {
    model
        .files
        .iter()
        .all(|f| path(f).map(|p| p.is_file()).unwrap_or(false))
}

/// The size of the files of `model` on disk, in bytes.
pub fn size_on_disk(model: &Model) -> u64 {
    model
        .files
        .iter()
        .filter_map(|f| path(f).and_then(|p| std::fs::metadata(p).ok()))
        .map(|m| m.len())
        .sum()
}

/// A size in bytes, as a panel says it.
pub fn size_text(bytes: u64) -> String {
    let mb = bytes as f64 / (1024.0 * 1024.0);
    if mb >= 1.0 {
        format!("{mb:.1} MB")
    } else {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    }
}

/// Where the models are downloaded from.
pub const HOST: &str = "https://tracedraw.metoo.dpdns.org/models";

/// The address of one model file.
pub fn url(file: &str) -> String {
    format!("{HOST}/{file}")
}

fn progress_id(model: &Model) -> Id {
    Id::new(("ai_model_download", model.key))
}

/// What a download of `model` is doing: the bytes written and the total,
/// or `None` when nothing is running.
pub fn progress(ctx: &egui::Context, model: &Model) -> Option<(u64, u64)> {
    ctx.data(|d| d.get_temp::<(u64, u64)>(progress_id(model)))
}

/// The error the last download of `model` stopped with, if it did.
pub fn failed(ctx: &egui::Context, model: &Model) -> Option<String> {
    ctx.data(|d| {
        d.get_temp::<String>(Id::new(("ai_model_failed", model.key)))
    })
}

/// Fetch the files of `model` that are not there yet, on a thread of its
/// own; the panel reads [`progress`] while it runs. Nothing is fetched
/// twice at once.
pub fn download(ctx: &egui::Context, model: &Model) {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (ctx, model);
        return;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        if progress(ctx, model).is_some() {
            return;
        }
        let Some(dir) = dir() else {
            return;
        };
        ctx.data_mut(|d| {
            d.remove::<String>(Id::new(("ai_model_failed", model.key)));
            d.insert_temp(progress_id(model), (0u64, 0u64));
        });
        let ctx = ctx.clone();
        let model = *model;
        std::thread::spawn(move || {
            let mut total = 0u64;
            let mut done = 0u64;
            for file in model.files {
                let url = url(file);
                if url.is_empty() {
                    ctx.data_mut(|d| {
                        d.insert_temp(
                            Id::new(("ai_model_failed", model.key)),
                            "no host for the models".to_string(),
                        );
                        d.remove::<(u64, u64)>(progress_id(&model));
                    });
                    ctx.request_repaint();
                    return;
                }
                let target = dir.join(file);
                if target.is_file() {
                    continue;
                }
                match crate::updates::open_stream(&url) {
                    Ok(mut body) => {
                        let mut out = Vec::new();
                        let mut buf = vec![0u8; 64 * 1024];
                        loop {
                            match std::io::Read::read(&mut body, &mut buf) {
                                Ok(0) => break,
                                Ok(n) => {
                                    out.extend_from_slice(&buf[..n]);
                                    done += n as u64;
                                    total = total.max(done);
                                    ctx.data_mut(|d| {
                                        d.insert_temp(progress_id(&model), (done, total));
                                    });
                                    ctx.request_repaint();
                                }
                                Err(e) => {
                                    ctx.data_mut(|d| {
                                        d.insert_temp(
                                            Id::new(("ai_model_failed", model.key)),
                                            format!("{file}: {e}"),
                                        );
                                        d.remove::<(u64, u64)>(progress_id(&model));
                                    });
                                    ctx.request_repaint();
                                    return;
                                }
                            }
                        }
                        if let Err(e) = std::fs::write(&target, &out) {
                            ctx.data_mut(|d| {
                                d.insert_temp(
                                    Id::new(("ai_model_failed", model.key)),
                                    format!("{file}: {e}"),
                                );
                                d.remove::<(u64, u64)>(progress_id(&model));
                            });
                            ctx.request_repaint();
                            return;
                        }
                    }
                    Err(e) => {
                        ctx.data_mut(|d| {
                            d.insert_temp(
                                Id::new(("ai_model_failed", model.key)),
                                format!("{file}: {e}"),
                            );
                            d.remove::<(u64, u64)>(progress_id(&model));
                        });
                        ctx.request_repaint();
                        return;
                    }
                }
            }
            ctx.data_mut(|d| d.remove::<(u64, u64)>(progress_id(&model)));
            ctx.request_repaint();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_csv_keeps_quoted_separators() {
        let t = Table::parse("name,city\n\"Li, Wei\",Beijing\nWang,Shanghai\n", "t.csv").unwrap();
        assert_eq!(t.header, vec!["name", "city"]);
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[0][0], "Li, Wei");
        assert_eq!(t.rows[1][1], "Shanghai");
    }

    #[test]
    fn a_tsv_and_a_quoted_quote() {
        let t = Table::parse("a\tb\n\"x\"\"y\"\t2\n", "t.tsv").unwrap();
        assert_eq!(t.header, vec!["a", "b"]);
        assert_eq!(t.rows[0][0], "x\"y");
    }

    #[test]
    fn empty_rows_are_dropped() {
        let t = Table::parse("a,b\n1,2\n,\n3,4\n", "t.csv").unwrap();
        assert_eq!(t.rows.len(), 2);
    }
}
