//! Dialog windows with a one-pixel grey
//! border, a white 31 px title bar with the title on the left and a close
//! button on the right, a white 5 px frame round a light grey body, centred
//! over the main window. Also the form controls its dialogs use: labels
//! right-aligned before 28 px fields, white text fields and grey lists
//! with a grey border (blue while typing), spin fields, push buttons.

use crate::theme::Tokens;
use crate::ui::mobile::M3;
use egui::{
    Align2, Color32, Context, CornerRadius, FontId, Frame, Margin, Pos2, Rect, Sense, Stroke, Ui,
    Vec2,
};

/// Height of the title bar (under the top border).
pub const TITLE_BAR: f32 = 31.0;
/// Height of the headline a dialog wears on a phone, in place of the
/// title bar.
const HEAD_H: f32 = 64.0;
/// The white frame round the body.
pub const EDGE: f32 = 5.0;
/// Height of fields, lists and buttons.
pub const FIELD_H: f32 = 28.0;
/// Rows of a form, one field tall plus the gap.
pub const ROW_PITCH: f32 = 33.0;
/// The close button's width.
const CLOSE_W: f32 = 46.0;
pub const FIELD_FOCUS: Color32 = Color32::from_rgb(0x00, 0xAD, 0xFE);
/// The close button turns red under the pointer.
const CLOSE_HOVER: Color32 = Color32::from_rgb(0xE8, 0x11, 0x23);
const TITLE_FONT: f32 = 13.0;

/// The Material colours of a dialog, when the interface is laid out for a
/// phone: every prompt window is then the card Android draws, not the
/// desktop's framed window with its 31 px title bar.
fn material(ctx: &Context) -> Option<M3> {
    crate::ui::mobile::active(ctx).then(M3::of)
}

/// The shadow under a Material dialog: a soft, wide lift off the drawing.
fn dialog_shadow() -> egui::epaint::Shadow {
    crate::theme::shadow(egui::epaint::Shadow {
        offset: [0, 6],
        blur: 28,
        spread: 2,
        color: Color32::from_black_alpha(70),
    })
}

/// A modal-looking window of `size` (borders and title bar included)
/// centred on the screen. `add` draws the body, whose rect starts under
/// the title bar at the window's left edge. Returns true when the close
/// button was clicked or Escape pressed.
pub fn dialog(ctx: &Context, id: &str, title: &str, size: Vec2, add: impl FnOnce(&mut Ui)) -> bool {
    let mut closed = false;
    let m3 = material(ctx);
    // A phone wears a Material headline instead of the 31 px title bar;
    // the body keeps the size the dialog asks for, under it.
    let head = if m3.is_some() { HEAD_H } else { TITLE_BAR };
    let body_h = size.y - TITLE_BAR;
    let want = Vec2::new(size.x, head + body_h);
    // On a screen smaller than the dialog, the window takes what there
    // is and its body scrolls (both ways) over the dialog's full layout.
    let edge = if m3.is_some() { 24.0 } else { 16.0 };
    let room = (ctx.content_rect().size() - Vec2::splat(edge)).max(Vec2::splat(200.0));
    let shown = want.min(room);
    if shown != want {
        let w = egui::Window::new(id)
            .id(egui::Id::new(id))
            .order(egui::Order::Foreground)
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_size(shown);
        placed(w, ctx, egui::Id::new(id))
            .frame(dialog_frame(&m3))
            .show(ctx, |ui| {
                ui.set_min_size(shown);
                let (bar, _) = ui.allocate_exact_size(Vec2::new(shown.x, head), Sense::hover());
                if head_bar(ui, bar, title, id, &m3) {
                    closed = true;
                }
                egui::ScrollArea::both()
                    .id_salt((id, "scroll"))
                    .max_width(shown.x)
                    .max_height(shown.y - head)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let (r, _) =
                            ui.allocate_exact_size(Vec2::new(size.x, body_h), Sense::hover());
                        let body = Rect::from_min_max(Pos2::new(r.left() - 1.0, r.top()), r.max);
                        body_backdrop(ui, body, &m3);
                        ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
                            form_style(ui);
                            add(ui)
                        });
                    });
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            closed = true;
        }
        return closed;
    }
    let w = egui::Window::new(id)
        .id(egui::Id::new(id))
        .order(egui::Order::Foreground)
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .fixed_size(shown);
    placed(w, ctx, egui::Id::new(id))
        .frame(dialog_frame(&m3))
        .show(ctx, |ui| {
            ui.set_min_size(shown);
            let (bar, _) = ui.allocate_exact_size(Vec2::new(shown.x, head), Sense::hover());
            if head_bar(ui, bar, title, id, &m3) {
                closed = true;
            }
            // Body coordinates: x from the window's outer edge, y from
            // under the title bar.
            let body = Rect::from_min_max(
                Pos2::new(bar.left() - 1.0, bar.bottom()),
                Pos2::new(bar.right(), bar.top() + shown.y),
            );
            body_backdrop(ui, body, &m3);
            ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
                form_style(ui);
                add(ui)
            });
        });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        closed = true;
    }
    closed
}

