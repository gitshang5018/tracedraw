//! The network itself: the layers a `.param` names, run one after another
//! over a picture. Only the layers the upscalers use are here, which is what
//! the two models in the bucket are made of: a convolution with its own
//! activation, a split, a concat, an element-wise add and a nearest resize.
//!
//! Tensors are held channel-last (`[y][x][c]`), not the channel-first order
//! NCNN keeps, because a convolution reads one pixel's channels together and
//! that is the inner loop here. The weights are turned to match
//! (`[out][kh][kw][in]`), so the same inner loop runs over memory that is
//! already in cache.
//!
//! See `docs/behavior/ai-tools.md`.

use super::param::{Conv, Layer, Weighed};
use std::collections::HashMap;
use std::sync::Arc;

/// A picture, or a picture's worth of features, channel-last.
#[derive(Debug, Clone, PartialEq)]
pub struct Tensor {
    pub c: usize,
    pub h: usize,
    pub w: usize,
    pub data: Vec<f32>,
}

impl Tensor {
    pub fn new(c: usize, h: usize, w: usize, data: Vec<f32>) -> Self {
        Tensor { c, h, w, data }
    }

    /// A black picture of that shape.
    pub fn zeros(c: usize, h: usize, w: usize) -> Self {
        Tensor {
            c,
            h,
            w,
            data: vec![0.0; c * h * w],
        }
    }
}

/// One thing the network does.
enum Step {
    /// The picture goes in here.
    Input { top: String },
    Conv {
        bottoms: Vec<String>,
        tops: Vec<String>,
        conv: Box<Conv>,
    },
    /// One blob under several names: the copies are the same tensor.
    Split { bottoms: Vec<String>, tops: Vec<String> },
    /// Blobs joined along their channels.
    Concat { bottoms: Vec<String>, tops: Vec<String> },
    /// Blobs added pixel by pixel, each weighted when the layer says so:
    /// an upscaler's residual blocks add their own input back at a fifth.
    Add {
        bottoms: Vec<String>,
        tops: Vec<String>,
        coeffs: Vec<f32>,
    },
    /// Nearest neighbour, by a whole factor.
    Resize {
        bottoms: Vec<String>,
        tops: Vec<String>,
        scale: usize,
    },
}

/// A model ready to run.
pub struct Net {
    steps: Vec<Step>,
    /// The blob the picture comes out of: the last top of the network, or
    /// the blob named `output`.
    out: String,
    /// The blobs each step reads for the last time. A network of a thousand
    /// layers holds every feature map it ever made unless they go: a tile's
    /// worth is tens of megabytes each, and a phone has not the room.
    frees: Vec<Vec<String>>,
}

impl Net {
    /// Build the network from its layers and the weights read for them.
    pub fn new(layers: &[Layer], weighed: &[Weighed]) -> Result<Net, String> {
        if layers.len() != weighed.len() {
            return Err("the layers and their weights do not line up".to_string());
        }
        let mut steps = Vec::with_capacity(layers.len());
        let mut out = String::new();
        for (layer, w) in layers.iter().zip(weighed) {
            let bottoms = layer.bottoms.clone();
            let tops = layer.tops.clone();
            let step = match layer.kind.as_str() {
                "Input" => Step::Input {
                    top: tops.first().cloned().unwrap_or_default(),
                },
                "Convolution" => {
                    let Weighed::Conv(conv) = w else {
                        return Err(format!("{}: a convolution with no weights", layer.name));
                    };
                    let mut conv = conv.clone();
                    kernel_major(&mut conv);
                    Step::Conv {
                        bottoms,
                        tops,
                        conv,
                    }
                }
                "Split" => Step::Split { bottoms, tops },
                "Concat" => Step::Concat { bottoms, tops },
                "Eltwise" | "BinaryOp" => Step::Add {
                    bottoms,
                    tops,
                    coeffs: coeffs_of(layer),
                },
                "Interp" => {
                    let scale = layer
                        .floats(1)
                        .first()
                        .copied()
                        .unwrap_or(1.0)
                        .round()
                        .max(1.0) as usize;
                    Step::Resize {
                        bottoms,
                        tops,
                        scale,
                    }
                }
                other => {
                    return Err(format!(
                        "{}: a {other} layer, which the upscalers do not use",
                        layer.name
                    ))
                }
            };
            if let Some(top) = tops.last() {
                if top == "output" {
                    out = top.clone();
                }
            }
            steps.push(step);
        }
        if out.is_empty() {
            out = steps
                .iter()
                .rev()
                .find_map(|s| match s {
                    Step::Input { .. } => None,
                    Step::Conv { tops, .. }
                    | Step::Split { tops, .. }
                    | Step::Concat { tops, .. }
                    | Step::Add { tops, .. }
                    | Step::Resize { tops, .. } => tops.last().cloned(),
                })
                .ok_or("the network has no output")?;
        }
        Ok(Net {
            steps,
            out,
            frees: free_lists(&steps, &out),
        })
    }

