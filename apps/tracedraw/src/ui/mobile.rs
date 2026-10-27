//! The interface on a phone screen, in the shape Android expects.
//!
//! The desktop workspace (menu bar, toolbars in rows, toolbox column,
//! docker columns, palette rows, rulers and status bar) needs a wide
//! window: on a phone the drawing would be a strip between two walls of
//! chrome, and every button would be too small for a finger. This module
//! arranges the same panels the way a Material app does:
//!
//! - a top app bar, under the status bar, with the menu, the drawing's
//!   name and undo/redo;
//! - the drawing between the bars, without rulers (pinch zooms, two
//!   fingers pan: `ui::drawing_window` already does it);
//! - a strip of tools along the bottom, over a navigation bar of four
//!   destinations (properties, panels, palette, files) that open as bottom
//!   sheets, with 48 point targets and rounded state layers;
//! - a snackbar for what the last command said.
//!
//! Nothing here is a second interface: every sheet draws the desktop's own
//! widgets (`toolbar::property_bar`, `dockers::dockers`, `palette`, the
//! menus), so a change to one shows on both.

use crate::app::{App, DockerPlace, DockerTab};
use crate::i18n::tr;
use crate::theme::Tokens;
use crate::ui::icons::{draw_action, Action};
use egui::{Color32, CornerRadius, Frame, Id, Rect, Response, Sense, Stroke, Ui, Vec2};

/// The top app bar: the menu, the drawing's name, undo and redo.
const TOP_H: f32 = 64.0;
/// The strip of tools above the navigation bar.
const TOOLS_H: f32 = 72.0;
/// The navigation bar: four destinations.
const NAV_H: f32 = 76.0;
/// The tools rail down the left edge of a phone on its side.
const RAIL_W: f32 = 76.0;
/// The destinations rail down the right edge there.
const NAV_RAIL_W: f32 = 84.0;
/// The shortest side a finger aims at.
const TOUCH: f32 = 48.0;
/// The side of an icon inside a target.
const ICON: f32 = 24.0;

/// The colours of the interface, in the roles Material gives them: the
/// theme's own panels and text, so the phone follows the desktop theme.
pub(crate) struct M3 {
    /// The background of the bars and of a sheet.
    pub(crate) surface: Color32,
    /// A step up from the surface: the tool strip and the navigation bar.
    pub(crate) container: Color32,
    /// A step up again: fields and chips.
    pub(crate) high: Color32,
    /// Text and icons on the surface.
    pub(crate) on: Color32,
    /// Text and icons that say less.
    pub(crate) variant: Color32,
    /// The accent: the chosen tool, the chosen destination.
    pub(crate) primary: Color32,
    /// A light wash of the accent, behind what is chosen.
    pub(crate) primary_container: Color32,
    /// The frames of fields and chips.
    pub(crate) outline: Color32,
}

impl M3 {
    pub(crate) fn of() -> Self {
        let surface = Tokens::panel();
        let primary = Tokens::ACCENT;
        Self {
            surface,
            container: Tokens::panel_dark(),
            high: Tokens::field(),
            on: Tokens::text(),
            variant: Tokens::text_dim(),
            primary,
            primary_container: mix(primary, surface, 0.16),
            outline: Tokens::border(),
        }
    }
}

/// `a` over `b`, `a` taking `share` of the result.
fn mix(a: Color32, b: Color32, share: f32) -> Color32 {
    let f = |x: u8, y: u8| (x as f32 * share + y as f32 * (1.0 - share)).round() as u8;
    Color32::from_rgb(f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b()))
}

/// The sheets and pages over the drawing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sheet {
    None,
    /// The navigation drawer: the menus, and the way into the settings.
    Drawer,
    /// The items of the menu at that index (`menus::all_menus`).
    MenuPage(usize),
    /// The list of settings pages.
    Settings,
    /// One page of the settings.
    SettingsPage(crate::ui::options::OptionsPage),
    Properties,
    Panels,
    Palette,
    Files,
    Drawings,
    /// The other tools of toolbox group `n`.
    Tools(usize),
}

/// Whether the interface is laid out for a phone: always in the Android
/// build, and in any build started with `TRACEDRAW_MOBILE=1` (the visual
/// checks and a narrow window on a desktop).
pub fn active(_ctx: &egui::Context) -> bool {
    phone()
}

/// [`active`] for the drawing code that has no context at hand: the
/// palette's swatches, a finger wide.
pub fn phone() -> bool {
    #[cfg(target_os = "android")]
    {
        true
    }
    #[cfg(not(target_os = "android"))]
    {
        preview()
    }
}

/// The phone layout asked for on a desktop (`TRACEDRAW_MOBILE=1`); false
/// in the Android build, where it is always on.
#[cfg_attr(target_os = "android", allow(dead_code))]
pub fn preview() -> bool {
    #[cfg(target_os = "android")]
    {
        false
    }
    #[cfg(not(target_os = "android"))]
    {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ON.get_or_init(|| {
            std::env::var("TRACEDRAW_MOBILE").is_ok_and(|v| !v.is_empty() && v != "0")
        })
    }
}

/// The window a preview starts in: a phone's 412 by 915 points, or the
/// size `TRACEDRAW_MOBILE_SIZE` names ("412x915"), for a tablet or a
/// landscape check.
#[cfg_attr(target_os = "android", allow(dead_code))]
pub fn preview_size() -> Option<Vec2> {
    if !preview() {
        return None;
    }
    let text = std::env::var("TRACEDRAW_MOBILE_SIZE").unwrap_or_default();
    let mut parts = text.split(['x', 'X', ',']);
    let side = |v: Option<&str>, fallback: f32| {
        v.and_then(|v| v.trim().parse::<f32>().ok())
            .filter(|v| *v >= 200.0)
            .unwrap_or(fallback)
    };
    Some(Vec2::new(
        side(parts.next(), 412.0),
        side(parts.next(), 915.0),
    ))
}

// ---------------------------------------------------------------------------
// The phone itself
// ---------------------------------------------------------------------------

/// Hand the activity over as the app starts: the system bars are measured
/// through it.
#[cfg(target_os = "android")]
pub fn set_android_app(app: winit::platform::android::activity::AndroidApp) {
    let sdk = winit::platform::android::activity::AndroidApp::sdk_version();
    // Edge to edge first: the window must cover the system bars before
    // their insets mean anything.
    crate::android_system::go_edge_to_edge(app.activity_as_ptr(), app.vm_as_ptr(), sdk);
    crate::android_system::set_phone(app);
}

/// How much of the screen the system bars take, in points: the status bar
/// at the top, the gesture bar at the bottom — or, with the phone on its
/// side, at one of the edges.
#[derive(Debug, Clone, Copy, Default)]
struct Safe {
    top: f32,
    bottom: f32,
    left: f32,
    right: f32,
}

#[cfg(target_os = "android")]
fn system_insets(ctx: &egui::Context) -> Safe {
    let Some(phone) = crate::android_system::phone() else {
        return Safe::default();
    };
    let ppp = ctx.pixels_per_point().max(0.1);
    // The insets change with the window (a rotation, the keyboard), not
    // with the frame: they are read again only when the surface changes.
    let key = Id::new("mobile_insets");
    let size = ctx.content_rect().size() * ppp;
    let px = (size.x.round() as i32, size.y.round() as i32);
    if let Some((seen, got)) =
        ctx.data(|d| d.get_temp::<((i32, i32), (f32, f32, f32, f32))>(key))
    {
        if seen == px && got != (0.0, 0.0, 0.0, 0.0) {
            return Safe {
                top: got.0,
                bottom: got.1,
                left: got.2,
                right: got.3,
            };
        }
    }
    let sdk = winit::platform::android::activity::AndroidApp::sdk_version();
    let got = crate::android_system::read(phone.activity_as_ptr(), phone.vm_as_ptr(), sdk)
        .map(|i| {
            (
                i.top as f32 / ppp,
                i.bottom as f32 / ppp,
                i.left as f32 / ppp,
                i.right as f32 / ppp,
            )
        })
        .unwrap_or((0.0, 0.0, 0.0, 0.0));
    ctx.data_mut(|d| d.insert_temp(key, (px, got)));
    Safe {
        top: got.0,
        bottom: got.1,
        left: got.2,
        right: got.3,
    }
}

#[cfg(not(target_os = "android"))]
fn system_insets(_ctx: &egui::Context) -> Safe {
    Safe::default()
}

/// Dark icons on the system bars when the interface is light, light ones
/// when it is dark; set once per change.
#[cfg(target_os = "android")]
fn system_bar_icons(ctx: &egui::Context) {
    let Some(phone) = crate::android_system::phone() else {
        return;
    };
    let light = !crate::theme::theme().is_dark();
    let key = Id::new("mobile_bar_icons");
    if ctx.data(|d| d.get_temp::<bool>(key)) == Some(light) {
        return;
    }
    let sdk = winit::platform::android::activity::AndroidApp::sdk_version();
    crate::android_system::set_light_bars(phone.activity_as_ptr(), phone.vm_as_ptr(), sdk, light);
    ctx.data_mut(|d| d.insert_temp(key, light));
}

#[cfg(not(target_os = "android"))]
fn system_bar_icons(_ctx: &egui::Context) {}

// ---------------------------------------------------------------------------
// The frame
// ---------------------------------------------------------------------------