/// The frame of a fixed-size dialog: the desktop's white window, or the
/// rounded card of a Material dialog.
fn dialog_frame(m3: &Option<M3>) -> Frame {
    match m3 {
        Some(m3) => Frame::new()
            .fill(m3.surface)
            .corner_radius(CornerRadius::same(28))
            .shadow(dialog_shadow())
            .inner_margin(Margin::ZERO),
        None => Frame::new()
            .fill(crate::theme::Tokens::title_bar())
            .stroke(Stroke::new(1.0, crate::theme::Tokens::window_border()))
            .shadow(crate::theme::shadow(egui::epaint::Shadow {
                offset: [0, 4],
                blur: 16,
                spread: 0,
                color: Color32::from_black_alpha(60),
            }))
            .inner_margin(Margin::ZERO),
    }
}

/// The body's background: the panel colour inset by the white edge on the
/// desktop, the card itself on a phone.
fn body_backdrop(ui: &Ui, body: Rect, m3: &Option<M3>) {
    if m3.is_some() {
        return;
    }
    ui.painter().rect_filled(
        Rect::from_min_max(
            body.min + Vec2::new(EDGE, 0.0),
            body.max - Vec2::new(EDGE, EDGE),
        ),
        0.0,
        Tokens::panel(),
    );
}

/// Place a dialog window `id`: it can always be dragged by its title bar.
/// When it opens (it was not shown on the frame before) it starts at the
/// centre of the main window, or, with Center dialog boxes off, where it
/// was left; while open it stays where the user put it.
fn placed<'a>(window: egui::Window<'a>, ctx: &Context, id: egui::Id) -> egui::Window<'a> {
    let frame = ctx.cumulative_frame_nr();
    let key = id.with("chrome_last_frame");
    let last: Option<u64> = ctx.data(|d| d.get_temp(key));
    ctx.data_mut(|d| d.insert_temp(key, frame));
    let opened = last.is_none_or(|l| l + 1 < frame);
    let window = window
        .movable(true)
        .pivot(Align2::CENTER_CENTER)
        .default_pos(ctx.content_rect().center())
        .constrain(true);
    if opened && center_dialogs(ctx) {
        window.current_pos(ctx.content_rect().center())
    } else {
        window
    }
}

/// An auto-sized dialog window in the same chrome as [`dialog`]: the white
/// title bar with the close button, a white edge and the light grey body,
/// centred over the main window. The close button asks the dialog system
/// to close the dialog (see [`take_close_request`]).
pub struct Window {
    title: String,
    id: egui::Id,
}

/// The chrome window for a dialog titled `title`.
pub fn window(title: impl Into<String>) -> Window {
    let title = title.into();
    let id = egui::Id::new(("chrome_dialog", title.clone()));
    Window { title, id }
}

/// The widest a dialog's title bar must be for its title and close button.
fn title_width(ui: &Ui, title: &str) -> f32 {
    let galley = ui.painter().layout_no_wrap(
        title.to_string(),
        FontId::proportional(TITLE_FONT),
        crate::theme::Tokens::text(),
    );
    galley.size().x + 12.0 + 24.0 + CLOSE_W
}

impl Window {
    /// Draw the window with `add` as its body. Returns what `add` returned.
    pub fn show<R>(self, ctx: &Context, add: impl FnOnce(&mut Ui) -> R) -> Option<R> {
        self.show_with_footer(ctx, add, |_| {})
    }

    /// A window whose actions stay put: `footer` is drawn under the body,
    /// outside the part that scrolls, so OK and Cancel are always in
    /// reach — the way a phone's dialog keeps its buttons at the bottom
    /// of the card.
    pub fn show_with_footer<R, F>(
        self,
        ctx: &Context,
        add: impl FnOnce(&mut Ui) -> R,
        footer: F,
    ) -> Option<R>
    where
        F: FnOnce(&mut Ui),
    {
        match material(ctx) {
            Some(m3) => self.show_material(ctx, &m3, add, footer),
            None => self.show_desktop(ctx, add, footer),
        }
    }