    /// How many layers the network has.
    pub fn layers(&self) -> usize {
        self.steps.len()
    }

    /// Run the picture through the network.
    pub fn forward(&self, input: Tensor) -> Result<Tensor, String> {
        let mut blobs: HashMap<String, Arc<Tensor>> = HashMap::new();
        let mut first = true;
        let input = Arc::new(input);
        for (index, step) in self.steps.iter().enumerate() {
            match step {
                Step::Input { top } => {
                    if !first {
                        return Err("the network has more than one input".to_string());
                    }
                    first = false;
                    blobs.insert(top.clone(), input.clone());
                }
                Step::Conv {
                    bottoms,
                    tops,
                    conv,
                } => {
                    let src = blob(&blobs, bottoms.first())?;
                    let out = conv_run(conv, &src);
                    store(&mut blobs, tops, out);
                }
                Step::Split { bottoms, tops } => {
                    let src = blob(&blobs, bottoms.first())?;
                    for top in tops {
                        blobs.insert(top.clone(), src.clone());
                    }
                }
                Step::Concat { bottoms, tops } => {
                    let out = concat(&blobs, bottoms)?;
                    store(&mut blobs, tops, out);
                }
                Step::Add {
                    bottoms,
                    tops,
                    coeffs,
                } => {
                    let out = add(&blobs, bottoms, coeffs)?;
                    store(&mut blobs, tops, out);
                }
                Step::Resize {
                    bottoms,
                    tops,
                    scale,
                } => {
                    let src = blob(&blobs, bottoms.first())?;
                    let out = resize(&src, *scale);
                    store(&mut blobs, tops, out);
                }
            }
            if let Some(names) = self.frees.get(index) {
                for name in names {
                    blobs.remove(name);
                }
            }
        }
        let out = blobs
            .remove(&self.out)
            .ok_or_else(|| format!("the network never made {}", self.out))?;
        Ok(Arc::try_unwrap(out).unwrap_or_else(|arc| (*arc).clone()))
    }
}

fn blob(blobs: &HashMap<String, Arc<Tensor>>, name: Option<&String>) -> Result<Arc<Tensor>, String> {
    let name = name.ok_or("a layer with no input")?;
    blobs
        .get(name)
        .cloned()
        .ok_or_else(|| format!("{name} was never made"))
}

fn store(blobs: &mut HashMap<String, Arc<Tensor>>, tops: &[String], t: Tensor) {
    let arc = Arc::new(t);
    for top in tops {
        blobs.insert(top.clone(), arc.clone());
    }
}

/// What a step reads.
fn bottoms_of(step: &Step) -> &[String] {
    match step {
        Step::Input { .. } => &[],
        Step::Conv { bottoms, .. }
        | Step::Split { bottoms, .. }
        | Step::Concat { bottoms, .. }
        | Step::Add { bottoms, .. }
        | Step::Resize { bottoms, .. } => bottoms,
    }
}

/// For each step, the blobs read for the last time there.
fn free_lists(steps: &[Step], out: &str) -> Vec<Vec<String>> {
    let mut last: HashMap<String, usize> = HashMap::new();
    for (i, step) in steps.iter().enumerate() {
        for bottom in bottoms_of(step) {
            last.insert(bottom.clone(), i);
        }
    }
    let mut frees = vec![Vec::new(); steps.len()];
    for (name, i) in last {
        if name != out {
            if let Some(slot) = frees.get_mut(i) {
                slot.push(name);
            }
        }
    }
    frees
}

/// The weights as the convolution reads them: `[out][kh][kw][in]`, so one
/// filter tap is a run of channels with nothing in between.
fn kernel_major(c: &mut Conv) {
    let (oc, ic, kh, kw) = (c.out_c, c.in_c, c.kh, c.kw);
    if c.weight.len() != oc * ic * kh * kw {
        return;
    }
    let mut out = vec![0f32; c.weight.len()];
    for o in 0..oc {
        for ci in 0..ic {
            for ky in 0..kh {
                for kx in 0..kw {
                    out[((o * kh + ky) * kw + kx) * ic + ci] =
                        c.weight[((o * ic + ci) * kh + ky) * kw + kx];
                }
            }
        }
    }
    c.weight = out;
}