/// Hide the system bars while the phone lies on its side, and bring them
/// back when it stands: a wide screen has no row to spare for them.
fn system_bars(ctx: &egui::Context, hidden: bool) {
    #[cfg(target_os = "android")]
    {
        let key = Id::new("mobile_bars_hidden");
        if ctx.data(|d| d.get_temp::<bool>(key)) == Some(hidden) {
            return;
        }
        ctx.data_mut(|d| d.insert_temp(key, hidden));
        if let Some(phone) = crate::android_system::phone() {
            crate::android_system::set_fullscreen(phone, hidden);
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (ctx, hidden);
    }
}

/// Whether the phone lies on its side: a screen wider than it is tall has
/// room for a rail on each edge, and the bars move there.
fn landscape(screen: Rect) -> bool {
    screen.width() > screen.height() * 1.15
}

/// Draw the phone layout; true when it took the whole screen (the Welcome
/// Screen and the page sorter do, as on the desktop).
pub fn frame(app: &mut App, ui: &mut Ui) -> bool {
    let ctx = ui.ctx().clone();
    let window = ui.max_rect();
    let m3 = M3::of();
    // A finger is not a mouse pointer: bigger targets and room between
    // them, whatever size the desktop was set to.
    crate::theme::set_button_sizes(
        crate::settings::ButtonSize::Large,
        Some(crate::settings::ButtonSize::Large),
    );
    touch_style(&ctx);
    system_bar_icons(&ctx);
    let land = landscape(window);
    // A phone lying on its side gives the whole screen to the drawing: the
    // system bars go, and come back when it stands up again.
    system_bars(&ctx, land);
    let inset = if land { Safe::default() } else { system_insets(&ctx) };

    let mut open = sheet(&ctx);
    // A file dialog asked anywhere (File > Open, an import) becomes the
    // file sheet: a phone has no dialog of the system's to show. It is
    // opened once for each request, not held open: the sheet is the
    // dialog, so closing the sheet cancels the request (see the end of
    // this function), and a request already shown must not bring it back.
    #[cfg(target_os = "android")]
    {
        let asked = crate::files::ask_generation();
        let seen: u64 = ctx
            .data(|d| d.get_temp(Id::new("mobile_ask_seen")))
            .unwrap_or(0);
        if asked != seen && crate::files::ask().is_some() {
            ctx.data_mut(|d| d.insert_temp(Id::new("mobile_ask_seen"), asked));
            open = Sheet::Files;
        }
    }

    // The visual checks name the sheet they want to see, so a screenshot
    // needs no pointer at all: TRACEDRAW_MOBILE_SHEET=properties (see
    // scripts/visual/mobile_shot.ps1). A build on a phone never reads it.
    #[cfg(not(target_os = "android"))]
    if !ctx.data(|d| d.get_temp::<bool>(Id::new("mobile_sheet_hook")).unwrap_or(false)) {
        ctx.data_mut(|d| d.insert_temp(Id::new("mobile_sheet_hook"), true));
        if let Some(which) = std::env::var("TRACEDRAW_MOBILE_SHEET")
            .ok()
            .as_deref()
            .and_then(sheet_by_name)
        {
            open = which;
        }
        // The floating tools of a group, for the same checks.
        if let Some(gi) = std::env::var("TRACEDRAW_MOBILE_TOOLS")
            .ok()
            .and_then(|n| n.parse::<usize>().ok())
        {
            let anchor = Rect::from_center_size(
                egui::pos2(window.center().x, window.bottom() - 110.0),
                Vec2::splat(TOUCH),
            );
            show_tools(&ctx, gi, anchor);
        }
    }

    // The back gesture of the phone arrives as Escape: it puts away the
    // layer in front — the sheet, or the file list that stands for a
    // dialog — while a prompt window of its own keeps the key.
    if open != Sheet::None
        && matches!(app.dialog, crate::ui::dialogs::Dialog::None)
        && ctx.input(|i| i.key_pressed(egui::Key::Escape))
    {
        open = Sheet::None;
    }

    top_bar(app, ui, &m3, inset.top, &mut open);
    if land {
        // On its side the phone keeps the tools down the left edge and
        // the destinations down the right one, out of the drawing's way.
        tools_rail(app, ui, &m3, inset);
        nav_rail(app, ui, &m3, inset, &mut open);
    } else {
        nav_bar(app, ui, &m3, inset.bottom, &mut open);
        tools_bar(app, ui, &m3);
    }
    // A group whose button was tapped or held floats its other tools over
    // the drawing, above the bars.
    tool_pop(app, &ctx, &m3);

    let welcome = app.show_welcome;
    let sorter = app.page_sorter && !app.multipage;
    if welcome {
        egui::CentralPanel::default()
            .frame(Frame::new().fill(m3.surface))
            .show(ui, |ui| {
                egui::ScrollArea::both()
                    .id_salt("mobile_welcome")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        Frame::new()
                            .inner_margin(egui::Margin::symmetric(16, 4))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                crate::ui::welcome::welcome_screen(app, ui);
                            });
                    });
            });
    } else if sorter {
        egui::CentralPanel::default()
            .frame(Frame::new().fill(m3.surface))
            .show(ui, |ui| {
                Frame::new()
                    .inner_margin(egui::Margin::symmetric(16, 8))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        crate::ui::preview::page_sorter(app, ui);
                    });
            });
    } else {
        egui::CentralPanel::default()
            .frame(Frame::NONE)
            .show(ui, |ui| {
                let full = ui.available_rect_before_wrap();
                // No rulers, no scrollbars, no navigator: a phone pans and
                // zooms with two fingers, and the chips below say where the
                // view is.
                crate::ui::drawing_window_bare(app, ui, full);
                view_chips(app, ui, &m3, full, &mut open);
            });
    }

    // The bar above the tools, or above the sheet that covers them; on
    // its side the sheet is on the right, so it stays at the bottom.
    let floor = if open != Sheet::None && open != Sheet::Palette && !land {
        window.bottom() - sheet_height(open, window.height())
    } else if land {
        window.bottom() - inset.bottom
    } else {
        window.bottom() - inset.bottom - NAV_H - TOOLS_H
    };
    snackbar(app, ui, floor);
    if open == Sheet::Palette {
        palette_float(app, &ctx, window, &m3, &mut open);
    } else {
        // The card is not the one in front: the next opening starts over.
        ctx.data_mut(|d| d.remove::<u64>(Id::new("mobile_palette_since")));
        if open != Sheet::None {
            sheet_ui(app, ui, open, &mut open, window, &m3, inset);
        }
    }
    // The prompt windows are the shared dialog system's: the phone layout
    // draws them itself only when it takes the whole screen (the Welcome
    // Screen and the page sorter return early, before `ui::mod`'s own
    // call), and never twice.
    if welcome || sorter {
        crate::ui::dialogs::show(app, &ctx);
    }
    let before = sheet(&ctx);
    if open != before {
        // The file list is the file dialog on a phone: closing the list
        // cancels the request, so nothing is left waiting for a file that
        // can never come (and the list stays closed).
        #[cfg(target_os = "android")]
        if before == Sheet::Files
            && open != Sheet::Files
            && matches!(app.dialog, crate::ui::dialogs::Dialog::None)
        {
            crate::files::cancel_ask();
        }
        set_sheet(&ctx, open);
        ctx.request_repaint();
    }
    welcome || sorter
}

fn sheet(ctx: &egui::Context) -> Sheet {
    ctx.data(|d| d.get_temp(Id::new("mobile_sheet")))
        .unwrap_or(Sheet::None)
}

/// The sheet a name stands for, for the visual checks.
fn sheet_by_name(name: &str) -> Option<Sheet> {
    let name = name.trim().to_ascii_lowercase();
    if let Some(n) = name.strip_prefix("menu") {
        return n.parse().ok().map(Sheet::MenuPage);
    }
    if let Some(n) = name.strip_prefix("tools") {
        return n.parse().ok().map(Sheet::Tools);
    }
    Some(match name.as_str() {
        "drawer" => Sheet::Drawer,
        "settings" => Sheet::Settings,
        "properties" => Sheet::Properties,
        "panels" => Sheet::Panels,
        "palette" => Sheet::Palette,
        "files" => Sheet::Files,
        "drawings" => Sheet::Drawings,
        _ => return None,
    })
}

fn set_sheet(ctx: &egui::Context, s: Sheet) {
    ctx.data_mut(|d| d.insert_temp(Id::new("mobile_sheet"), s));
}

/// Room to hit things, text that reads on a phone, and the rounded
/// controls a Material app draws: the same widgets the desktop uses, in
/// the shape Android gives them.
fn touch_style(ctx: &egui::Context) {
    let want = Vec2::new(TOUCH + 4.0, TOUCH);
    let spacing = &ctx.global_style().spacing;
    let corners = ctx.global_style().visuals.widgets.inactive.corner_radius;
    if spacing.interact_size != want
        || spacing.item_spacing != Vec2::new(8.0, 6.0)
        || corners.nw < 10
    {
        ctx.all_styles_mut(|s| {
            s.spacing.interact_size = want;
            s.spacing.item_spacing = Vec2::new(8.0, 6.0);
            s.spacing.button_padding = Vec2::new(12.0, 8.0);
            s.spacing.slider_width = 120.0;
            s.spacing.scroll.bar_width = 6.0;
            // Fields, buttons, lists and pop-ups rounded, as every Android
            // control is; menus and windows rounder still.
            let round = CornerRadius::same(10);
            for w in [
                &mut s.visuals.widgets.noninteractive,
                &mut s.visuals.widgets.inactive,
                &mut s.visuals.widgets.hovered,
                &mut s.visuals.widgets.active,
                &mut s.visuals.widgets.open,
            ] {
                w.corner_radius = round;
            }
            s.visuals.window_corner_radius = CornerRadius::same(28);
            s.visuals.menu_corner_radius = CornerRadius::same(16);
            s.visuals.popup_shadow = egui::epaint::Shadow {
                offset: [0, 6],
                blur: 24,
                spread: 0,
                color: Color32::from_black_alpha(60),
            };
        });
    }
}

// ---------------------------------------------------------------------------
// The bars
// ---------------------------------------------------------------------------

