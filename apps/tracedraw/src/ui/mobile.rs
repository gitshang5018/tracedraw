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
use crate::i18n::{tr, trf};
use crate::ui::icons::{draw_action, Action};
use egui::{Color32, CornerRadius, Frame, Id, Rect, Response, Sense, Stroke, Ui, Vec2};

/// The top app bar: the menu, the drawing's name, undo and redo.
const TOP_H: f32 = 64.0;
/// The strip of tools above the navigation bar.
const TOOLS_H: f32 = 72.0;
/// The strip of properties above the tools, shown while something is
/// selected: never shorter than this, taller when the bar's fields are.
const SELECTION_H: f32 = 44.0;
/// The navigation bar: four destinations.
const NAV_H: f32 = 76.0;
/// How wide one of them is: fixed, since the commands of the selection
/// follow them along the same bar and the row scrolls sideways.
const NAV_ITEM_W: f32 = 72.0;
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
    /// The page behind the cards and the bars.
    pub(crate) surface: Color32,
    /// A card, a bar, a sheet: white on the page.
    pub(crate) container: Color32,
    /// A field or a chip inside a card.
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
    /// The hairline between two rows of a card.
    pub(crate) separator: Color32,
    /// The arrow at the end of a row that opens something.
    pub(crate) chevron: Color32,
}

/// The phone's own light or dark setting, as the system last said it: 1
/// dark, 0 light, and -1 while nothing has said yet. The interface on a
/// phone follows the phone rather than the app's own theme, which the
/// desktop picks by hand.
static SYSTEM_THEME: std::sync::atomic::AtomicI8 = std::sync::atomic::AtomicI8::new(-1);

/// Take the system's light or dark setting for this frame.
fn note_system_theme(ctx: &egui::Context) {
    let said = match ctx.system_theme() {
        Some(egui::Theme::Dark) => 1,
        Some(egui::Theme::Light) => 0,
        None => -1,
    };
    SYSTEM_THEME.store(said, std::sync::atomic::Ordering::Relaxed);
}

/// The colours of the tiles at the head of a list's rows: warm to cool, the
/// way the phone's own settings page sets its icons, so a list reads as a
/// set of destinations rather than a block of text.
pub(crate) const TILES: [Color32; 9] = [
    Color32::from_rgb(0xFF, 0x9F, 0x0A),
    Color32::from_rgb(0xFF, 0x5B, 0x2E),
    Color32::from_rgb(0xA8, 0x55, 0xF7),
    Color32::from_rgb(0x2A, 0xC7, 0xF0),
    Color32::from_rgb(0x4A, 0x7C, 0xFF),
    Color32::from_rgb(0x34, 0xC7, 0x59),
    Color32::from_rgb(0x5B, 0x6C, 0xFF),
    Color32::from_rgb(0x1F, 0xB6, 0xE8),
    Color32::from_rgb(0x2E, 0xC4, 0x8A),
];

impl M3 {
    pub(crate) fn of() -> Self {
        let dark = match SYSTEM_THEME.load(std::sync::atomic::Ordering::Relaxed) {
            1 => true,
            0 => false,
            // Nothing has said yet: the app's own theme stands in.
            _ => crate::theme::theme().is_dark(),
        };
        let c = |r: u8, g: u8, b: u8| Color32::from_rgb(r, g, b);
        if dark {
            Self {
                surface: c(0x0E, 0x0E, 0x10),
                container: c(0x1C, 0x1C, 0x1E),
                high: c(0x2C, 0x2C, 0x2E),
                on: c(0xF5, 0xF5, 0xF7),
                variant: c(0x9A, 0x9A, 0xA0),
                primary: c(0x5B, 0x8B, 0xFF),
                primary_container: c(0x1E, 0x2A, 0x44),
                outline: c(0x2C, 0x2C, 0x2E),
                separator: c(0x2C, 0x2C, 0x2E),
                chevron: c(0x5A, 0x5A, 0x5E),
            }
        } else {
            Self {
                surface: c(0xF2, 0xF3, 0xF5),
                container: c(0xFF, 0xFF, 0xFF),
                high: c(0xF2, 0xF3, 0xF5),
                on: c(0x17, 0x17, 0x1A),
                variant: c(0x8A, 0x8A, 0x8F),
                primary: c(0x3D, 0x6D, 0xFF),
                primary_container: c(0xE9, 0xEF, 0xFF),
                outline: c(0xED, 0xED, 0xF0),
                separator: c(0xED, 0xED, 0xF0),
                chevron: c(0xC4, 0xC4, 0xC8),
            }
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
pub enum Sheet {
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
    /// The chip at the left edge: pictures and drawings from a prompt.
    AiGenerate,
    /// The chip at the left edge: the background taken out of a picture.
    Matting,
    /// The chip at the left edge: one drawing filled from a table.
    Variable,
    /// The chip at the left edge: a picture turned into curves.
    Trace,
    /// The chip at the left edge: the selected parts packed onto a sheet.
    Arrange,
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
    status_bar_visible: bool,
    /// How much of `top` the camera cutout accounts for: with the status
    /// bar hidden this is all the room the bar still owes the notch.
    cutout_top: f32,
}

/// How many frames the system bars may take to follow a request before
/// their last word is taken as final (see `system_insets`).
#[cfg(target_os = "android")]
const BAR_SETTLE_FRAMES: u32 = 40;

/// The insets read last, with the surface and the bars request they
/// belong to: `(pixels, bars_hidden, insets, frames waited)`.
#[cfg(target_os = "android")]
type InsetsCache = ((i32, i32), bool, Safe, u32);

#[cfg(target_os = "android")]
fn system_insets(ctx: &egui::Context, bars_hidden: bool) -> Safe {
    let Some(phone) = crate::android_system::phone() else {
        return Safe::default();
    };
    let ppp = ctx.pixels_per_point().max(0.1);
    // The insets change with the window (a rotation, the keyboard), not
    // with the frame: they are read again only when the surface changes.
    // Hiding or showing the bars changes them too without touching the
    // surface, so the request is part of the key, and a reading that
    // still disagrees with it is not kept: the bars take a frame or two
    // to follow, and a stale status bar height would keep the top bar
    // pushed down for as long as the phone lies on its side.
    let key = Id::new("mobile_insets");
    let size = ctx.content_rect().size() * ppp;
    let px = (size.x.round() as i32, size.y.round() as i32);
    let mut waited = 0;
    if let Some((seen, hidden, got, frames)) = ctx.data(|d| d.get_temp::<InsetsCache>(key)) {
        if seen == px && hidden == bars_hidden {
            // The bars agree with the request (a hidden status bar for a
            // phone on its side, a shown one for a phone standing), or
            // they were given their frames to: either way the reading
            // stands until the surface or the request changes again.
            if got.status_bar_visible != bars_hidden || frames >= BAR_SETTLE_FRAMES {
                return got;
            }
            waited = frames;
        }
    }
    let sdk = winit::platform::android::activity::AndroidApp::sdk_version();
    let safe = match crate::android_system::read(phone.activity_as_ptr(), phone.vm_as_ptr(), sdk) {
        Some(i) => Safe {
            top: i.top as f32 / ppp,
            bottom: i.bottom as f32 / ppp,
            left: i.left as f32 / ppp,
            right: i.right as f32 / ppp,
            status_bar_visible: i.status_bar_visible,
            cutout_top: i.cutout_top as f32 / ppp,
        },
        None => Safe::default(),
    };
    let settled = safe.status_bar_visible != bars_hidden;
    let safe = edge_to_edge(safe, bars_hidden);
    ctx.data_mut(|d| {
        d.insert_temp(
            key,
            (px, bars_hidden, safe, if settled { 0 } else { waited + 1 }),
        )
    });
    safe
}

/// How tall the phone's keyboard is right now, in points; zero while it is
/// down, and on every system that has no keyboard of its own.
fn ime_height(ctx: &egui::Context) -> f32 {
    #[cfg(target_os = "android")]
    if let Some(phone) = crate::android_system::phone() {
        let sdk = winit::platform::android::activity::AndroidApp::sdk_version();
        let px = crate::android_system::ime_bottom(
            phone.activity_as_ptr(),
            phone.vm_as_ptr(),
            sdk,
        );
        return px.max(0) as f32 / ctx.pixels_per_point().max(0.1);
    }
    #[cfg(not(target_os = "android"))]
    let _ = ctx;
    0.0
}

/// A phone on its side draws edge to edge: the system bars are gone, and
/// the camera cutout sits in the middle of a short edge, with the top bar
/// and the rails running straight past it. No side room is kept for it, so
/// no icon is pushed in from the edge. A phone standing up keeps the room
/// the bars and the cutout take.
#[cfg(any(target_os = "android", test))]
fn edge_to_edge(safe: Safe, land: bool) -> Safe {
    if land {
        Safe {
            left: 0.0,
            right: 0.0,
            ..safe
        }
    } else {
        safe
    }
}

#[cfg(not(target_os = "android"))]
fn system_insets(_ctx: &egui::Context, _bars_hidden: bool) -> Safe {
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

/// How much of the window the system bars take, in points: `(top, bottom,
/// left, right)`. Zero everywhere but a phone, whose interface draws edge to
/// edge and so has to keep its own menus clear of them.
pub fn bars(ctx: &egui::Context) -> (f32, f32, f32, f32) {
    #[cfg(target_os = "android")]
    {
        let safe = system_insets(ctx, landscape(ctx.content_rect()));
        (safe.top, safe.bottom, safe.left, safe.right)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = ctx;
        (0.0, 0.0, 0.0, 0.0)
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
    note_system_theme(&ctx);
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
    let inset = system_insets(&ctx, land);

    // What a panel asked for on the frame before: it runs here, after the
    // card that says it is being made has been drawn, so the phone shows
    // the card while the work takes the frame.
    if let Some(job) = job_take(&ctx) {
        match job {
            Job::Arrange {
                width,
                height,
                gap,
                rotate,
                irregular,
            } => app.arrange_selection(width, height, gap, rotate, irregular),
            Job::Contour {
                expand,
                detail,
                smooth,
                white,
                clip,
            } => contour_run(app, expand, detail, smooth, white, clip),
            Job::MattingColour { settings } => {
                app.apply_to_bitmaps("Remove Background", move |img, _| {
                    crate::bg_remove::remove_background(img, &settings, 1.0)
                });
            }
            Job::MergeDocument { print } => app.create_merged_document(print),
        }
        ctx.data_mut(|d| d.remove::<String>(Id::new("mobile_busy")));
        ctx.request_repaint();
    }

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
            show_tools(&ctx, gi, anchor, false);
        }
    }

    // The back gesture of the phone arrives as Escape: it puts away the
    // layer in front — the sheet, or the file list that stands for a
    // dialog — while a prompt window of its own keeps the key.
    if matches!(app.dialog, crate::ui::dialogs::Dialog::None) {
        if open != Sheet::None
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            open = match open {
                Sheet::MenuPage(_) => Sheet::Drawer,
                Sheet::SettingsPage(_) => Sheet::Settings,
                _ => Sheet::None,
            };
        } else if ctx
            .data(|d| d.get_temp::<(usize, Rect, bool)>(Id::new("mobile_tool_pop")))
            .is_some()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            ctx.data_mut(|d| d.remove::<(usize, Rect, bool)>(Id::new("mobile_tool_pop")));
        }
    }

    top_bar(app, ui, &m3, inset, &mut open);
    // The phone's keyboard lies over the bottom of the window without the
    // window shrinking. A band of its height, taken first, is what keeps
    // the whole bottom of the interface above the keyboard: the bars and
    // the strip of properties all move up together, so nothing is left
    // under the keyboard and the strip keeps its place among them. Taking
    // the band after the bars left them underneath it, and the strip alone
    // floating over the keyboard, which is not a bar that can be typed in.
    let ime = ime_height(&ctx);
    // The properties of what is selected sit above the bars, and take no
    // room at all while nothing is selected.
    let strip_h = if land {
        // On its side the phone keeps the tools down the left edge and
        // the destinations down the right one, out of the drawing's way.
        keyboard_gap(ui, ime);
        tools_rail(app, ui, &m3, inset);
        nav_rail(app, ui, &m3, inset, &mut open);
        selection_strip(app, ui, &m3, open)
    } else {
        keyboard_gap(ui, ime);
        nav_bar(app, ui, &m3, inset.bottom, &mut open);
        tools_bar(app, ui, &m3);
        selection_strip(app, ui, &m3, open)
    };
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
                ai_chip(app, ui, &m3, full, &mut open);
            });
    }

    // The bar above the tools, or above the sheet that covers them; on
    // its side the sheet is on the right, so it stays at the bottom.
    let card_rect = ctx.data(|d| d.get_temp::<Rect>(Id::new("mobile_sheet_rect")));
    let floor = if open != Sheet::None && open != Sheet::Palette && !land {
        match card_rect {
            Some(rect) => rect.top() - 8.0,
            None => window.bottom() - inset.bottom,
        }
    } else if land {
        window.bottom() - inset.bottom
    } else {
        window.bottom() - inset.bottom - NAV_H - TOOLS_H - strip_h - ime
    };
    snackbar(app, ui, floor);
    if open == Sheet::Palette {
        palette_float(app, &ctx, window, &m3, &mut open, strip_h, ime);
    } else {
        // The card is not the one in front: the next opening starts over.
        ctx.data_mut(|d| d.remove::<u64>(Id::new("mobile_palette_since")));
        ctx.data_mut(|d| d.remove::<Rect>(Id::new("mobile_sheet_rect")));
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
    // The panel has been drawn, so a button in it that asked for work has
    // had its say: nothing stays open while something is being made, and
    // the card in the middle is the only thing on screen.
    let busy = busy_what(&ctx, app);
    if busy.is_some() {
        open = Sheet::None;
    }
    if let Some(what) = &busy {
        busy_card(&ctx, window, &m3, what);
        // A ring that does not turn is a picture of a ring.
        ctx.request_repaint();
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

/// Work a panel asked for, done on the frame after the one that asked: the
/// card that says something is being made is drawn first, so the phone
/// shows it rather than a frozen picture of the panel it came from.
#[derive(Clone, Debug)]
pub(crate) enum Job {
    Arrange {
        width: f64,
        height: f64,
        gap: f64,
        rotate: bool,
        irregular: bool,
    },
    Contour {
        expand: f64,
        detail: f64,
        smooth: f64,
        white: bool,
        clip: bool,
    },
    MattingColour {
        settings: crate::bg_remove::BgSettings,
    },
    MergeDocument {
        print: bool,
    },
}

/// Ask for `job`, and for the card that says `what` is being made.
pub(crate) fn defer(ctx: &egui::Context, job: Job, what: &str) {
    ctx.data_mut(|d| {
        d.insert_temp(Id::new("mobile_job"), job);
        d.insert_temp(Id::new("mobile_busy"), what.to_string());
    });
    ctx.request_repaint();
}

/// The work a panel asked for, taken off the queue. `remove` only drops a
/// value, so it is read first and then dropped.
fn job_take(ctx: &egui::Context) -> Option<Job> {
    let job = ctx.data(|d| d.get_temp::<Job>(Id::new("mobile_job")));
    if job.is_some() {
        ctx.data_mut(|d| d.remove::<Job>(Id::new("mobile_job")));
    }
    job
}

/// What is being made right now, if anything: the word a panel asked for,
/// or one of the jobs that keep their own progress.
fn busy_what(ctx: &egui::Context, app: &App) -> Option<String> {
    if let Some(what) = ctx.data(|d| d.get_temp::<String>(Id::new("mobile_busy"))) {
        return Some(what);
    }
    if app.ai_image.busy() {
        return Some(tr("aigen.in_progress"));
    }
    if ctx
        .data(|d| d.get_temp::<(usize, usize)>(Id::new("matting_progress")))
        .is_some()
    {
        return Some(tr("mobile.working"));
    }
    None
}

/// The card in the middle of the screen while something is being made: a
/// ring that turns, and a word for what it is. Nothing else is in reach
/// while it shows, so it covers the panels as well as the drawing.
fn busy_card(ctx: &egui::Context, screen: Rect, m3: &M3, what: &str) {
    egui::Area::new(Id::new("mobile_busy_card"))
        .order(egui::Order::Foreground)
        .pivot(egui::Align2::CENTER_CENTER)
        .fixed_pos(screen.center())
        .interactable(false)
        .show(ctx, |ui| {
            Frame::new()
                .fill(m3.surface)
                .corner_radius(20.0)
                .shadow(crate::theme::shadow(egui::epaint::Shadow {
                    offset: [0, 8],
                    blur: 32,
                    spread: 0,
                    color: Color32::from_black_alpha(90),
                }))
                .inner_margin(egui::Margin::symmetric(30, 24))
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add(egui::Spinner::new().size(44.0).color(m3.primary));
                        ui.add_space(12.0);
                        ui.label(egui::RichText::new(what).size(14.0).color(m3.on));
                    });
                });
        });
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

