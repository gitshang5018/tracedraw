//! Batch banners: a set of slogans, each on a banner of its own.
//!
//! Standard metric sizes for sign and print shops:
//! - Standard roll widths: 400 to 1200 mm (700 mm default)
//! - Side margins for tying ropes: 400 mm
//! - Top and bottom margins: 50 mm
//! - Letter spacing: 50 mm
//! - Automatic length calculation based on text length
//! - Classic color presets (red cloth with yellow text, etc.)
//! - Output as separate pages (one banner per page) or laid out on current page

use crate::app::App;
use tracedraw_core::{
    document::{ParagraphStyle, Shape, ShapeKind, TextAlign, TextSpan},
    geometry::{Point, Rect, Size},
    Color, Command, Fill, Stroke,
};

/// The font the banners are written in.
const FONT: &str = "sans";

/// Standard cloth roll widths in millimetres (400 to 1200 mm).
pub const PRESET_HEIGHTS_MM: [f64; 8] = [
    400.0, 500.0, 600.0, 700.0, 800.0, 900.0, 1000.0, 1200.0,
];

/// How banner length is determined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthMode {
    /// Auto calculate banner length from text count and spacing.
    Auto,
    /// Fixed total length in millimetres.
    Fixed,
}

/// How punctuation marks in banner text should be cleaned up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PunctuationMode {
    Space,
    Strip,
    Keep,
}

/// Classic color presets for banners.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorPreset {
    RedYellow,
    RedWhite,
    BlueWhite,
    WhiteRed,
    WhiteBlack,
    YellowRed,
    Custom,
}

impl ColorPreset {
    pub const ALL: [ColorPreset; 6] = [
        ColorPreset::RedYellow,
        ColorPreset::RedWhite,
        ColorPreset::BlueWhite,
        ColorPreset::WhiteRed,
        ColorPreset::WhiteBlack,
        ColorPreset::YellowRed,
    ];

    pub fn bg_color(self) -> Color {
        match self {
            ColorPreset::RedYellow | ColorPreset::RedWhite => Color::rgb8(218, 24, 24),
            ColorPreset::BlueWhite => Color::rgb8(18, 78, 192),
            ColorPreset::WhiteRed | ColorPreset::WhiteBlack => Color::WHITE,
            ColorPreset::YellowRed => Color::rgb8(250, 210, 20),
            ColorPreset::Custom => Color::rgb8(218, 24, 24),
        }
    }

    pub fn text_color(self) -> Color {
        match self {
            ColorPreset::RedYellow => Color::rgb8(255, 235, 30),
            ColorPreset::RedWhite | ColorPreset::BlueWhite => Color::WHITE,
            ColorPreset::WhiteRed | ColorPreset::YellowRed => Color::rgb8(218, 24, 24),
            ColorPreset::WhiteBlack => Color::BLACK,
            ColorPreset::Custom => Color::rgb8(255, 235, 30),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ColorPreset::RedYellow => "banner.color_red_yellow",
            ColorPreset::RedWhite => "banner.color_red_white",
            ColorPreset::BlueWhite => "banner.color_blue_white",
            ColorPreset::WhiteRed => "banner.color_white_red",
            ColorPreset::WhiteBlack => "banner.color_white_black",
            ColorPreset::YellowRed => "banner.color_yellow_red",
            ColorPreset::Custom => "banner.color_custom",
        }
    }
}

/// How batch banners are outputted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    /// Each banner becomes an independent page of its own exact dimensions.
    Pages,
    /// Stack banners down the current canvas page.
    CurrentPage,
}