fn top_bar(app: &mut App, ui: &mut Ui, m3: &M3, top: f32, open: &mut Sheet) {
    egui::Panel::top("mobile_top")
        .exact_size(TOP_H + top)
        .resizable(false)
        .show_separator_line(false)
        .frame(Frame::new().fill(m3.surface))
        .show(ui, |ui| {
            ui.add_space(top);
            ui.horizontal_centered(|ui| {
                ui.add_space(4.0);
                if icon_target(ui, m3, Glyph::Menu, true).clicked() {
                    *open = if *open == Sheet::Drawer {
                        Sheet::None
                    } else {
                        Sheet::Drawer
                    };
                }
                let undo = app.engine.undo_label().is_some();
                let redo = app.engine.redo_label().is_some();
                // The drawing's name takes the room the buttons leave; a
                // tap lists the open drawings.
                let name = if app.has_document() && !app.show_welcome {
                    app.document_tab_title(app.active_doc)
                } else {
                    tr("welcome.title")
                };
                let gap = ui.spacing().item_spacing.x;
                let room = (ui.available_width() - 2.0 * (TOUCH + gap) - 4.0).max(48.0);
                let (rect, title) = ui.allocate_exact_size(Vec2::new(room, TOUCH), Sense::click());
                ui.painter().text(
                    egui::pos2(rect.left() + 4.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    elide(&name, 22),
                    egui::FontId::proportional(22.0),
                    m3.on,
                );
                if title.clicked() {
                    *open = Sheet::Drawings;
                }
                if icon_target(ui, m3, Glyph::Undo, undo).clicked() && undo {
                    app.undo();
                }
                if icon_target(ui, m3, Glyph::Redo, redo).clicked() && redo {
                    app.redo();
                }
            });
        });
}

/// The tools as a strip of round targets, over the navigation bar.
fn tools_bar(app: &mut App, ui: &mut Ui, m3: &M3) {
    egui::Panel::bottom("mobile_tools")
        .exact_size(TOOLS_H)
        .resizable(false)
        .show_separator_line(false)
        .frame(Frame::new().fill(m3.container))
        .show(ui, |ui| {
            let mut pick = None;
            let mut pop = None;
            egui::ScrollArea::horizontal()
                .id_salt("mobile_tools_scroll")
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.horizontal_centered(|ui| {
                        ui.add_space(6.0);
                        for (gi, group) in crate::tools::GROUPS.iter().enumerate() {
                            if !crate::ui::toolbox::group_visible(app, gi) {
                                continue;
                            }
                            let shown = shown_tool(app, gi);
                            let active = group.tools.contains(&app.tool);
                            let resp = tool_target(ui, m3, shown, active);
                            if resp.clicked() {
                                pick = Some(shown);
                            }
                            // Held, the group's other tools come out to be
                            // slid onto: a tap alone never opens a window.
                            if holding(ui, &resp) && group.tools.len() > 1 {
                                pop = Some((gi, resp.rect));
                            }
                        }
                        ui.add_space(6.0);
                    });
                });
            if let Some(tool) = pick {
                app.set_tool(tool);
            }
            if let Some((gi, rect)) = pop {
                show_tools(ui.ctx(), gi, rect);
            }
        });
}

/// Remember the group whose tools float over the drawing while its button
/// is held, and where that button is: letting go over one of them picks
/// it, as a phone's own tool rows do.
fn show_tools(ctx: &egui::Context, gi: usize, anchor: Rect) {
    ctx.data_mut(|d| d.insert_temp(Id::new("mobile_tool_pop"), (gi, anchor)));
}

/// The floating row of a group's tools: no window, no scrim — a strip that
/// appears over the drawing beside the button, and the tool under the
/// finger highlighted while it slides.
fn tool_pop(app: &mut App, ctx: &egui::Context, m3: &M3) {
    let key = Id::new("mobile_tool_pop");
    let Some((gi, anchor)) = ctx.data(|d| d.get_temp::<(usize, Rect)>(key)) else {
        return;
    };
    let clear = |ctx: &egui::Context| ctx.data_mut(|d| d.remove::<(usize, Rect)>(key));
    let Some(group) = crate::tools::GROUPS.get(gi).filter(|g| g.tools.len() > 1) else {
        clear(ctx);
        return;
    };
    // Each tool with its name under it, as in the strip: the icons alone
    // do not say which is which.
    let font = egui::FontId::proportional(10.5);
    let cells: Vec<(crate::tools::Tool, String, f32)> = group
        .tools
        .iter()
        .map(|tool| {
            let name = tool.name();
            // Enough room for the name at 10.5 points, measured before the
            // row is laid out (the width places the row on the screen).
            let text: f32 = name
                .chars()
                .map(|c| if c.is_ascii() { 5.9 } else { 10.6 })
                .sum();
            (*tool, name, (text + 16.0).max(TOUCH))
        })
        .collect();
    let pad = 8.0;
    let cell_h = 62.0;
    let width = cells.iter().map(|(_, _, w)| w).sum::<f32>() + pad * 2.0;
    let height = cell_h + pad * 2.0;
    let screen = ctx.content_rect();
    let mut at = egui::pos2(anchor.center().x - width / 2.0, anchor.top() - height - 10.0);
    at.x = at.x.clamp(8.0, (screen.right() - width - 8.0).max(8.0));
    if at.y < 8.0 {
        // No room above: under the button, over the drawing.
        at.y = anchor.bottom() + 10.0;
    }
    let pointer = ctx.pointer_latest_pos();
    let mut under = None;
    egui::Area::new(Id::new("mobile_tool_pop_area"))
        .order(egui::Order::Tooltip)
        .fixed_pos(at)
        // The row takes no input of its own: the press belongs to the
        // button, and the row only says what is under the finger.
        .interactable(false)
        .show(ctx, |ui| {
            Frame::new()
                .fill(m3.high)
                .corner_radius(20.0)
                .shadow(crate::theme::shadow(egui::epaint::Shadow {
                    offset: [0, 6],
                    blur: 24,
                    spread: 0,
                    color: Color32::from_black_alpha(70),
                }))
                .inner_margin(egui::Margin::same(pad as i8))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.horizontal(|ui| {
                        for (tool, name, w) in &cells {
                            let (rect, _) =
                                ui.allocate_exact_size(Vec2::new(*w, cell_h), Sense::hover());
                            let hot = pointer.is_some_and(|p| rect.contains(p));
                            let body = Rect::from_center_size(
                                egui::pos2(rect.center().x, rect.top() + 24.0),
                                Vec2::new(TOUCH - 8.0, 46.0),
                            );
                            if hot {
                                ui.painter().rect_filled(body, 14.0, m3.primary_container);
                            }
                            let icon = Rect::from_center_size(body.center(), Vec2::splat(ICON));
                            crate::ui::icons::draw(
                                ui.painter(),
                                icon,
                                *tool,
                                if hot { m3.primary } else { m3.on },
                            );
                            let color = if hot { m3.primary } else { m3.variant };
                            let galley = ui.painter().layout_no_wrap(
                                name.clone(),
                                font.clone(),
                                color,
                            );
                            ui.painter().galley(
                                egui::pos2(
                                    rect.center().x - galley.size().x / 2.0,
                                    rect.top() + 44.0,
                                ),
                                galley,
                                color,
                            );
                            if hot {
                                under = Some(*tool);
                            }
                        }
                    });
                });
        });
    // Letting go over one of them picks it; letting go anywhere else
    // just puts the row away.
    if ctx.input(|i| i.pointer.any_released()) {
        if let Some(tool) = under {
            app.set_tool(tool);
        }
        clear(ctx);
    }
    // The finger may still be moving over the row.
    ctx.request_repaint();
}

/// Whether a button is being held, with the finger still on it: the
/// long press of a phone, which egui reports as `long_touched` only once
/// the finger has left the screen.
fn holding(ui: &Ui, resp: &Response) -> bool {
    const HOLD: f64 = 0.4;
    resp.is_pointer_button_down_on()
        && ui.input(|i| {
            i.pointer.primary_down()
                && i.pointer
                    .press_start_time()
                    .map(|t| i.time - t)
                    .unwrap_or(0.0)
                    >= HOLD
        })
}

/// The tool a group's button shows: the active one, else the last used
/// from it, else its first.
fn shown_tool(app: &App, gi: usize) -> crate::tools::Tool {    let Some(group) = crate::tools::GROUPS.get(gi) else {
        return crate::tools::Tool::Pick;
    };
    if group.tools.contains(&app.tool) {
        return app.tool;
    }
    app.toolbox_last
        .get(gi)
        .copied()
        .filter(|t| group.tools.contains(t))
        .unwrap_or(group.tools[0])
}

/// The two chips a phone needs on the drawing: how far it is zoomed (a tap
/// fits the page) and, in a drawing of several pages, which page is shown
/// (a tap lists them).
fn view_chips(app: &mut App, ui: &mut Ui, m3: &M3, full: Rect, open: &mut Sheet) {
    let mut right = full.right() - 16.0;
    let bottom = full.bottom() - 16.0;
    let pages = app.doc().pages.len();
    let page = app.page_index() + 1;
    if pages > 1 {
        let rect = Rect::from_min_max(
            egui::pos2(right - 64.0, bottom - 36.0),
            egui::pos2(right, bottom),
        );
        let resp = ui.interact(rect, Id::new("mobile_page_chip"), Sense::click());
        paint_chip(ui, m3, rect, &format!("{page}/{pages}"), resp.hovered());
        if resp.clicked() {
            app.open_docker(DockerTab::Pages);
            *open = Sheet::Panels;
        }
        right -= 72.0;
    }
    let rect = Rect::from_min_max(
        egui::pos2(right - 76.0, bottom - 36.0),
        egui::pos2(right, bottom),
    );
    let resp = ui.interact(rect, Id::new("mobile_zoom_chip"), Sense::click());
    paint_chip(
        ui,
        m3,
        rect,
        &format!("{:.0}%", app.zoom_percent()),
        resp.hovered(),
    );
    if resp.clicked() {
        app.zoom_to_fit();
    }
}

/// A small rounded label over the drawing.
fn paint_chip(ui: &Ui, m3: &M3, rect: Rect, text: &str, hovered: bool) {
    let fill = if hovered {
        mix(m3.primary, m3.surface, 0.12)
    } else {
        m3.container
    };
    let painter = ui.painter();
    painter.rect_filled(rect, 18.0, fill);
    painter.rect_stroke(
        rect,
        18.0,
        Stroke::new(1.0, m3.outline),
        egui::StrokeKind::Inside,
    );
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(13.0),
        m3.on,
    );
}

/// A name cut to what a narrow bar holds, with a mark that it was cut.
fn elide(text: &str, keep: usize) -> String {
    if text.chars().count() <= keep {
        return text.to_string();
    }
    let mut out: String = text.chars().take(keep.saturating_sub(1)).collect();
    out.push('\u{2026}');
    out
}

/// The navigation bar: the panels the desktop keeps beside the drawing,
/// each a destination that opens as a sheet.
fn nav_bar(app: &mut App, ui: &mut Ui, m3: &M3, bottom: f32, open: &mut Sheet) {
    let _ = app;
    egui::Panel::bottom("mobile_nav")
        .exact_size(NAV_H + bottom)
        .resizable(false)
        .show_separator_line(false)
        .frame(Frame::new().fill(m3.container))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                let items = destinations();
                let each = ui.available_width() / items.len() as f32;
                for (which, glyph, label) in items {
                    let active = *open == which;
                    if nav_item(ui, m3, glyph, &label, active, each, NAV_H).clicked() {
                        *open = if active { Sheet::None } else { which };
                    }
                }
            });
            ui.add_space(bottom);
        });
}