pub fn set_sheet(ctx: &egui::Context, s: Sheet) {
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
            s.visuals.window_corner_radius = CornerRadius::same(14);
            s.visuals.menu_corner_radius = CornerRadius::same(10);
            s.visuals.popup_shadow = egui::epaint::Shadow {
                offset: [0, 3],
                blur: 10,
                spread: 0,
                color: Color32::from_black_alpha(44),
            };
        });
    }
}

// ---------------------------------------------------------------------------
// The bars
// ---------------------------------------------------------------------------

/// Top padding for the top app bar, drawer, and headers.
///
/// A phone on its side asks for the system bars to go: with the status bar
/// hidden the bar sits on the very top edge of the screen, and the only
/// room it still owes is what the camera cutout itself takes (nothing at
/// all for a notch in the middle of the edge). Only a status bar that is
/// still shown earns the few extra points that clear it. A phone standing
/// up uses the natural safe inset.
fn top_padding(safe: Safe, land: bool) -> f32 {
    if land {
        if safe.status_bar_visible && safe.top > safe.cutout_top {
            safe.top + 6.0
        } else {
            safe.cutout_top
        }
    } else {
        safe.top
    }
}

fn top_bar(app: &mut App, ui: &mut Ui, m3: &M3, safe: Safe, open: &mut Sheet) {
    let screen = ui.ctx().content_rect();
    let land = landscape(screen);
    let top_pad = top_padding(safe, land);
    egui::Panel::top("mobile_top")
        .exact_size(TOP_H + top_pad)
        .resizable(false)
        .show_separator_line(false)
        .frame(Frame::new().fill(m3.surface))
        .show(ui, |ui| {
            ui.add_space(top_pad);
            ui.horizontal_centered(|ui| {
                ui.add_space(4.0 + safe.left);
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
                let room = (ui.available_width() - 2.0 * (TOUCH + gap) - 4.0 - safe.right).max(48.0);
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
                if safe.right > 0.0 {
                    ui.add_space(safe.right);
                }
            });
        });
}

/// The room the phone's keyboard takes, between the bars and the strip of
/// properties above them: nothing is drawn there, and the keyboard covers
/// it.
fn keyboard_gap(ui: &mut Ui, ime: f32) {
    if ime <= 0.5 {
        return;
    }
    egui::Panel::bottom("mobile_selection_gap")
        .exact_size(ime)
        .resizable(false)
        .show_separator_line(false)
        .frame(Frame::new())
        .show(ui, |_| {});
}

/// The properties of what is selected, as a strip above the tools: a text
/// brings its font and size, a connector the way it runs, a shape its
/// outline, and the strip is swiped sideways for the rest of them. Nothing
/// selected means no strip, and the drawing keeps the room. Returns the
/// height it took, so the snackbar and the floating card stay above it.
fn selection_strip(app: &mut App, ui: &mut Ui, m3: &M3, open: Sheet) -> f32 {
    if open != Sheet::None || !app.drawing_shown() {
        return 0.0;
    }
    if app.selected_rc().is_empty() && app.selected_guides.is_empty() {
        return 0.0;
    }
    // The bar's own drawing, a flat band the width of the screen: the
    // phone's finger-sized controls would make its rows taller than the
    // bar's height, which both pushes it up and leaves its icons out of
    // line, so one row height is set for all of them.
    let body = |app: &mut App, ui: &mut Ui| {
        egui::ScrollArea::horizontal()
            .id_salt("mobile_selection_scroll")
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                Frame::new()
                    .inner_margin(egui::Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        ui.spacing_mut().interact_size.y = 22.0;
                        ui.spacing_mut().button_padding = Vec2::new(6.0, 1.0);
                        crate::ui::toolbar::property_bar(app, ui);
                    });
            });
    };
    let shown = egui::Panel::bottom("mobile_selection")
        .min_size(SELECTION_H)
        .resizable(false)
        .show_separator_line(false)
        .frame(Frame::new().fill(m3.container))
        .show(ui, |ui| body(app, ui));
    shown.response.rect.height()
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
            let was_holding = ui.ctx().data(|d| {
                d.get_temp::<(usize, Rect, bool)>(Id::new("mobile_tool_pop"))
                    .map(|(gi, _, held)| (gi, held))
            });
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
                            let is_held_group = was_holding == Some((gi, true));
                            if resp.clicked() && !is_held_group {
                                pick = Some(shown);
                            }
                            // Held, the group's other tools come out to be
                            // slid onto: a tap alone never opens a window.
                            if holding(ui, &resp) && group.tools.len() > 1 {
                                pop = Some((gi, resp.rect, true));
                            }
                        }
                        ui.add_space(6.0);
                    });
                });
            if let Some(tool) = pick {
                app.set_tool(tool);
                hide_tools(ui.ctx());
            }
            if let Some((gi, rect, held)) = pop {
                show_tools(ui.ctx(), gi, rect, held);
            }
        });
}

/// Remember the group whose tools float over the drawing, and where its
/// button is. `held` says the finger is still on that button: letting go
/// over one of the tools picks it, and letting go anywhere else leaves the
/// row up to be tapped, as a phone's own tool rows do.
fn show_tools(ctx: &egui::Context, gi: usize, anchor: Rect, held: bool) {
    ctx.data_mut(|d| d.insert_temp(Id::new("mobile_tool_pop"), (gi, anchor, held)));
}

/// Put the row away.
fn hide_tools(ctx: &egui::Context) {
    ctx.data_mut(|d| d.remove::<(usize, Rect, bool)>(Id::new("mobile_tool_pop")));
}