    /// The dialog of the phone: a rounded card in the middle of the
    /// screen, its name as a Material headline, a round close button, and
    /// the body under them — the shape every Android app's prompt has.
    fn show_material<R, F>(
        self,
        ctx: &Context,
        m3: &M3,
        add: impl FnOnce(&mut Ui) -> R,
        footer: F,
    ) -> Option<R>
    where
        F: FnOnce(&mut Ui),
    {
        let screen = ctx.content_rect();
        let width = (screen.width() - 32.0).clamp(240.0, 420.0);
        let mut out = None;
        let mut closed = false;
        egui::Window::new(&self.title)
            .id(self.id)
            .order(egui::Order::Foreground)
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .movable(false)
            .pivot(Align2::CENTER_CENTER)
            .fixed_pos(screen.center())
            .frame(
                Frame::new()
                    .fill(m3.surface)
                    .corner_radius(CornerRadius::same(28))
                    .shadow(dialog_shadow()),
            )
            .show(ctx, |ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing.y = 8.0;
                // The headline, and the way out of the dialog.
                let (head, _) = ui.allocate_exact_size(Vec2::new(width, HEAD_H), Sense::hover());
                ui.painter().text(
                    Pos2::new(head.left() + 24.0, head.center().y),
                    Align2::LEFT_CENTER,
                    elide(&self.title, width - 24.0 - 56.0, 22.0),
                    crate::theme::bold(22.0),
                    m3.on,
                );
                if close_target(ui, head.right_center() - Vec2::new(36.0, 0.0), m3, &format!("{:?}", self.id)) {
                    closed = true;
                }
                // The body scrolls when the phone is short: a dialog that
                // cannot be scrolled is a dialog with no way out.
                let max_h = (screen.height() * 0.58).max(140.0);
                let body = egui::ScrollArea::vertical()
                    .id_salt("dialog_body")
                    .max_height(max_h)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        Frame::new()
                            .inner_margin(Margin {
                                left: 24,
                                right: 24,
                                top: 0,
                                bottom: 8,
                            })
                            .show(ui, |ui| {
                                ui.set_width(width - 48.0);
                                form_style(ui);
                                add(ui)
                            })
                            .inner
                    });
                out = Some(body.inner);
                // The actions stay at the bottom of the card while the
                // body above them scrolls.
                ui.add_space(8.0);
                Frame::new()
                    .inner_margin(Margin {
                        left: 24,
                        right: 24,
                        top: 0,
                        bottom: 16,
                    })
                    .show(ui, |ui| {
                        ui.set_width(width - 48.0);
                        form_style(ui);
                        footer(ui);
                    });
            });
        if closed || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            request_close(ctx);
        }
        out
    }

    fn show_desktop<R, F>(
        self,
        ctx: &Context,
        add: impl FnOnce(&mut Ui) -> R,
        footer: F,
    ) -> Option<R>
    where
        F: FnOnce(&mut Ui),
    {
        let mut out = None;
        let mut closed = false;
        let window = egui::Window::new(&self.title)
            .id(self.id)
            .order(egui::Order::Foreground)
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .frame(window_frame());
        // Dragged by the title bar; opens centred (or where it was left).
        let window = placed(window, ctx, self.id);
        window.show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            let min_w = title_width(ui, &self.title).max(280.0);
            ui.set_min_width(min_w);
            let top = ui.cursor().min;
            ui.add_space(TITLE_BAR);
            let inner = Frame::new()
                .fill(Tokens::panel())
                .inner_margin(Margin {
                    left: 14,
                    right: 14,
                    top: 12,
                    bottom: 12,
                })
                .outer_margin(Margin {
                    left: EDGE as i8,
                    right: EDGE as i8,
                    top: 0,
                    bottom: EDGE as i8,
                })
                .show(ui, |ui| {
                    form_style(ui);
                    // A dialog taller than the window scrolls.
                    let max_h = (ui.ctx().content_rect().height() - TITLE_BAR - 60.0).max(150.0);
                    // The window starts from the size it had last frame, so
                    // the body may grow up to `max_h` before it scrolls.
                    let r = egui::ScrollArea::vertical()
                        .id_salt("dialog_body")
                        .max_height(max_h)
                        .min_scrolled_height(max_h)
                        .show(ui, |ui| add(ui))
                        .inner;
                    // The desktop's dialogs put their buttons in the body;
                    // a footer drawn here would follow the scroll.
                    footer(ui);
                    r
                });
            out = Some(inner.inner);
            let width = ui.min_rect().width().max(min_w);
            let bar = Rect::from_min_size(top, Vec2::new(width, TITLE_BAR));
            if title_bar(ui, bar, &self.title, &format!("{:?}", self.id)) {
                closed = true;
            }
        });
        if closed {
            request_close(ctx);
        }
        out
    }
}

/// Dialogs are modal: while one is open, a transparent layer over the
/// whole window, under the dialogs (which sit in the foreground order),
/// takes every click, drag and scroll meant for the menus, toolbars,
/// dockers and the drawing.
pub fn modal_blocker(ctx: &Context) {
    modal_blocker_except(ctx, &[]);
}

/// [`modal_blocker`] leaving `holes` open: the toolbars whose buttons
/// the Customization options let the user move (#563).
pub fn modal_blocker_except(ctx: &Context, holes: &[Rect]) {
    // A colour picker's eyedropper waits for a click on the drawing.
    if crate::ui::color_picker::picking(ctx) {
        return;
    }
    let screen = ctx.content_rect();
    let pieces = if holes.is_empty() {
        vec![screen]
    } else {
        uncovered(screen, holes)
    };
    for (i, r) in pieces.into_iter().enumerate() {
        let id = if i == 0 {
            egui::Id::new("modal_blocker")
        } else {
            egui::Id::new(("modal_blocker", i))
        };
        egui::Area::new(id)
            .order(egui::Order::Middle)
            .fixed_pos(r.min)
            .interactable(true)
            .show(ctx, |ui| {
                ui.set_min_size(r.size());
                ui.interact(r, id.with("cover"), Sense::click_and_drag());
            });
        ctx.move_to_top(egui::LayerId::new(egui::Order::Middle, id));
    }
}

