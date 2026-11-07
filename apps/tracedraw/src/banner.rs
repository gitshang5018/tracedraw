//! Batch banners: a set of slogans, each on a banner of its own.
//!
//! A banner is a rectangle of colour with the slogan written across it and
//! the signature under it, the way a shop hangs a banner over its door. A
//! batch of them is a list of slogans, one line each, and the same banner
//! is made of every line: the same size, the same colours, the same
//! letter, the same signature.
//!
//! The banners are laid out down the page, one under the other, and each
//! is a group of its own: the rectangle, the slogan and the signature can
//! be picked apart afterwards, and the whole batch is one undo step.

use crate::app::App;
use tracedraw_core::{
    document::{ParagraphStyle, Shape, ShapeKind, TextAlign, TextSpan},
    geometry::{Point, Rect, Size},
    Color, Command, Fill, Stroke,
};

/// The font the banners are written in.
const FONT: &str = "sans";

/// What a batch of banners is made of.
#[derive(Debug, Clone, PartialEq)]
/// Standard cloth roll widths in millimetres (400 to 1200 mm).
pub const PRESET_HEIGHTS_MM: [f64; 8] = [
    400.0, 500.0, 600.0, 700.0, 800.0, 900.0, 1000.0, 1200.0,
];

pub struct Banner {
    /// The slogans, one banner each. An empty line is left out.
    pub slogans: Vec<String>,
    /// The signature under every slogan.
    pub signature: String,
    /// The width of a banner, in millimetres.
    pub width_mm: f64,
    /// The height of a banner, in millimetres.
    pub height_mm: f64,
    /// The room between two banners, in millimetres.
    pub gap_mm: f64,
    /// The size of the slogan, in points.
    pub slogan_pt: f64,
    /// The size of the signature, in points.
    pub signature_pt: f64,
    /// The colour of the banner itself.
    pub background: Color,
    /// The colour of the slogan.
    pub slogan_color: Color,
    /// The colour of the signature.
    pub signature_color: Color,
    /// The colour of the band along the bottom of the banner.
    pub band_color: Color,
    /// Whether the slogan is written along the banner or down it.
    pub vertical: bool,
}

impl Default for Banner {
    fn default() -> Banner {
        Banner {
            slogans: vec!["开业大吉".into(), "欢迎光临".into()],
            signature: "示例商贸".into(),
            width_mm: 6000.0,
            height_mm: 700.0,
            gap_mm: 100.0,
            slogan_pt: 1400.0,
            signature_pt: 500.0,
            background: Color::rgb8(0xC8, 0x1E, 0x2E),
            slogan_color: Color::rgb8(0xFF, 0xE7, 0xA8),
            signature_color: Color::rgb8(0xFF, 0xFF, 0xFF),
            band_color: Color::rgb8(0x8A, 0x14, 0x1F),
            vertical: false,
        }
    }
}

