//! Full-screen preview (F9) and the Page Sorter view.

use crate::app::App;
use crate::i18n::tr;
use crate::theme::Tokens;
use egui::Ui;
use tracedraw_core::Command;

pub fn fullscreen(app: &mut App, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(egui::Color32::from_gray(40)))
        .show(ui, |ui| {
            let rect = ui.available_rect_before_wrap();
            let page = app.page_rect();
            let bounds = if app.preview_selected_only {
                app.selection_bounds().unwrap_or(page)
            } else {
                page
            };
            let painter = ui.painter_at(rect);
            // Fit the page into the screen.
            let margin = 20.0;
            let sx = (rect.width() - 2.0 * margin) as f64 / bounds.width().max(1e-6);
            let sy = (rect.height() - 2.0 * margin) as f64 / bounds.height().max(1e-6);
            let zoom = sx.min(sy);
            let w = (bounds.width() * zoom).ceil() as u32;
            let h = (bounds.height() * zoom).ceil() as u32;
            let doc = app.engine.document().clone();
            let view = tracedraw_render::ViewTransform {
                zoom,
                origin_x: -bounds.x0 * zoom,
                origin_y: bounds.y1 * zoom,
            };
            let opts = tracedraw_render::RenderOptions {
                width: w.max(1),
                height: h.max(1),
                view,
                preview: None,
                wireframe: false,
                ..tracedraw_render::RenderOptions::default()
            };
            let page_id = app.page;
            if let Some(mut pm) = tracedraw_render::render_page(&doc, page_id, &opts) {
                if !app.preview_selected_only {
                    let mut white = tiny_skia::Pixmap::new(pm.width(), pm.height())
                        .unwrap_or_else(|| tiny_skia::Pixmap::new(1, 1).expect("pixmap"));
                    white.fill(tiny_skia::Color::WHITE);
                    white.draw_pixmap(
                        0,
                        0,
                        pm.as_ref(),
                        &Default::default(),
                        tiny_skia::Transform::identity(),
                        None,
                    );
                    pm = white;
                }
                let image = egui::ColorImage::from_rgba_premultiplied(
                    [pm.width() as usize, pm.height() as usize],
                    pm.data(),
                );
                let tex =
                    ctx.load_texture("fullscreen_preview", image, egui::TextureOptions::LINEAR);
                let size = egui::vec2(pm.width() as f32, pm.height() as f32);
                let pos = rect.center() - size / 2.0;
                let page_rect = egui::Rect::from_min_size(pos, size);
                painter.image(
                    tex.id(),
                    page_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
                // Options > Display > Full-screen preview > Show page border.
                if !app.preview_selected_only && app.settings.preview_page_border {
                    painter.rect_stroke(
                        page_rect,
                        0.0,
                        egui::Stroke::new(1.0, Tokens::page_border()),
                        egui::StrokeKind::Outside,
                    );
                }
            }
            painter.text(
                egui::pos2(rect.center().x, rect.max.y - 10.0),
                egui::Align2::CENTER_BOTTOM,
                tr("preview.exit_hint"),
                egui::FontId::proportional(12.0),
                egui::Color32::from_gray(200),
            );
        });
    let exit = ctx.input(|i| {
        i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::F9) || i.pointer.any_click()
    });
    if exit {
        app.fullscreen_preview = false;
        app.preview_selected_only = false;
    }
}