/// The floating row of a group's tools: no window, no scrim — a strip that
/// appears over the drawing beside the button, and the tool under the
/// finger highlighted while it slides.
fn tool_pop(app: &mut App, ctx: &egui::Context, m3: &M3) {
    let key = Id::new("mobile_tool_pop");
    let Some((gi, anchor, held)) = ctx.data(|d| d.get_temp::<(usize, Rect, bool)>(key)) else {
        return;
    };
    let clear = |ctx: &egui::Context| ctx.data_mut(|d| d.remove::<(usize, Rect, bool)>(key));
    let Some(group) = crate::tools::GROUPS.get(gi).filter(|g| g.tools.len() > 1) else {
        clear(ctx);
        return;
    };
    // Each tool with its name under it, as in the strip: the icons alone
    // do not say which is which.
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
    let raw_w: f32 = cells.iter().map(|(_, _, w)| w).sum();
    let screen = ctx.content_rect();
    let land = screen.width() > screen.height();
    let max_avail = (screen.width() - 16.0 - pad * 2.0).max(cells.len() as f32 * 36.0);
    let scale = if raw_w > max_avail && raw_w > 0.0 {
        max_avail / raw_w
    } else {
        1.0
    };
    let width = raw_w * scale + pad * 2.0;
    let height = cell_h + pad * 2.0;
    let font = egui::FontId::proportional(if scale < 0.88 {
        9.0
    } else if scale < 0.95 {
        9.8
    } else {
        10.5
    });
    let (at_x, at_y) = if land && anchor.center().x < screen.width() * 0.5 {
        // Landscape left rail: float to the right of the button
        let x = anchor.right() + 8.0;
        let y = (anchor.center().y - height / 2.0)
            .clamp(8.0, (screen.bottom() - height - 8.0).max(8.0));
        (x, y)
    } else {
        // Portrait bottom bar (or general): float above the button
        let mut y = anchor.top() - height - 10.0;
        if y < 8.0 {
            y = anchor.bottom() + 10.0;
        }
        let x = (anchor.center().x - width / 2.0)
            .clamp(8.0, (screen.right() - width - 8.0).max(8.0));
        (x, y)
    };
    let at = egui::pos2(at_x, at_y);
    let pointer = ctx.pointer_latest_pos();
    let mut under = None;
    let mut tapped = None;
    let row = egui::Area::new(Id::new("mobile_tool_pop_area"))
        .order(egui::Order::Tooltip)
        .fixed_pos(at)
        // While the finger holds the button the row takes no input of its
        // own (the press belongs to the button, and the row only says what
        // is under the finger); left up, it is tapped like any menu.
        .interactable(!held)
        .show(ctx, |ui| {
            Frame::new()
                .fill(m3.container)
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
                        for (tool, name, orig_w) in &cells {
                            let w = *orig_w * scale;
                            let sense = if held {
                                Sense::hover()
                            } else {
                                Sense::click()
                            };
                            let (rect, resp) =
                                ui.allocate_exact_size(Vec2::new(w, cell_h), sense);
                            let hit_rect = Rect::from_x_y_ranges(
                                rect.x_range(),
                                (rect.top() - 12.0)..=(rect.bottom() + 12.0),
                            );
                            let hot = if held {
                                pointer.is_some_and(|p| hit_rect.contains(p))
                            } else {
                                resp.hovered()
                            };
                            let body_w = (rect.width() - 4.0).clamp(28.0, TOUCH - 8.0);
                            let body = Rect::from_center_size(
                                egui::pos2(rect.center().x, rect.top() + 24.0),
                                Vec2::new(body_w, 46.0),
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
                            ui.painter().with_clip_rect(rect).galley(
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
                            if resp.clicked() {
                                tapped = Some(*tool);
                            }
                        }
                    });
                });
        });
    if held {
        // Letting go over one of them picks it; letting go anywhere else
        // leaves the row up, to be tapped without holding the button again.
        if ctx.input(|i| i.pointer.any_released()) {
            match under {
                Some(tool) => {
                    app.set_tool(tool);
                    clear(ctx);
                }
                None => show_tools(ctx, gi, anchor, false),
            }
        }
    } else if let Some(tool) = tapped {
        app.set_tool(tool);
        clear(ctx);
    } else if ctx.input(|i| i.pointer.any_pressed()) {
        // A press anywhere else puts the row away.
        let outside = ctx
            .pointer_latest_pos()
            .is_none_or(|p| !row.response.rect.contains(p));
        if outside {
            clear(ctx);
        }
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
    let chip_key = Id::new("mobile_zoom_held");
    let is_held = holding(ui, &resp);
    let was_held: bool = ui.ctx().data(|d| d.get_temp(chip_key)).unwrap_or(false);
    ui.ctx().data_mut(|d| d.insert_temp(chip_key, is_held));
    let long_pressed = is_held && !was_held;

    if resp.clicked() || long_pressed {
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

// ---------------------------------------------------------------------------
// The image tools at the left edge
// ---------------------------------------------------------------------------

/// What the three panels hold, kept while they are closed.
fn ai_state(ctx: &egui::Context) -> crate::ai_tools::AiTools {
    ctx.data(|d| d.get_temp(Id::new("ai_tools_state")))
        .unwrap_or_default()
}

fn save_ai_state(ctx: &egui::Context, st: &crate::ai_tools::AiTools) {
    ctx.data_mut(|d| d.insert_temp(Id::new("ai_tools_state"), st.clone()));
}

/// What a row of the AI chip's menu does: open a sheet, or act on the
/// drawing at once.
#[derive(Clone, Copy)]
enum AiMenu {
    Sheet(Sheet),
    Arrange,
    Group,
}

/// The chip at the left edge over the drawing: the image tools whose models
/// the project's bucket serves, and the two ways of tidying a set of objects
/// they leave behind. A tap opens them just above it, and a row either opens
/// its own sheet or works on what is selected.
fn ai_chip(app: &mut App, ui: &mut Ui, m3: &M3, full: Rect, open: &mut Sheet) {
    let rect = Rect::from_min_max(
        egui::pos2(full.left() + 16.0, full.bottom() - 52.0),
        egui::pos2(full.left() + 92.0, full.bottom() - 16.0),
    );
    let resp = ui.interact(rect, Id::new("mobile_ai_chip"), Sense::click());
    paint_chip(ui, m3, rect, &tr("ai.tools"), resp.hovered());
    let menu_key = Id::new("mobile_ai_menu");
    let mut shown: bool = ui.ctx().data(|d| d.get_temp(menu_key)).unwrap_or(false);
    if resp.clicked() {
        shown = !shown;
        ui.ctx().data_mut(|d| d.insert_temp(menu_key, shown));
    }
    if !shown {
        return;
    }
    let items = [
        (AiMenu::Sheet(Sheet::AiGenerate), "docker.ai_generate"),
        (AiMenu::Sheet(Sheet::Matting), "ai.matting"),
        (AiMenu::Sheet(Sheet::Variable), "ai.variable"),
        (AiMenu::Sheet(Sheet::Trace), "ai.trace"),
        (AiMenu::Arrange, "docker.smart_arrange"),
        (AiMenu::Group, "docker.smart_group"),
    ];
    let row = 46.0;
    let height = row * items.len() as f32 + 12.0;
    let top = (rect.top() - height - 8.0).max(full.top() + 8.0);
    let ctx = ui.ctx().clone();
    let menu = egui::Area::new(Id::new("mobile_ai_menu_area"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(rect.left(), top))
        .show(&ctx, |ui| {
            // The same card the tool strip opens with: the same corner, the
            // same shadow, and rows that light up the way its buttons do.
            Frame::new()
                .fill(m3.container)
                .corner_radius(20.0)
                .shadow(crate::theme::shadow(egui::epaint::Shadow {
                    offset: [0, 6],
                    blur: 24,
                    spread: 0,
                    color: Color32::from_black_alpha(70),
                }))
                .inner_margin(egui::Margin::same(6))
                .show(ui, |ui| {
                    ui.set_width(140.0);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for (action, label) in items {
                        let (at, resp) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), row),
                            Sense::click(),
                        );
                        let hot = resp.hovered() || resp.is_pointer_button_down_on();
                        if hot {
                            ui.painter().rect_filled(at, 14.0, m3.primary_container);
                        }
                        ui.painter().text(
                            egui::pos2(at.left() + 14.0, at.center().y),
                            egui::Align2::LEFT_CENTER,
                            tr(label),
                            egui::FontId::proportional(15.0),
                            if hot { m3.primary } else { m3.on },
                        );
                        if resp.clicked() {
                            match action {
                                AiMenu::Sheet(sheet) => *open = sheet,
                                AiMenu::Arrange => *open = Sheet::Arrange,
                                AiMenu::Group => app.smart_group(),
                            }
                            ui.ctx().data_mut(|d| d.insert_temp(menu_key, false));
                        }
                    }
                });
        });
    // A touch anywhere but on the menu or on the chip itself puts it away,
    // the way a menu over a drawing behaves everywhere else.
    if ctx.input(|i| i.pointer.any_pressed()) {
        if let Some(at) = ctx.input(|i| i.pointer.interact_pos()) {
            if !menu.response.rect.contains(at) && !rect.contains(at) {
                ctx.data_mut(|d| d.insert_temp(menu_key, false));
            }
        }
    }
}

/// One model of a panel: the choice, what it is doing, and the way to fetch
/// it. The choice is a row of its own, a finger tall.
fn model_choice(
    ui: &mut Ui,
    m3: &M3,
    model: &crate::ai_tools::Model,
    chosen: bool,
    pick: &mut usize,
    index: usize,
) {
    let ctx = ui.ctx().clone();
    let width = ui.available_width();
    let text = egui::RichText::new(tr(model.title))
        .size(14.0)
        .color(if chosen { m3.primary } else { m3.on });
    let mut button = egui::Button::new(text)
        .corner_radius(12.0)
        .min_size(Vec2::new(width, 44.0));
    button = if chosen {
        button.fill(m3.primary_container)
    } else {
        button.fill(m3.container).stroke(Stroke::new(1.0, m3.outline))
    };
    if ui.add(button).clicked() {
        *pick = index;
    }
    if let Some((done, total)) = crate::ai_tools::progress(&ctx, model) {
        let share = if total > 0 {
            done as f32 / total as f32
        } else {
            0.0
        };
        ui.add(
            egui::ProgressBar::new(share)
                .text(format!("{} / {}", crate::ai_tools::size_text(done), crate::ai_tools::size_text(total))),
        );
    } else if crate::ai_tools::ready(model) {
        ui.label(
            egui::RichText::new(format!(
                "{} \u{00B7} {}",
                tr("ai.model_ready"),
                crate::ai_tools::size_text(crate::ai_tools::size_on_disk(model))
            ))
            .size(11.5)
            .color(m3.variant),
        );
    } else {
        if let Some(e) = crate::ai_tools::failed(&ctx, model) {
            ui.label(
                egui::RichText::new(elide(&e, 60))
                    .size(11.0)
                    .color(m3.variant),
            );
        }
        if action_chip(ui, m3, &tr("ai.download_model")).clicked() {
            crate::ai_tools::download(&ctx, model);
        }
    }
    ui.add_space(4.0);
}

/// The way an identification photo is made: the size of the paper, the
/// colour behind the person, and how much of the height the person takes.
fn id_photo_settings(ui: &mut Ui, how: &mut crate::matting::IdPhoto) {
    use crate::matting::ID_SIZES;
    let current = ID_SIZES.get(how.size).unwrap_or(&ID_SIZES[0]);
    let mut picked = how.size;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(tr("ai.id_photo_size")).size(13.0));
        egui::ComboBox::from_id_salt("id_photo_size")
            .selected_text(format!(
                "{} {}x{} mm",
                tr(current.key),
                current.width_mm,
                current.height_mm
            ))
            .show_ui(ui, |ui| {
                for (i, size) in ID_SIZES.iter().enumerate() {
                    ui.selectable_value(
                        &mut picked,
                        i,
                        format!(
                            "{} {}x{} mm",
                            tr(size.key),
                            size.width_mm,
                            size.height_mm
                        ),
                    );
                }
            });
    });
    how.size = picked;
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new(tr("ai.id_photo_background")).size(13.0));
        for (key, rgb) in [
            ("idphoto.white", [255u8, 255, 255, 255]),
            ("idphoto.blue", [67, 142, 219, 255]),
            ("idphoto.red", [214, 0, 0, 255]),
        ] {
            let on = how.background == rgb;
            if ui.add(egui::Button::new(tr(key)).selected(on)).clicked() {
                how.background = rgb;
            }
        }
        let mut custom = egui::Color32::from_rgba_unmultiplied(
            how.background[0],
            how.background[1],
            how.background[2],
            how.background[3],
        );
        if ui.color_edit_button_srgba(&mut custom).changed() {
            how.background = custom.to_array();
        }
    });
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(tr("ai.id_photo_head")).size(13.0));
        ui.add(egui::Slider::new(&mut how.head_margin, 0.0..=0.25).fixed_decimals(2));
    });
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(tr("ai.id_photo_fill")).size(13.0));
        ui.add(egui::Slider::new(&mut how.fill, 0.5..=1.0).fixed_decimals(2));
    });
}