/// What a batch of banners is made of.
#[derive(Debug, Clone, PartialEq)]
pub struct Banner {
    /// The slogans, one banner each. An empty line is left out.
    pub slogans: Vec<String>,
    /// Optional signature under every slogan. Empty if none.
    pub signature: String,
    /// The cloth roll width (height of horizontal banner), in millimetres.
    pub height_mm: f64,
    /// Length mode: auto by text or fixed.
    pub length_mode: LengthMode,
    /// The width/length of a banner in fixed mode (millimetres).
    pub width_mm: f64,
    /// Left and right tying margins in millimetres (default: 400 mm).
    pub margin_lr_mm: f64,
    /// Top and bottom safety margins in millimetres (default: 50 mm).
    pub margin_tb_mm: f64,
    /// Character spacing between letters in millimetres (default: 50 mm).
    pub spacing_mm: f64,
    /// Character height ratio of available cloth height (default: 0.75).
    pub height_ratio: f64,
    /// The room between two banners when stacked on current page.
    pub gap_mm: f64,
    /// The size of the slogan, in points (auto computed if auto).
    pub slogan_pt: f64,
    /// The size of the signature, in points.
    pub signature_pt: f64,
    /// Punctuation cleaning mode.
    pub punctuation: PunctuationMode,
    /// Color preset.
    pub color_preset: ColorPreset,
    /// The colour of the banner itself.
    pub background: Color,
    /// The colour of the slogan.
    pub slogan_color: Color,
    /// The colour of the signature.
    pub signature_color: Color,
    /// The colour of the band along the bottom of the banner.
    pub band_color: Color,
    /// Whether to generate grommet marks (eyelets).
    pub grommets: bool,
    /// Output mode: separate pages (one per banner) or stacked on current page.
    pub output_mode: OutputMode,
    /// Whether the slogan is written along the banner or down it.
    pub vertical: bool,
}

impl Default for Banner {
    fn default() -> Banner {
        Banner {
            slogans: vec!["热烈庆祝中华人民共和国成立七十五周年".into(), "不忘初心 牢记使命 砥砺前行".into()],
            signature: String::new(),
            height_mm: 700.0,
            length_mode: LengthMode::Auto,
            width_mm: 6000.0,
            margin_lr_mm: 400.0,
            margin_tb_mm: 50.0,
            spacing_mm: 50.0,
            height_ratio: 0.75,
            gap_mm: 100.0,
            slogan_pt: 1400.0,
            signature_pt: 500.0,
            punctuation: PunctuationMode::Space,
            color_preset: ColorPreset::RedYellow,
            background: Color::rgb8(218, 24, 24),
            slogan_color: Color::rgb8(255, 235, 30),
            signature_color: Color::WHITE,
            band_color: Color::rgb8(138, 20, 31),
            grommets: false,
            output_mode: OutputMode::Pages,
            vertical: false,
        }
    }
}

fn is_punct(c: char) -> bool {
    matches!(
        c,
        ',' | '.' | '!' | '?' | ';' | ':' | '，' | '。' | '！' | '？' | '；'
            | '：' | '、' | '“' | '”' | '‘' | '’' | '《' | '》' | '(' | ')'
            | '（' | '）' | '[' | ']' | '【' | '】' | '-' | '\u{2014}' | '~' | '～'
    )
}

impl Banner {
    /// Clean banner text according to punctuation mode.
    pub fn clean_slogan(&self, raw: &str) -> String {
        match self.punctuation {
            PunctuationMode::Keep => raw.trim().to_string(),
            PunctuationMode::Strip => {
                raw.chars().filter(|&c| !is_punct(c)).collect::<String>().trim().to_string()
            }
            PunctuationMode::Space => {
                let mut res = String::with_capacity(raw.len());
                let mut last_was_space = false;
                for c in raw.chars() {
                    if is_punct(c) || c.is_whitespace() {
                        if !last_was_space && !res.is_empty() {
                            res.push(' ');
                            last_was_space = true;
                        }
                    } else {
                        res.push(c);
                        last_was_space = false;
                    }
                }
                res.trim().to_string()
            }
        }
    }

