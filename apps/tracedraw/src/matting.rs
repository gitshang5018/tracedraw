//! AI matting: a picture through a model that says which of its pixels are
//! the subject.
//!
//! Two models, both ONNX: MODNet for people, RMBG 1.4 for anything. Each
//! has its own idea of the picture it wants (a side to scale to, a mean and
//! a spread per channel), and each answers with one value per pixel: how
//! much of the subject is there, from 0 to 1.
//!
//! The browser build carries no models, so it does without; the colour
//! pass in `bg_remove` is what a picture gets when no model is on hand.
//!
//! See `docs/behavior/ai-tools.md`.

use image::{GrayImage, Luma, RgbaImage};
use std::path::Path;

/// What a model expects of the picture it is given.
pub struct Recipe {
    /// The side the picture is squared off to.
    pub side: usize,
    /// Per channel: `(v / 255 - mean) / scale`.
    pub mean: [f32; 3],
    pub scale: [f32; 3],
}

/// MODNet: people, on a picture scaled to half a thousand pixels either way
/// and taken to `[-1, 1]`.
pub const MODNET: Recipe = Recipe {
    side: 512,
    mean: [0.5, 0.5, 0.5],
    scale: [0.5, 0.5, 0.5],
};

/// RMBG 1.4: anything, on a picture scaled to a thousand and twenty four
/// pixels and taken to `[-0.5, 0.5]`, which is what its own preprocessing
/// does: a half taken off, and nothing divided. The ImageNet spread other
/// models use is wrong here, and a model fed the wrong spread answers
/// nonsense that still looks like a mask.
pub const RMBG: Recipe = Recipe {
    side: 1024,
    mean: [0.5, 0.5, 0.5],
    scale: [1.0, 1.0, 1.0],
};

/// The recipe of the model named `key`.
pub fn recipe(key: &str) -> Recipe {
    if key.starts_with("rmbg") {
        RMBG
    } else {
        MODNET
    }
}

/// The subject's mask for `img`, one byte per pixel, the size of the
/// picture: white where the subject is.
pub fn matte(img: &RgbaImage, path: &Path, key: &str) -> Result<GrayImage, String> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (img, path, key);
        return Err("the browser build carries no models".to_string());
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let (w, h) = img.dimensions();
        if w == 0 || h == 0 {
            return Err("an empty picture".to_string());
        }
        let r = recipe(key);
        let side = r.side;
        let small = image::imageops::resize(
            img,
            side as u32,
            side as u32,
            image::imageops::FilterType::Triangle,
        );
        // Channel first, the order a model reads, each taken to its range.
        let mut data = vec![0f32; 3 * side * side];
        for c in 0..3 {
            for y in 0..side {
                for x in 0..side {
                    let p = small.get_pixel(x as u32, y as u32);
                    data[c * side * side + y * side + x] =
                        (p[c] as f32 / 255.0 - r.mean[c]) / r.scale[c];
                }
            }
        }
        let mut mask = run(path, &data, side)?;
        if key.starts_with("rmbg") {
            // RMBG answers a field of its own scale, not a mask: what it
            // says is stretched between its own smallest and largest value,
            // which is what its preprocessing does before anything is shown.
            // Clamped instead, a field that runs past 0 and 1 comes back
            // as flat black and white.
            mask = spread(&mask);
        }
        let mut out = GrayImage::new(side as u32, side as u32);
        for y in 0..side {
            for x in 0..side {
                let v = refine(mask.get(y * side + x).copied().unwrap_or(0.0));
                out.put_pixel(x as u32, y as u32, Luma([(v * 255.0).round() as u8]));
            }
        }
        Ok(image::imageops::resize(
            &out,
            w,
            h,
            image::imageops::FilterType::Triangle,
        ))
    }
}