/// A convolution, its bias and the activation it carries.
pub fn conv_run(c: &Conv, input: &Tensor) -> Tensor {
    let (ih, iw, ic) = (input.h, input.w, input.c);
    let (kh, kw, oc) = (c.kh, c.kw, c.out_c);
    let s = c.stride.max(1);
    let oh = ((ih + 2 * c.pad_h).saturating_sub(kh)) / s + 1;
    let ow = ((iw + 2 * c.pad_w).saturating_sub(kw)) / s + 1;
    let mut out = vec![0f32; oh * ow * oc];
    if oc == 0 || ic == 0 || oh == 0 || ow == 0 {
        return Tensor::new(oc, oh, ow, out);
    }
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(8)
        .max(1);
    let rows = (oh + threads - 1) / threads;
    if threads == 1 || rows == 0 {
        for oy in 0..oh {
            let band = &mut out[oy * ow * oc..(oy + 1) * ow * oc];
            conv_row(c, input, oy, ow, band);
        }
        return Tensor::new(oc, oh, ow, out);
    }
    std::thread::scope(|scope| {
        let mut y0 = 0;
        for band in out.chunks_mut(rows * ow * oc) {
            let start = y0;
            scope.spawn(move || {
                for (j, row) in band.chunks_mut(ow * oc).enumerate() {
                    conv_row(c, input, start + j, ow, row);
                }
            });
            y0 += rows;
        }
    });
    Tensor::new(oc, oh, ow, out)
}

/// One row of a convolution's output, `ow * oc` values.
fn conv_row(c: &Conv, input: &Tensor, oy: usize, ow: usize, row: &mut [f32]) {
    let (ih, iw, ic) = (input.h, input.w, input.c);
    let (kh, kw, oc) = (c.kh, c.kw, c.out_c);
    let s = c.stride.max(1);
    let iy0 = oy * s;
    for ox in 0..ow {
        let dst = &mut row[ox * oc..(ox + 1) * oc];
        let ix0 = ox * s;
        for ky in 0..kh {
            let iy = iy0 + ky;
            if iy < c.pad_h || iy >= ih + c.pad_h {
                continue;
            }
            let sy = iy - c.pad_h;
            for kx in 0..kw {
                let ix = ix0 + kx;
                if ix < c.pad_w || ix >= iw + c.pad_w {
                    continue;
                }
                let sx = ix - c.pad_w;
                let base = (sy * iw + sx) * ic;
                let src = &input.data[base..base + ic];
                for o in 0..oc {
                    let wbase = ((o * kh + ky) * kw + kx) * ic;
                    let w = &c.weight[wbase..wbase + ic];
                    let mut acc = 0f32;
                    for ci in 0..ic {
                        acc += w[ci] * src[ci];
                    }
                    dst[o] += acc;
                }
            }
        }
        for o in 0..oc {
            if let Some(b) = &c.bias {
                dst[o] += b[o];
            }
            if let Some(slope) = c.leaky {
                if dst[o] < 0.0 {
                    dst[o] *= slope;
                }
            }
        }
    }
}

/// The blobs joined along their channels, in the order they are given.
fn concat(blobs: &HashMap<String, Arc<Tensor>>, bottoms: &[String]) -> Result<Tensor, String> {
    let first = blob(blobs, bottoms.first())?;
    let (h, w) = (first.h, first.w);
    let mut c = 0;
    for name in bottoms {
        let t = blob(blobs, Some(name))?;
        if t.h != h || t.w != w {
            return Err("a concat joins blobs of different sizes".to_string());
        }
        c += t.c;
    }
    let mut out = Tensor::zeros(c, h, w);
    let mut at = 0;
    for name in bottoms {
        let t = blob(blobs, Some(name))?;
        for i in 0..(h * w) {
            let dst = i * c + at;
            out.data[dst..dst + t.c].copy_from_slice(&t.data[i * t.c..(i + 1) * t.c]);
        }
        at += t.c;
    }
    Ok(out)
}

/// The blobs added pixel by pixel, each taken by its own coefficient when
/// the layer gives them: an Eltwise with no coefficients adds as they are.
fn add(
    blobs: &HashMap<String, Arc<Tensor>>,
    bottoms: &[String],
    coeffs: &[f32],
) -> Result<Tensor, String> {
    let first = blob(blobs, bottoms.first())?;
    let mut out = (*first).clone();
    if let Some(k) = coeffs.first() {
        if *k != 1.0 {
            for v in out.data.iter_mut() {
                *v *= *k;
            }
        }
    }
    for (i, name) in bottoms.iter().enumerate().skip(1) {
        let t = blob(blobs, Some(name))?;
        if t.c != out.c || t.h != out.h || t.w != out.w {
            return Err("an add joins blobs of different sizes".to_string());
        }
        let k = coeffs.get(i).copied().unwrap_or(1.0);
        if k == 1.0 {
            for (a, b) in out.data.iter_mut().zip(t.data.iter()) {
                *a += *b;
            }
        } else {
            for (a, b) in out.data.iter_mut().zip(t.data.iter()) {
                *a += k * b;
            }
        }
    }
    Ok(out)
}

