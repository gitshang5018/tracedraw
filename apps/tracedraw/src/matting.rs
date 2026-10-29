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
/// pixels and taken to the ImageNet spread it was trained on.
pub const RMBG: Recipe = Recipe {
    side: 1024,
    mean: [0.485, 0.456, 0.406],
    scale: [0.229, 0.224, 0.225],
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
        let mask = run(path, &data, side)?;
        let mut out = GrayImage::new(side as u32, side as u32);
        for y in 0..side {
            for x in 0..side {
                let v = mask.get(y * side + x).copied().unwrap_or(0.0).clamp(0.0, 1.0);
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
    let model = tract_onnx::onnx()
        .model_for_path(path)
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
    let data = answer
        .as_slice::<f32>()
        .map_err(|e| format!("the model's answer is not a mask: {e}"))?;
    // The batch and channel dimensions are one apiece, so the mask is the
    // run of values that is left.
    if data.len() != h * w {
        return Err(format!("the model answered {shape:?}, which is not one mask"));
    }
    let out: Vec<f32> = data.to_vec();
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
    fn the_two_models_have_their_own_recipes() {
        assert_eq!(recipe("modnet").side, 512);
        assert_eq!(recipe("rmbg-1.4").side, 1024);
        assert!((recipe("rmbg-1.4").mean[0] - 0.485).abs() < 1e-6);
    }
}