/// The panels the desktop keeps beside the drawing, as the four
/// destinations of the bar (or the rail, on its side).
fn destinations() -> [(Sheet, Glyph, String); 4] {
    [
        (Sheet::Properties, Glyph::Sliders, tr("mobile.properties")),
        (Sheet::Panels, Glyph::Panels, tr("mobile.panels")),
        (Sheet::Palette, Glyph::Palette, tr("mobile.palette")),
        (Sheet::Files, Glyph::Files, tr("mobile.files")),
    ]
}

/// The tools down the left edge, for a phone on its side: the same round
/// targets the strip carries, one under the next.
fn tools_rail(app: &mut App, ui: &mut Ui, m3: &M3, safe: Safe) {
    egui::Panel::left("mobile_tools")
        .exact_size(RAIL_W + safe.left)
        .resizable(false)
        .show_separator_line(false)
        .frame(Frame::new().fill(m3.container))
        .show(ui, |ui| {
            // The gesture bar of a phone on its side takes the edge.
            let rail = Rect::from_min_max(
                ui.max_rect().min + Vec2::new(safe.left, 0.0),
                ui.max_rect().max,
            );
            ui.scope_builder(egui::UiBuilder::new().max_rect(rail), |ui| {
            let mut pick = None;
            let mut pop = None;
            egui::ScrollArea::vertical()
                .id_salt("mobile_tools_rail")
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .auto_shrink([true, false])
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(6.0);
                        for (gi, group) in crate::tools::GROUPS.iter().enumerate() {
                            if !crate::ui::toolbox::group_visible(app, gi) {
                                continue;
                            }
                            let shown = shown_tool(app, gi);
                            let active = group.tools.contains(&app.tool);
                            let resp = tool_target(ui, m3, shown, active);
                            if resp.clicked() {
                                pick = Some(shown);
                            }
                            // Held, the group's other tools come out to be
                            // slid onto: a tap alone never opens a window.
                            if holding(ui, &resp) && group.tools.len() > 1 {
                                pop = Some((gi, resp.rect));
                            }
                        }
                        ui.add_space(6.0);
                    });
                });
            if let Some(tool) = pick {
                app.set_tool(tool);
            }
            if let Some((gi, rect)) = pop {
                show_tools(ui.ctx(), gi, rect);
            }
            });
        });
}

/// The destinations down the right edge, for a phone on its side.
fn nav_rail(app: &mut App, ui: &mut Ui, m3: &M3, safe: Safe, open: &mut Sheet) {
    let _ = app;
    egui::Panel::right("mobile_nav")
        .exact_size(NAV_RAIL_W + safe.right)
        .resizable(false)
        .show_separator_line(false)
        .frame(Frame::new().fill(m3.container))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(6.0);
                    for (which, glyph, label) in destinations() {
                        let active = *open == which;
                        if nav_item(ui, m3, glyph, &label, active, NAV_RAIL_W, NAV_H).clicked() {
                            *open = if active { Sheet::None } else { which };
                        }
                    }
                });
                // The gesture bar of a phone on its side.
                ui.add_space(safe.right);
            });
        });
}

/// What the last command said, in a snackbar over the bars: it goes away
/// on its own after a few seconds.
fn snackbar(app: &App, ui: &mut Ui, floor: f32) {
    if app.status.is_empty() {
        return;
    }
    let ctx = ui.ctx().clone();
    let now = ctx.input(|i| i.time);
    let key = Id::new("mobile_snackbar");
    let shown: Option<(String, f64)> = ctx.data(|d| d.get_temp(key));
    let start = match &shown {
        Some((text, at)) if *text == app.status => *at,
        _ => {
            ctx.data_mut(|d| d.insert_temp(key, (app.status.clone(), now)));
            now
        }
    };
    const LIFE: f64 = 4.0;
    let age = now - start;
    if age > LIFE {
        return;
    }
    // The last half second fades out.
    let fade = ((LIFE - age) / 0.5).clamp(0.0, 1.0) as f32;
    let screen = ui.max_rect();
    let width = (screen.width() - 32.0).min(560.0);
    let height = 48.0;
    // Above the sheet that is open, or above the tool strip.
    let y = floor - height - 12.0;
    let rect = Rect::from_min_size(
        egui::pos2(screen.center().x - width / 2.0, y),
        Vec2::new(width, height),
    );
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        Id::new("mobile_snackbar_layer"),
    ));
    let alpha = (235.0 * fade) as u8;
    painter.rect_filled(
        rect,
        12.0,
        Color32::from_rgba_unmultiplied(0x2B, 0x29, 0x30, alpha),
    );
    painter.text(
        rect.left_center() + Vec2::new(16.0, 0.0),
        egui::Align2::LEFT_CENTER,
        tail_of(&app.status, 52),
        egui::FontId::proportional(14.0),
        Color32::from_rgba_unmultiplied(0xF2, 0xEE, 0xF4, alpha),
    );
    ctx.request_repaint_after(std::time::Duration::from_millis(100));
}

// ---------------------------------------------------------------------------
// The sheets and the pages
// ---------------------------------------------------------------------------

fn sheet_ui(
    app: &mut App,
    ui: &mut Ui,
    which: Sheet,
    open: &mut Sheet,
    screen: Rect,
    m3: &M3,
    safe: Safe,
) {
    match which {
        Sheet::Drawer => return drawer(app, ui, m3, screen, open),
        Sheet::MenuPage(index) => return menu_page(app, ui, m3, screen, open, index),
        Sheet::Settings => return settings_list(app, ui, m3, screen, open),
        Sheet::SettingsPage(page) => return settings_page(app, ui, m3, screen, open, page),
        _ => {}
    }
    let ctx = ui.ctx().clone();
    scrim(&ctx, screen, open);

    if landscape(screen) {
        // A phone on its side docks the sheet on the right, between the
        // drawing and the destinations rail: a wide screen has no room to
        // spare at the bottom, and the rail stays in reach.
        let width = (screen.width() * 0.42).clamp(280.0, 440.0);
        let top = TOP_H + safe.top;
        let rect = Rect::from_min_max(
            egui::pos2(screen.right() - safe.right - NAV_RAIL_W - width, top),
            egui::pos2(
                screen.right() - safe.right - NAV_RAIL_W,
                screen.bottom() - safe.bottom,
            ),
        );
        let height = rect.height();
        egui::Area::new(Id::new("mobile_sheet"))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min)
            .show(&ctx, |ui| {
                Frame::new()
                    .fill(m3.surface)
                    .corner_radius(CornerRadius {
                        nw: 28,
                        ne: 0,
                        sw: 28,
                        se: 0,
                    })
                    .inner_margin(egui::Margin {
                        left: 0,
                        right: 0,
                        top: 8,
                        bottom: 0,
                    })
                    .show(ui, |ui| {
                        ui.set_width(width);
                        ui.set_min_height(height - 8.0);
                        sheet_content(app, ui, which, open, m3, height, false);
                    });
            });
    } else {
        let height = sheet_height(which, screen.height());
        egui::Area::new(Id::new("mobile_sheet"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::pos2(screen.left(), screen.bottom() - height))
            .show(&ctx, |ui| {
                Frame::new()
                    .fill(m3.surface)
                    .corner_radius(CornerRadius {
                        nw: 28,
                        ne: 28,
                        sw: 0,
                        se: 0,
                    })
                    .inner_margin(egui::Margin {
                        left: 0,
                        right: 0,
                        top: 8,
                        bottom: 0,
                    })
                    .show(ui, |ui| {
                        ui.set_width(screen.width());
                        ui.set_min_height(height - 8.0);
                        sheet_content(app, ui, which, open, m3, height, true);
                    });
            });
    }

    // A command that opens a dialog of its own (Options, Save As...) takes
    // the screen: the sheet has done its part.
    if !matches!(app.dialog, crate::ui::dialogs::Dialog::None) {
        *open = Sheet::None;
    }
}

/// What a sheet holds, wherever it is drawn: the drag handle of a bottom
/// sheet, the name and the close button, then the body.
fn sheet_content(
    app: &mut App,
    ui: &mut Ui,
    which: Sheet,
    open: &mut Sheet,
    m3: &M3,
    height: f32,
    handle: bool,
) {
    if handle {
        let (bar, _) = ui.allocate_exact_size(Vec2::new(32.0, 4.0), Sense::hover());
        let grab = Rect::from_center_size(
            egui::pos2(ui.max_rect().center().x, bar.center().y),
            Vec2::new(32.0, 4.0),
        );
        ui.painter().rect_filled(grab, 2.0, m3.variant);
        ui.add_space(10.0);
    }
    if sheet_header(ui, which, m3) {
        *open = Sheet::None;
    }
    egui::ScrollArea::vertical()
        .id_salt(("mobile_sheet_scroll", sheet_title(which)))
        .max_height(height - 92.0)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            Frame::new()
                .inner_margin(egui::Margin {
                    left: 16,
                    right: 16,
                    top: 0,
                    bottom: 16,
                })
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    sheet_body(app, ui, which, m3, open);
                });
        });
}

/// The dimmed drawing behind a sheet, a drawer or a page: a tap there puts
/// it away.
fn scrim(ctx: &egui::Context, screen: Rect, open: &mut Sheet) {
    let dim = egui::Area::new(Id::new("mobile_dim"))
        .order(egui::Order::Middle)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            ui.painter()
                .rect_filled(screen, 0.0, Color32::from_black_alpha(115));
            ui.allocate_response(screen.size(), Sense::click())
        });
    if dim.inner.clicked() {
        *open = Sheet::None;
    }
}

/// How much of the screen a sheet takes: a list of menus needs more of it
/// than a row of page fields. The two short ones — the property bar and the
/// palettes — take only what their cards need.
fn sheet_height(which: Sheet, screen: f32) -> f32 {
    let share = match which {
        Sheet::Panels | Sheet::Tools(_) => 0.62,
        Sheet::Files | Sheet::Drawings => 0.52,
        Sheet::Properties => 0.46,
        Sheet::Palette => 0.24,
        Sheet::None | Sheet::Drawer | Sheet::MenuPage(_) | Sheet::Settings | Sheet::SettingsPage(_) => 0.0,
    };
    (screen * share).clamp(180.0, (screen - TOP_H - 40.0).max(180.0))
}