/// Matting: the background of the selected pictures goes, by the colours
/// learnt from their border or by the model picked.
fn matting_sheet(app: &mut App, ui: &mut Ui, m3: &M3) {
    let ctx = ui.ctx().clone();
    // A mask that finished since the last frame goes in first.
    matting_apply(app, &ctx);
    let mut st = ai_state(&ctx);
    ui.label(
        egui::RichText::new(tr("ai.matting_hint"))
            .size(12.5)
            .color(m3.variant),
    );
    ui.add_space(6.0);
    property_label(ui, m3, &tr("ai.model"));
    for (i, model) in crate::ai_tools::MATTING.iter().enumerate() {
        model_choice(ui, m3, model, st.matting == i, &mut st.matting, i);
    }
    if st.matting == 0 {
        // The portrait model was trained on people: it will cut a bee out
        // of a photo of a bee only roughly, and saying so saves the hunt.
        ui.label(
            egui::RichText::new(tr("ai.matting_portrait_only"))
                .size(11.5)
                .color(m3.variant),
        );
    }
    ui.add_space(6.0);
    property_label(ui, m3, &tr("ai.matting_cut"));
    ui.checkbox(&mut st.matting_split, tr("ai.matting_split"));
    ui.checkbox(&mut st.matting_trim, tr("ai.matting_trim"));
    if st.matting == 0 {
        // An identification photo is made from the portrait model: its
        // subject is a person, and the photo has rules of its own about
        // the size of the paper and what stands behind the person.
        ui.add_space(6.0);
        property_label(ui, m3, &tr("ai.id_photo"));
        ui.checkbox(&mut st.id_photo_on, tr("ai.id_photo_make"));
        if st.id_photo_on {
            id_photo_settings(ui, &mut st.id_photo);
        }
    }
    ui.add_space(6.0);
    property_label(ui, m3, &tr("ai.matting_settings"));
    let mut s = st.matting_settings.clone();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(tr("dialog.tolerance")).size(13.0));
        ui.add(egui::Slider::new(&mut s.tolerance, 0.0..=100.0));
    });
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(tr("dialog.edge_softness")).size(13.0));
        ui.add(egui::Slider::new(&mut s.softness, 0.0..=20.0).suffix(" px"));
    });
    ui.checkbox(&mut s.contiguous, tr("dialog.bg_contiguous"));
    ui.checkbox(&mut s.despeckle, tr("dialog.bg_despeckle"));
    ui.checkbox(&mut s.clean_edges, tr("dialog.bg_clean_edges"));
    st.matting_settings = s;
    ui.add_space(10.0);
    if let Some((done, total)) =
        ctx.data(|d| d.get_temp::<(usize, usize)>(Id::new("matting_progress")))
    {
        let share = if total > 0 {
            done as f32 / total as f32
        } else {
            0.0
        };
        ui.add(egui::ProgressBar::new(share).text(format!("{done} / {total}")));
        ui.add_space(6.0);
    }
    if let Some(e) = ctx.data(|d| d.get_temp::<String>(Id::new("matting_error"))) {
        ui.label(egui::RichText::new(elide(&e, 80)).size(11.5).color(m3.variant));
        ui.add_space(6.0);
    }
    let model = crate::ai_tools::MATTING.get(st.matting).copied();
    let ready = model.map(|m| crate::ai_tools::ready(&m)).unwrap_or(false);
    if !ready {
        // No model on the phone: the colours learnt from the picture's
        // border are what takes the background out, and that is worth
        // saying, since it fails on a busy edge where a model would not.
        ui.label(
            egui::RichText::new(tr("ai.matting_colour"))
                .size(11.5)
                .color(m3.variant),
        );
        ui.add_space(6.0);
    }
    if action_chip(ui, m3, &tr("ai.apply")).clicked() {
        match model {
            Some(m) if ready => matting_start(app, &ctx, &m, &st),
            _ => {
                let settings = st.matting_settings.clone();
                defer(&ctx, Job::MattingColour { settings }, &tr("mobile.working"));
            }
        }
    }
    save_ai_state(&ctx, &st);
}

/// Take the background out of the selected pictures with the model chosen,
/// on a thread of its own: a model over a picture takes seconds, and the
/// sheet has to stay live while it runs.
fn matting_start(
    app: &mut App,
    ctx: &egui::Context,
    model: &crate::ai_tools::Model,
    how: &crate::ai_tools::AiTools,
) {
    let shapes = app.bitmap_shapes();
    if shapes.is_empty() {
        app.status = tr("ai.need_picture");
        return;
    }
    let Some(path) = crate::ai_tools::path(model.files[0]) else {
        app.status = tr("ai.model_missing");
        return;
    };
    if ctx
        .data(|d| d.get_temp::<(usize, usize)>(Id::new("matting_progress")))
        .is_some()
    {
        return;
    }
    let total = shapes.len();
    ctx.data_mut(|d| {
        d.remove::<Vec<(tracedraw_core::ShapeId, Vec<crate::matting::Piece>)>>(Id::new(
            "matting_done",
        ));
        d.remove::<String>(Id::new("matting_error"));
        d.insert_temp(Id::new("matting_progress"), (0usize, total));
    });
    let key = model.key.to_string();
    let split = how.matting_split;
    let trim = how.matting_trim;
    // An identification photo is made from the portrait model alone: the
    // general one does not know where a person's head is.
    let id_photo = if how.id_photo_on && !key.starts_with("rmbg") {
        Some(how.id_photo.clone())
    } else {
        None
    };
    let worker = ctx.clone();
    std::thread::spawn(move || {
        let mut done: Vec<(tracedraw_core::ShapeId, Vec<crate::matting::Piece>)> = Vec::new();
        let mut error = None;
        for (i, (id, _rect, _, _, png)) in shapes.into_iter().enumerate() {
            let Some(img) = crate::bitmap_fx::decode(&png) else {
                continue;
            };
            let mask = match crate::matting::matte(&img, &path, &key) {
                Ok(mask) => mask,
                Err(e) => {
                    error = Some(e);
                    break;
                }
            };
            let (w, h) = img.dimensions();
            let whole = (0, 0, w, h);
            let pieces = match &id_photo {
                Some(how) => {
                    let cut = crate::matting::applied(&img, &mask);
                    match crate::matting::id_photo(&cut, how) {
                        Ok(photo) => vec![crate::matting::Piece {
                            img: photo,
                            bounds: whole,
                        }],
                        Err(e) => {
                            error = Some(e);
                            break;
                        }
                    }
                }
                // A picture whose background is already transparent has
                // its separate subjects taken apart, each cut to what it
                // fills: a scanned sheet of several pieces becomes the
                // pieces, not one picture with holes between them.
                None if split => {
                    // A subject smaller than a two thousandth of the
                    // picture is a speck the matte left behind.
                    let min_area = (w as u64 * h as u64 / 2000).max(100) as u32;
                    crate::matting::split(&img, &mask, 8, min_area)
                }
                None if trim => vec![crate::matting::trimmed(&img, &mask, 8)],
                None => vec![crate::matting::Piece {
                    img: crate::matting::applied(&img, &mask),
                    bounds: whole,
                }],
            };
            done.push((id, pieces));
            worker.data_mut(|d| d.insert_temp(Id::new("matting_progress"), (i + 1, total)));
            worker.request_repaint();
        }
        worker.data_mut(|d| {
            d.remove::<(usize, usize)>(Id::new("matting_progress"));
            if let Some(e) = error {
                d.insert_temp(Id::new("matting_error"), e);
            }
            d.insert_temp(Id::new("matting_done"), done);
        });
        worker.request_repaint();
    });
}

/// Put the cut-out pictures where the ones they came from were.
fn matting_apply(app: &mut App, ctx: &egui::Context) {
    let Some(done) = ctx.data(|d| {
        d.get_temp::<Vec<(tracedraw_core::ShapeId, Vec<crate::matting::Piece>)>>(Id::new(
            "matting_done",
        ))
    }) else {
        return;
    };
    ctx.data_mut(|d| {
        d.remove::<Vec<(tracedraw_core::ShapeId, Vec<crate::matting::Piece>)>>(Id::new(
            "matting_done",
        ))
    });
    let mut cut = 0;
    for (id, pieces) in done {
        let Some((_, rect, w, h, _)) = app
            .bitmap_shapes()
            .into_iter()
            .find(|(shape, ..)| *shape == id)
        else {
            continue;
        };
        let per_x = rect.width() / (w.max(1) as f64);
        let per_y = rect.height() / (h.max(1) as f64);
        let mut first = true;
        for piece in pieces {
            let (bx, by, bw, bh) = piece.bounds;
            // Where the piece sat in the picture, on the page.
            let sub = tracedraw_core::geometry::Rect::new(
                rect.x0 + bx as f64 * per_x,
                rect.y0 + by as f64 * per_y,
                rect.x0 + (bx + bw) as f64 * per_x,
                rect.y0 + (by + bh) as f64 * per_y,
            );
            if first {
                // The picture keeps its place and its id: the pieces that
                // came apart from it are the new objects.
                app.replace_bitmap(id, sub, &piece.img);
                first = false;
            } else if let Some(png) = crate::bitmap_fx::encode(&piece.img) {
                let (width_px, height_px) = piece.size();
                let _ = app.new_shape(tracedraw_core::document::ShapeKind::Bitmap {
                    rect: sub,
                    width_px,
                    height_px,
                    png: png.into(),
                    fx: None,
                });
            }
            cut += 1;
        }
    }
    if cut > 0 {
        app.status = tr("ai.matting_done");
    }
}

/// The contour: the outline of the selected picture as closed paths, which
/// is what a cutter follows. It grows out of the subject by a distance in
/// millimetres, or shrinks into it when the distance is negative.
/// The smart layout: the sheet the parts are packed onto, the least room
/// left between two of them, and whether a part may be turned, or turned to
/// any angle and settled by its own outline.
fn arrange_sheet(_app: &mut App, ui: &mut Ui, m3: &M3) {
    let ctx = ui.ctx().clone();
    let mut st = ai_state(&ctx);
    property_label(ui, m3, &tr("arrange.params"));
    ui.add_space(2.0);
    let field = ((ui.available_width() - 16.0) / 3.0).max(56.0);
    egui::Grid::new("arrange_numbers")
        .num_columns(3)
        .spacing([8.0, 2.0])
        .show(ui, |ui| {
            for key in ["resample.width", "resample.height", "arrange.gap"] {
                ui.add_sized(
                    [field, 16.0],
                    egui::Label::new(egui::RichText::new(tr(key)).size(11.5).color(m3.variant)),
                );
            }
            ui.end_row();
            for value in [
                &mut st.arrange_width_mm,
                &mut st.arrange_height_mm,
                &mut st.arrange_gap_mm,
            ] {
                ui.add_sized(
                    [field, 34.0],
                    egui::DragValue::new(value)
                        .speed(1.0)
                        .range(1.0..=20000.0)
                        .fixed_decimals(0),
                );
            }
            ui.end_row();
        });
    ui.add_space(10.0);
    ui.checkbox(&mut st.arrange_rotate, tr("arrange.rotate"));
    ui.checkbox(&mut st.arrange_irregular, tr("arrange.irregular"));
    ui.add_space(10.0);
    if action_chip(ui, m3, &tr("arrange.run")).clicked() {
        defer(
            &ctx,
            Job::Arrange {
                width: st.arrange_width_mm,
                height: st.arrange_height_mm,
                gap: st.arrange_gap_mm,
                rotate: st.arrange_rotate,
                irregular: st.arrange_irregular,
            },
            &tr("mobile.working"),
        );
    }
    save_ai_state(&ctx, &st);
}