pub fn page_sorter(app: &mut App, ui: &mut Ui) {
    let phone = crate::ui::mobile::active(ui.ctx());
    if phone {
        phone_sorter(app, ui);
        return;
    }
    ui.horizontal(|ui| {
        ui.heading(tr("menu.view.page_sorter"));
        if ui.button(tr("preview.back_to_page")).clicked() {
            app.page_sorter = false;
        }
        if ui.button(tr("menu.layout.insert_page")).clicked() {
            app.add_page();
        }
    });
    ui.separator();
    let pages: Vec<(tracedraw_core::PageId, String, tracedraw_core::Size)> = app
        .doc()
        .pages
        .iter()
        .map(|p| (p.id, p.name.clone(), p.size))
        .collect();
    let n = pages.len();
    let doc = app.engine.document().clone();
    let mut goto: Option<usize> = None;
    let mut mv: Option<(tracedraw_core::PageId, usize)> = None;
    let mut del: Option<tracedraw_core::PageId> = None;
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            for (i, (id, name, size)) in pages.iter().enumerate() {
                let thumb_h = 150.0f32;
                let thumb_w = (thumb_h as f64 * size.width / size.height.max(1e-6)) as f32;
                ui.vertical(|ui| {
                    let (rect, resp) = ui.allocate_exact_size(
                        egui::vec2(thumb_w.max(40.0), thumb_h),
                        egui::Sense::click(),
                    );
                    let active = *id == app.page;
                    ui.painter().rect_filled(
                        rect.expand(3.0),
                        3.0,
                        if active {
                            Tokens::tool_active()
                        } else {
                            Tokens::panel_dark()
                        },
                    );
                    let dpi = thumb_h as f64 / (size.height / 25.4);
                    if let Some(pm) = tracedraw_render::render_page_image(&doc, *id, dpi.max(4.0)) {
                        let image = egui::ColorImage::from_rgba_premultiplied(
                            [pm.width() as usize, pm.height() as usize],
                            pm.data(),
                        );
                        let tex = ui.ctx().load_texture(
                            format!("sorter_{}", id.raw()),
                            image,
                            egui::TextureOptions::LINEAR,
                        );
                        ui.painter().image(
                            tex.id(),
                            rect,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                    }
                    if resp.clicked() {
                        goto = Some(i);
                    }
                    if resp.double_clicked() {
                        goto = Some(i);
                        app.page_sorter = false;
                    }
                    ui.horizontal(|ui| {
                        ui.label(format!("{}. {name}", i + 1));
                        if i > 0 && ui.small_button("\u{25C0}").clicked() {
                            mv = Some((*id, i - 1));
                        }
                        if i + 1 < n && ui.small_button("\u{25B6}").clicked() {
                            mv = Some((*id, i + 1));
                        }
                        if n > 1 && ui.small_button("x").clicked() {
                            del = Some(*id);
                        }
                    });
                });
                ui.add_space(12.0);
            }
        });
    });
    if let Some(i) = goto {
        app.goto_page(i);
    }
    if let Some((id, to)) = mv {
        app.run(Command::MovePage { page: id, to });
    }
    if let Some(id) = del {
        app.run(Command::DeletePage { page: id });
        if app.page == id {
            app.page = app.doc().pages[0].id;
        }
    }
}