/// `screen` without `holes`, as rectangles: bands between the holes'
/// top and bottom edges, each cut where holes cross it.
pub fn uncovered(screen: Rect, holes: &[Rect]) -> Vec<Rect> {
    let holes: Vec<Rect> = holes
        .iter()
        .map(|h| h.intersect(screen))
        .filter(|h| h.is_positive())
        .collect();
    let mut ys = vec![screen.top(), screen.bottom()];
    for h in &holes {
        ys.push(h.top());
        ys.push(h.bottom());
    }
    ys.sort_by(f32::total_cmp);
    ys.dedup();
    let mut out = Vec::new();
    for w in ys.windows(2) {
        let (y0, y1) = (w[0], w[1]);
        if y1 - y0 < 0.5 {
            continue;
        }
        let mut xs: Vec<(f32, f32)> = holes
            .iter()
            .filter(|h| h.top() <= y0 && h.bottom() >= y1)
            .map(|h| (h.left(), h.right()))
            .collect();
        xs.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut x = screen.left();
        for (a, b) in xs {
            if a > x {
                out.push(Rect::from_min_max(Pos2::new(x, y0), Pos2::new(a, y1)));
            }
            x = x.max(b);
        }
        if x < screen.right() {
            out.push(Rect::from_min_max(
                Pos2::new(x, y0),
                Pos2::new(screen.right(), y1),
            ));
        }
    }
    out
}

/// The frame of every dialog window: white, a grey border and a shadow.
fn window_frame() -> Frame {
    Frame::new()
        .fill(crate::theme::Tokens::title_bar())
        .stroke(Stroke::new(1.0, crate::theme::Tokens::window_border()))
        .shadow(crate::theme::shadow(egui::epaint::Shadow {
            offset: [0, 4],
            blur: 16,
            spread: 0,
            color: Color32::from_black_alpha(60),
        }))
        .inner_margin(Margin::ZERO)
}

fn center_id() -> egui::Id {
    egui::Id::new("chrome_center_dialogs")
}

/// Customization > Appearance > Center Dialog Boxes, for the dialogs
/// drawn this frame.
pub fn set_center_dialogs(ctx: &Context, on: bool) {
    ctx.data_mut(|d| d.insert_temp(center_id(), on));
}

/// Dialogs open in the middle and stay there (the default).
pub fn center_dialogs(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp(center_id())).unwrap_or(true)
}

fn close_request_id() -> egui::Id {
    egui::Id::new("chrome_dialog_close_request")
}

/// Ask the dialog system to close the open dialog (its close button).
pub fn request_close(ctx: &Context) {
    ctx.data_mut(|d| d.insert_temp(close_request_id(), true));
}

/// True once after a dialog's close button was clicked.
pub fn take_close_request(ctx: &Context) -> bool {
    ctx.data_mut(|d| d.remove_temp::<bool>(close_request_id()))
        .unwrap_or(false)
}

/// The icon a message box shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageIcon {
    Question,
    /// Something to know: an "i".
    Info,
    /// Work going on (checking for updates): a turning ring of dashes,
    /// the text in bold.
    Busy,
}

/// Paint a 32 px message icon at `rect`: a blue disc with a white
/// question mark or "i".
fn paint_message_icon(ui: &Ui, rect: Rect, icon: MessageIcon) {
    let p = ui.painter();
    let c = rect.center();
    let r = rect.width().min(rect.height()) / 2.0;
    let mark = match icon {
        MessageIcon::Question => "?",
        MessageIcon::Info => "i",
        MessageIcon::Busy => {
            // Eight dashes; the brightest one goes round once a second.
            let t = ui.input(|i| i.time);
            let lead = (t * 8.0) as usize % 8;
            for k in 0..8usize {
                let a = k as f32 * std::f32::consts::TAU / 8.0;
                let d = Vec2::new(a.cos(), a.sin());
                let age = (lead + 8 - k) % 8;
                let grey = (40 + age * 24).min(220) as u8;
                p.line_segment(
                    [c + d * r * 0.45, c + d * r * 0.8],
                    Stroke::new(2.0, Color32::from_gray(grey)),
                );
            }
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(60));
            return;
        }
    };
    p.circle_filled(c, r, Color32::from_rgb(0x1F, 0x6F, 0xC5));
    p.circle_stroke(
        c,
        r - 0.5,
        Stroke::new(1.0, Color32::from_rgb(0x14, 0x52, 0x96)),
    );
    p.text(
        c + Vec2::new(0.0, 0.5),
        Align2::CENTER_CENTER,
        mark,
        crate::theme::bold(r * 1.25),
        Color32::WHITE,
    );
}

/// A button label with its access key (the first letter) underlined.
fn access_label(text: &str) -> egui::WidgetText {
    let mut job = egui::text::LayoutJob::default();
    let font = FontId::proportional(13.0);
    let mut chars = text.chars();
    if let Some(first) = chars.next() {
        job.append(
            &first.to_string(),
            0.0,
            egui::TextFormat {
                font_id: font.clone(),
                color: Tokens::text(),
                underline: Stroke::new(1.0, Tokens::text()),
                ..Default::default()
            },
        );
        job.append(
            chars.as_str(),
            0.0,
            egui::TextFormat {
                font_id: font,
                color: Tokens::text(),
                ..Default::default()
            },
        );
    }
    job.into()
}