/// The coefficients an element-wise layer weights its inputs by. NCNN keeps
/// them as a list whose first number is how many follow, under either the
/// plain key or its own extended one.
fn coeffs_of(layer: &Layer) -> Vec<f32> {
    for key in [-23301, 1] {
        let list = layer.floats(key);
        if list.len() >= 2 {
            // The first number is how many follow.
            return list[1..].to_vec();
        }
    }
    Vec::new()
}

/// Nearest neighbour by a whole factor: each pixel becomes a square of them.
fn resize(t: &Tensor, scale: usize) -> Tensor {
    let (c, h, w) = (t.c, t.h, t.w);
    let (oh, ow) = (h * scale, w * scale);
    let mut out = vec![0f32; c * oh * ow];
    for y in 0..oh {
        let sy = y / scale;
        for x in 0..ow {
            let sx = x / scale;
            let src = (sy * w + sx) * c;
            let dst = (y * ow + x) * c;
            out[dst..dst + c].copy_from_slice(&t.data[src..src + c]);
        }
    }
    Tensor::new(c, oh, ow, out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::upscale::param;

    fn conv_of(oc: usize, ic: usize, weight: Vec<f32>) -> Conv {
        Conv {
            out_c: oc,
            in_c: ic,
            kh: 3,
            kw: 3,
            stride: 1,
            pad_h: 1,
            pad_w: 1,
            weight,
            bias: Some(vec![0.0; oc]),
            leaky: None,
        }
    }

    #[test]
    fn a_convolution_pads_with_zeros_and_sums_its_taps() {
        // One output channel, one input channel, every weight 1: the output
        // is the sum of the nine neighbours, the padding counted as zero.
        let c = conv_of(1, 1, vec![1.0; 9]);
        let input = Tensor::new(1, 2, 2, vec![1.0, 2.0, 3.0, 4.0]);
        let out = conv_run(&c, &input);
        assert_eq!((out.h, out.w, out.c), (2, 2, 1));
        assert_eq!(out.data[0], 1.0 + 2.0 + 3.0);
        assert_eq!(out.data[3], 4.0 + 3.0 + 2.0);
    }

    #[test]
    fn a_leaky_relu_keeps_its_slope() {
        let mut c = conv_of(1, 1, vec![1.0; 9]);
        c.leaky = Some(0.1);
        let input = Tensor::new(1, 1, 1, vec![-5.0]);
        let out = conv_run(&c, &input);
        assert!((out.data[0] - (-0.5)).abs() < 1e-6);
    }

    #[test]
    fn a_split_shares_and_a_concat_joins() {
        let layers = param::parse(
            "7767517\n4 6\nInput data 0 1 data\nSplit s 1 2 data a b\nConcat k 2 1 a b out\nEltwise e 2 1 out out z 0=1\n",
        )
        .unwrap();
        let weighed = vec![
            param::Weighed::None,
            param::Weighed::None,
            param::Weighed::None,
            param::Weighed::None,
        ];
        let net = Net::new(&layers, &weighed).unwrap();
        let input = Tensor::new(2, 1, 1, vec![1.0, 2.0]);
        let out = net.forward(input).unwrap();
        // out is a and b joined (four channels), added to itself.
        assert_eq!(out.c, 4);
        assert_eq!(out.data, vec![2.0, 4.0, 2.0, 4.0]);
    }

    #[test]
    fn a_resize_makes_each_pixel_a_square() {
        let t = Tensor::new(1, 1, 2, vec![1.0, 2.0]);
        let out = resize(&t, 2);
        assert_eq!((out.h, out.w), (2, 4));
        assert_eq!(out.data, vec![1.0, 1.0, 2.0, 2.0, 1.0, 1.0, 2.0, 2.0]);
    }

    #[test]
    fn a_layer_the_upscalers_do_not_use_is_refused() {
        let layers = param::parse("7767517\n1 1\nPooling p 1 1 a b\n").unwrap();
        let weighed = vec![param::Weighed::None];
        assert!(Net::new(&layers, &weighed).is_err());
    }
}