fn trace_sheet(_app: &mut App, ui: &mut Ui, m3: &M3, open: &mut Sheet) {
    let _ = open;
    let ctx = ui.ctx().clone();
    let mut st = ai_state(&ctx);
    ui.label(
        egui::RichText::new(tr("ai.contour_hint"))
            .size(12.5)
            .color(m3.variant),
    );
    ui.add_space(8.0);
    property_label(ui, m3, &tr("ai.contour_expand"));
    ui.add(egui::Slider::new(&mut st.contour_expand_mm, -5.0..=5.0).fixed_decimals(2));
    property_label(ui, m3, &tr("dialog.detail"));
    ui.add(egui::Slider::new(&mut st.contour_detail, 0.0..=100.0));
    property_label(ui, m3, &tr("dialog.smoothing"));
    ui.add(egui::Slider::new(&mut st.contour_smooth, 0.0..=100.0));
    ui.add_space(8.0);
    ui.checkbox(&mut st.contour_white_backing, tr("ai.contour_white_backing"));
    ui.checkbox(&mut st.contour_power_clip, tr("ai.contour_power_clip"));
    ui.add_space(10.0);
    if action_chip(ui, m3, &tr("ai.contour_run")).clicked() {
        defer(
            &ctx,
            Job::Contour {
                expand: st.contour_expand_mm,
                detail: st.contour_detail,
                smooth: st.contour_smooth,
                white: st.contour_white_backing,
                clip: st.contour_power_clip,
            },
            &tr("mobile.working"),
        );
    }
    save_ai_state(&ctx, &st);
}

/// Put the contour of the selected picture in the drawing, on the picture's
/// own layer and over it: one closed outline with its rings as subpaths, a
/// hairline to follow, filled white when a backing is asked for, and with
/// the picture put inside it when the outline is to be a container.
fn contour_run(
    app: &mut App,
    expand_mm: f64,
    detail: f64,
    smoothing: f64,
    white_backing: bool,
    power_clip: bool,
) {
    let Some((bitmap, rect, _, _, png)) = app.bitmap_shapes().into_iter().next() else {
        app.status = tr("ai.need_picture");
        return;
    };
    let Some(img) = crate::bitmap_fx::decode(&png) else {
        return;
    };
    if img.width() == 0 || img.height() == 0 {
        return;
    }
    // The rect a picture carries is where it was drawn, not where it is: a
    // move, a turn or a scale goes into the shape's transform, and the rect
    // stays as it was made. The outline follows the picture, so it takes the
    // same transform; without this a picture dragged away from the middle of
    // the page got its contour back where it was first made.
    let place = app
        .doc()
        .parent_transform(bitmap)
        .unwrap_or(tracedraw_core::geometry::Affine::IDENTITY)
        * app
            .doc()
            .shape(bitmap)
            .map(|(_, s)| s.transform)
            .unwrap_or(tracedraw_core::geometry::Affine::IDENTITY);
    // A millimetre in the drawing is this many pixels of the picture, taken
    // on the page, where the picture is now and at the size it is now.
    let page_w = (place * tracedraw_core::geometry::Point::new(rect.x1, rect.y0)
        - place * tracedraw_core::geometry::Point::new(rect.x0, rect.y0))
    .hypot();
    let mm_per_px = (page_w / img.width() as f64).max(1e-9);
    let paths = crate::trace::contour_paths(
        &img,
        expand_mm / mm_per_px,
        detail,
        smoothing,
    );
    if paths.is_empty() {
        app.status = tr("ai.contour_none");
        return;
    }
    // The picture's own layer, and after it in that layer: a contour put on
    // the layer being drawn on lands under the picture whenever that layer
    // is below it, and a cutter's line is of no use under the picture it
    // follows.
    let layer = app
        .doc()
        .all_layers()
        .find(|l| l.shapes.iter().any(|s| s.id == bitmap))
        .map(|l| l.id)
        .or_else(|| app.active_layer());
    let Some(layer) = layer else {
        return;
    };
    // One object for the whole outline, its rings as subpaths: a cutter
    // follows it as one line, and a picture put inside it is clipped by the
    // outline and its holes together, which a ring of its own could not do.
    let mut outline = tracedraw_core::geometry::BezPath::new();
    for path in &paths {
        outline.extend(crate::trace::to_rect(path, img.width(), img.height(), rect));
    }
    // The rings are on the picture as it was made; the transform is what took
    // it to the page.
    let outline = place * outline;
    let id = app.engine.new_shape_id();
    let mut shape = tracedraw_core::document::Shape::new(
        id,
        tracedraw_core::document::ShapeKind::Path {
            path: outline,
            closed: true,
        },
    );
    // The white backing is the outline itself filled white, under the
    // picture, so the picture is printed on its own white rather than on
    // whatever the page shows through it.
    shape.fill = if white_backing {
        tracedraw_core::Fill::Solid(tracedraw_core::Color::WHITE)
    } else {
        tracedraw_core::Fill::None
    };
    shape.stroke = Some(tracedraw_core::Stroke::hairline(
        tracedraw_core::Color::BLACK,
    ));
    let mut cmds = vec![tracedraw_core::Command::AddShape { layer, shape }];
    if power_clip {
        // The picture goes inside the outline, which becomes its container:
        // what is outside the cut line is not printed and not cut.
        cmds.push(tracedraw_core::Command::PlaceInside {
            contents: vec![bitmap],
            frame: id,
        });
    }
    if let Err(e) = app.engine.run_batch("Contour", &cmds) {
        app.status = e.to_string();
        return;
    }
    if power_clip {
        // The picture sits where the outline follows it, so the container
        // is one that keeps what is put in it where it is: a piece added
        // later belongs to the outline it was cut for, not to its middle.
        app.clip_frames_keeping_place.insert(id);
    }
    app.status = tr("ai.contour_done");
}

/// Variable data: the desktop's Print Merge panel, a copy of the drawing a
/// row of the table.
fn variable_sheet(app: &mut App, ui: &mut Ui, m3: &M3) {
    let _ = m3;
    crate::ui::print_merge_docker::print_merge(app, ui);
}

/// A name cut to what a narrow bar holds, with a mark that it was cut.
pub(crate) fn elide(text: &str, keep: usize) -> String {
    if text.chars().count() <= keep {
        return text.to_string();
    }
    let mut out: String = text.chars().take(keep.saturating_sub(1)).collect();
    out.push('\u{2026}');
    out
}

/// The navigation bar: the panels the desktop keeps beside the drawing,
/// each a destination that opens as a sheet.
/// The bottom bar: the four destinations, and after them the commands of
/// what is selected, which a swipe sideways brings into reach.
fn nav_bar(app: &mut App, ui: &mut Ui, m3: &M3, bottom: f32, open: &mut Sheet) {
    egui::Panel::bottom("mobile_nav")
        .exact_size(NAV_H + bottom)
        .resizable(false)
        .show_separator_line(false)
        .frame(Frame::new().fill(m3.container))
        .show(ui, |ui| {
            egui::ScrollArea::horizontal()
                .id_salt("mobile_nav_scroll")
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.horizontal_centered(|ui| {
                        ui.add_space(4.0);
                        for (which, glyph, label) in destinations() {
                            let active = *open == which;
                            if nav_item(ui, m3, glyph, &label, active, NAV_ITEM_W, NAV_H).clicked() {
                                *open = if active { Sheet::None } else { which };
                            }
                        }
                        // The commands the floating toolbar of CorelDRAW
                        // carries: what is done to the set of objects, how
                        // they line up, and what is selected by kind.
                        crate::ui::align_docker::align_part(app, ui);
                        ui.add_space(4.0);
                    });
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
            let was_holding = ui.ctx().data(|d| {
                d.get_temp::<(usize, Rect, bool)>(Id::new("mobile_tool_pop"))
                    .map(|(gi, _, held)| (gi, held))
            });
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
                            let is_held_group = was_holding == Some((gi, true));
                            if resp.clicked() && !is_held_group {
                                pick = Some(shown);
                            }
                            // Held, the group's other tools come out to be
                            // slid onto: a tap alone never opens a window.
                            if holding(ui, &resp) && group.tools.len() > 1 {
                                pop = Some((gi, resp.rect, true));
                            }
                        }
                        ui.add_space(6.0);
                    });
                });
            if let Some(tool) = pick {
                app.set_tool(tool);
                hide_tools(ui.ctx());
            }
            if let Some((gi, rect, held)) = pop {
                show_tools(ui.ctx(), gi, rect, held);
            }
            });
        });
}