/// Put the picture through the model and take its one channel back.
#[cfg(not(target_arch = "wasm32"))]
fn run(path: &Path, data: &[f32], side: usize) -> Result<Vec<f32>, String> {
    use tract_onnx::prelude::*;
    // The picture's size is given to the model before it is optimised: a
    // resize layer whose output size follows from its input can then be
    // worked out once, at load, instead of being asked to resize by a
    // factor it cannot see at run time.
    let model = tract_onnx::onnx()
        .model_for_path(path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .with_input_fact(0, f32::fact([1, 3, side, side]).into())
        .map_err(|e| format!("{}: {e}", path.display()))?
        .into_optimized()
        .map_err(|e| format!("{}: {e}", path.display()))?
        .into_runnable()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let input = Tensor::from_shape(&[1, 3, side, side], data)
        .map_err(|e| format!("the picture could not be shaped: {e}"))?;
    let outputs = model
        .run(tvec!(input.into()))
        .map_err(|e| format!("the model stopped: {e}"))?;
    let answer = outputs.first().ok_or("the model answered nothing")?;
    let shape: Vec<usize> = answer.shape().to_vec();
    let (h, w) = match shape.len() {
        4 => (shape[2], shape[3]),
        3 => (shape[1], shape[2]),
        2 => (shape[0], shape[1]),
        _ => return Err(format!("the model answered {shape:?}")),
    };
    // The batch and channel dimensions are one apiece, so what the view
    // walks is the mask itself; a walk of the values is asked for rather
    // than a slice, since the shape gymnastics of a view are what changes
    // between one version of the reader and the next.
    let view = answer
        .to_plain_array_view::<f32>()
        .map_err(|e| format!("the model's answer is not a mask: {e}"))?;
    let out: Vec<f32> = view.iter().copied().collect();
    if out.len() != h * w {
        return Err(format!("the model answered {shape:?}, which is not one mask"));
    }
    if h == side && w == side {
        return Ok(out);
    }
    // A model that answers at another size is taken back to the one asked
    // for, so the caller has one shape to deal with.
    let bytes: Vec<u8> = out
        .iter()
        .map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
        .collect();
    let Some(small) = GrayImage::from_raw(w as u32, h as u32, bytes) else {
        return Err("the model's answer has the wrong size".to_string());
    };
    let big = image::imageops::resize(
        &small,
        side as u32,
        side as u32,
        image::imageops::FilterType::Triangle,
    );
    Ok(big.pixels().map(|p| p[0] as f32 / 255.0).collect())
}

/// A model's answer stretched between its own smallest and largest value.
fn spread(values: &[f32]) -> Vec<f32> {
    let mut lo = f32::INFINITY;
    let mut hi = f32::NEG_INFINITY;
    for v in values {
        if v.is_finite() {
            lo = lo.min(*v);
            hi = hi.max(*v);
        }
    }
    if !(hi > lo) {
        return values.to_vec();
    }
    values
        .iter()
        .map(|v| ((v - lo) / (hi - lo)).clamp(0.0, 1.0))
        .collect()
}

/// A model's answer with its edge pulled apart: what it was unsure about,
/// around the half mark, becomes either the subject or the background, so
/// the cut looks cut rather than smudged. This is a smoothstep over the
/// narrow band a matte's edge lives in, the same the plugin's edge pass
/// uses; the middle of the subject and the middle of the background are
/// left alone.
fn refine(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    let t = ((v - 0.02) / (0.98 - 0.02)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// `img` with its alpha taken from `mask`, which is scaled to fit.
pub fn applied(img: &RgbaImage, mask: &GrayImage) -> RgbaImage {
    let (w, h) = img.dimensions();
    let (mw, mh) = mask.dimensions();
    let mut out = img.clone();
    if mw == 0 || mh == 0 {
        return out;
    }
    for (x, y, p) in out.enumerate_pixels_mut() {
        let mx = (x.min(w.saturating_sub(1)) as u64 * mw as u64 / w.max(1) as u64) as u32;
        let my = (y.min(h.saturating_sub(1)) as u64 * mh as u64 / h.max(1) as u64) as u32;
        let m = mask.get_pixel(mx.min(mw - 1), my.min(mh - 1))[0] as u32;
        p[3] = ((p[3] as u32 * m) / 255) as u8;
    }
    out
}

/// A piece of the subject: its own pixels, and the box they came from.
#[derive(Clone)]
pub struct Piece {
    /// The pixels, cut to the box and with the mask's alpha.
    pub img: RgbaImage,
    /// Where the piece sat in the picture: `(x, y, width, height)`.
    pub bounds: (u32, u32, u32, u32),
}

impl Piece {
    /// The width and the height of the piece, in pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.bounds.2, self.bounds.3)
    }
}

/// The box around everything `mask` keeps, or the whole of it when the
/// mask keeps nothing: a picture whose subject was not found is not
/// cropped to nothing.
pub fn content_box(mask: &GrayImage, threshold: u8) -> (u32, u32, u32, u32) {
    let (w, h) = mask.dimensions();
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0u32, 0u32);
    for y in 0..h {
        for x in 0..w {
            if mask.get_pixel(x, y)[0] <= threshold {
                continue;
            }
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    if x0 == u32::MAX {
        return (0, 0, w, h);
    }
    (x0, y0, x1 - x0 + 1, y1 - y0 + 1)
}

/// The boxes of the separate subjects in `mask`, the largest first: two
/// pixels belong to the same subject when they touch, sides or corners.
/// Boxes smaller than `min_area` pixels are left out, which is what keeps
/// the specks a matte leaves behind from becoming objects of their own.
pub fn components(mask: &GrayImage, threshold: u8, min_area: u32) -> Vec<(u32, u32, u32, u32)> {
    let (w, h) = mask.dimensions();
    if w == 0 || h == 0 {
        return Vec::new();
    }
    let mut seen = vec![false; w as usize * h as usize];
    let mut boxes = Vec::new();
    let mut stack: Vec<(u32, u32)> = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let first = y as usize * w as usize + x as usize;
            if seen.get(first).copied().unwrap_or(true) {
                continue;
            }
            if mask.get_pixel(x, y)[0] <= threshold {
                if let Some(s) = seen.get_mut(first) {
                    *s = true;
                }
                continue;
            }
            let (mut x0, mut y0, mut x1, mut y1) = (x, y, x, y);
            let mut area = 0u32;
            stack.push((x, y));
            if let Some(s) = seen.get_mut(first) {
                *s = true;
            }
            while let Some((cx, cy)) = stack.pop() {
                area += 1;
                x0 = x0.min(cx);
                y0 = y0.min(cy);
                x1 = x1.max(cx);
                y1 = y1.max(cy);
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let nx = cx as i32 + dx;
                        let ny = cy as i32 + dy;
                        if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                            continue;
                        }
                        let (nx, ny) = (nx as u32, ny as u32);
                        let i = ny as usize * w as usize + nx as usize;
                        if seen.get(i).copied().unwrap_or(true) {
                            continue;
                        }
                        if let Some(s) = seen.get_mut(i) {
                            *s = true;
                        }
                        if mask.get_pixel(nx, ny)[0] > threshold {
                            stack.push((nx, ny));
                        }
                    }
                }
            }
            if area >= min_area {
                boxes.push((x0, y0, x1 - x0 + 1, y1 - y0 + 1));
            }
        }
    }
    boxes.sort_by_key(|b| std::cmp::Reverse(b.2 as u64 * b.3 as u64));
    boxes
}