/// The sheet's name and its close button; true when the button was hit.
fn sheet_header(ui: &mut Ui, which: Sheet, m3: &M3) -> bool {
    // A file list standing in for a file dialog says what it is asking
    // for, and offers the Cancel every dialog has.
    let picking = matches!(which, Sheet::Files) && asking();
    let title = if picking {
        plain(&tr("menu.file.open"))
    } else {
        sheet_title(which)
    };
    // The row is laid out by hand: the target of the close button is a
    // rectangle of its own at the right end, so a finger always finds it
    // (a row of nested layouts put it out of reach of the pointer).
    let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 56.0), Sense::hover());
    ui.painter().text(
        egui::pos2(row.left() + 16.0, row.center().y),
        egui::Align2::LEFT_CENTER,
        title,
        crate::theme::bold(18.0),
        m3.on,
    );
    let target = if picking {
        Rect::from_min_max(
            egui::pos2(row.right() - 104.0, row.top() + 4.0),
            egui::pos2(row.right() - 8.0, row.bottom() - 4.0),
        )
    } else {
        Rect::from_center_size(
            egui::pos2(row.right() - 28.0, row.center().y),
            Vec2::splat(48.0),
        )
    };
    let resp = ui.put(
        target,
        egui::Button::new(
            egui::RichText::new(if picking { tr("dialog.cancel") } else { String::new() })
                .size(14.0)
                .color(m3.primary),
        )
        .fill(Color32::TRANSPARENT)
        .corner_radius(22.0)
        .min_size(target.size()),
    );
    if !picking {
        // The cross is drawn, not typed: the bundled font has no
        // multiplication sign, and a missing glyph shows as a box.
        draw_glyph(
            ui,
            Rect::from_center_size(target.center(), Vec2::splat(ICON)),
            Glyph::Close,
            m3.on,
        );
    }
    resp.clicked()
}

/// Whether a file dialog is waiting for the interface's answer.
fn asking() -> bool {
    #[cfg(target_os = "android")]
    {
        crate::files::ask().is_some()
    }
    #[cfg(not(target_os = "android"))]
    {
        false
    }
}

fn sheet_title(which: Sheet) -> String {
    match which {
        Sheet::Properties => tr("mobile.properties"),
        Sheet::Panels => tr("mobile.panels"),
        Sheet::Palette => tr("mobile.palette"),
        Sheet::Files => tr("mobile.files"),
        Sheet::Drawings => tr("mobile.drawings"),
        Sheet::Tools(gi) => crate::tools::GROUPS
            .get(gi)
            .map(|g| g.tools[0].name())
            .unwrap_or_default(),
        Sheet::None
        | Sheet::Drawer
        | Sheet::MenuPage(_)
        | Sheet::Settings
        | Sheet::SettingsPage(_) => String::new(),
    }
}

fn sheet_body(app: &mut App, ui: &mut Ui, which: Sheet, m3: &M3, open: &mut Sheet) {
    match which {
        Sheet::Properties => properties_sheet(app, ui, m3),
        Sheet::Panels => panels_sheet(app, ui, m3),
        Sheet::Palette => palette_grids(app, ui),
        Sheet::Files => files_sheet(app, ui, m3),
        Sheet::Drawings => drawings_sheet(app, ui, m3),
        Sheet::Tools(gi) => tools_sheet(app, ui, m3, gi, open),
        Sheet::None
        | Sheet::Drawer
        | Sheet::MenuPage(_)
        | Sheet::Settings
        | Sheet::SettingsPage(_) => {}
    }
}

// ----- The navigation drawer ------------------------------------------------

/// The menus in a drawer that slides over the drawing from the left, the
/// way an Android app keeps its sections, and the way into the settings.
fn drawer(app: &mut App, ui: &mut Ui, m3: &M3, screen: Rect, open: &mut Sheet) {
    let _ = app;
    let ctx = ui.ctx().clone();
    scrim(&ctx, screen, open);
    let width = (screen.width() * 0.86).min(320.0);
    egui::Area::new(Id::new("mobile_drawer"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(&ctx, |ui| {
            Frame::new()
                .fill(m3.surface)
                .corner_radius(CornerRadius {
                    nw: 0,
                    ne: 16,
                    sw: 0,
                    se: 16,
                })
                .show(ui, |ui| {
                    ui.set_width(width);
                    ui.set_min_height(screen.height());
                    // Clear of the status bar, as the top bar is.
                    ui.add_space(system_insets(&ctx).top);
                    Frame::new()
                        .fill(m3.container)
                        .inner_margin(egui::Margin {
                            left: 20,
                            right: 20,
                            top: 26,
                            bottom: 20,
                        })
                        .show(ui, |ui| {
                            ui.set_width(width - 40.0);
                            ui.label(
                                egui::RichText::new("TraceDraw")
                                    .size(22.0)
                                    .strong()
                                    .color(m3.on),
                            );
                            ui.label(
                                egui::RichText::new(env!("CARGO_PKG_VERSION"))
                                    .size(12.0)
                                    .color(m3.variant),
                            );
                        });
                    egui::ScrollArea::vertical()
                        .id_salt("mobile_drawer_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.add_space(8.0);
                            for (i, (key, _)) in crate::ui::menus::all_menus().iter().enumerate() {
                                if drawer_row(ui, m3, &tr(key), None).clicked() {
                                    *open = Sheet::MenuPage(i);
                                }
                            }
                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(4.0);
                            if drawer_row(ui, m3, &tr("mobile.settings"), Some(Glyph::Gear))
                                .clicked()
                            {
                                *open = Sheet::Settings;
                            }
                            ui.add_space(12.0);
                        });
                });
        });
}

/// A row of the drawer or of the settings: the whole width, a finger high.
fn drawer_row(ui: &mut Ui, m3: &M3, label: &str, glyph: Option<Glyph>) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 52.0), Sense::click());
    if resp.hovered() || resp.is_pointer_button_down_on() {
        let fill = mix(m3.on, m3.surface, 0.08);
        ui.painter()
            .rect_filled(rect.shrink2(Vec2::new(8.0, 2.0)), 26.0, fill);
    }
    let mut left = rect.left() + 20.0;
    if let Some(glyph) = glyph {
        draw_glyph(
            ui,
            Rect::from_center_size(
                egui::pos2(left + 10.0, rect.center().y),
                Vec2::splat(20.0),
            ),
            glyph,
            m3.variant,
        );
        left += 36.0;
    }
    ui.painter().text(
        egui::pos2(left, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(16.0),
        m3.on,
    );
    resp
}

// ----- Pages ----------------------------------------------------------------

/// A whole screen of its own, as an Android app opens one: an app bar with
/// a back arrow and the title, and a body that scrolls.
fn page_ui(
    app: &mut App,
    ui: &mut Ui,
    m3: &M3,
    screen: Rect,
    open: &mut Sheet,
    title: &str,
    back: Sheet,
    wide: bool,
    body: impl FnOnce(&mut App, &mut Ui),
) {
    let ctx = ui.ctx().clone();
    let top = system_insets(&ctx).top;
    egui::Area::new(Id::new("mobile_page"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(&ctx, |ui| {
            Frame::new().fill(m3.surface).show(ui, |ui| {
                ui.set_width(screen.width());
                ui.set_min_height(screen.height());
                ui.add_space(top);
                // The bar takes a row of its own at the top: a centred row
                // would sit in the middle of the page.
                ui.allocate_ui_with_layout(
                    Vec2::new(screen.width(), TOUCH),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.add_space(4.0);
                        if icon_target(ui, m3, Glyph::Back, true).clicked() {
                            *open = back;
                        }
                        ui.label(
                            egui::RichText::new(elide(title, 24))
                                .size(22.0)
                                .color(m3.on),
                        );
                    },
                );
                let scroll = if wide {
                    egui::ScrollArea::both()
                } else {
                    egui::ScrollArea::vertical()
                };
                scroll
                    .id_salt(("mobile_page_scroll", title))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        Frame::new()
                            .inner_margin(egui::Margin {
                                left: 16,
                                right: 16,
                                top: 4,
                                bottom: 28,
                            })
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                body(app, ui);
                            });
                    });
            });
        });
}

/// The items of one menu, on a page of their own: a phone has no room for
/// twelve menus side by side, and no pointer to hover a submenu open with.
fn menu_page(app: &mut App, ui: &mut Ui, m3: &M3, screen: Rect, open: &mut Sheet, index: usize) {
    let menus = crate::ui::menus::all_menus();
    let Some((key, menu)) = menus.get(index).copied() else {
        *open = Sheet::Drawer;
        return;
    };
    let title = tr(key);
    page_ui(app, ui, m3, screen, open, &title, Sheet::Drawer, false, |app, ui| {
        crate::ui::menus::body(ui, |ui| menu(app, ui));
    });
}

/// Every setting, grouped as the desktop's Options dialog groups them.
fn settings_list(app: &mut App, ui: &mut Ui, m3: &M3, screen: Rect, open: &mut Sheet) {
    // The page keeps the sheet to itself while it is drawn: the choice is
    // kept beside it and taken up after.
    let mut back = Sheet::None;
    let chosen = std::cell::Cell::new(Sheet::None);
    page_ui(
        app,
        ui,
        m3,
        screen,
        &mut back,
        &tr("mobile.settings"),
        Sheet::Drawer,
        false,
        |app, ui| {
            for set in crate::ui::options::OptionsSet::ALL {
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new(set.tab_label())
                        .size(13.0)
                        .strong()
                        .color(m3.primary),
                );
                for page in set.pages() {
                    if drawer_row(ui, m3, &tr(page.key()), None).clicked() {
                        app.options_page = *page;
                        chosen.set(Sheet::SettingsPage(*page));
                    }
                }
            }
        },
    );
    let next = if chosen.get() != Sheet::None {
        chosen.get()
    } else {
        back
    };
    if next != Sheet::None {
        *open = next;
    }
}

/// One page of the settings, drawn by the same code the desktop dialog
/// draws it with: every setting works here as it does there.
fn settings_page(
    app: &mut App,
    ui: &mut Ui,
    m3: &M3,
    screen: Rect,
    open: &mut Sheet,
    page: crate::ui::options::OptionsPage,
) {
    app.options_page = page;
    let title = tr(page.key());
    page_ui(app, ui, m3, screen, open, &title, Sheet::Settings, true, |app, ui| {
        // The page's own controls (check boxes, lists, number fields) are
        // drawn at the size a finger needs.
        ui.spacing_mut().interact_size = Vec2::new(TOUCH, TOUCH - 8.0);
        ui.spacing_mut().button_padding = Vec2::new(10.0, 6.0);
        crate::ui::options::draw_page(app, ui);
    });
}

// ----- The bottom sheets ----------------------------------------------------