    /// The slogans that get a banner: the empty lines are left out, and
    /// the lines are cleaned.
    pub fn lines(&self) -> Vec<String> {
        self.slogans
            .iter()
            .map(|s| self.clean_slogan(s))
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Apply a color preset to the banner's colors.
    pub fn apply_color_preset(&mut self, preset: ColorPreset) {
        self.color_preset = preset;
        if preset != ColorPreset::Custom {
            self.background = preset.bg_color();
            self.slogan_color = preset.text_color();
            self.signature_color = preset.text_color();
            self.band_color = preset.bg_color();
        }
    }

    /// Calculate dimensions (width_mm, height_mm, font_size_pt) for a single slogan.
    pub fn calculate_size(&self, slogan: &str) -> (f64, f64, f64) {
        let cloth_h = self.height_mm.max(100.0);
        let avail_h = (cloth_h - 2.0 * self.margin_tb_mm.max(0.0)).max(10.0);
        let char_h_mm = (avail_h * self.height_ratio.clamp(0.4, 0.95)).max(10.0);
        let font_size_pt = char_h_mm * (72.0 / 25.4);

        let chars: Vec<char> = slogan.chars().collect();
        let mut text_width_mm = 0.0;
        for &c in &chars {
            if c.is_ascii_graphic() || c.is_ascii_digit() {
                text_width_mm += char_h_mm * 0.55;
            } else if c.is_whitespace() {
                text_width_mm += char_h_mm * 0.45;
            } else {
                text_width_mm += char_h_mm;
            }
        }
        let gaps = (chars.len().saturating_sub(1)) as f64 * self.spacing_mm.max(0.0);

        let width = match self.length_mode {
            LengthMode::Auto => (2.0 * self.margin_lr_mm.max(0.0) + text_width_mm + gaps).max(200.0),
            LengthMode::Fixed => self.width_mm.max(2.0 * self.margin_lr_mm.max(0.0) + 100.0),
        };

        (width, cloth_h, font_size_pt)
    }

    /// The banners as shapes, laid out down the page from `top`.
    /// Used when placing directly onto a page.
    pub fn build(&self, top: f64, left: f64) -> Vec<Vec<(ShapeKind, Fill, Option<Stroke>)>> {
        let mut out: Vec<Vec<(ShapeKind, Fill, Option<Stroke>)>> = Vec::new();
        let h = self.height_mm.max(1.0);
        let has_sign = !self.signature.trim().is_empty();
        let band = if has_sign { (h * 0.18).max(6.0) } else { 0.0 };

        for (k, slogan) in self.lines().iter().enumerate() {
            let (w, _, size_pt) = self.calculate_size(slogan);
            let y0 = top + k as f64 * (h + self.gap_mm.max(0.0));
            let mut banner: Vec<(ShapeKind, Fill, Option<Stroke>)> = Vec::new();

            // 1. Rectangle
            banner.push((
                ShapeKind::Rect {
                    rect: Rect::new(left, y0, left + w, y0 + h),
                    radius: 0.0,
                    corners: None,
                },
                Fill::Solid(self.background),
                None,
            ));

            // 2. Optional bottom band if signature is present
            if has_sign {
                banner.push((
                    ShapeKind::Rect {
                        rect: Rect::new(left, y0 + h - band, left + w, y0 + h),
                        radius: 0.0,
                        corners: None,
                    },
                    Fill::Solid(self.band_color),
                    None,
                ));
            }

            // 3. Slogan
            let char_h = size_pt * (25.4 / 72.0);
            let text_box_w = (w - 2.0 * self.margin_lr_mm.max(0.0)).max(10.0);
            let text_box_h = if has_sign { h - band } else { h };
            let origin_y = y0 + (text_box_h - char_h) * 0.5;

            let span = TextSpan {
                bold: true,
                tracking_pct: if char_h > 0.0 { (self.spacing_mm / char_h) * 100.0 } else { 0.0 },
                fill: Some(Fill::Solid(self.slogan_color)),
                ..TextSpan::new(slogan, FONT, size_pt)
            };

            banner.push((
                ShapeKind::Text {
                    spans: vec![span],
                    origin: Point::new(left + self.margin_lr_mm.max(0.0), origin_y),
                    frame: Some(Size::new(text_box_w, char_h)),
                    align: TextAlign::Center,
                    para: ParagraphStyle::default(),
                    on_path: None,
                },
                Fill::Solid(self.slogan_color),
                None,
            ));

            // 4. Signature
            if has_sign {
                let sign_pt = (size_pt * 0.35).max(12.0);
                let sign_h = sign_pt * (25.4 / 72.0);
                let frame = Size::new(w * 0.9, sign_h * 1.5);
                banner.push((
                    ShapeKind::Text {
                        spans: vec![TextSpan {
                            text: self.signature.trim().into(),
                            font_family: FONT.into(),
                            size_pt: sign_pt,
                            bold: false,
                            fill: Some(Fill::Solid(self.signature_color)),
                            ..TextSpan::new(self.signature.trim(), FONT, sign_pt)
                        }],
                        origin: Point::new(
                            left + w * 0.5 - frame.width * 0.5,
                            y0 + h - band * 0.5 - frame.height * 0.5,
                        ),
                        frame: Some(frame),
                        align: TextAlign::Center,
                        para: ParagraphStyle::default(),
                        on_path: None,
                    },
                    Fill::Solid(self.signature_color),
                    None,
                ));
            }

            out.push(banner);
        }
        out
    }
}

impl App {
    /// Make a batch of banners: either as separate pages (one per banner, recommended)
    /// or laid out down the current page.
    pub fn create_banners(&mut self, banner: &Banner) {
        let lines = banner.lines();
        if lines.is_empty() {
            self.status = crate::i18n::tr("banner.need_slogan");
            return;
        }

        let mark = self.engine.begin_group();

        match banner.output_mode {
            OutputMode::Pages => {
                let mut first_page: Option<tracedraw_core::PageId> = None;
                for (idx, slogan) in lines.iter().enumerate() {
                    let (w, h, size_pt) = banner.calculate_size(slogan);
                    let add_cmd = Command::AddPage {
                        name: Some(format!("{} {}", crate::i18n::tr("banner.title"), idx + 1)),
                        size: Size::new(w, h),
                        layer: None,
                    };
                    if self.engine.run(&add_cmd).is_err() {
                        continue;
                    }
                    let Some(page) = self.engine.document().pages.last() else {
                        continue;
                    };
                    let pid = page.id;
                    if first_page.is_none() {
                        first_page = Some(pid);
                    }
                    let Some(layer) = page.layers.first().map(|l| l.id) else {
                        continue;
                    };

                    let has_sign = !banner.signature.trim().is_empty();
                    let band = if has_sign { (h * 0.18).max(6.0) } else { 0.0 };

                    // 1. Background cloth
                    let mut bg = Shape::new(
                        self.engine.new_shape_id(),
                        ShapeKind::Rect {
                            rect: Rect::new(0.0, 0.0, w, h),
                            radius: 0.0,
                            corners: None,
                        },
                    );
                    bg.fill = Some(Fill::Solid(banner.background));
                    bg.stroke = None;
                    let _ = self.engine.run(&Command::AddShape { layer, shape: bg });

                    // 2. Optional bottom band if signature is present
                    if has_sign {
                        let mut band_shape = Shape::new(
                            self.engine.new_shape_id(),
                            ShapeKind::Rect {
                                rect: Rect::new(0.0, h - band, w, h),
                                radius: 0.0,
                                corners: None,
                            },
                        );
                        band_shape.fill = Some(Fill::Solid(banner.band_color));
                        let _ = self.engine.run(&Command::AddShape { layer, shape: band_shape });
                    }

                    // 3. Banner text
                    let char_h = size_pt * (25.4 / 72.0);
                    let text_box_w = (w - 2.0 * banner.margin_lr_mm.max(0.0)).max(10.0);
                    let text_box_h = if has_sign { h - band } else { h };
                    let origin_y = (text_box_h - char_h) * 0.5;

                    let span = TextSpan {
                        bold: true,
                        tracking_pct: if char_h > 0.0 { (banner.spacing_mm / char_h) * 100.0 } else { 0.0 },
                        fill: Some(Fill::Solid(banner.slogan_color)),
                        ..TextSpan::new(slogan, FONT, size_pt)
                    };

                    let text_shape = Shape::new(
                        self.engine.new_shape_id(),
                        ShapeKind::Text {
                            spans: vec![span],
                            origin: Point::new(banner.margin_lr_mm.max(0.0), origin_y),
                            frame: Some(Size::new(text_box_w, char_h)),
                            align: TextAlign::Center,
                            para: ParagraphStyle::default(),
                            on_path: None,
                        },
                    );
                    let _ = self.engine.run(&Command::AddShape { layer, shape: text_shape });

                    // 4. Optional signature
                    if has_sign {
                        let sign_pt = (size_pt * 0.35).max(12.0);
                        let sign_h = sign_pt * (25.4 / 72.0);
                        let frame = Size::new(w * 0.9, sign_h * 1.5);
                        let sign_shape = Shape::new(
                            self.engine.new_shape_id(),
                            ShapeKind::Text {
                                spans: vec![TextSpan {
                                    text: banner.signature.trim().into(),
                                    font_family: FONT.into(),
                                    size_pt: sign_pt,
                                    bold: false,
                                    fill: Some(Fill::Solid(banner.signature_color)),
                                    ..TextSpan::new(banner.signature.trim(), FONT, sign_pt)
                                }],
                                origin: Point::new(
                                    (w - frame.width) * 0.5,
                                    h - band * 0.5 - frame.height * 0.5,
                                ),
                                frame: Some(frame),
                                align: TextAlign::Center,
                                para: ParagraphStyle::default(),
                                on_path: None,
                            },
                        );
                        let _ = self.engine.run(&Command::AddShape { layer, shape: sign_shape });
                    }

                    // 5. Optional grommets
                    if banner.grommets {
                        let r = 6.0;
                        let m = 18.0;
                        let mut pts = vec![
                            Point::new(m, m),
                            Point::new(w - m, m),
                            Point::new(m, h - m),
                            Point::new(w - m, h - m),
                        ];
                        let inner_w = w - 2.0 * m;
                        if inner_w > 1800.0 {
                            let count = (inner_w / 1200.0).round() as usize;
                            if count > 1 {
                                let step = inner_w / count as f64;
                                for k in 1..count {
                                    let gx = m + k as f64 * step;
                                    pts.push(Point::new(gx, m));
                                    pts.push(Point::new(gx, h - m));
                                }
                            }
                        }
                        for pt in pts {
                            let mut circle = Shape::new(
                                self.engine.new_shape_id(),
                                ShapeKind::Ellipse {
                                    rect: Rect::new(pt.x - r, pt.y - r, pt.x + r, pt.y + r),
                                    arc: None,
                                },
                            );
                            circle.fill = Some(Fill::Solid(Color::WHITE));
                            circle.stroke = Some(Stroke::hairline(Color::rgb8(120, 120, 120)));
                            let _ = self.engine.run(&Command::AddShape { layer, shape: circle });
                        }
                    }
                }
                if let Some(first) = first_page {
                    self.page = first;
                }
            }
            OutputMode::CurrentPage => {
                let Some(layer) = self.active_layer() else {
                    self.engine.end_group(mark, "Batch banners");
                    return;
                };
                let (top, left) = match self.doc().page(self.page) {
                    Ok(p) => (p.size.height, (p.size.width - banner.width_mm.max(1.0)) / 2.0),
                    Err(_) => (297.0, 0.0),
                };
                let built = banner.build(top, left);
                let mut created: Vec<tracedraw_core::ShapeId> = Vec::new();
                for shapes in built {
                    let mut ids: Vec<tracedraw_core::ShapeId> = Vec::new();
                    for (kind, fill, stroke) in shapes {
                        let id = self.engine.new_shape_id();
                        let mut s = Shape::new(id, kind);
                        s.fill = Some(fill);
                        s.stroke = stroke;
                        ids.push(id);
                        let _ = self.engine.run(&Command::AddShape { layer, shape: s });
                    }
                    if ids.len() > 1 {
                        let _ = self.engine.run(&Command::Group { shapes: ids });
                        if let Some(s) = self.doc().shapes_on(layer).last() {
                            created.push(s.id);
                        }
                    } else if let Some(&first) = ids.first() {
                        created.push(first);
                    }
                }
                self.selection = created;
            }
        }

        self.engine.end_group(mark, "Batch banners");
        let count = lines.len();
        self.status = crate::i18n::trf("banner.done", &[("n", &count.to_string())]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A batch of banners: one banner per slogan, each a group of its own.
    #[test]
    fn a_batch_of_banners_is_a_group_for_each_slogan() {
        let mut app = App::headless();
        let banner = Banner {
            slogans: vec!["第一".into(), "第二".into(), "第三".into()],
            signature: "落款".into(),
            output_mode: OutputMode::CurrentPage,
            ..Banner::default()
        };
        app.create_banners(&banner);
        let shapes = app.selected_shapes();
        // every banner is a group, and the groups are what is selected
        let groups = shapes
            .iter()
            .filter(|s| matches!(s.kind, ShapeKind::Group { .. }))
            .count();
        assert_eq!(groups, 3, "{groups} groups in {shapes:?}");
    }

    /// The empty lines are left out: a blank line is not a banner.
    #[test]
    fn a_blank_line_is_not_a_banner() {
        let banner = Banner {
            slogans: vec!["一".into(), "  ".into(), "二".into(), "".into()],
            ..Banner::default()
        };
        assert_eq!(banner.lines().len(), 2, "{:?}", banner.lines());
    }

    /// Clean text replaces punctuation with spaces by default.
    #[test]
    fn clean_slogan_replaces_punctuation() {
        let banner = Banner::default();
        assert_eq!(banner.clean_slogan("热烈庆祝，祖国华诞！"), "热烈庆祝 祖国华诞");
    }

    /// Multi-page mode generates separate pages.
    #[test]
    fn multi_page_output_generates_pages() {
        let mut app = App::headless();
        let banner = Banner {
            slogans: vec!["横幅一".into(), "横幅二".into()],
            output_mode: OutputMode::Pages,
            ..Banner::default()
        };
        app.create_banners(&banner);
        assert_eq!(app.doc().pages.len(), 3); // initial 1 page + 2 banner pages
    }
}
