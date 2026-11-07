//! Batch banners: a set of slogans, each on a banner of its own.
//!
//! Standard metric sizes for sign and print shops:
//! - Standard roll widths: 400 to 1200 mm (700 mm default)
//! - Side margins for tying ropes: 400 mm
//! - Top and bottom margins: 50 mm
//! - Letter spacing: 50 mm (auto expanded for few characters)
//! - Automatic text compression in fixed length mode (no truncation)
//! - Horizontal and vertical banner orientations
//! - Classic color presets (red cloth with yellow text, etc.)
//! - Output to current canvas (default) or separate pages

use crate::app::App;
use tracedraw_core::{
    document::{ParagraphStyle, Shape, ShapeKind, TextAlign, TextSpan, VerticalAlign},
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
    /// Fixed total length in millimetres (default).
    Fixed,
    /// Auto calculate banner length from text count and spacing.
    Auto,
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
    /// Stack banners on the current canvas page (default).
    CurrentPage,
    /// Each banner becomes an independent page of its own exact dimensions.
    Pages,
}

/// What a batch of banners is made of.
#[derive(Debug, Clone, PartialEq)]
pub struct Banner {
    /// The slogans, one banner each. An empty line is left out.
    pub slogans: Vec<String>,
    /// Optional signature under every slogan. Empty if none.
    pub signature: String,
    /// The cloth roll width (height of horizontal banner, width of vertical banner) in millimetres.
    pub height_mm: f64,
    /// Length mode: fixed total length (default) or auto by text.
    pub length_mode: LengthMode,
    /// The length of a banner in fixed mode (millimetres).
    pub width_mm: f64,
    /// Tying margins at both ends in millimetres (default: 400 mm).
    pub margin_lr_mm: f64,
    /// Top and bottom safety margins in millimetres (default: 50 mm).
    pub margin_tb_mm: f64,
    /// Character spacing between letters in millimetres (default: 50 mm).
    pub spacing_mm: f64,
    /// Character height ratio of available cloth height (default: 0.75).
    pub height_ratio: f64,
    /// The room between two banners when stacked on current page.
    pub gap_mm: f64,
    /// The size of the slogan, in points (auto computed).
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
    /// Output mode: current canvas (default) or separate pages.
    pub output_mode: OutputMode,
    /// Whether the banner is vertical (竖幅) or horizontal (横幅, default).
    pub vertical: bool,
}