/// The destinations down the right edge, for a phone on its side.
fn nav_rail(app: &mut App, ui: &mut Ui, m3: &M3, safe: Safe, open: &mut Sheet) {
    egui::Panel::right("mobile_nav")
        .exact_size(NAV_RAIL_W + safe.right)
        .resizable(false)
        .show_separator_line(false)
        .frame(Frame::new().fill(m3.container))
        .show(ui, |ui| {
            // The rail is short on a phone on its side: the destinations
            // and the commands of the selection scroll under the finger,
            // the same two groups the bar carries when it stands.
            //
            // The column is given the whole rail rather than a row's height
            // (`ui.horizontal` lays its content out in `interact_size.y`
            // and the scroll under it had room for the first icon only).
            let rail = Rect::from_min_max(
                ui.max_rect().min,
                ui.max_rect().max - Vec2::new(safe.right, 0.0),
            );
            ui.scope_builder(egui::UiBuilder::new().max_rect(rail), |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("mobile_nav_rail_scroll")
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .auto_shrink([true, false])
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.add_space(6.0);
                            for (which, glyph, label) in destinations() {
                                let active = *open == which;
                                if nav_item(ui, m3, glyph, &label, active, NAV_RAIL_W, NAV_H)
                                    .clicked()
                                {
                                    *open = if active { Sheet::None } else { which };
                                }
                            }
                            crate::ui::align_docker::align_part(app, ui);
                            ui.add_space(6.0);
                        });
                    });
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
    // How much of the screen the keyboard covers: a panel that reaches under
    // it has the field being typed in taken out of sight, so what is left is
    // what a panel is laid out in.
    let ime = ime_height(&ctx);

    if landscape(screen) {
        // A phone on its side docks the sheet on the right, between the
        // drawing and the destinations rail: a wide screen has no room to
        // spare at the bottom, and the rail stays in reach.
        let width = (screen.width() * 0.42).clamp(280.0, 440.0);
        let top = TOP_H + top_padding(safe, true);
        let rect = Rect::from_min_max(
            egui::pos2(screen.right() - safe.right - NAV_RAIL_W - width, top),
            egui::pos2(
                screen.right() - safe.right - NAV_RAIL_W,
                (screen.bottom() - safe.bottom - ime).max(top + 160.0),
            ),
        );
        let height = rect.height();
        egui::Area::new(Id::new("mobile_sheet_land"))
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
        // A card in the middle of the screen rather than a sheet against the
        // bottom. The keyboard comes up over a sheet and takes the field
        // being typed in with it; a card centred in what the keyboard leaves,
        // and shrunk to fit in it, stays where it can be read and typed in.
        let top = TOP_H + top_padding(safe, false);
        let floor = (screen.bottom() - safe.bottom - ime).max(top + 120.0);
        let room = (floor - top).max(120.0);
        let side = 12.0;
        let width = (screen.width() - side * 2.0).max(200.0);
        let mid = egui::pos2(screen.center().x, top + room * 0.5);
        let card = egui::Area::new(Id::new("mobile_sheet_port"))
            .order(egui::Order::Foreground)
            // The pivot centres the card on the middle of the room by its
            // own size, which is not known until it is laid out: it settles
            // on the frame after the one that opened it.
            .pivot(egui::Align2::CENTER_CENTER)
            .fixed_pos(mid)
            .show(&ctx, |ui| {
                Frame::new()
                    .fill(m3.surface)
                    .corner_radius(24.0)
                    .shadow(crate::theme::shadow(egui::epaint::Shadow {
                        offset: [0, 8],
                        blur: 32,
                        spread: 0,
                        color: Color32::from_black_alpha(90),
                    }))
                    .inner_margin(egui::Margin {
                        left: 0,
                        right: 0,
                        top: 8,
                        bottom: 8,
                    })
                    .show(ui, |ui| {
                        ui.set_width(width);
                        sheet_content(app, ui, which, open, m3, room - 16.0, false);
                    });
            });
        // Where the card came to rest, for what is drawn above it: a card
        // whose height follows its contents cannot be guessed at from a
        // share of the screen.
        let rect = card.response.rect;
        ctx.data_mut(|d| d.insert_temp(Id::new("mobile_sheet_rect"), rect));
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
    let head_top = ui.cursor().min.y;
    if sheet_header(ui, which, m3) {
        *open = Sheet::None;
    }
    // The generate panel keeps its button in view under its own scroll: a
    // phone should not have to scroll past the settings to reach the one
    // thing the panel is for.
    let bar = if matches!(which, Sheet::AiGenerate) {
        118.0
    } else {
        0.0
    };
    // The image tools' panels are short enough to read in one go, and a bar
    // down the side of them is a desk's furniture: a phone drags the content
    // itself, with nothing drawn over it. The lists, which can be long, keep
    // theirs.
    let bar_shown = !matches!(
        which,
        Sheet::AiGenerate | Sheet::Matting | Sheet::Variable | Sheet::Trace | Sheet::Arrange
    );
    // What is left of the card for the body: the header has been drawn, and
    // the bar a panel keeps at its foot is not the body's room. Measured off
    // the cursor rather than guessed, so a card is not cut short by a margin
    // nobody uses, and a dock that fills its side still measures right.
    let used = (ui.cursor().min.y - head_top).max(0.0);
    let body_room = (height - used - bar - 8.0).max(80.0);
    egui::ScrollArea::vertical()
        .id_salt(("mobile_sheet_scroll", sheet_title(which)))
        .max_height(body_room)
        // The card is as tall as what it holds, up to the room there is: a
        // panel of three numbers has no business covering the drawing for
        // the sake of a height chosen for the tallest panel.
        .auto_shrink([false, true])
        .scroll_bar_visibility(if bar_shown {
            egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded
        } else {
            egui::scroll_area::ScrollBarVisibility::AlwaysHidden
        })
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
                    if !bar_shown {
                        // The image tools are a short list of controls that
                        // a phone reads in one go: set closer together, so
                        // most of them fit without scrolling at all.
                        ui.spacing_mut().item_spacing.y = 6.0;
                    }
                    sheet_body(app, ui, which, m3, open);
                });
        });
    if bar > 0.0 {
        crate::ui::ai_generate_docker::ai_generate_footer(app, ui);
    }
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
        Sheet::AiGenerate => tr("docker.ai_generate"),
        Sheet::Matting => tr("ai.matting"),
        Sheet::Variable => tr("ai.variable"),
        Sheet::Trace => tr("ai.trace"),
        Sheet::Arrange => tr("docker.smart_arrange"),
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
        Sheet::AiGenerate => crate::ui::ai_generate_docker::ai_generate(app, ui),
        Sheet::Matting => matting_sheet(app, ui, m3),
        Sheet::Variable => variable_sheet(app, ui, m3),
        Sheet::Trace => trace_sheet(app, ui, m3, open),
        Sheet::Arrange => arrange_sheet(app, ui, m3),
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
    let ctx = ui.ctx().clone();
    scrim(&ctx, screen, open);
    let land = landscape(screen);
    let safe = system_insets(&ctx, land);
    let top_pad = top_padding(safe, land);
    let width = (screen.width() * 0.86).min(320.0 + safe.left);
    let area_id = if land {
        Id::new("mobile_drawer_land")
    } else {
        Id::new("mobile_drawer_port")
    };
    egui::Area::new(area_id)
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .constrain_to(screen)
        .show(&ctx, |ui| {
            ui.set_width(width);
            ui.set_max_width(width);
            ui.set_max_height(screen.height());
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
                    ui.set_max_width(width);
                    ui.set_max_height(screen.height());
                    // Clear of the status bar, as the top bar is.
                    ui.add_space(top_pad);
                    Frame::new()
                        .fill(m3.container)
                        .inner_margin(egui::Margin {
                            left: (20.0 + safe.left) as i8,
                            right: 20,
                            top: 26,
                            bottom: 20,
                        })
                        .show(ui, |ui| {
                            ui.set_width(width - 40.0 - safe.left);
                            ui.label(
                                egui::RichText::new("TraceDraw")
                                    .size(22.0)
                                    .strong()
                                    .color(m3.on),
                            );
                            // The version, and the way to a newer one beside
                            // it: Help > Updates is three taps away in the
                            // menus, and this is where a build is named.
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(crate::updates::version())
                                        .size(12.0)
                                        .color(m3.variant),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let button = egui::Button::new(
                                            egui::RichText::new(tr("updates.check_now"))
                                                .size(12.0)
                                                .color(m3.primary),
                                        )
                                        .fill(m3.high)
                                        .stroke(Stroke::new(1.0, m3.primary))
                                        .corner_radius(CornerRadius::same(18))
                                        .min_size(Vec2::new(0.0, 36.0));
                                        if ui.add(button).clicked() {
                                            app.check_for_updates(Some(ui.ctx().clone()));
                                            *open = Sheet::None;
                                        }
                                    },
                                );
                            });
                        });
                    let scroll_max_h = (screen.height() - (ui.cursor().top() - screen.top()) - safe.bottom).max(80.0);
                    egui::ScrollArea::vertical()
                        .id_salt("mobile_drawer_scroll")
                        .max_height(scroll_max_h)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.add_space(8.0);
                            let menus = crate::ui::menus::all_menus();
                            card(ui, m3, 0.0, |ui| {
                                for (i, (key, _)) in menus.iter().enumerate() {
                                    let last = i + 1 == menus.len();
                                    if drawer_row(ui, m3, &tr(key), None, last).clicked() {
                                        *open = Sheet::MenuPage(i);
                                    }
                                }
                            });
                            ui.add_space(12.0);
                            card(ui, m3, 0.0, |ui| {
                                if drawer_row(ui, m3, &tr("mobile.settings"), Some(Glyph::Gear), true)
                                    .clicked()
                                {
                                    *open = Sheet::Settings;
                                }
                            });
                            ui.add_space(12.0 + safe.bottom);
                        });
                });
        });
}

/// The colour of the tile a label gets: the same label always the same one,
/// and two rows of a list rarely the same.
fn tile_color(label: &str) -> Color32 {
    let mut h: usize = 7;
    for c in label.chars() {
        h = h.wrapping_mul(31).wrapping_add(c as usize);
    }
    TILES[h % TILES.len()]
}

/// A card of rows on the page: white, rounded, `inset` clear of the edges
/// on either side. The rows are laid out downwards whatever the layout of
/// the caller: a card is a column of rows, never a row of them.
fn card(ui: &mut Ui, m3: &M3, inset: f32, add: impl FnOnce(&mut Ui)) {
    let width = ui.available_width();
    Frame::new()
        .fill(m3.container)
        .corner_radius(18.0)
        .outer_margin(egui::Margin {
            left: inset as i8,
            right: inset as i8,
            top: 0,
            bottom: 0,
        })
        .inner_margin(egui::Margin::symmetric(0, 2))
        .show(ui, |ui| {
            ui.set_width((width - inset * 2.0).max(80.0));
            ui.vertical(|ui| add(ui));
        });
}

/// One row of a card: the icon the row already carries on a tile of its
/// own when it has one, the label, the arrow that says it opens something,
/// and the hairline that keeps it from the row below.
fn list_row(ui: &mut Ui, m3: &M3, label: &str, glyph: Option<Glyph>, last: bool) -> Response {
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 58.0), Sense::click());
    if resp.hovered() || resp.is_pointer_button_down_on() {
        ui.painter()
            .rect_filled(rect, 0.0, mix(m3.on, m3.container, 0.05));
    }
    let mut text_left = rect.left() + 16.0;
    if let Some(glyph) = glyph {
        let tile = Rect::from_center_size(
            egui::pos2(rect.left() + 16.0 + 20.0, rect.center().y),
            Vec2::splat(40.0),
        );
        ui.painter().rect_filled(tile, 11.0, tile_color(label));
        draw_glyph(ui, tile.shrink(10.0), glyph, Color32::WHITE);
        text_left = tile.right() + 14.0;
    }
    ui.painter().text(
        egui::pos2(text_left, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(16.5),
        m3.on,
    );
    // The arrow at the end of the row: two strokes meeting at a point.
    let tip = rect.right() - 20.0;
    let middle = rect.center().y;
    let arrow = Stroke::new(1.6, m3.chevron);
    ui.painter().line_segment(
        [egui::pos2(tip - 4.0, middle - 5.0), egui::pos2(tip + 1.0, middle)],
        arrow,
    );
    ui.painter().line_segment(
        [egui::pos2(tip + 1.0, middle), egui::pos2(tip - 4.0, middle + 5.0)],
        arrow,
    );
    if !last {
        let y = rect.bottom() - 0.5;
        ui.painter().line_segment(
            [egui::pos2(text_left, y), egui::pos2(rect.right() - 16.0, y)],
            Stroke::new(1.0, m3.separator),
        );
    }
    resp
}