/// The pixels of `img` inside `bounds`, with the mask's alpha: the size of
/// the box and nothing of the transparent room around it.
pub fn cut(img: &RgbaImage, mask: &GrayImage, bounds: (u32, u32, u32, u32)) -> Piece {
    let (bx, by, bw, bh) = bounds;
    let (iw, ih) = img.dimensions();
    let (mw, mh) = mask.dimensions();
    let mut out = RgbaImage::new(bw.max(1), bh.max(1));
    for y in 0..bh {
        for x in 0..bw {
            let (sx, sy) = (bx + x, by + y);
            if sx >= iw || sy >= ih {
                continue;
            }
            let mut p = *img.get_pixel(sx, sy);
            if mw > 0 && mh > 0 {
                let mx = (sx as u64 * mw as u64 / iw.max(1) as u64).min(mw as u64 - 1) as u32;
                let my = (sy as u64 * mh as u64 / ih.max(1) as u64).min(mh as u64 - 1) as u32;
                let m = mask.get_pixel(mx, my)[0] as u32;
                p[3] = ((p[3] as u32 * m) / 255) as u8;
            }
            if x < out.width() && y < out.height() {
                out.put_pixel(x, y, p);
            }
        }
    }
    Piece {
        img: out,
        bounds: (bx, by, bw.max(1), bh.max(1)),
    }
}