impl Default for Banner {
    fn default() -> Banner {
        Banner {
            slogans: vec!["热烈庆祝中华人民共和国成立七十五周年".into(), "不忘初心 牢记使命 砥砺前行".into()],
            signature: String::new(),
            height_mm: 700.0,
            length_mode: LengthMode::Fixed,
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
            output_mode: OutputMode::CurrentPage,
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

    /// The slogans that get a banner: empty lines are left out, and lines are cleaned.
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

    /// Calculate dimensions (width_mm, height_mm, font_size_pt, eff_spacing_mm) for a single slogan.
    pub fn calculate_size(&self, slogan: &str) -> (f64, f64, f64, f64) {
        let chars: Vec<char> = slogan.chars().collect();
        let n_chars = chars.len();
        let n_gaps = n_chars.saturating_sub(1);
        let base_spacing = self.spacing_mm.max(0.0);
        let has_sign = !self.signature.trim().is_empty();

        if self.vertical {
            // Vertical banner: cloth roll is width (w), banner length is height (h).
            let cloth_w = self.height_mm.max(100.0);
            let margin_side = self.margin_tb_mm.max(0.0);
            let avail_w = (cloth_w - 2.0 * margin_side).max(20.0);
            let target_char_h = (avail_w * self.height_ratio.clamp(0.4, 0.95)).max(15.0);
            let sign_h = if has_sign { (target_char_h * 2.0).clamp(60.0, 300.0) } else { 0.0 };

            let mut raw_chars_h = 0.0;
            for &c in &chars {
                if c.is_ascii_graphic() || c.is_ascii_digit() {
                    raw_chars_h += target_char_h * 0.55;
                } else if c.is_whitespace() {
                    raw_chars_h += target_char_h * 0.45;
                } else {
                    raw_chars_h += target_char_h;
                }
            }
            let raw_total_h = raw_chars_h + (n_gaps as f64) * base_spacing;
            let margin_ends = self.margin_lr_mm.max(0.0);

            let (banner_len, char_h, eff_spacing) = match self.length_mode {
                LengthMode::Auto => {
                    let banner_len = (2.0 * margin_ends + sign_h + raw_total_h).max(500.0);
                    (banner_len, target_char_h, base_spacing)
                }
                LengthMode::Fixed => {
                    let banner_len = self.width_mm.max(2.0 * margin_ends + 100.0);
                    let avail_h = (banner_len - 2.0 * margin_ends - sign_h).max(50.0);
                    if raw_total_h > avail_h {
                        // Overflow: compress font and spacing proportionally so text fits without truncation
                        let scale = avail_h / raw_total_h;
                        (banner_len, target_char_h * scale, base_spacing * scale)
                    } else {
                        // Few characters: expand spacing to gracefully fill ~72% of the banner
                        let target_span = avail_h * 0.72;
                        if raw_total_h < target_span && n_gaps > 0 {
                            let needed_gap_space = target_span - raw_chars_h;
                            let expanded = (needed_gap_space / n_gaps as f64).clamp(base_spacing, target_char_h * 2.5);
                            (banner_len, target_char_h, expanded)
                        } else if n_chars == 1 {
                            let enlarged = (target_char_h * 1.15).min(avail_w * 0.9);
                            (banner_len, enlarged, base_spacing)
                        } else {
                            (banner_len, target_char_h, base_spacing)
                        }
                    }
                }
            };

            let font_size_pt = char_h * (72.0 / 25.4);
            (cloth_w, banner_len, font_size_pt, eff_spacing)
        } else {
            // Horizontal banner: cloth roll is height (h), banner length is width (w).
            let cloth_h = self.height_mm.max(100.0);
            let margin_tb = self.margin_tb_mm.max(0.0);
            let sign_h = if has_sign { (cloth_h * 0.20).clamp(40.0, 160.0) } else { 0.0 };
            let avail_h = (cloth_h - sign_h - 2.0 * margin_tb).max(20.0);
            let target_char_h = (avail_h * self.height_ratio.clamp(0.4, 0.95)).max(15.0);

            let mut raw_chars_w = 0.0;
            for &c in &chars {
                if c.is_ascii_graphic() || c.is_ascii_digit() {
                    raw_chars_w += target_char_h * 0.55;
                } else if c.is_whitespace() {
                    raw_chars_w += target_char_h * 0.45;
                } else {
                    raw_chars_w += target_char_h;
                }
            }
            let raw_total_w = raw_chars_w + (n_gaps as f64) * base_spacing;
            let margin_lr = self.margin_lr_mm.max(0.0);

            let (banner_w, char_h, eff_spacing) = match self.length_mode {
                LengthMode::Auto => {
                    let banner_w = (2.0 * margin_lr + raw_total_w).max(500.0);
                    (banner_w, target_char_h, base_spacing)
                }
                LengthMode::Fixed => {
                    let banner_w = self.width_mm.max(2.0 * margin_lr + 100.0);
                    let avail_w = (banner_w - 2.0 * margin_lr).max(50.0);
                    if raw_total_w > avail_w {
                        // Overflow: compress font and spacing proportionally so text fits without truncation
                        let scale = avail_w / raw_total_w;
                        (banner_w, target_char_h * scale, base_spacing * scale)
                    } else {
                        // Few characters: expand spacing to gracefully fill ~72% of the banner
                        let target_span = avail_w * 0.72;
                        if raw_total_w < target_span && n_gaps > 0 {
                            let needed_gap_space = target_span - raw_chars_w;
                            let expanded = (needed_gap_space / n_gaps as f64).clamp(base_spacing, target_char_h * 2.5);
                            (banner_w, target_char_h, expanded)
                        } else if n_chars == 1 {
                            let enlarged = (target_char_h * 1.15).min(avail_h * 0.9);
                            (banner_w, enlarged, base_spacing)
                        } else {
                            (banner_w, target_char_h, base_spacing)
                        }
                    }
                }
            };

            let font_size_pt = char_h * (72.0 / 25.4);
            (banner_w, cloth_h, font_size_pt, eff_spacing)
        }
    }

    /// Build shapes for a single banner at (left, top).
    pub fn build_single(&self, slogan: &str, left: f64, top: f64) -> (f64, f64, Vec<(ShapeKind, Fill, Option<Stroke>)>) {
        let (w, h, size_pt, eff_spacing) = self.calculate_size(slogan);
        let mut shapes: Vec<(ShapeKind, Fill, Option<Stroke>)> = Vec::new();
        let has_sign = !self.signature.trim().is_empty();

        // 1. Cloth rectangle
        shapes.push((
            ShapeKind::Rect {
                rect: Rect::new(left, top, left + w, top + h),
                radius: 0.0,
                corners: None,
            },
            Fill::Solid(self.background),
            None,
        ));

        let char_h = size_pt * (25.4 / 72.0);
        let tracking_pct = if char_h > 0.0 { (eff_spacing / char_h) * 100.0 } else { 0.0 };

        if self.vertical {
            // Vertical banner
            let margin_side = self.margin_tb_mm.max(0.0);
            let margin_ends = self.margin_lr_mm.max(0.0);
            let sign_h = if has_sign { (char_h * 2.0).clamp(60.0, 300.0) } else { 0.0 };
            let avail_h = (h - 2.0 * margin_ends - sign_h).max(10.0);
            let avail_w = (w - 2.0 * margin_side).max(10.0);

            // Slogan
            let slogan_span = TextSpan {
                bold: true,
                tracking_pct,
                fill: Some(Fill::Solid(self.slogan_color)),
                ..TextSpan::new(slogan, FONT, size_pt)
            };
            shapes.push((
                ShapeKind::Text {
                    spans: vec![slogan_span],
                    origin: Point::new(left + margin_side, top + margin_ends + sign_h),
                    frame: Some(Size::new(avail_w, avail_h)),
                    align: TextAlign::Center,
                    para: ParagraphStyle {
                        vertical: true,
                        vertical_align: VerticalAlign::Center,
                        ..Default::default()
                    },
                    on_path: None,
                },
                Fill::Solid(self.slogan_color),
                None,
            ));

            // Optional signature at bottom of vertical banner
            if has_sign {
                let sign_pt = (size_pt * 0.40).clamp(14.0, 72.0);
                let sign_span = TextSpan {
                    bold: false,
                    fill: Some(Fill::Solid(self.signature_color)),
                    ..TextSpan::new(self.signature.trim(), FONT, sign_pt)
                };
                shapes.push((
                    ShapeKind::Text {
                        spans: vec![sign_span],
                        origin: Point::new(left + margin_side, top + margin_ends),
                        frame: Some(Size::new(avail_w, sign_h)),
                        align: TextAlign::Center,
                        para: ParagraphStyle {
                            vertical: true,
                            vertical_align: VerticalAlign::Bottom,
                            ..Default::default()
                        },
                        on_path: None,
                    },
                    Fill::Solid(self.signature_color),
                    None,
                ));
            }
        } else {
            // Horizontal banner
            let margin_lr = self.margin_lr_mm.max(0.0);
            let margin_tb = self.margin_tb_mm.max(0.0);
            let sign_h = if has_sign { (h * 0.20).clamp(40.0, 160.0) } else { 0.0 };
            let avail_w = (w - 2.0 * margin_lr).max(10.0);
            let avail_h = (h - 2.0 * margin_tb - sign_h).max(10.0);

            // Slogan
            let slogan_span = TextSpan {
                bold: true,
                tracking_pct,
                fill: Some(Fill::Solid(self.slogan_color)),
                ..TextSpan::new(slogan, FONT, size_pt)
            };
            shapes.push((
                ShapeKind::Text {
                    spans: vec![slogan_span],
                    origin: Point::new(left + margin_lr, top + margin_tb + sign_h),
                    frame: Some(Size::new(avail_w, avail_h)),
                    align: TextAlign::Center,
                    para: ParagraphStyle {
                        vertical_align: VerticalAlign::Center,
                        ..Default::default()
                    },
                    on_path: None,
                },
                Fill::Solid(self.slogan_color),
                None,
            ));

            // Optional signature at bottom-right of horizontal banner
            if has_sign {
                let sign_pt = (size_pt * 0.35).clamp(14.0, 72.0);
                let sign_span = TextSpan {
                    bold: false,
                    fill: Some(Fill::Solid(self.signature_color)),
                    ..TextSpan::new(self.signature.trim(), FONT, sign_pt)
                };
                shapes.push((
                    ShapeKind::Text {
                        spans: vec![sign_span],
                        origin: Point::new(left + margin_lr, top + margin_tb),
                        frame: Some(Size::new(avail_w, sign_h)),
                        align: TextAlign::Right,
                        para: ParagraphStyle {
                            vertical_align: VerticalAlign::Center,
                            ..Default::default()
                        },
                        on_path: None,
                    },
                    Fill::Solid(self.signature_color),
                    None,
                ));
            }
        }

        // Grommets
        if self.grommets {
            let r = 6.0;
            let m = 18.0;
            let mut pts = vec![
                Point::new(left + m, top + m),
                Point::new(left + w - m, top + m),
                Point::new(left + m, top + h - m),
                Point::new(left + w - m, top + h - m),
            ];
            if self.vertical {
                let inner_h = h - 2.0 * m;
                if inner_h > 1800.0 {
                    let count = (inner_h / 1200.0).round() as usize;
                    if count > 1 {
                        let step = inner_h / count as f64;
                        for k in 1..count {
                            let gy = top + m + k as f64 * step;
                            pts.push(Point::new(left + m, gy));
                            pts.push(Point::new(left + w - m, gy));
                        }
                    }
                }
            } else {
                let inner_w = w - 2.0 * m;
                if inner_w > 1800.0 {
                    let count = (inner_w / 1200.0).round() as usize;
                    if count > 1 {
                        let step = inner_w / count as f64;
                        for k in 1..count {
                            let gx = left + m + k as f64 * step;
                            pts.push(Point::new(gx, top + m));
                            pts.push(Point::new(gx, top + h - m));
                        }
                    }
                }
            }
            for pt in pts {
                shapes.push((
                    ShapeKind::Ellipse {
                        rect: Rect::new(pt.x - r, pt.y - r, pt.x + r, pt.y + r),
                        arc: None,
                    },
                    Fill::Solid(Color::WHITE),
                    Some(Stroke::hairline(Color::rgb8(120, 120, 120))),
                ));
            }
        }

        (w, h, shapes)
    }

    /// The banners as shapes, laid out from `(top, left)`.
    pub fn build(&self, top: f64, left: f64) -> Vec<Vec<(ShapeKind, Fill, Option<Stroke>)>> {
        let mut out: Vec<Vec<(ShapeKind, Fill, Option<Stroke>)>> = Vec::new();
        let mut cur_x = left;
        let mut cur_y = top;

        for slogan in self.lines() {
            let (w, h, shapes) = self.build_single(&slogan, cur_x, cur_y);
            out.push(shapes);
            if self.vertical {
                cur_x += w + self.gap_mm.max(0.0);
            } else {
                cur_y += h + self.gap_mm.max(0.0);
            }
        }
        out
    }
}

impl App {
    /// Make a batch of banners: either as separate pages (one per banner)
    /// or laid out on the current page.
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
                    let (w, h, shapes) = banner.build_single(slogan, 0.0, 0.0);
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

                    for (kind, fill, stroke) in shapes {
                        let id = self.engine.new_shape_id();
                        let mut s = Shape::new(id, kind);
                        s.fill = fill;
                        s.stroke = stroke;
                        let _ = self.engine.run(&Command::AddShape { layer, shape: s });
                    }
                }
                if let Some(first) = first_page {
                    self.page = first;
                }
                self.zoom_to_page();
            }
            OutputMode::CurrentPage => {
                let Some(layer) = self.active_layer() else {
                    self.engine.end_group(mark, "Batch banners");
                    return;
                };

                let empty_page = self.doc().layer(layer).map(|l| l.shapes.is_empty()).unwrap_or(true);
                let (top, left) = if empty_page {
                    // Empty canvas: calculate total extents and resize page to fit nicely
                    let mut tot_w = 0.0;
                    let mut tot_h = 0.0;
                    for (i, slogan) in lines.iter().enumerate() {
                        let (w, h, _, _) = banner.calculate_size(slogan);
                        if banner.vertical {
                            tot_w += if i == 0 { w } else { w + banner.gap_mm.max(0.0) };
                            tot_h = tot_h.max(h);
                        } else {
                            tot_w = tot_w.max(w);
                            tot_h += if i == 0 { h } else { h + banner.gap_mm.max(0.0) };
                        }
                    }
                    let _ = self.engine.run(&Command::ResizePage {
                        page: self.page,
                        size: Size::new(tot_w.max(100.0), tot_h.max(100.0)),
                    });
                    (0.0, 0.0)
                } else {
                    let page_h = self.doc().page(self.page).map(|p| p.size.height).unwrap_or(297.0);
                    (page_h + banner.gap_mm.max(50.0), 0.0)
                };

                let built = banner.build(top, left);
                let mut created: Vec<tracedraw_core::ShapeId> = Vec::new();
                for shapes in built {
                    let mut ids: Vec<tracedraw_core::ShapeId> = Vec::new();
                    for (kind, fill, stroke) in shapes {
                        let id = self.engine.new_shape_id();
                        let mut s = Shape::new(id, kind);
                        s.fill = fill;
                        s.stroke = stroke;
                        ids.push(id);
                        let _ = self.engine.run(&Command::AddShape { layer, shape: s });
                    }
                    if ids.len() > 1 {
                        let _ = self.engine.run(&Command::Group { shapes: ids });
                        if let Ok(l) = self.doc().layer(layer) {
                            if let Some(s) = l.shapes.last() {
                                created.push(s.id);
                            }
                        }
                    } else if let Some(&first) = ids.first() {
                        created.push(first);
                    }
                }
                self.selection = created;
                self.zoom_to_selection();
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

    /// Multi-page output generates pages.
    #[test]
    fn multi_page_output_generates_pages() {
        let mut app = App::headless();
        let banner = Banner {
            slogans: vec!["横幅一".into(), "横幅二".into()],
            output_mode: OutputMode::Pages,
            ..Banner::default()
        };
        app.create_banners(&banner);
        assert_eq!(app.doc().pages.len(), 3);
    }

    /// Long text in fixed mode scales down character size and spacing to fit.
    #[test]
    fn fixed_mode_compresses_overflowing_text() {
        let banner = Banner {
            slogans: vec!["这是一个非常非常非常非常非常非常非常非常非常长的标语测试".into()],
            width_mm: 3000.0,
            length_mode: LengthMode::Fixed,
            ..Banner::default()
        };
        let (w, _h, size_pt, spacing) = banner.calculate_size(&banner.slogans[0]);
        assert_eq!(w, 3000.0);
        let char_h = size_pt * (25.4 / 72.0);
        assert!(char_h < 400.0, "font should be scaled down: {char_h}");
        assert!(spacing < banner.spacing_mm, "spacing should be scaled down: {spacing}");
    }

    /// Short text in fixed mode expands spacing to fill a suitable span.
    #[test]
    fn few_characters_expand_spacing() {
        let banner = Banner {
            slogans: vec!["庆祝元旦".into()],
            width_mm: 6000.0,
            length_mode: LengthMode::Fixed,
            ..Banner::default()
        };
        let (w, _h, _size_pt, spacing) = banner.calculate_size(&banner.slogans[0]);
        assert_eq!(w, 6000.0);
        assert!(spacing > banner.spacing_mm, "spacing should be expanded: {spacing}");
    }

    /// Vertical banner sets roll width as width and length as height.
    #[test]
    fn vertical_banner_dimensions_and_text() {
        let banner = Banner {
            slogans: vec!["欢度国庆".into()],
            height_mm: 700.0,
            width_mm: 5000.0,
            vertical: true,
            length_mode: LengthMode::Fixed,
            ..Banner::default()
        };
        let (w, h, size_pt, spacing) = banner.calculate_size(&banner.slogans[0]);
        assert_eq!(w, 700.0, "cloth roll width is width");
        assert_eq!(h, 5000.0, "banner length is height");
        assert!(size_pt > 0.0);
        assert!(spacing >= banner.spacing_mm);
    }
}