/// A row of 100 x 27 push buttons aligned to the right of what was drawn
/// above them. The first is the default button (a blue border, Enter);
/// the last answers Escape. With `access_keys`, each button's first letter
/// is underlined and pressing it (with no field focused) picks it. Returns
/// the index of the button picked this frame.
pub fn button_row(ui: &mut Ui, labels: &[&str], access_keys: bool) -> Option<usize> {
    let mut picked = None;
    let m3 = material(ui.ctx());
    // A phone's dialog ends in text buttons: no frames, capitalised, a
    // finger high, the confirming one first.
    let (w, h, spacing) = if m3.is_some() {
        (88.0, 40.0, 4.0)
    } else {
        (100.0, 27.0, 8.0)
    };
    let total = labels.len() as f32 * w + (labels.len().saturating_sub(1)) as f32 * spacing;
    let left = (ui.min_rect().width() - total).max(0.0);
    ui.add_space(if m3.is_some() { 6.0 } else { 10.0 });
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = spacing;
        ui.add_space(left);
        for (i, label) in labels.iter().enumerate() {
            let resp = ui
                .scope(|ui| {
                    if let Some(m3) = m3.as_ref() {
                        let text = egui::RichText::new(*label)
                            .size(14.0)
                            .strong()
                            .color(m3.primary);
                        return ui.add_sized([w, h], egui::Button::new(text).frame(false));
                    }
                    let text: egui::WidgetText = if access_keys {
                        access_label(label)
                    } else {
                        (*label).into()
                    };
                    if i == 0 {
                        let w = &mut ui.visuals_mut().widgets;
                        w.inactive.bg_stroke = Stroke::new(1.0, Tokens::ACCENT);
                        w.inactive.weak_bg_fill = crate::theme::Tokens::tool_hover();
                    }
                    ui.add_sized([w, h], egui::Button::new(text))
                })
                .inner;
            if resp.clicked() {
                picked = Some(i);
            }
        }
    });
    let typing = ui.memory(|m| m.focused().is_some());
    ui.input(|inp| {
        if picked.is_some() {
            return;
        }
        if inp.key_pressed(egui::Key::Enter) && !typing {
            picked = Some(0);
        } else if inp.key_pressed(egui::Key::Escape) && labels.len() > 1 {
            picked = Some(labels.len() - 1);
        } else if access_keys && !typing && inp.modifiers.is_none() {
            for ev in &inp.events {
                if let egui::Event::Text(t) = ev {
                    let typed = t.to_lowercase();
                    if let Some(i) = labels.iter().position(|l| {
                        l.chars()
                            .next()
                            .map(|c| c.to_lowercase().to_string() == typed)
                            .unwrap_or(false)
                    }) {
                        picked = Some(i);
                    }
                }
            }
        }
    });
    picked
}

/// A message box: the application name as title, an icon, the message and
/// a row of buttons (see [`button_row`]). Returns the index of the button
/// picked, the last one when the close button was clicked.
pub fn message_box(
    ctx: &Context,
    icon: MessageIcon,
    text: &str,
    buttons: &[&str],
) -> Option<usize> {
    message_box_with(ctx, icon, text, buttons, None)
}

/// A [`message_box`] with a check box under the message ("Do not show
/// this message again").
pub fn message_box_check(
    ctx: &Context,
    icon: MessageIcon,
    text: &str,
    buttons: &[&str],
    check: &str,
    checked: &mut bool,
) -> Option<usize> {
    message_box_with(ctx, icon, text, buttons, Some((check, checked)))
}

fn message_box_with(
    ctx: &Context,
    icon: MessageIcon,
    text: &str,
    buttons: &[&str],
    check: Option<(&str, &mut bool)>,
) -> Option<usize> {
    let mut picked = None;
    let before = take_close_request(ctx);
    // A phone's dialog takes the width it is given: the card is already
    // nearly as wide as the screen.
    let phone = material(ctx).is_some();
    window("TraceDraw").show(ctx, |ui| {
        if !phone {
            ui.set_min_width(360.0);
        }
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::hover());
            paint_message_icon(ui, rect, icon);
            ui.add_space(12.0);
            ui.vertical(|ui| {
                ui.add_space(6.0);
                ui.set_max_width(if phone { 300.0 } else { 380.0 });
                let mut rich = egui::RichText::new(text).size(13.5);
                if icon == MessageIcon::Busy {
                    rich = rich.strong();
                }
                ui.add(egui::Label::new(rich).wrap());
            });
        });
        if let Some((label, checked)) = check {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.add_space(48.0);
                ui.checkbox(checked, label);
            });
        }
        ui.add_space(14.0);
        picked = button_row(ui, buttons, true);
    });
    if before || take_close_request(ctx) {
        picked = Some(buttons.len().saturating_sub(1));
    }
    picked
}

/// The head of a dialog: the desktop's title bar, or the Material
/// headline a phone's dialog opens with. Returns true when it was closed.
fn head_bar(ui: &mut Ui, bar: Rect, title: &str, id: &str, m3: &Option<M3>) -> bool {
    let Some(m3) = m3 else {
        return title_bar(ui, bar, title, id);
    };
    let room = (bar.width() - 24.0 - 56.0).max(40.0);
    ui.painter().text(
        Pos2::new(bar.left() + 24.0, bar.center().y),
        Align2::LEFT_CENTER,
        elide(title, room, 22.0),
        crate::theme::bold(22.0),
        m3.on,
    );
    close_target(ui, bar.right_center() - Vec2::new(36.0, 0.0), m3, id)
}