impl Banner {
    /// The slogans that get a banner: the empty lines are left out, and
    /// the lines are trimmed.
    pub fn lines(&self) -> Vec<String> {
        self.slogans
            .iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// The banners as shapes, laid out down the page from `top`.
    ///
    /// Each banner is a list of its own: the rectangle of colour, the band
    /// along the bottom, the slogan and the signature. The rectangle is
    /// the one the others are placed against, so it comes first.
    pub fn build(&self, top: f64, left: f64) -> Vec<Vec<(ShapeKind, Fill, Option<Stroke>)>> {
        let mut out: Vec<Vec<(ShapeKind, Fill, Option<Stroke>)>> = Vec::new();
        let w = self.width_mm.max(1.0);
        let h = self.height_mm.max(1.0);
        let band = (h * 0.18).max(6.0);
        let pad = (h * 0.08).max(4.0);
        for (k, slogan) in self.lines().iter().enumerate() {
            let y0 = top + k as f64 * (h + self.gap_mm.max(0.0));
            let mut banner: Vec<(ShapeKind, Fill, Option<Stroke>)> = Vec::new();
            // the rectangle of colour
            banner.push((
                ShapeKind::Rect {
                    rect: Rect::new(left, y0, left + w, y0 + h),
                    radius: 0.0,
                    corners: None,
                },
                Fill::Solid(self.background),
                Some(Stroke::hairline(self.band_color)),
            ));
            // the band along the bottom, which the signature is written on
            banner.push((
                ShapeKind::Rect {
                    rect: Rect::new(left, y0 + h - band, left + w, y0 + h),
                    radius: 0.0,
                    corners: None,
                },
                Fill::Solid(self.band_color),
                None,
            ));
            // The slogan, across the banner or down it.
            //
            // A frame's origin is its bottom left corner, so the frame is
            // placed by where its bottom left goes, and the text is
            // centred in it.
            let slogan_h = self.slogan_pt * 0.3528 * 1.6;
            let (slogan_origin, slogan_frame, vertical) = if self.vertical {
                // Down the banner: the frame is as tall as the room above
                // the band and as wide as the letters need, and the text
                // is set the way CJK typesetting sets it, in columns read
                // from the top right.
                let frame = Size::new(slogan_h * 1.6, h - band - pad * 2.0);
                (
                    Point::new(
                        left + w * 0.5 - frame.width * 0.5,
                        y0 + band + pad,
                    ),
                    frame,
                    true,
                )
            } else {
                let frame = Size::new(w - pad * 2.0, slogan_h * 1.6);
                (
                    Point::new(
                        left + pad,
                        y0 + (h - band) * 0.5 - frame.height * 0.5,
                    ),
                    frame,
                    false,
                )
            };
            banner.push((
                ShapeKind::Text {
                    spans: vec![TextSpan {
                        text: slogan.clone(),
                        font_family: FONT.into(),
                        size_pt: self.slogan_pt,
                        bold: true,
                        ..TextSpan::new(slogan, FONT, self.slogan_pt)
                    }],
                    origin: slogan_origin,
                    frame: Some(slogan_frame),
                    align: TextAlign::Center,
                    para: ParagraphStyle {
                        vertical,
                        ..ParagraphStyle::default()
                    },
                    on_path: None,
                },
                Fill::Solid(self.slogan_color),
                None,
            ));
            // the signature, along the band
            if !self.signature.trim().is_empty() {
                let sign_h = self.signature_pt * 0.3528 * 1.6;
                let frame = Size::new(w * 0.9, sign_h * 1.6);
                banner.push((
                    ShapeKind::Text {
                        spans: vec![TextSpan {
                            text: self.signature.trim().into(),
                            font_family: FONT.into(),
                            size_pt: self.signature_pt,
                            bold: false,
                            ..TextSpan::new(self.signature.trim(), FONT, self.signature_pt)
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
    /// Make a batch of banners on the current page.
    ///
    /// The banners are laid out from the top of the page, one under the
    /// other, each a group of its own. The whole batch is one undo step.
    pub fn create_banners(&mut self, banner: &Banner) {
        let lines = banner.lines();
        if lines.is_empty() {
            self.status = crate::i18n::tr("banner.need_slogan");
            return;
        }
        let Some(layer) = self.active_layer() else {
            return;
        };
        // the page's own frame, and the room the banners take
        let (top, left) = match self.doc().page(self.page) {
            Ok(p) => (p.size.height, (p.size.width - banner.width_mm.max(1.0)) / 2.0),
            Err(_) => (297.0, 0.0),
        };
        let built = banner.build(top, left);
        let mut cmds: Vec<Command> = Vec::new();
        for shapes in built {
            let mut ids: Vec<tracedraw_core::ShapeId> = Vec::new();
            for (kind, fill, stroke) in shapes {
                let id = self.engine.new_shape_id();
                let mut s = Shape::new(id, kind);
                s.fill = fill;
                s.stroke = stroke;
                ids.push(id);
                cmds.push(Command::AddShape { layer, shape: s });
            }
            // The four objects of a banner are a group, so the banner is
            // picked up and moved as one thing.
            if ids.len() > 1 {
                cmds.push(Command::Group { shapes: ids });
            }
        }
        // The banners are what is made, so they are what is selected. The
        // group is a shape of its own with an id of its own, so it is
        // picked up from the layer by where it was put.
        let before = self
            .doc()
            .page(self.page)
            .ok()
            .and_then(|p| p.layers.iter().find(|l| l.id == layer))
            .map(|l| l.shapes.len())
            .unwrap_or(0);
        let _ = self.engine.run_batch("Batch banners", &cmds);
        let made: Vec<tracedraw_core::ShapeId> = self
            .doc()
            .page(self.page)
            .ok()
            .and_then(|p| p.layers.iter().find(|l| l.id == layer))
            .map(|l| {
                l.shapes
                    .iter()
                    .skip(before)
                    .map(|s| s.id)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if !made.is_empty() {
            self.select(made);
        }
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

    /// A banner is a rectangle, a band, a slogan and a signature, and the
    /// banner is as wide as it was told to be.
    #[test]
    fn a_banner_is_the_size_it_was_given() {
        let banner = Banner {
            slogans: vec!["标语".into()],
            signature: "落款".into(),
            width_mm: 600.0,
            height_mm: 200.0,
            ..Banner::default()
        };
        let built = banner.build(0.0, 0.0);
        assert_eq!(built.len(), 1, "one banner");
        let shapes = &built[0];
        assert_eq!(shapes.len(), 4, "a rectangle, a band, a slogan and a signature");
        match &shapes[0].0 {
            ShapeKind::Rect { rect, .. } => {
                assert!((rect.width() - 600.0).abs() < 1e-9, "{rect:?}");
                assert!((rect.height() - 200.0).abs() < 1e-9, "{rect:?}");
            }
            other => panic!("the banner is not a rectangle: {other:?}"),
        }
    }

    /// The banners are one under the other, with the room between them
    /// that was asked for.
    #[test]
    fn the_banners_are_one_under_the_other() {
        let banner = Banner {
            slogans: vec!["一".into(), "二".into()],
            width_mm: 400.0,
            height_mm: 100.0,
            gap_mm: 30.0,
            ..Banner::default()
        };
        let built = banner.build(500.0, 0.0);
        assert_eq!(built.len(), 2);
        let top_of = |i: usize| match &built[i][0].0 {
            ShapeKind::Rect { rect, .. } => rect.y0,
            _ => f64::MIN,
        };
        assert!((top_of(0) - 500.0).abs() < 1e-9, "{}", top_of(0));
        // the second is a height and a gap below the first
        assert!(
            (top_of(1) - (top_of(0) + 100.0 + 30.0)).abs() < 1e-9,
            "{}",
            top_of(1)
        );
    }

    /// The words lie where they belong: the slogan in the room above the
    /// band, the signature on the band.
    #[test]
    fn the_words_lie_inside_the_banner() {
        let banner = Banner {
            slogans: vec!["标语".into()],
            signature: "落款".into(),
            width_mm: 600.0,
            height_mm: 200.0,
            ..Banner::default()
        };
        let built = banner.build(0.0, 0.0);
        let shapes = &built[0];
        let rect_of = |i: usize| match &shapes[i].0 {
            ShapeKind::Rect { rect, .. } => *rect,
            other => panic!("not a rectangle: {other:?}"),
        };
        let frame_of = |i: usize| match &shapes[i].0 {
            ShapeKind::Text { origin, frame, .. } => (*origin, frame.unwrap()),
            other => panic!("not text: {other:?}"),
        };
        let banner_rect = rect_of(0);
        let band_rect = rect_of(1);
        let (slogan_origin, slogan_frame) = frame_of(2);
        let (sign_origin, sign_frame) = frame_of(3);
        // The slogan is above the band and inside the banner's sides.
        assert!(
            slogan_origin.y >= banner_rect.y0,
            "the slogan is above the banner: {slogan_origin:?}"
        );
        assert!(
            slogan_origin.y + slogan_frame.height <= band_rect.y0 + 1e-9,
            "the slogan runs into the band: {slogan_origin:?} {:?}",
            slogan_frame.height
        );
        assert!(
            slogan_origin.x >= banner_rect.x0 && slogan_origin.x + slogan_frame.width <= banner_rect.x1,
            "the slogan runs out of the banner: {slogan_origin:?} {:?}",
            slogan_frame.width
        );
        // The signature is on the band.
        assert!(
            sign_origin.y >= band_rect.y0 - 1e-9 && sign_origin.y + sign_frame.height <= band_rect.y1 + 1e-9,
            "the signature is not on the band: {sign_origin:?} {:?}",
            sign_frame.height
        );
    }

    /// A slogan written down the banner is in a frame that is taller than
    /// it is wide.
    #[test]
    fn a_vertical_slogan_is_in_a_tall_frame() {
        let banner = Banner {
            slogans: vec!["标语".into()],
            signature: "落款".into(),
            width_mm: 600.0,
            height_mm: 200.0,
            vertical: true,
            ..Banner::default()
        };
        let built = banner.build(0.0, 0.0);
        match &built[0][2].0 {
            ShapeKind::Text { frame, para, .. } => {
                let f = frame.unwrap();
                assert!(f.height > f.width, "the frame is {f:?}");
                assert!(para.vertical, "the text is not set vertically");
            }
            other => panic!("not text: {other:?}"),
        }
    }

    /// A banner with no signature is a rectangle, a band and a slogan.
    #[test]
    fn a_banner_without_a_signature_has_three_shapes() {
        let banner = Banner {
            slogans: vec!["标语".into()],
            signature: String::new(),
            ..Banner::default()
        };
        let built = banner.build(0.0, 0.0);
        assert_eq!(built.len(), 1);
        assert_eq!(built[0].len(), 3, "{:?}", built[0].len());
    }
}