/// The properties of what is selected, in the sheet's width. With nothing
/// selected they are the page's, and a phone shows them as a form of its
/// own; with something selected they are the property bar's, in a strip a
/// finger swipes sideways.
fn properties_sheet(app: &mut App, ui: &mut Ui, m3: &M3) {
    if app.selection.is_empty() {
        page_properties(app, ui, m3);
        return;
    }
    ui.add_space(4.0);
    Frame::new()
        .fill(m3.container)
        .corner_radius(16.0)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::ScrollArea::horizontal()
                .id_salt("mobile_property_strip")
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                    crate::ui::toolbar::property_bar(app, ui);
                });
        });
}

/// The name of a field of the page form.
fn property_label(ui: &mut Ui, m3: &M3, text: &str) {
    ui.add_space(2.0);
    ui.label(egui::RichText::new(text).size(12.5).color(m3.variant));
}

/// Room for one row of the page form.
fn property_row(ui: &mut Ui, width: f32, height: f32) -> Rect {
    ui.allocate_exact_size(Vec2::new(width, height), Sense::hover())
        .0
}

/// A choice of two states (portrait and landscape), the chosen one washed
/// with the accent.
fn toggle_button(ui: &mut Ui, m3: &M3, label: &str, chosen: bool, width: f32) -> Response {
    let text = egui::RichText::new(label)
        .size(14.0)
        .color(if chosen { m3.primary } else { m3.on });
    let mut button = egui::Button::new(text)
        .corner_radius(12.0)
        .min_size(Vec2::new(width, 46.0));
    button = if chosen {
        button.fill(m3.primary_container)
    } else {
        button
            .fill(m3.high)
            .stroke(Stroke::new(1.0, m3.outline))
    };
    ui.add(button)
}

/// The page's own properties as a phone form: the size, the width and the
/// height, the way round it lies and the button that fits it to the
/// drawing — one control to a row, a finger high.
fn page_properties(app: &mut App, ui: &mut Ui, m3: &M3) {
    let width = ui.available_width();
    let units = app.units;
    let size = app.page_size();
    let presets = crate::ui::dialogs::paper_presets();
    let current = presets
        .iter()
        .find(|(_, s)| {
            ((s.width - size.width).abs() < 0.05 && (s.height - size.height).abs() < 0.05)
                || ((s.height - size.width).abs() < 0.05 && (s.width - size.height).abs() < 0.05)
        })
        .map(|(n, _)| n.to_string())
        .unwrap_or_else(|| tr("toolbar.custom"));

    // The page size, by name.
    property_label(ui, m3, &tr("newdoc.page_size"));
    let r = property_row(ui, width, 46.0);
    let mut chosen = None;
    crate::ui::chrome::combo(ui, "mobile_page_size", r, current.clone(), |ui| {
        for (n, s) in &presets {
            if ui.selectable_label(*n == current, *n).clicked() {
                chosen = Some(*s);
            }
        }
    });
    if let Some(s) = chosen {
        // The page keeps the way round it already lies.
        let landscape = size.width > size.height;
        let s = if landscape {
            tracedraw_core::geometry::Size::new(s.height.max(s.width), s.height.min(s.width))
        } else {
            tracedraw_core::geometry::Size::new(s.width.min(s.height), s.width.max(s.height))
        };
        crate::ui::propbar::resize_pages(app, s);
        app.fit_pending = true;
    }

    // Its width and its height, in the drawing's units.
    let mut new_size = None;
    property_label(ui, m3, &tr("newdoc.width"));
    let r = property_row(ui, width, 46.0);
    if let Some(v) = crate::ui::chrome::spin_field(
        ui,
        "mobile_page_width",
        r,
        units.from_mm(size.width),
        1,
        units.short(),
        0.1,
        0.1..=1.0e7,
    ) {
        new_size = Some(tracedraw_core::geometry::Size::new(
            units.to_mm(v).max(0.1),
            size.height,
        ));
    }
    property_label(ui, m3, &tr("newdoc.height"));
    let r = property_row(ui, width, 46.0);
    if let Some(v) = crate::ui::chrome::spin_field(
        ui,
        "mobile_page_height",
        r,
        units.from_mm(size.height),
        1,
        units.short(),
        0.1,
        0.1..=1.0e7,
    ) {
        new_size = Some(tracedraw_core::geometry::Size::new(
            size.width,
            units.to_mm(v).max(0.1),
        ));
    }
    if let Some(s) = new_size {
        crate::ui::propbar::resize_pages(app, s);
        app.fit_pending = true;
    }

    // Portrait or landscape, then the page fitted to what is on it.
    ui.add_space(8.0);
    let portrait = size.height >= size.width;
    let half = (width - 8.0) / 2.0;
    ui.horizontal(|ui| {
        if toggle_button(ui, m3, &tr("dialog.portrait"), portrait, half).clicked() && !portrait {
            crate::ui::propbar::resize_pages(
                app,
                tracedraw_core::geometry::Size::new(size.height, size.width),
            );
            app.fit_pending = true;
        }
        if toggle_button(ui, m3, &tr("dialog.landscape"), !portrait, half).clicked() && portrait {
            crate::ui::propbar::resize_pages(
                app,
                tracedraw_core::geometry::Size::new(size.height, size.width),
            );
            app.fit_pending = true;
        }
    });
    ui.add_space(10.0);
    if tonal_button(ui, m3, &tr("toolbar.autofit_page"), true).clicked() {
        app.fit_page_to_drawing_with_margin(app.settings.fit_page_margin);
        app.fit_pending = true;
    }
}

/// The docker column: a row of chips naming its panels, the one in front
/// under them.
fn panels_sheet(app: &mut App, ui: &mut Ui, m3: &M3) {
    // The column is shown while its sheet is: the sheet is its window.
    app.set_docker_column(DockerPlace::Right, true, None);
    let (_, front, _) = app.docker_column(DockerPlace::Right);
    let mut pick = None;
    egui::ScrollArea::horizontal()
        .id_salt("mobile_dockers")
        .max_height(48.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                for tab in DockerTab::ALL {
                    let chosen = tab == front;
                    if chip(ui, m3, &tr(tab.key()), chosen).clicked() {
                        pick = Some(tab);
                    }
                }
            });
        });
    if let Some(tab) = pick {
        app.open_docker(tab);
    }
    ui.add_space(12.0);
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(width, ui.available_height().max(480.0)),
        Sense::hover(),
    );
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        crate::ui::dockers::dockers(app, ui, DockerPlace::Right);
    });
}

/// The colour palettes as grids of swatches a finger can hit: as many to a
/// row as fit, the rest under them, swiped rather than scrolled by arrows.
fn palette_grids(app: &mut App, ui: &mut Ui) {
    crate::ui::palette::grid(app, ui, crate::ui::palette::Row::Main);
    ui.add_space(16.0);
    crate::ui::palette::grid(app, ui, crate::ui::palette::Row::Document);
}

/// The palettes as a card floating over the drawing, in the corner the
/// tools are not in: a colour can be picked without losing sight of what
/// it goes on. No scrim — the drawing stays live behind it.
fn palette_float(app: &mut App, ctx: &egui::Context, screen: Rect, m3: &M3, open: &mut Sheet) {
    let land = landscape(screen);
    let width = (screen.width() * 0.78).clamp(232.0, 360.0);
    let height = (screen.height() * 0.34).clamp(190.0, 300.0);
    let pos = if land {
        // On its side: under the top bar, against the destinations rail.
        egui::pos2(
            screen.right() - NAV_RAIL_W - width - 12.0,
            screen.top() + TOP_H + 12.0,
        )
    } else {
        // Standing: above the tool strip, against the right edge.
        egui::pos2(
            screen.right() - width - 12.0,
            screen.bottom() - NAV_H - TOOLS_H - height - 12.0,
        )
    };
    let card = egui::Area::new(Id::new("mobile_palette_float"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos)
        .show(ctx, |ui| {
            Frame::new()
                .fill(m3.surface)
                .corner_radius(20.0)
                .shadow(crate::theme::shadow(egui::epaint::Shadow {
                    offset: [0, 8],
                    blur: 28,
                    spread: 2,
                    color: Color32::from_black_alpha(80),
                }))
                .inner_margin(egui::Margin {
                    left: 0,
                    right: 0,
                    top: 0,
                    bottom: 0,
                })
                .show(ui, |ui| {
                    ui.set_width(width);
                    ui.set_min_height(height);
                    // No close button: a tap anywhere else puts it away.
                    let (row, _) =
                        ui.allocate_exact_size(Vec2::new(width, 44.0), Sense::hover());
                    ui.painter().text(
                        egui::pos2(row.left() + 16.0, row.center().y),
                        egui::Align2::LEFT_CENTER,
                        sheet_title(Sheet::Palette),
                        crate::theme::bold(16.0),
                        m3.on,
                    );
                    egui::ScrollArea::vertical()
                        .id_salt("mobile_palette_scroll")
                        .max_height(height - 44.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            Frame::new()
                                .inner_margin(egui::Margin {
                                    left: 14,
                                    right: 14,
                                    top: 0,
                                    bottom: 12,
                                })
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    palette_grids(app, ui);
                                });
                        });
                });
        });
    // A press outside the card puts it away: it floats over the drawing,
    // so there is nothing else to tap. Not the press that opened it,
    // though — that one is on the bar under the card.
    let since_id = Id::new("mobile_palette_since");
    let since = match ctx.data(|d| d.get_temp::<u64>(since_id)) {
        Some(frame) => frame,
        None => {
            let frame = ctx.cumulative_frame_nr();
            ctx.data_mut(|d| d.insert_temp(since_id, frame));
            frame
        }
    };
    if ctx.cumulative_frame_nr() > since && ctx.input(|i| i.pointer.any_pressed()) {
        if let Some(at) = ctx.pointer_latest_pos() {
            if !card.response.rect.contains(at) {
                *open = Sheet::None;
            }
        }
    }
}

/// The other tools of a toolbox group, as a list to choose from.
fn tools_sheet(app: &mut App, ui: &mut Ui, m3: &M3, gi: usize, open: &mut Sheet) {
    let Some(group) = crate::tools::GROUPS.get(gi) else {
        return;
    };
    let mut pick = None;
    for tool in group.tools {
        let chosen = app.tool == *tool;
        if tool_row(ui, m3, *tool, chosen).clicked() {
            pick = Some(*tool);
        }
    }
    if let Some(tool) = pick {
        app.set_tool(tool);
        *open = Sheet::None;
    }
}

