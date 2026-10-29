//! The upscalers: a picture through one of the NCNN models the bucket
//! serves, four times its size.
//!
//! A whole picture will not go through at once: a thousand layers of
//! sixty-four feature maps over a 512 pixel picture is a gigabyte and more
//! of floats, which a phone has not. The picture is cut into square tiles,
//! each carried a little beyond its own edge so the seams have something to
//! read, and the margin is thrown away at the join.
//!
//! See `docs/behavior/ai-tools.md`.

pub mod net;
pub mod param;

use image::RgbaImage;
use std::path::Path;

/// The side of a tile, in pixels of the picture being enlarged.
const TILE: usize = 160;
/// What a tile carries beyond its own side, thrown away at the join.
const MARGIN: usize = 12;

/// A model read from its `.param` and `.bin`, ready to run.
pub struct Upscaler {
    net: net::Net,
    /// What the network does to one pixel: 4 for both models in the bucket.
    pub scale: usize,
}

impl Upscaler {
    /// Read the model named `key` (`<key>.param` and `<key>.bin`) from `dir`.
    pub fn load(dir: &Path, key: &str) -> Result<Upscaler, String> {
        let param_path = dir.join(format!("{key}.param"));
        let bin_path = dir.join(format!("{key}.bin"));
        let text = std::fs::read_to_string(&param_path)
            .map_err(|e| format!("{}: {e}", param_path.display()))?;
        let bin = std::fs::read(&bin_path).map_err(|e| format!("{}: {e}", bin_path.display()))?;
        let layers = param::parse(&text)?;
        let weighed = param::weigh(&layers, &bin)?;
        let net = net::Net::new(&layers, &weighed)?;
        // The factor is what the network does to a single pixel, read by
        // putting one through it: a tile of one is quick even at 351
        // convolutions.
        let one = net.forward(net::Tensor::new(3, 1, 1, vec![0.0; 3]))?;
        if one.c != 3 {
            return Err(format!("the model makes {} channels, not 3", one.c));
        }
        Ok(Upscaler {
            net,
            scale: one.w.max(1),
        })
    }

    /// How many tiles a picture of that size is cut into.
    pub fn tiles(w: u32, h: u32) -> usize {
        let x = (w as usize + TILE - 1) / TILE;
        let y = (h as usize + TILE - 1) / TILE;
        x.max(1) * y.max(1)
    }

    /// The picture, `scale` times its size. `progress` is called after each
    /// tile with how many are done and how many there are.
    pub fn run(
        &self,
        input: &RgbaImage,
        progress: &mut dyn FnMut(usize, usize),
    ) -> Result<RgbaImage, String> {
        let (w, h) = input.dimensions();
        if w == 0 || h == 0 {
            return Err("an empty picture".to_string());
        }
        let scale = self.scale;
        let (w, h) = (w as usize, h as usize);
        let out_w = w * scale;
        let out_h = h * scale;
        if out_w.saturating_mul(out_h) > 64_000_000 {
            return Err("the enlarged picture would be too big".to_string());
        }
        let mut out = RgbaImage::new(out_w as u32, out_h as u32);
        let tiles_x = (w + TILE - 1) / TILE;
        let tiles_y = (h + TILE - 1) / TILE;
        let total = (tiles_x * tiles_y).max(1);
        let mut done = 0;
        for ty in 0..tiles_y {
            for tx in 0..tiles_x {
                let x0 = tx * TILE;
                let y0 = ty * TILE;
                let x1 = (x0 + TILE).min(w);
                let y1 = (y0 + TILE).min(h);
                // The tile with the margin it reads beyond its own edge.
                let sx0 = x0.saturating_sub(MARGIN);
                let sy0 = y0.saturating_sub(MARGIN);
                let sx1 = (x1 + MARGIN).min(w);
                let sy1 = (y1 + MARGIN).min(h);
                let tw = sx1 - sx0;
                let th = sy1 - sy0;
                let mut data = vec![0f32; 3 * th * tw];
                for y in 0..th {
                    for x in 0..tw {
                        let p = input.get_pixel((sx0 + x) as u32, (sy0 + y) as u32);
                        let i = (y * tw + x) * 3;
                        data[i] = p[0] as f32 / 255.0;
                        data[i + 1] = p[1] as f32 / 255.0;
                        data[i + 2] = p[2] as f32 / 255.0;
                    }
                }
                let result = self.net.forward(net::Tensor::new(3, th, tw, data))?;
                // What of the tile's own square, in the enlarged result.
                let ox = (x0 - sx0) * scale;
                let oy = (y0 - sy0) * scale;
                for y in 0..(y1 - y0) * scale {
                    for x in 0..(x1 - x0) * scale {
                        let rx = ox + x;
                        let ry = oy + y;
                        if rx >= result.w || ry >= result.h {
                            continue;
                        }
                        let i = (ry * result.w + rx) * result.c;
                        let px = [
                            byte(result.data[i]),
                            byte(result.data[i + 1]),
                            byte(result.data[i + 2]),
                            255,
                        ];
                        out.put_pixel((x0 * scale + x) as u32, (y0 * scale + y) as u32, px.into());
                    }
                }
                done += 1;
                progress(done, total);
            }
        }
        // The network reads colour; the cut a picture was given stays. Its
        // alpha is enlarged on its own, smoothly, since the network would
        // turn a hard edge into a soft one it never saw.
        if input.pixels().any(|p| p[3] != 255) {
            let mut alpha = image::GrayImage::new(w as u32, h as u32);
            for (x, y, p) in input.enumerate_pixels() {
                alpha.put_pixel(x, y, image::Luma([p[3]]));
            }
            let big = image::imageops::resize(
                &alpha,
                out_w as u32,
                out_h as u32,
                image::imageops::FilterType::Triangle,
            );
            for (x, y, p) in big.enumerate_pixels() {
                if let Some(px) = out.get_pixel_mut_checked(x, y) {
                    px[3] = p[0];
                }
            }
        }
        Ok(out)
    }
}

/// A channel of the model's output as a byte.
fn byte(v: f32) -> u8 {
    if v.is_finite() {
        (v.clamp(0.0, 1.0) * 255.0).round() as u8
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_output_is_clamped_and_never_wraps() {
        assert_eq!(byte(-2.0), 0);
        assert_eq!(byte(0.0), 0);
        assert_eq!(byte(0.5), 128);
        assert_eq!(byte(1.0), 255);
        assert_eq!(byte(9.0), 255);
        assert_eq!(byte(f32::NAN), 0);
    }

    #[test]
    fn a_picture_is_cut_into_tiles() {
        assert_eq!(Upscaler::tiles(160, 160), 1);
        assert_eq!(Upscaler::tiles(161, 160), 2);
        assert_eq!(Upscaler::tiles(320, 320), 4);
        assert_eq!(Upscaler::tiles(1, 1), 1);
    }
}