/// The Page Sorter as a phone shows it: the pages as cards two to a row,
/// each with its name and buttons a finger can hit — not a 12 px arrow.
fn phone_sorter(app: &mut App, ui: &mut Ui) {
    let m3 = crate::ui::mobile::M3::of();
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(tr("menu.view.page_sorter"))
                .size(22.0)
                .strong()
                .color(m3.on),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(4.0);
            if crate::ui::mobile::text_button(ui, &m3, &tr("preview.back_to_page")).clicked() {
                app.page_sorter = false;
            }
        });
    });
    ui.add_space(10.0);
    if crate::ui::mobile::filled_button(ui, &m3, &tr("menu.layout.insert_page")).clicked() {
        app.add_page();
    }
    ui.add_space(12.0);
    let pages: Vec<(tracedraw_core::PageId, String, tracedraw_core::Size)> = app
        .doc()
        .pages
        .iter()
        .map(|p| (p.id, p.name.clone(), p.size))
        .collect();
    let n = pages.len();
    let doc = app.engine.document().clone();
    let mut goto: Option<usize> = None;
    let mut mv: Option<(tracedraw_core::PageId, usize)> = None;
    let mut del: Option<tracedraw_core::PageId> = None;
    let width = ui.available_width();
    let cols = if width > 700.0 { 3 } else { 2 };
    let gap = 12.0;
    let tile_w = ((width - gap * (cols as f32 - 1.0)) / cols as f32).floor();
    egui::ScrollArea::vertical()
        .id_salt("phone_sorter")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let mut i = 0;
            while i < pages.len() {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for (k, (id, name, size)) in pages.iter().enumerate().skip(i).take(cols) {

                        let active = *id == app.page;
                        let thumb_h = (tile_w * 1.2).min(300.0);
                        let (rect, resp) = ui.allocate_exact_size(
                            egui::vec2(tile_w, thumb_h + 48.0),
                            egui::Sense::click(),
                        );
                        let painter = ui.painter();
                        painter.rect_filled(rect, 14.0, m3.container);
                        if active {
                            painter.rect_stroke(
                                rect,
                                14.0,
                                egui::Stroke::new(2.0, m3.primary),
                                egui::StrokeKind::Inside,
                            );
                        }
                        // The page itself, in the card's top.
                        let inner = egui::Rect::from_min_max(
                            rect.min + egui::vec2(8.0, 8.0),
                            egui::pos2(rect.max.x - 8.0, rect.min.y + 8.0 + thumb_h),
                        );
                        painter.rect_filled(inner, 6.0, egui::Color32::WHITE);
                        let dpi = thumb_h as f64 / (size.height / 25.4);
                        if let Some(pm) =
                            tracedraw_render::render_page_image(&doc, *id, dpi.max(4.0))
                        {
                            let image = egui::ColorImage::from_rgba_premultiplied(
                                [pm.width() as usize, pm.height() as usize],
                                pm.data(),
                            );
                            let tex = ui.ctx().load_texture(
                                format!("sorter_{}", id.raw()),
                                image,
                                egui::TextureOptions::LINEAR,
                            );
                            painter.image(
                                tex.id(),
                                inner,
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                egui::Color32::WHITE,
                            );
                        }
                        if resp.clicked() {
                            goto = Some(k);
                        }
                        if resp.double_clicked() {
                            goto = Some(k);
                            app.page_sorter = false;
                        }
                        // The name, then the three buttons.
                        let footer = egui::Rect::from_min_max(
                            egui::pos2(rect.left() + 10.0, inner.bottom()),
                            rect.max,
                        );
                        ui.painter().text(
                            footer.left_center(),
                            egui::Align2::LEFT_CENTER,
                            format!("{}. {name}", k + 1),
                            egui::FontId::proportional(14.0),
                            if active { m3.primary } else { m3.on },
                        );
                        let mut bx = rect.right() - 6.0;
                        for (glyph, enabled) in [
                            ("\u{2715}", n > 1),
                            ("\u{25B6}", k + 1 < n),
                            ("\u{25C0}", k > 0),
                        ] {
                            let target = egui::Rect::from_center_size(
                                egui::pos2(bx - 18.0, footer.center().y),
                                egui::vec2(36.0, 36.0),
                            );
                            bx -= 38.0;
                            if !enabled {
                                continue;
                            }
                            let hit = ui.interact(
                                target,
                                egui::Id::new(("sorter_btn", id.raw(), glyph)),
                                egui::Sense::click(),
                            );
                            if hit.hovered() {
                                ui.painter().circle_filled(
                                    target.center(),
                                    18.0,
                                    m3.primary_container,
                                );
                            }
                            ui.painter().text(
                                target.center(),
                                egui::Align2::CENTER_CENTER,
                                glyph,
                                egui::FontId::proportional(13.0),
                                m3.primary,
                            );
                            if hit.clicked() {
                                match glyph {
                                    "\u{25C0}" => mv = Some((*id, k - 1)),
                                    "\u{25B6}" => mv = Some((*id, k + 1)),
                                    _ => del = Some(*id),
                                }
                            }
                        }
                    }
                });
                i += cols;
                ui.add_space(gap);
            }
        });
    if let Some(i) = goto {
        app.goto_page(i);
    }
    if let Some((id, to)) = mv {
        app.run(Command::MovePage { page: id, to });
    }
    if let Some(id) = del {
        app.run(Command::DeletePage { page: id });
        if app.page == id {
            app.page = app.doc().pages[0].id;
        }
    }
}