/// The files on the phone, in a list a finger walks through: the app's own
/// folder while it may not see the storage, the shared Documents folder
/// once the user allows it. A phone has no file dialog of the system's, so
/// this list is the dialog.
/// A menu label without the dots a desktop dialog title ends with.
fn plain(label: &str) -> String {
    label
        .trim_end_matches("...")
        .trim_end_matches('\u{2026}')
        .trim()
        .to_string()
}

fn files_sheet(app: &mut App, ui: &mut Ui, m3: &M3) {
    let ctx = ui.ctx().clone();
    #[cfg(target_os = "android")]
    let granted =
        crate::android_system::phone().is_some_and(crate::android_system::storage_granted);
    #[cfg(not(target_os = "android"))]
    let granted = true;

    #[cfg(target_os = "android")]
    if granted {
        // The grant may have just been given in the system's screen.
        crate::files::refresh_android_documents();
    }

    #[cfg(target_os = "android")]
    let ask = crate::files::ask();
    #[cfg(not(target_os = "android"))]
    let ask: Option<()> = None;
    let picking = ask.is_some();

    if !granted {
        Frame::new()
            .fill(m3.primary_container)
            .corner_radius(12.0)
            .inner_margin(egui::Margin::symmetric(14, 12))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(tr("mobile.need_storage"))
                        .size(14.0)
                        .color(m3.on),
                );
                ui.add_space(10.0);
                if filled_button(ui, m3, &tr("mobile.grant_storage")).clicked() {
                    #[cfg(target_os = "android")]
                    if let Some(phone) = crate::android_system::phone() {
                        crate::android_system::ask_storage(phone);
                    }
                }
            });
        ui.add_space(12.0);
    }

    if !picking {
        ui.horizontal_wrapped(|ui| {
            if filled_button(ui, m3, &plain(&tr("menu.file.new"))).clicked() {
                app.request_new_document();
            }
            if filled_button(ui, m3, &plain(&tr("menu.file.open"))).clicked() {
                app.open_dialog();
            }
            let has = app.has_document();
            if tonal_button(ui, m3, &plain(&tr("menu.file.save")), has).clicked() {
                save_drawing(app, false);
            }
            if tonal_button(ui, m3, &plain(&tr("menu.file.save_as")), has).clicked() {
                save_drawing(app, true);
            }
        });
        ui.add_space(12.0);
    }

    // Where the list stands, and the way back up.
    let start = crate::files::android_documents();
    let key = Id::new("mobile_files_dir");
    let mut dir: std::path::PathBuf = ctx
        .data(|d| d.get_temp(key))
        .unwrap_or_else(|| start.clone());
    if !dir.is_dir() {
        dir = start.clone();
    }
    // The way up stops at the root of the shared storage: the folders
    // above it are the system's, not the user's.
    let root = crate::files::android_root();
    let up = dir
        .parent()
        .map(|p| p.to_path_buf())
        .filter(|p| p.starts_with(&root) && p != &root);
    ui.horizontal(|ui| {
        match &up {
            Some(up) => {
                if icon_target(ui, m3, Glyph::Back, true).clicked() {
                    ctx.data_mut(|d| d.insert_temp(key, up.clone()));
                }
            }
            None => {
                ui.add_space(TOUCH);
            }
        }
        ui.add(
            egui::Label::new(
                egui::RichText::new(dir.display().to_string())
                    .size(12.0)
                    .color(m3.variant),
            )
            .truncate(),
        );
    });

    #[cfg(target_os = "android")]
    let exts: Vec<String> = ask.as_ref().map(|a| a.exts.clone()).unwrap_or_default();
    #[cfg(not(target_os = "android"))]
    let exts: Vec<String> = Vec::new();
    // Browsing shows the drawings and the pictures they may hold; a dialog
    // asking for a file says which kinds it takes.
    let wanted = |path: &std::path::Path| -> bool {
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            return false;
        };
        let ext = ext.to_ascii_lowercase();
        if exts.is_empty() {
            DRAWINGS.contains(&ext.as_str())
        } else {
            exts.contains(&ext)
        }
    };

    let entries = crate::files::list_entries(&dir);
    let mut folders = Vec::new();
    let mut files = Vec::new();
    for entry in entries {
        if entry.is_dir {
            folders.push(entry);
        } else if wanted(&entry.path) {
            files.push(entry);
        }
    }

    if folders.is_empty() && files.is_empty() {
        ui.add_space(10.0);
        ui.label(
            egui::RichText::new(tr("mobile.no_files"))
                .size(14.0)
                .color(m3.variant),
        );
        return;
    }

    let mut descend = None;
    let mut chosen = None;
    for entry in folders.iter().take(120) {
        if folder_row(ui, m3, &entry.name).clicked() {
            descend = Some(entry.path.clone());
        }
    }
    for entry in files.iter().take(300) {
        if file_row(ui, m3, &entry.name, entry.size).clicked() {
            chosen = Some(entry.path.clone());
        }
    }
    if let Some(path) = descend {
        ctx.data_mut(|d| d.insert_temp(key, path));
    }
    if let Some(path) = chosen {
        // A dialog waiting for an answer takes the tap; otherwise the tap
        // opens the drawing.
        #[cfg(target_os = "android")]
        {
            if picking {
                crate::files::answer(app, vec![path]);
            } else {
                app.open_path(path);
            }
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = picking;
            app.open_path(path);
        }
    }
}

/// The kinds of file the list shows when no dialog asks for something
/// particular: the drawings TraceDraw opens, and the pictures they hold.
const DRAWINGS: [&str; 13] = [
    "tdraw", "cdr", "cmx", "svg", "pdf", "png", "jpg", "jpeg", "webp", "bmp", "gif", "tif", "tiff",
];

/// Save the drawing: back to its file, or as a new one named after it in
/// the app's folder. No Save As dialog: a phone has no room for the file
/// browser the desktop opens, and its folder is the one place the app may
/// write anyway.
fn save_drawing(app: &mut App, as_new: bool) {
    if !as_new {
        if let Some(path) = app.file.clone() {
            app.save_to(path);
            return;
        }
    }
    let name = save_name(app);
    if let Some(path) = crate::files::Dialog::new().set_file_name(name).save_path() {
        app.save_to(path);
    }
}

/// The file name a drawing is saved under: its own, without the marker of
/// unsaved changes, or a plain name for a new one.
fn save_name(app: &App) -> String {
    let title = app.drawing_title(app.active_doc);
    let stem = title.trim_end_matches('*').trim();
    let stem = stem.strip_suffix(".tdraw").unwrap_or(stem);
    let stem = if stem.is_empty() { "Drawing" } else { stem };
    format!("{stem}.tdraw")
}

/// The open drawings, as a list of tabs a finger picks from.
fn drawings_sheet(app: &mut App, ui: &mut Ui, m3: &M3) {
    let active = app.active_doc;
    let mut pick = None;
    for (i, _) in app.docs.iter().enumerate() {
        let title = app.document_tab_title(i);
        if drawing_row(ui, m3, &title, i == active).clicked() {
            pick = Some(i);
        }
    }
    if let Some(i) = pick {
        app.active_doc = i;
    }
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if filled_button(ui, m3, &plain(&tr("menu.file.new"))).clicked() {
            app.request_new_document();
        }
        if tonal_button(ui, m3, &plain(&tr("menu.file.open")), true).clicked() {
            app.open_dialog();
        }
    });
}

// ---------------------------------------------------------------------------
// Material pieces
// ---------------------------------------------------------------------------

/// The icons the bars draw: the app's own vector icons where one fits, a
/// few small glyphs of our own where none does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Glyph {
    Menu,
    Close,
    Back,
    Gear,
    Folder,
    Undo,
    Redo,
    Sliders,
    Panels,
    Palette,
    Files,
}

/// A round target of the bars: the state layer of a Material icon button,
/// the icon centred in it.
fn icon_target(ui: &mut Ui, m3: &M3, glyph: Glyph, enabled: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(TOUCH), Sense::click());
    if enabled && (resp.hovered() || resp.is_pointer_button_down_on()) {
        let fill = if resp.is_pointer_button_down_on() {
            mix(m3.primary, m3.surface, 0.16)
        } else {
            mix(m3.primary, m3.surface, 0.08)
        };
        ui.painter().circle_filled(rect.center(), TOUCH / 2.0, fill);
    }
    let color = if enabled { m3.on } else { m3.variant };
    // The icon is the Material 24 point one, centred in its target.
    draw_glyph(ui, Rect::from_center_size(rect.center(), Vec2::splat(ICON)), glyph, color);
    resp
}

/// A toolbox group's button: a rounded square behind the chosen tool, as a
/// Material button shows what is chosen.
fn tool_target(ui: &mut Ui, m3: &M3, tool: crate::tools::Tool, active: bool) -> Response {
    // The tool's name under its icon: the icons alone say little to
    // someone who has not learnt them yet, and a phone has the room.
    let name = tool.name();
    let font = egui::FontId::proportional(10.5);
    let galley = ui
        .painter()
        .layout_no_wrap(name, font.clone(), m3.variant);
    let width = (galley.size().x + 10.0).max(TOUCH);
    let height = TOOLS_H - 6.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
    let body = Rect::from_center_size(
        egui::pos2(rect.center().x, rect.top() + 22.0),
        Vec2::new(TOUCH - 8.0, 44.0),
    );
    if active {
        ui.painter().rect_filled(body, 12.0, m3.primary_container);
    } else if resp.hovered() || resp.is_pointer_button_down_on() {
        let fill = mix(m3.on, m3.container, 0.10);
        ui.painter().rect_filled(body, 12.0, fill);
    }
    let icon = Rect::from_center_size(body.center(), Vec2::splat(ICON));
    let color = if active {
        m3.primary
    } else {
        crate::ui::icons::tool_color(tool, m3.on)
    };
    crate::ui::icons::draw(ui.painter(), icon, tool, color);
    ui.painter().galley(
        egui::pos2(
            rect.center().x - galley.size().x / 2.0,
            rect.top() + 46.0,
        ),
        galley,
        if active { m3.primary } else { m3.variant },
    );
    resp
}