/// The round close button a Material dialog wears at the top right.
fn close_target(ui: &mut Ui, center: Pos2, m3: &M3, id: &str) -> bool {
    let rect = Rect::from_center_size(center, Vec2::splat(44.0));
    let resp = ui.interact(rect, egui::Id::new((id, "close")), Sense::click());
    if resp.hovered() || resp.is_pointer_button_down_on() {
        ui.painter().circle_filled(center, 22.0, m3.primary_container);
    }
    let d = 9.0;
    let s = Stroke::new(2.0, m3.on);
    ui.painter()
        .line_segment([center + Vec2::new(-d, -d), center + Vec2::new(d, d)], s);
    ui.painter()
        .line_segment([center + Vec2::new(d, -d), center + Vec2::new(-d, d)], s);
    resp.clicked()
}

/// `text` cut to `room` points at `size`, with a mark that it was cut.
fn elide(text: &str, room: f32, size: f32) -> String {
    let chars = text.chars().count();
    if chars <= 2 {
        return text.to_string();
    }
    // About half the size per character, which is close enough for a
    // title: the exact width needs the font, which the painter has.
    let fits = (room / (size * 0.52)).floor().max(2.0) as usize;
    if chars <= fits {
        return text.to_string();
    }
    let mut out: String = text.chars().take(fits.saturating_sub(1)).collect();
    out.push('\u{2026}');
    out
}

/// The white title bar: title at the left, the close button at the right.
/// Returns true when the close button was clicked.
fn title_bar(ui: &mut Ui, bar: Rect, title: &str, id: &str) -> bool {
    let painter = ui.painter();
    painter.rect_filled(bar, 0.0, crate::theme::Tokens::title_bar());
    painter.text(
        Pos2::new(bar.left() + 12.0, bar.center().y),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(TITLE_FONT),
        crate::theme::Tokens::text(),
    );
    let close = Rect::from_min_max(
        Pos2::new(bar.right() - CLOSE_W, bar.top()),
        bar.right_bottom(),
    );
    let resp = ui.interact(close, egui::Id::new((id, "close")), Sense::click());
    let color = if resp.hovered() {
        ui.painter().rect_filled(close, 0.0, CLOSE_HOVER);
        Color32::WHITE
    } else {
        crate::theme::Tokens::text()
    };
    let c = close.center();
    let s = Stroke::new(1.3, color);
    let d = 6.0;
    ui.painter()
        .line_segment([c + Vec2::new(-d, -d), c + Vec2::new(d, d)], s);
    ui.painter()
        .line_segment([c + Vec2::new(d, -d), c + Vec2::new(-d, d)], s);
    resp.clicked()
}

/// Controls inside a dialog body: square corners, grey borders, the blue
/// border while typing, grey lists and buttons. On a phone they are the
/// rounded controls of a Material dialog instead.
pub fn form_style(ui: &mut Ui) {
    let m3 = material(ui.ctx());
    let round = if m3.is_some() {
        CornerRadius::same(10)
    } else {
        CornerRadius::ZERO
    };
    let v = ui.visuals_mut();
    for w in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = round;
        w.expansion = 0.0;
    }
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, crate::theme::Tokens::control_border());
    v.widgets.inactive.weak_bg_fill = crate::theme::Tokens::control_fill();
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, FIELD_FOCUS);
    v.widgets.hovered.weak_bg_fill = crate::theme::Tokens::tool_hover();
    v.widgets.active.bg_stroke = Stroke::new(1.0, FIELD_FOCUS);
    v.widgets.open.bg_stroke = Stroke::new(1.0, FIELD_FOCUS);
    v.widgets.open.weak_bg_fill = crate::theme::Tokens::tool_hover();
    v.selection.stroke = Stroke::new(1.0, FIELD_FOCUS);
    v.extreme_bg_color = crate::theme::Tokens::field();
    v.text_edit_bg_color = Some(crate::theme::Tokens::field());
    // A chosen radio button shows a large black dot, as on the desktop.
    ui.spacing_mut().icon_width_inner = 12.0;
    if let Some(m3) = m3 {
        // Fields, lists and buttons as a phone draws them: a soft fill,
        // a hairline, and the accent where the desktop shows a frame.
        let v = ui.visuals_mut();
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, m3.outline);
        v.widgets.inactive.weak_bg_fill = m3.high;
        v.widgets.inactive.bg_fill = m3.high;
        v.widgets.hovered.bg_fill = m3.primary_container;
        v.widgets.hovered.weak_bg_fill = m3.primary_container;
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, m3.primary);
        v.widgets.active.bg_stroke = Stroke::new(1.0, m3.primary);
        v.widgets.open.bg_stroke = Stroke::new(1.0, m3.primary);
        v.selection.bg_fill = m3.primary_container;
        v.selection.stroke = Stroke::new(1.0, m3.primary);
        v.extreme_bg_color = m3.high;
        v.text_edit_bg_color = Some(m3.high);
    }
}

/// A white panel with a light border, for lists and page frames.
pub fn panel() -> Frame {
    Frame::new()
        .fill(crate::theme::Tokens::field())
        .stroke(Stroke::new(1.0, crate::theme::Tokens::border()))
        .inner_margin(Margin::same(6))
}