/// A row of the drawer or of the settings: the whole width, a finger high,
/// on the card its list is drawn on.
fn drawer_row(
    ui: &mut Ui,
    m3: &M3,
    label: &str,
    glyph: Option<Glyph>,
    last: bool,
) -> Response {
    list_row(ui, m3, label, glyph, last)
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
    let land = landscape(screen);
    let safe = system_insets(&ctx, land);
    let top_pad = top_padding(safe, land);
    let area_id = if land {
        Id::new("mobile_page_land")
    } else {
        Id::new("mobile_page_port")
    };
    egui::Area::new(area_id)
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .constrain_to(screen)
        .show(&ctx, |ui| {
            ui.set_width(screen.width());
            ui.set_max_width(screen.width());
            ui.set_max_height(screen.height());
            Frame::new().fill(m3.surface).show(ui, |ui| {
                ui.set_width(screen.width());
                ui.set_max_width(screen.width());
                ui.set_max_height(screen.height());
                ui.add_space(top_pad);
                // The bar takes a row of its own at the top: a centred row
                // would sit in the middle of the page.
                ui.allocate_ui_with_layout(
                    Vec2::new(screen.width(), TOUCH),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.add_space(4.0 + safe.left);
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
                let scroll_max_h = (screen.height() - (ui.cursor().top() - screen.top()) - safe.bottom).max(80.0);
                let scroll = if wide {
                    egui::ScrollArea::both()
                } else {
                    egui::ScrollArea::vertical()
                };
                scroll
                    .id_salt(("mobile_page_scroll", title))
                    .max_height(scroll_max_h)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        Frame::new()
                            .inner_margin(egui::Margin {
                                left: (16.0 + safe.left) as i8,
                                right: (16.0 + safe.right) as i8,
                                top: 4,
                                bottom: (28.0 + safe.bottom) as i8,
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
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui.label(
                        egui::RichText::new(set.tab_label())
                            .size(12.5)
                            .color(m3.variant),
                    );
                });
                ui.add_space(6.0);
                card(ui, m3, 16.0, |ui| {
                    let pages = set.pages();
                    for (i, page) in pages.iter().enumerate() {
                        let last = i + 1 == pages.len();
                        if drawer_row(ui, m3, &tr(page.key()), None, last).clicked() {
                            app.options_page = *page;
                            chosen.set(Sheet::SettingsPage(*page));
                        }
                    }
                });
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
    let width = ui.available_width();
    ui.add_space(4.0);
    Frame::new()
        .fill(m3.container)
        .corner_radius(16.0)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_width(width);
            egui::ScrollArea::horizontal()
                .id_salt("mobile_property_strip")
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                    crate::ui::toolbar::property_bar(app, ui);
                });
        });

    ui.add_space(8.0);
    selection_actions(app, ui, m3);
    ui.add_space(8.0);
    object_geometry(app, ui, m3, width);
    ui.add_space(8.0);
    object_stroke_presets(app, ui, m3);
}

fn action_chip(ui: &mut Ui, m3: &M3, label: &str) -> Response {
    let text = egui::RichText::new(label).size(13.0).color(m3.on);
    let button = egui::Button::new(text)
        .corner_radius(12.0)
        .fill(m3.container)
        .stroke(Stroke::new(1.0, m3.outline))
        .min_size(Vec2::new(0.0, 36.0));
    ui.add(button)
}

fn selection_actions(app: &mut App, ui: &mut Ui, m3: &M3) {
    property_label(ui, m3, &tr("toolbar.actions"));
    egui::ScrollArea::horizontal()
        .id_salt("mobile_sel_actions_scroll")
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // A phone has no double click to remember: the way back
                // into a text is a button.
                if !app.selection.is_empty()
                    && app.selection.iter().all(|id| {
                    app.doc()
                        .find_shape(*id)
                        .is_some_and(|s| matches!(s.kind, tracedraw_core::document::ShapeKind::Text { .. }))
                }) && action_chip(ui, m3, &tr("menu.text.edit_text")).clicked()
                {
                    app.edit_selected_text();
                }
                if action_chip(ui, m3, &tr("menu.edit.delete")).clicked() {
                    app.delete_selection();
                }
                if action_chip(ui, m3, &tr("menu.edit.duplicate")).clicked() {
                    app.duplicate();
                }
                if app.selection.len() > 1 && action_chip(ui, m3, &tr("menu.object.group_group")).clicked() {
                    app.group_selection();
                }
                if (app.selection.len() == 1 || app.selection_nested())
                    && action_chip(ui, m3, &tr("menu.object.group_ungroup")).clicked()
                {
                    app.ungroup_selection();
                }
                if action_chip(ui, m3, &tr("menu.object.to_front_of_page")).clicked() {
                    app.order(0);
                }
                if action_chip(ui, m3, &tr("menu.object.to_back_of_page")).clicked() {
                    app.order(3);
                }
                if action_chip(ui, m3, &tr("toolbar.mirror_h_short")).clicked() {
                    app.mirror(true);
                }
                if action_chip(ui, m3, &tr("toolbar.mirror_v_short")).clicked() {
                    app.mirror(false);
                }
                if action_chip(ui, m3, &tr("toolbar.rotate_90")).clicked() {
                    if let Some(b) = app.selection_bounds() {
                        app.transform_selection(tracedraw_core::Affine::rotate_about(
                            std::f64::consts::FRAC_PI_2,
                            b.center(),
                        ));
                    }
                }
                if action_chip(ui, m3, &tr("menu.object.convert_to_curves")).clicked() {
                    app.convert_to_curves();
                }
            });
        });
}

fn object_geometry(app: &mut App, ui: &mut Ui, m3: &M3, width: f32) {
    let Some(b) = app.selection_bounds() else {
        return;
    };
    let units = app.units;
    let o = app.ruler_origin();
    let refp = b.center();
    let half_w = ((width - 12.0) * 0.5).max(100.0);

    property_label(ui, m3, &tr("toolbar.position"));
    ui.horizontal(|ui| {
        let r_x = property_row(ui, half_w, 42.0);
        if let Some(vx) = crate::ui::chrome::spin_field(
            ui,
            "mobile_sel_x",
            r_x,
            units.from_mm(b.x0 - o.x),
            1,
            units.short(),
            0.5,
            -1.0e7..=1.0e7,
        ) {
            let target_x = units.to_mm(vx) + o.x;
            let dx = target_x - b.x0;
            app.transform_selection(tracedraw_core::Affine::translate(
                tracedraw_core::geometry::Vec2::new(dx, 0.0),
            ));
        }
        let r_y = property_row(ui, half_w, 42.0);
        if let Some(vy) = crate::ui::chrome::spin_field(
            ui,
            "mobile_sel_y",
            r_y,
            units.from_mm(b.y0 - o.y),
            1,
            units.short(),
            0.5,
            -1.0e7..=1.0e7,
        ) {
            let target_y = units.to_mm(vy) + o.y;
            let dy = target_y - b.y0;
            app.transform_selection(tracedraw_core::Affine::translate(
                tracedraw_core::geometry::Vec2::new(0.0, dy),
            ));
        }
    });

    property_label(ui, m3, &tr("toolbar.object_size"));
    ui.horizontal(|ui| {
        let cur_w = b.width();
        let r_w = property_row(ui, half_w, 42.0);
        if let Some(vw) = crate::ui::chrome::spin_field(
            ui,
            "mobile_sel_w",
            r_w,
            units.from_mm(cur_w),
            1,
            units.short(),
            0.5,
            0.01..=1.0e7,
        ) {
            let target_w = units.to_mm(vw).max(0.01);
            if cur_w > 1e-6 {
                let sx = target_w / cur_w;
                let t = tracedraw_core::Affine::translate((refp.x, refp.y))
                    * tracedraw_core::Affine::scale_non_uniform(sx, 1.0)
                    * tracedraw_core::Affine::translate((-refp.x, -refp.y));
                app.transform_selection(t);
            }
        }
        let cur_h = b.height();
        let r_h = property_row(ui, half_w, 42.0);
        if let Some(vh) = crate::ui::chrome::spin_field(
            ui,
            "mobile_sel_h",
            r_h,
            units.from_mm(cur_h),
            1,
            units.short(),
            0.5,
            0.01..=1.0e7,
        ) {
            let target_h = units.to_mm(vh).max(0.01);
            if cur_h > 1e-6 {
                let sy = target_h / cur_h;
                let t = tracedraw_core::Affine::translate((refp.x, refp.y))
                    * tracedraw_core::Affine::scale_non_uniform(1.0, sy)
                    * tracedraw_core::Affine::translate((-refp.x, -refp.y));
                app.transform_selection(t);
            }
        }
    });
}