/// `img` cut to the box its mask keeps: no transparent room above, below,
/// left or right.
pub fn trimmed(img: &RgbaImage, mask: &GrayImage, threshold: u8) -> Piece {
    cut(img, mask, content_box(mask, threshold))
}

/// The separate subjects of `img` as pieces of their own, each cut to the
/// box it fills. `min_area` is in pixels of the picture.
pub fn split(img: &RgbaImage, mask: &GrayImage, threshold: u8, min_area: u32) -> Vec<Piece> {
    let boxes = components(mask, threshold, min_area);
    if boxes.is_empty() {
        return vec![trimmed(img, mask, threshold)];
    }
    boxes
        .into_iter()
        .map(|b| cut(img, mask, b))
        .collect()
}

/// A size an identification photo is made at, in millimetres.
pub struct IdSize {
    /// The key of its name in the language tables.
    pub key: &'static str,
    pub width_mm: f64,
    pub height_mm: f64,
}

/// The sizes the identification photo panel offers.
pub const ID_SIZES: &[IdSize] = &[
    IdSize {
        key: "idphoto.one_inch",
        width_mm: 25.0,
        height_mm: 35.0,
    },
    IdSize {
        key: "idphoto.small_one_inch",
        width_mm: 22.0,
        height_mm: 32.0,
    },
    IdSize {
        key: "idphoto.two_inch",
        width_mm: 35.0,
        height_mm: 49.0,
    },
    IdSize {
        key: "idphoto.small_two_inch",
        width_mm: 33.0,
        height_mm: 48.0,
    },
];

/// The pixels a millimetre is printed at: the density a photo lab prints
/// an identification photo at.
pub const ID_DPI: f64 = 300.0;

/// The size in pixels of `mm` at [`ID_DPI`].
pub fn id_pixels(mm: f64) -> u32 {
    ((mm / 25.4) * ID_DPI).round().max(1.0) as u32
}

/// How an identification photo is made out of a cut-out person.
#[derive(Debug, Clone, PartialEq)]
pub struct IdPhoto {
    /// The index into [`ID_SIZES`].
    pub size: usize,
    /// The colour behind the person: the paper an identification photo is
    /// printed on, white by default.
    pub background: [u8; 4],
    /// The room above the head, as a part of the photo's height.
    pub head_margin: f32,
    /// How much of the photo's height the person takes.
    pub fill: f32,
}

impl Default for IdPhoto {
    fn default() -> Self {
        IdPhoto {
            size: 0,
            background: [255, 255, 255, 255],
            head_margin: 0.09,
            fill: 0.86,
        }
    }
}

/// One pixel of `src` laid over `dst`: straight alpha, the way a PNG keeps
/// it, so the person's own colours come through unchanged.
fn over(dst: [u8; 4], src: [u8; 4]) -> [u8; 4] {
    let sa = src[3] as u32;
    if sa == 0 {
        return dst;
    }
    if sa == 255 {
        return src;
    }
    let da = dst[3] as u32;
    // The two alphas added, less what they share.
    let out_a = sa + da * (255 - sa) / 255;
    if out_a == 0 {
        return [0, 0, 0, 0];
    }
    let mut out = [0u8; 4];
    for c in 0..3 {
        let s = src[c] as u32 * sa;
        let d = dst[c] as u32 * da * (255 - sa) / 255;
        out[c] = ((s + d) / out_a).min(255) as u8;
    }
    out[3] = out_a.min(255) as u8;
    out
}