/// A section heading: its text, then a light line to the right edge.
pub fn section(ui: &mut Ui, text: &str) {
    ui.add_space(4.0);
    let font = FontId::proportional(13.0);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), font, Tokens::text());
    let h = galley.size().y.max(18.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
    let text_w = galley.size().x;
    ui.painter().galley(
        Pos2::new(rect.left(), rect.center().y - galley.size().y / 2.0),
        galley,
        Tokens::text(),
    );
    let y = rect.center().y.round() + 0.5;
    ui.painter().hline(
        (rect.left() + text_w + 12.0)..=rect.right(),
        y,
        Stroke::new(1.0, crate::theme::Tokens::border()),
    );
    ui.add_space(4.0);
}

/// A push button of the standard dialog size (100 x 27).
pub fn button(ui: &mut Ui, text: &str) -> egui::Response {
    ui.add_sized([100.0, 27.0], egui::Button::new(text))
}

// ----- absolute form layout ---------------------------------------------------

/// A bold heading at a point of the body (General, Dimensions).
pub fn heading(ui: &mut Ui, at: Pos2, text: &str) {
    ui.painter().text(
        at,
        Align2::LEFT_CENTER,
        text,
        crate::theme::bold(13.0),
        crate::theme::Tokens::text(),
    );
}

/// A label right-aligned so it ends at `right`, centred on `y`.
pub fn label_right(ui: &mut Ui, right: f32, y: f32, text: &str) {
    ui.painter().text(
        Pos2::new(right, y),
        Align2::RIGHT_CENTER,
        text,
        FontId::proportional(13.0),
        crate::theme::Tokens::text(),
    );
}

/// A label left-aligned from `at`.
pub fn label_at(ui: &mut Ui, at: Pos2, text: &str) {
    ui.painter().text(
        at,
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(13.0),
        crate::theme::Tokens::text(),
    );
}

/// A one-line text field filling `rect`.
pub fn text_field(ui: &mut Ui, rect: Rect, text: &mut String) -> egui::Response {
    ui.put(
        rect,
        egui::TextEdit::singleline(text)
            .vertical_align(egui::Align::Center)
            .margin(Margin::symmetric(4, 0))
            .min_size(rect.size()),
    )
}

/// A push button filling `rect`.
pub fn button_at(ui: &mut Ui, rect: Rect, text: &str) -> egui::Response {
    ui.put(rect, egui::Button::new(text))
}

/// A list (combo box) filling `rect`.
pub fn combo<R>(
    ui: &mut Ui,
    id: &str,
    rect: Rect,
    selected: impl Into<egui::WidgetText>,
    add: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    let mut out = None;
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.spacing_mut().interact_size.y = rect.height();
        ui.spacing_mut().button_padding = Vec2::new(4.0, 0.0);
        out = egui::ComboBox::from_id_salt(id)
            .width(rect.width())
            .height(400.0)
            .selected_text(selected)
            .show_ui(ui, add)
            .inner;
    });
    out
}

/// A number field with the up and down arrows at its right end (a spin
/// box): typing commits on Enter or when the
/// field loses focus, the arrows step by `step`. Returns the new value.
#[allow(clippy::too_many_arguments)]
pub fn spin_field(
    ui: &mut Ui,
    id: &str,
    rect: Rect,
    value: f64,
    decimals: usize,
    suffix: &str,
    step: f64,
    range: std::ops::RangeInclusive<f64>,
) -> Option<f64> {
    let arrows = 23.0;
    let field = Rect::from_min_max(
        rect.min,
        Pos2::new(rect.right() - arrows + 1.0, rect.bottom()),
    );
    let mut out = number_field(ui, id, field, value, decimals, suffix, range.clone());
    // The arrows: a column at the right inside the same border.
    let col = Rect::from_min_max(Pos2::new(field.right() - 1.0, rect.top()), rect.max);
    let painter = ui.painter();
    painter.rect(
        col,
        0.0,
        crate::theme::Tokens::field(),
        Stroke::new(1.0, crate::theme::Tokens::control_border()),
        egui::StrokeKind::Inside,
    );
    let half = col.height() / 2.0;
    for (i, up) in [(0, true), (1, false)] {
        let r = Rect::from_min_size(
            Pos2::new(col.left(), col.top() + i as f32 * half),
            Vec2::new(col.width(), half),
        );
        let a = ui.interact(r, egui::Id::new((id, "spin", i)), Sense::click());
        if a.hovered() {
            ui.painter()
                .rect_filled(r.shrink(1.0), 0.0, crate::theme::Tokens::tool_hover());
        }
        let c = r.center();
        let pts = if up {
            vec![
                c + Vec2::new(-3.5, 1.5),
                c + Vec2::new(3.5, 1.5),
                c + Vec2::new(0.0, -2.0),
            ]
        } else {
            vec![
                c + Vec2::new(-3.5, -1.5),
                c + Vec2::new(3.5, -1.5),
                c + Vec2::new(0.0, 2.0),
            ]
        };
        ui.painter().add(egui::epaint::PathShape::convex_polygon(
            pts,
            Tokens::text_dim(),
            Stroke::NONE,
        ));
        if a.clicked() {
            let base = out.unwrap_or(value);
            let v = (base + if up { step } else { -step }).clamp(*range.start(), *range.end());
            out = Some(v);
        }
    }
    out
}