fn object_stroke_presets(app: &mut App, ui: &mut Ui, m3: &M3) {
    property_label(ui, m3, &tr("toolbar.outline_width"));
    ui.horizontal_wrapped(|ui| {
        for (label, w_mm) in [
            (tr("toolbar.outline_none"), 0.0),
            (tr("toolbar.outline_hairline"), 0.0762),
            ("0.5 mm".into(), 0.5),
            ("1.0 mm".into(), 1.0),
            ("2.0 mm".into(), 2.0),
            ("4.0 mm".into(), 4.0),
        ] {
            if action_chip(ui, m3, &label).clicked() {
                app.apply_outline_width(w_mm);
            }
        }
        if action_chip(ui, m3, &format!("{}...", tr("outline_pen.title"))).clicked() {
            app.open_outline_editor();
        }
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
fn palette_float(
    app: &mut App,
    ctx: &egui::Context,
    screen: Rect,
    m3: &M3,
    open: &mut Sheet,
    strip: f32,
    ime: f32,
) {
    let land = landscape(screen);
    let width = (screen.width() * 0.78).clamp(232.0, 360.0);
    let height = (screen.height() * 0.34).clamp(190.0, 300.0);
    // The corner the card hangs from, rather than a corner and a height:
    // it is as tall as the colours in it, and grows away from the corner
    // it is pinned to.
    let (pos, pivot) = if land {
        // On its side: under the top bar, against the destinations rail.
        (
            egui::pos2(
                screen.right() - NAV_RAIL_W - 12.0,
                screen.top() + TOP_H + 12.0,
            ),
            egui::Align2::RIGHT_TOP,
        )
    } else {
        // Standing: above the tool strip, against the right edge.
        (
            egui::pos2(
                screen.right() - 12.0,
                screen.bottom() - ime - NAV_H - TOOLS_H - strip - 12.0,
            ),
            egui::Align2::RIGHT_BOTTOM,
        )
    };
    let card = egui::Area::new(Id::new("mobile_palette_float"))
        .order(egui::Order::Foreground)
        .pivot(pivot)
        .fixed_pos(pos)
        .show(ctx, |ui| {
            Frame::new()
                .fill(m3.container)
                .corner_radius(12.0)
                .shadow(crate::theme::shadow(egui::epaint::Shadow {
                    offset: [0, 4],
                    blur: 12,
                    spread: 0,
                    color: Color32::from_black_alpha(48),
                }))
                .inner_margin(egui::Margin {
                    left: 0,
                    right: 0,
                    top: 0,
                    bottom: 0,
                })
                .show(ui, |ui| {
                    ui.set_width(width);
                    // The title is the way out: a tap on it, or on the cross
                    // beside it, puts the card away. A tap elsewhere on the
                    // card picks a colour, so the header is what closes it.
                    let (row, row_resp) =
                        ui.allocate_exact_size(Vec2::new(width, 44.0), Sense::click());
                    ui.painter().text(
                        egui::pos2(row.left() + 16.0, row.center().y),
                        egui::Align2::LEFT_CENTER,
                        sheet_title(Sheet::Palette),
                        crate::theme::bold(16.0),
                        m3.on,
                    );
                    let close = Rect::from_center_size(
                        egui::pos2(row.right() - 28.0, row.center().y),
                        Vec2::splat(48.0),
                    );
                    let close_resp =
                        ui.interact(close, Id::new("mobile_palette_close"), Sense::click());
                    draw_glyph(
                        ui,
                        Rect::from_center_size(close.center(), Vec2::splat(ICON)),
                        Glyph::Close,
                        m3.on,
                    );
                    if row_resp.clicked() || close_resp.clicked() {
                        *open = Sheet::None;
                    }
                    egui::ScrollArea::vertical()
                        .id_salt("mobile_palette_scroll")
                        .max_height(height - 44.0)
                        .auto_shrink([false, true])
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
            // A drawing of another format comes in beside the one open,
            // which is what File > Import does on a desktop.
            let has = app.has_document();
            if tonal_button(ui, m3, &plain(&tr("menu.file.import")), has).clicked() {
                app.import();
            }
            if tonal_button(ui, m3, &plain(&tr("menu.file.save")), has).clicked() {
                save_drawing(app, false);
            }
            if tonal_button(ui, m3, &plain(&tr("menu.file.save_as")), has).clicked() {
                save_drawing(app, true);
            }
            // 转曲保存: the drawing as curves, into Download, then the share
            // sheet, which is how a phone sends a file to WeChat or QQ.
            if tonal_button(ui, m3, &tr("mobile.save_curves"), has).clicked() {
                if let Err(e) = app.start_curves_save(crate::curves_cdr::X8) {
                    app.status = e;
                }
            }
            // Fonts dropped into the Fonts folder of the drawings' folder
            // are read at start; this reads the ones added since.
            if tonal_button(ui, m3, &tr("mobile.reload_fonts"), true).clicked() {
                let added = crate::fonts_folder::scan();
                app.status =
                    trf("status.fonts_loaded", &[("n", &added.to_string())]);
            }
            if tonal_button(ui, m3, &plain(&tr("menu.file.export")), has).clicked() {
                app.dialog = crate::ui::dialogs::Dialog::Export(crate::ui::dialogs::ExportState::default());
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
    // The way up goes as far as the root of the shared storage, which is
    // `/storage/emulated/0`: the folders above it are the system's, not the
    // user's.
    let root = crate::files::android_root();
    let up = dir
        .parent()
        .map(|p| p.to_path_buf())
        .filter(|p| p.starts_with(&root));
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
    let many = app.docs.len() > 1;
    let mut pick = None;
    let mut shut = None;
    for (i, _) in app.docs.iter().enumerate() {
        let title = app.document_tab_title(i);
        let (row, close) = drawing_row(ui, m3, &elide(&title, 26), i == active, many);
        if close {
            shut = Some(i);
        } else if row.clicked() {
            pick = Some(i);
        }
    }
    if let Some(i) = pick {
        app.active_doc = i;
    }
    if let Some(i) = shut {
        // Closing goes through the document's own way, so a drawing with
        // unsaved changes still asks before it goes.
        app.active_doc = i;
        app.close_document();
    }
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if filled_button(ui, m3, &plain(&tr("menu.file.new"))).clicked() {
            app.request_new_document();
        }
        if tonal_button(ui, m3, &plain(&tr("menu.file.open")), true).clicked() {
            app.open_dialog();
        }
        // A drawing of another format comes in beside the one open, which
        // is what File > Import does on a desktop.
        if tonal_button(ui, m3, &plain(&tr("menu.file.import")), true).clicked() {
            app.import();
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
fn drawing_row(
    ui: &mut Ui,
    m3: &M3,
    title: &str,
    active: bool,
    closable: bool,
) -> (Response, bool) {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 56.0), Sense::click());
    if active {
        ui.painter().rect_filled(rect, 12.0, m3.primary_container);
    } else if resp.hovered() || resp.is_pointer_button_down_on() {
        let fill = mix(m3.on, m3.surface, 0.08);
        ui.painter().rect_filled(rect, 12.0, fill);
    }
    // The name gives way to the close target at the row's end.
    let room = if closable { 48.0 } else { 12.0 };
    ui.painter().text(
        egui::pos2(rect.left() + 12.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(16.0),
        if active { m3.primary } else { m3.on },
    );
    let mut close = false;
    if closable {
        let side = 40.0;
        let target = Rect::from_center_size(
            egui::pos2(rect.right() - room / 2.0, rect.center().y),
            Vec2::splat(side),
        );
        let hit = ui.interact(target, resp.id.with("close"), Sense::click());
        if hit.hovered() {
            ui.painter()
                .circle_filled(target.center(), side / 2.0, mix(m3.on, m3.surface, 0.10));
        }
        draw_glyph(
            ui,
            Rect::from_center_size(target.center(), Vec2::splat(18.0)),
            Glyph::Close,
            m3.variant,
        );
        close = hit.clicked();
    }
    (resp, close)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn safe(top: f32, cutout_top: f32, visible: bool) -> Safe {
        Safe {
            top,
            cutout_top,
            status_bar_visible: visible,
            ..Safe::default()
        }
    }

    #[test]
    fn a_phone_on_its_side_keeps_the_bar_on_the_top_edge() {
        // Bars hidden, the notch in the middle of an edge: nothing to clear.
        assert_eq!(top_padding(safe(30.0, 0.0, false), true), 0.0);
        // Bars hidden, but the cutout reaches the top edge: clear that much.
        assert_eq!(top_padding(safe(30.0, 26.0, false), true), 26.0);
        // The status bar still on screen is cleared, and a little more.
        assert_eq!(top_padding(safe(30.0, 0.0, true), true), 36.0);
        // A cutout taller than the status bar is not counted twice.
        assert_eq!(top_padding(safe(26.0, 26.0, true), true), 26.0);
    }

    #[test]
    fn a_phone_standing_up_uses_the_whole_top_inset() {
        assert_eq!(top_padding(safe(28.0, 28.0, true), false), 28.0);
        assert_eq!(top_padding(safe(0.0, 0.0, true), false), 0.0);
    }

    #[test]
    fn the_camera_cutout_never_pushes_an_icon_sideways_on_its_side() {
        let mut s = safe(0.0, 0.0, false);
        s.left = 32.0;
        s.right = 12.0;
        // On its side the interface runs edge to edge, past the cutout.
        let flat = edge_to_edge(s, true);
        assert_eq!((flat.left, flat.right), (0.0, 0.0));
        // Standing up the bars reach the edges the interface is drawn on.
        let up = edge_to_edge(s, false);
        assert_eq!((up.left, up.right), (32.0, 12.0));
    }

    #[test]
    fn the_rows_of_a_card_are_one_under_the_other() {
        // A card is a column: every row below the one before it, and none
        // of them drawn over another. It was a row of rows once, and every
        // label of the drawer landed on top of the one before it.
        let ctx = crate::theme::ui_context();
        let m3 = M3::of();
        let labels = ["Sounds", "Notifications", "Scenes", "Battery"];
        let mut seen: Vec<Rect> = Vec::new();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(360.0, 800.0),
            )),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            card(ui, &m3, 16.0, |ui| {
                let last = labels.len() - 1;
                for (i, label) in labels.iter().enumerate() {
                    seen.push(list_row(ui, &m3, label, None, i == last).rect);
                }
            });
        });
        out.textures_delta.clear();
        assert_eq!(seen.len(), labels.len());
        for pair in seen.windows(2) {
            assert!(
                pair[1].top() >= pair[0].bottom() - 0.5,
                "a row is drawn over the one before it: {:?} then {:?}",
                pair[0],
                pair[1]
            );
        }
        // Every row the same width, and inside the card's own margin.
        let width = seen.first().map(|r| r.width()).unwrap_or(0.0);
        assert!(width > 100.0 && width < 360.0, "a row is {width} wide");
        for row in &seen {
            assert!((row.width() - width).abs() < 0.5);
        }
    }

    #[test]
    fn the_contour_follows_its_picture_and_is_not_centred_on_the_page() {
        // A picture away from the middle of the page: the outline it is cut
        // for sits on the picture, not in the middle of the page.
        let mut app = App::headless();
        let mut img = image::RgbaImage::from_pixel(64, 64, image::Rgba([255, 255, 255, 255]));
        for y in 16..48 {
            for x in 16..48 {
                img.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
            }
        }
        let Some(png) = crate::bitmap_fx::encode(&img) else {
            panic!("a picture");
        };
        let rect = tracedraw_core::geometry::Rect::new(300.0, 400.0, 340.0, 440.0);
        let id = app
            .new_shape(tracedraw_core::document::ShapeKind::Bitmap {
                rect,
                width_px: 64,
                height_px: 64,
                png: png.clone().into(),
                fx: None,
            })
            .expect("a bitmap");
        app.select(vec![id]);
        contour_run(&mut app, 0.0, 60.0, 40.0, false, false);
        let outline = app
            .doc()
            .all_layers()
            .flat_map(|l| l.shapes.iter())
            .find(|s| matches!(s.kind, tracedraw_core::document::ShapeKind::Path { .. }))
            .map(|s| s.bounds());
        let Some(outline) = outline else {
            panic!("no outline was made: {}", app.status);
        };
        // The black square is the middle half of the picture, so its outline
        // is the middle half of the rect: from 310 to 330 across.
        assert!(
            (outline.x0 - 310.0).abs() < 3.0,
            "the outline starts at {} rather than at the picture's 310",
            outline.x0
        );
        assert!(
            (outline.x1 - 330.0).abs() < 3.0,
            "the outline ends at {} rather than at the picture's 330",
            outline.x1
        );
        // Put inside the outline as a container, the picture keeps its
        // place too: nothing about the contour moves anything.
        let mut app = App::headless();
        let id = app
            .new_shape(tracedraw_core::document::ShapeKind::Bitmap {
                rect,
                width_px: 64,
                height_px: 64,
                png: png.clone().into(),
                fx: None,
            })
            .expect("a bitmap");
        app.select(vec![id]);
        contour_run(&mut app, 0.0, 60.0, 40.0, false, true);
        let picture = app
            .doc()
            .all_layers()
            .flat_map(|l| l.shapes.iter())
            .find_map(|s| match &s.kind {
                tracedraw_core::document::ShapeKind::ClipFrame { frame, contents } => contents
                    .iter()
                    .find_map(|c| match &c.kind {
                        tracedraw_core::document::ShapeKind::Bitmap { rect, .. } => Some(*rect),
                        _ => None,
                    })
                    .or(Some(frame.bounds())),
                _ => None,
            });
        let Some(picture) = picture else {
            panic!("the picture was not put in the outline");
        };
        assert!(
            (picture.x0 - 300.0).abs() < 3.0 && (picture.y0 - 400.0).abs() < 3.0,
            "the picture moved to {picture:?} instead of staying at 300, 400"
        );
    }

    #[test]
    fn a_moved_picture_gets_its_contour_where_it_is_now() {
        // A picture moved after it was made keeps its rect and takes the move
        // in its transform. The contour follows the picture, not the place it
        // started from, which is what put every outline in the middle of the
        // page once the picture had been dragged away.
        let mut app = App::headless();
        let mut img = image::RgbaImage::from_pixel(64, 64, image::Rgba([255, 255, 255, 255]));
        for y in 16..48 {
            for x in 16..48 {
                img.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
            }
        }
        let Some(png) = crate::bitmap_fx::encode(&img) else {
            panic!("a picture");
        };
        let rect = tracedraw_core::geometry::Rect::new(300.0, 400.0, 340.0, 440.0);
        let id = app
            .new_shape(tracedraw_core::document::ShapeKind::Bitmap {
                rect,
                width_px: 64,
                height_px: 64,
                png: png.into(),
                fx: None,
            })
            .expect("a bitmap");
        app.engine
            .run_batch(
                "Move",
                &[tracedraw_core::Command::TransformShapes {
                    shapes: vec![id],
                    transform: tracedraw_core::geometry::Affine::translate((100.0, 60.0)),
                }],
            )
            .expect("a move");
        app.select(vec![id]);
        contour_run(&mut app, 0.0, 60.0, 40.0, false, false);
        let outline = app
            .doc()
            .all_layers()
            .flat_map(|l| l.shapes.iter())
            .find(|s| matches!(s.kind, tracedraw_core::document::ShapeKind::Path { .. }))
            .map(|s| s.bounds());
        let Some(outline) = outline else {
            panic!("no outline was made: {}", app.status);
        };
        assert!(
            (outline.x0 - 410.0).abs() < 3.0,
            "the outline starts at {} rather than at the moved picture's 410",
            outline.x0
        );
        assert!(
            (outline.y0 - 470.0).abs() < 3.0,
            "the outline starts at {} rather than at the moved picture's 470",
            outline.y0
        );
    }
}