/// The cut-out person of `matted` made into an identification photo: the
/// size asked for, the person scaled to `fill` of its height with
/// `head_margin` of room above the head, and `background` behind it.
///
/// The head is the top of the person's own alpha, which is the top of the
/// hair: the mask knows where the person starts and stops, and that is
/// what the room above the head is measured from. No face is looked for,
/// so a picture of a person from the waist up and one from the knees up
/// are both framed by their own outline.
pub fn id_photo(matted: &RgbaImage, how: &IdPhoto) -> Result<RgbaImage, String> {
    let (iw, ih) = matted.dimensions();
    if iw == 0 || ih == 0 {
        return Err("an empty picture".to_string());
    }
    let size = ID_SIZES.get(how.size).unwrap_or(&ID_SIZES[0]);
    let out_w = id_pixels(size.width_mm);
    let out_h = id_pixels(size.height_mm);
    let aspect = out_w as f32 / out_h as f32;
    let (bx, by, bw, bh) = alpha_box(matted, 8);
    let fill = how.fill.clamp(0.4, 1.0);
    let margin = how.head_margin.clamp(0.0, 0.35);
    // The height that holds the person at the share of the photo asked
    // for, and the height that holds their width with room beside it.
    let mut crop_h = bh as f32 / fill;
    let beside = 0.92;
    crop_h = crop_h.max((bw as f32 / beside) / aspect.max(1e-6));
    let crop_w = crop_h * aspect;
    let cx = bx as f32 + bw as f32 / 2.0;
    let left = cx - crop_w / 2.0;
    let top = by as f32 - margin * crop_h;
    // A crop that runs past the picture is kept: what is outside is the
    // background colour, which is what an identification photo is printed
    // on, and cutting the person to keep inside would undo the framing.
    if crop_w < 1.0 || crop_h < 1.0 {
        return Err("the person is too small to frame".to_string());
    }
    // Where the picture's own pixels go in the crop.
    let mut canvas = RgbaImage::from_pixel(crop_w.ceil() as u32, crop_h.ceil() as u32, {
        image::Rgba(how.background)
    });
    for y in 0..canvas.height() {
        for x in 0..canvas.width() {
            let sx = left + x as f32;
            let sy = top + y as f32;
            if sx < 0.0 || sy < 0.0 || sx >= iw as f32 || sy >= ih as f32 {
                continue;
            }
            let (sx, sy) = (sx as u32, sy as u32);
            let src = *matted.get_pixel(sx, sy);
            let dst = *canvas.get_pixel(x, y);
            canvas.put_pixel(x, y, image::Rgba(over(dst.0, src.0)));
        }
    }
    Ok(image::imageops::resize(
        &canvas,
        out_w,
        out_h,
        image::imageops::FilterType::Lanczos3,
    ))
}