/// A number typed in a text field: it commits on Enter or when the field
/// loses focus. Returns the new value.
pub fn number_field(
    ui: &mut Ui,
    id: &str,
    rect: Rect,
    value: f64,
    decimals: usize,
    suffix: &str,
    range: std::ops::RangeInclusive<f64>,
) -> Option<f64> {
    let edit_id = egui::Id::new((id, "number_text"));
    let shown = format_number(value, decimals, suffix);
    let mut text = ui
        .ctx()
        .data(|d| d.get_temp::<String>(edit_id))
        .unwrap_or_else(|| shown.clone());
    let resp = text_field(ui, rect, &mut text);
    let mut out = None;
    if resp.has_focus() {
        ui.ctx().data_mut(|d| d.insert_temp(edit_id, text.clone()));
    } else {
        if resp.lost_focus() {
            if let Some(v) = parse_number(&text) {
                let v = v.clamp(*range.start(), *range.end());
                if (v - value).abs() > 1e-12 {
                    out = Some(v);
                }
            }
        }
        ui.ctx().data_mut(|d| d.remove::<String>(edit_id));
    }
    out
}

/// A number as the fields show it: `decimals` places, the suffix after a
/// space.
pub fn format_number(v: f64, decimals: usize, suffix: &str) -> String {
    let s = format!("{v:.decimals$}");
    if suffix.is_empty() {
        s
    } else {
        format!("{s} {suffix}")
    }
}

/// The number typed in a field's text (a unit after it is ignored; a
/// comma counts as the decimal point; sums such as `10+5` work).
pub fn parse_number(text: &str) -> Option<f64> {
    crate::ui::field::parse(text, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The screen less the holes is covered once, and no piece reaches
    /// into a hole (#563).
    #[test]
    fn the_blocker_leaves_the_holes_open() {
        let screen = Rect::from_min_size(Pos2::ZERO, egui::vec2(1000.0, 800.0));
        let holes = [
            Rect::from_min_max(Pos2::new(0.0, 24.0), Pos2::new(700.0, 60.0)),
            Rect::from_min_max(Pos2::new(300.0, 100.0), Pos2::new(500.0, 140.0)),
        ];
        let pieces = uncovered(screen, &holes);
        let area: f32 = pieces.iter().map(|r| r.area()).sum();
        let holes_area: f32 = holes.iter().map(|r| r.area()).sum();
        assert!((area + holes_area - screen.area()).abs() < 1.0, "{area}");
        for p in &pieces {
            for h in &holes {
                assert!(!p.intersect(*h).is_positive(), "{p:?} {h:?}");
            }
        }
        assert_eq!(uncovered(screen, &[]), vec![screen]);
    }

    /// A dialog opens centred, moves when its title bar is dragged and
    /// stays there while open; opened again it is centred again.
    #[test]
    fn dialogs_open_centred_and_move_by_their_title_bar() {
        let ctx = crate::theme::ui_context();
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0));
        let run = |ctx: &Context, events: Vec<egui::Event>, show: bool| -> Option<Rect> {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                events,
                ..Default::default()
            };
            let mut rect = None;
            let mut out = ctx.run_ui(input, |ui| {
                if show {
                    window("Movable")
                        .show(ui.ctx(), |ui| {
                            ui.label("body");
                            ui.add_space(80.0);
                        })
                        .map(|_| ());
                    rect = ui.ctx().memory(|m| {
                        m.area_rect(egui::Id::new(("chrome_dialog", "Movable".to_string())))
                    });
                }
            });
            out.textures_delta.clear();
            rect
        };
        for _ in 0..3 {
            run(&ctx, Vec::new(), true);
        }
        let r = run(&ctx, Vec::new(), true).expect("shown");
        assert!((r.center() - screen.center()).length() < 2.0, "{r:?}");
        // Drag the title bar 200 px right and 100 px down.
        let from = Pos2::new(r.left() + 40.0, r.top() + TITLE_BAR / 2.0);
        let to = from + Vec2::new(200.0, 100.0);
        let press = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        run(&ctx, vec![egui::Event::PointerMoved(from)], true);
        run(&ctx, vec![press(from, true)], true);
        for i in 1..=5 {
            let p = from + (to - from) * (i as f32 / 5.0);
            run(&ctx, vec![egui::Event::PointerMoved(p)], true);
        }
        run(&ctx, vec![press(to, false)], true);
        let moved = run(&ctx, Vec::new(), true).expect("shown");
        assert!(
            (moved.center() - (r.center() + Vec2::new(200.0, 100.0))).length() < 3.0,
            "{r:?} -> {moved:?}"
        );
        // Closed for a frame, then opened again: centred.
        run(&ctx, Vec::new(), false);
        run(&ctx, Vec::new(), false);
        run(&ctx, Vec::new(), true);
        let again = run(&ctx, Vec::new(), true).expect("shown");
        assert!(
            (again.center() - screen.center()).length() < 2.0,
            "{again:?}"
        );
    }

    #[test]
    fn fields_read_numbers_with_units_and_commas() {
        assert_eq!(parse_number("210.0 mm"), Some(210.0));
        assert_eq!(parse_number(" 1920,5 px"), Some(1920.5));
        assert_eq!(parse_number("-3"), Some(-3.0));
        assert_eq!(parse_number("mm"), None);
        assert_eq!(parse_number(""), None);
        assert_eq!(format_number(210.0, 1, "mm"), "210.0 mm");
        assert_eq!(format_number(3.0, 0, ""), "3");
    }
}