/// A destination of the navigation bar: the icon in a pill that fills when
/// chosen, its name under it.
fn nav_item(
    ui: &mut Ui,
    m3: &M3,
    glyph: Glyph,
    label: &str,
    active: bool,
    width: f32,
    height: f32,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
    let pill = Rect::from_center_size(
        egui::pos2(rect.center().x, rect.top() + height * 0.34),
        Vec2::new(56.0, 32.0),
    );
    if active {
        ui.painter().rect_filled(pill, 16.0, m3.primary_container);
    } else if resp.hovered() || resp.is_pointer_button_down_on() {
        let fill = mix(m3.on, m3.container, 0.08);
        ui.painter().rect_filled(pill, 16.0, fill);
    }
    let icon = Rect::from_center_size(pill.center(), Vec2::splat(ICON));
    draw_glyph(ui, icon, glyph, if active { m3.primary } else { m3.variant });
    ui.painter().text(
        egui::pos2(rect.center().x, rect.top() + height * 0.72),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(12.0),
        if active { m3.primary } else { m3.variant },
    );
    resp
}

/// A chip: a rounded label that says whether it is the one in front.
fn chip(ui: &mut Ui, m3: &M3, label: &str, chosen: bool) -> Response {
    let mut button = egui::Button::new(egui::RichText::new(label).size(13.0))
        .corner_radius(8.0)
        .min_size(Vec2::new(0.0, 36.0));
    button = if chosen {
        button.fill(m3.primary_container)
    } else {
        button
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, m3.outline))
    };
    ui.add(button)
}

/// A filled button: the one action a sheet offers first.
pub(crate) fn filled_button(ui: &mut Ui, m3: &M3, label: &str) -> Response {
    ui.add(
        egui::Button::new(egui::RichText::new(label).size(14.0).color(Color32::WHITE))
            .fill(m3.primary)
            .corner_radius(20.0)
            .min_size(Vec2::new(0.0, 40.0)),
    )
}

/// A tonal button: an action beside the first one.
fn tonal_button(ui: &mut Ui, m3: &M3, label: &str, enabled: bool) -> Response {
    let text = egui::RichText::new(label).size(14.0).color(if enabled {
        m3.primary
    } else {
        m3.variant
    });
    ui.add_enabled(
        enabled,
        egui::Button::new(text)
            .fill(if enabled { m3.primary_container } else { m3.high })
            .corner_radius(20.0)
            .min_size(Vec2::new(0.0, 40.0)),
    )
}

/// A text button: the plain, capitalised action a Material dialog ends
/// with, and the one a sheet header puts beside its title.
pub(crate) fn text_button(ui: &mut Ui, m3: &M3, label: &str) -> Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new(label)
                .size(14.0)
                .strong()
                .color(m3.primary),
        )
        .fill(Color32::TRANSPARENT)
        .corner_radius(20.0)
        .min_size(Vec2::new(64.0, 40.0)),
    )
}

/// A tool in the list of a group: its icon, its name, and a wash behind
/// the one in use.
fn tool_row(ui: &mut Ui, m3: &M3, tool: crate::tools::Tool, chosen: bool) -> Response {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 56.0), Sense::click());
    if chosen {
        ui.painter().rect_filled(rect, 12.0, m3.primary_container);
    } else if resp.hovered() || resp.is_pointer_button_down_on() {
        let fill = mix(m3.on, m3.surface, 0.08);
        ui.painter().rect_filled(rect, 12.0, fill);
    }
    let icon = Rect::from_center_size(
        egui::pos2(rect.left() + 32.0, rect.center().y),
        Vec2::splat(ICON),
    );
    let color = if chosen {
        m3.primary
    } else {
        crate::ui::icons::tool_color(tool, m3.on)
    };
    crate::ui::icons::draw(ui.painter(), icon, tool, color);
    ui.painter().text(
        egui::pos2(rect.left() + 64.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        tool.name(),
        egui::FontId::proportional(16.0),
        if chosen { m3.primary } else { m3.on },
    );
    resp
}

/// A folder in the file list: a folder mark, its name, the whole row a
/// target for a finger.
fn folder_row(ui: &mut Ui, m3: &M3, name: &str) -> Response {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 52.0), Sense::click());
    if resp.hovered() || resp.is_pointer_button_down_on() {
        let fill = mix(m3.on, m3.surface, 0.08);
        ui.painter().rect_filled(rect, 12.0, fill);
    }
    let icon = Rect::from_center_size(
        egui::pos2(rect.left() + 20.0, rect.center().y),
        Vec2::splat(20.0),
    );
    draw_glyph(ui, icon, Glyph::Folder, m3.variant);
    ui.painter().text(
        egui::pos2(rect.left() + 40.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        name,
        egui::FontId::proportional(16.0),
        m3.on,
    );
    resp
}

/// A file in the list: its name at the left, its size at the right, the
/// whole row a target for a finger.
fn file_row(ui: &mut Ui, m3: &M3, name: &str, size: u64) -> Response {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 56.0), Sense::click());
    if resp.hovered() || resp.is_pointer_button_down_on() {
        let fill = mix(m3.on, m3.surface, 0.08);
        ui.painter().rect_filled(rect, 12.0, fill);
    }
    ui.painter().text(
        egui::pos2(rect.left() + 8.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        name,
        egui::FontId::proportional(16.0),
        m3.on,
    );
    ui.painter().text(
        egui::pos2(rect.right() - 8.0, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        bytes_text(size),
        egui::FontId::proportional(12.0),
        m3.variant,
    );
    resp
}

/// An open drawing in the list of them.
fn drawing_row(ui: &mut Ui, m3: &M3, title: &str, active: bool) -> Response {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 56.0), Sense::click());
    if active {
        ui.painter().rect_filled(rect, 12.0, m3.primary_container);
    } else if resp.hovered() || resp.is_pointer_button_down_on() {
        let fill = mix(m3.on, m3.surface, 0.08);
        ui.painter().rect_filled(rect, 12.0, fill);
    }
    ui.painter().text(
        egui::pos2(rect.left() + 12.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(16.0),
        if active { m3.primary } else { m3.on },
    );
    resp
}

/// Draw one of the bars' glyphs in `rect`.
fn draw_glyph(ui: &Ui, rect: Rect, glyph: Glyph, color: Color32) {
    let painter = ui.painter();
    let c = rect.center();
    let s = rect.width();
    let stroke = Stroke::new(2.0, color);
    match glyph {
        Glyph::Menu => {
            let w = s * 0.36;
            for i in [-1.0f32, 0.0, 1.0] {
                let y = c.y + i * (s * 0.22);
                painter.line_segment([egui::pos2(c.x - w, y), egui::pos2(c.x + w, y)], stroke);
            }
        }
        Glyph::Close => {
            let a = s * 0.24;
            painter.line_segment(
                [egui::pos2(c.x - a, c.y - a), egui::pos2(c.x + a, c.y + a)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(c.x - a, c.y + a), egui::pos2(c.x + a, c.y - a)],
                stroke,
            );
        }
        Glyph::Back => {
            // An arrow pointing left, the way back of every Android page.
            let w = s * 0.30;
            painter.line_segment(
                [egui::pos2(c.x + w, c.y), egui::pos2(c.x - w, c.y)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(c.x - w, c.y), egui::pos2(c.x - w * 0.1, c.y - w * 0.9)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(c.x - w, c.y), egui::pos2(c.x - w * 0.1, c.y + w * 0.9)],
                stroke,
            );
        }
        Glyph::Gear => {
            painter.circle_stroke(c, s * 0.18, Stroke::new(1.8, color));
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let (sin, cos) = a.sin_cos();
                painter.line_segment(
                    [
                        egui::pos2(c.x + cos * s * 0.24, c.y + sin * s * 0.24),
                        egui::pos2(c.x + cos * s * 0.34, c.y + sin * s * 0.34),
                    ],
                    Stroke::new(1.8, color),
                );
            }
        }
        Glyph::Folder => {
            let body = Rect::from_center_size(
                egui::pos2(c.x, c.y + s * 0.04),
                Vec2::new(s * 0.62, s * 0.46),
            );
            painter.rect_stroke(body, 3.0, Stroke::new(1.6, color), egui::StrokeKind::Inside);
            painter.line_segment(
                [
                    egui::pos2(body.left(), body.top()),
                    egui::pos2(body.left() + s * 0.20, body.top() - s * 0.12),
                ],
                Stroke::new(1.6, color),
            );
        }
        Glyph::Undo => draw_action(painter, rect, Action::Undo, color),
        Glyph::Redo => draw_action(painter, rect, Action::Redo, color),
        Glyph::Files => draw_action(painter, rect, Action::Open, color),
        Glyph::Sliders => {
            // Three lines with a knob each.
            for (i, x) in [-0.26f32, 0.0, 0.26].iter().enumerate() {
                let px = c.x + x * s;
                painter.line_segment(
                    [egui::pos2(px, c.y - s * 0.28), egui::pos2(px, c.y + s * 0.28)],
                    Stroke::new(1.6, color),
                );
                let y = c.y + if i == 1 { 0.10 } else { -0.10 } * s;
                painter.circle_filled(egui::pos2(px, y), s * 0.07, color);
            }
        }
        Glyph::Panels => {
            let body = Rect::from_center_size(c, Vec2::splat(s * 0.62));
            painter.rect_stroke(body, 3.0, Stroke::new(1.6, color), egui::StrokeKind::Inside);
            painter.line_segment(
                [
                    egui::pos2(body.left() + body.width() * 0.42, body.top() + 2.0),
                    egui::pos2(body.left() + body.width() * 0.42, body.bottom() - 2.0),
                ],
                Stroke::new(1.6, color),
            );
        }
        Glyph::Palette => {
            painter.circle_stroke(c, s * 0.30, Stroke::new(1.6, color));
            for (dx, dy) in [(-0.13f32, -0.06f32), (0.02, -0.16), (0.13, 0.02)] {
                painter.circle_filled(egui::pos2(c.x + dx * s, c.y + dy * s), s * 0.055, color);
            }
        }
    }
}

fn bytes_text(n: u64) -> String {
    const KB: f64 = 1024.0;
    let n = n as f64;
    if n < KB {
        format!("{n:.0} B")
    } else if n < KB * KB {
        format!("{:.0} KB", n / KB)
    } else {
        format!("{:.1} MB", n / (KB * KB))
    }
}

/// The end of a message, for a line too narrow to hold all of it: a
/// message that names a file starts with a folder the phone has no room
/// for, and the rest of the sentence is what says what happened.
fn tail_of(text: &str, keep: usize) -> String {
    let count = text.chars().count();
    if count <= keep {
        return text.to_string();
    }
    let mut out = String::from("\u{2026}");
    out.extend(text.chars().skip(count - keep));
    out
}