/// The box the picture's own alpha keeps, the whole of it when every pixel
/// is transparent.
pub fn alpha_box(img: &RgbaImage, threshold: u8) -> (u32, u32, u32, u32) {
    let (w, h) = img.dimensions();
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0u32, 0u32);
    for y in 0..h {
        for x in 0..w {
            if img.get_pixel(x, y)[3] <= threshold {
                continue;
            }
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    if x0 == u32::MAX {
        return (0, 0, w, h);
    }
    (x0, y0, x1 - x0 + 1, y1 - y0 + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mask_takes_the_alpha_down() {
        let mut img = RgbaImage::new(2, 1);
        img.put_pixel(0, 0, image::Rgba([10, 20, 30, 200]));
        img.put_pixel(1, 0, image::Rgba([10, 20, 30, 200]));
        let mut mask = GrayImage::new(2, 1);
        mask.put_pixel(0, 0, Luma([255]));
        mask.put_pixel(1, 0, Luma([0]));
        let out = applied(&img, &mask);
        assert_eq!(out.get_pixel(0, 0)[3], 200);
        assert_eq!(out.get_pixel(1, 0)[3], 0);
        // The colour is left alone.
        assert_eq!(out.get_pixel(1, 0)[0], 10);
    }

    #[test]
    fn a_field_is_stretched_between_its_own_ends() {
        let out = spread(&[-5.0, 0.0, 5.0]);
        assert_eq!(out, vec![0.0, 0.5, 1.0]);
        // A field that is flat has nothing to stretch.
        assert_eq!(spread(&[2.0, 2.0]), vec![2.0, 2.0]);
    }

    #[test]
    fn the_two_models_have_their_own_recipes() {
        assert_eq!(recipe("modnet").side, 512);
        assert_eq!(recipe("rmbg-1.4").side, 1024);
        // RMBG takes a half off and divides by nothing; the ImageNet spread
        // it was first written with made it answer nonsense.
        assert_eq!(recipe("rmbg-1.4").mean, [0.5, 0.5, 0.5]);
        assert_eq!(recipe("rmbg-1.4").scale, [1.0, 1.0, 1.0]);
        assert_eq!(recipe("modnet").mean, [0.5, 0.5, 0.5]);
        assert_eq!(recipe("modnet").scale, [0.5, 0.5, 0.5]);
    }

    /// A mask with `blobs` filled, everything else empty.
    fn mask_with(w: u32, h: u32, blobs: &[(u32, u32, u32, u32)]) -> GrayImage {
        let mut mask = GrayImage::new(w, h);
        for &(x0, y0, x1, y1) in blobs {
            for y in y0..=y1 {
                for x in x0..=x1 {
                    if x < w && y < h {
                        mask.put_pixel(x, y, Luma([255]));
                    }
                }
            }
        }
        mask
    }

    #[test]
    fn two_subjects_apart_are_two_pieces() {
        let mut mask = mask_with(10, 6, &[(0, 0, 1, 1), (6, 3, 8, 5)]);
        // A hole in the larger subject, so the piece shows its own shape
        // rather than a filled box.
        mask.put_pixel(7, 4, Luma([0]));
        let boxes = components(&mask, 10, 2);
        assert_eq!(boxes.len(), 2);
        // The larger one comes first.
        assert_eq!(boxes[0], (6, 3, 3, 3));
        assert_eq!(boxes[1], (0, 0, 2, 2));
        let img = RgbaImage::from_pixel(10, 6, image::Rgba([9, 9, 9, 255]));
        let pieces = split(&img, &mask, 10, 2);
        assert_eq!(pieces.len(), 2);
        assert_eq!(pieces[0].size(), (3, 3));
        assert_eq!(pieces[0].bounds, (6, 3, 3, 3));
        // The hole the mask keeps out is transparent in the piece.
        assert_eq!(pieces[0].img.get_pixel(1, 1)[3], 0);
        assert_eq!(pieces[0].img.get_pixel(0, 0)[3], 255);
    }

    #[test]
    fn specks_below_the_smallest_area_are_not_subjects() {
        let mask = mask_with(10, 6, &[(0, 0, 0, 0), (6, 3, 8, 5)]);
        let boxes = components(&mask, 10, 4);
        assert_eq!(boxes.len(), 1);
        assert_eq!(boxes[0], (6, 3, 3, 3));
    }

    #[test]
    fn the_transparent_room_around_a_subject_is_cut_away() {
        let mask = mask_with(12, 12, &[(3, 4, 5, 6)]);
        let img = RgbaImage::from_pixel(12, 12, image::Rgba([1, 2, 3, 255]));
        let piece = trimmed(&img, &mask, 10);
        assert_eq!(piece.bounds, (3, 4, 3, 3));
        assert_eq!(piece.size(), (3, 3));
    }

    #[test]
    fn a_photo_is_the_size_asked_for_on_its_background() {
        // A person filling the middle of a picture, cut out.
        let mask = mask_with(120, 160, &[(40, 10, 80, 150)]);
        let img = RgbaImage::from_pixel(120, 160, image::Rgba([200, 150, 120, 255]));
        let matted = applied(&img, &mask);
        let how = IdPhoto {
            size: 0,
            background: [255, 255, 255, 255],
            ..IdPhoto::default()
        };
        let photo = id_photo(&matted, &how).expect("a photo");
        // One inch is 25 by 35 mm, printed at 300 pixels to the inch.
        assert_eq!(photo.dimensions(), (295, 413));
        // Above the head the paper shows, and the person is in the middle.
        assert_eq!(photo.get_pixel(0, 0).0, [255, 255, 255, 255]);
        assert_eq!(photo.get_pixel(147, 250).0, [200, 150, 120, 255]);
    }

    #[test]
    fn a_blue_background_is_the_colour_behind_the_person() {
        let mask = mask_with(40, 60, &[(15, 5, 25, 55)]);
        let img = RgbaImage::from_pixel(40, 60, image::Rgba([10, 20, 30, 255]));
        let matted = applied(&img, &mask);
        let how = IdPhoto {
            size: 1,
            background: [67, 142, 219, 255],
            ..IdPhoto::default()
        };
        let photo = id_photo(&matted, &how).expect("a photo");
        assert_eq!(photo.dimensions(), (id_pixels(22.0), id_pixels(32.0)));
        assert_eq!(photo.get_pixel(0, 0).0, [67, 142, 219, 255]);
    }

    #[test]
    fn an_empty_picture_makes_no_photo() {
        let empty = RgbaImage::new(0, 0);
        assert!(id_photo(&empty, &IdPhoto::default()).is_err());
    }
}
