//! The NCNN model an upscaler is: `.param` is the network as text, `.bin`
//! the weights, layer after layer in the order the `.param` names them.
//!
//! A `.param` line is `type name bottom_count top_count <bottoms> <tops>
//! key=value...`. A key may be negative, which is NCNN's own extension for
//! the parameters a layer keeps outside the numbered set, and a value may be
//! a comma separated list. Nothing here interprets a layer; `net` does that.
//!
//! See `docs/behavior/ai-tools.md`.

use std::collections::HashMap;

/// The magic number every `.param` starts with.
const MAGIC: &str = "7767517";

/// One layer of the network, as its `.param` line says.
#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub kind: String,
    pub name: String,
    pub bottoms: Vec<String>,
    pub tops: Vec<String>,
    /// The line's `key=value` pairs. A key may be negative.
    pub params: HashMap<i32, Vec<f32>>,
}

impl Layer {
    /// A parameter as a whole number, or `default` when the line left it out.
    pub fn int(&self, key: i32, default: i32) -> i32 {
        self.params
            .get(&key)
            .and_then(|v| v.first())
            .map(|v| *v as i32)
            .unwrap_or(default)
    }

    /// A parameter as a list, empty when the line left it out.
    pub fn floats(&self, key: i32) -> &[f32] {
        self.params.get(&key).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

/// Read the layers of a `.param`.
pub fn parse(text: &str) -> Result<Vec<Layer>, String> {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let magic = lines.next().ok_or("the model's .param is empty")?;
    if magic.trim() != MAGIC {
        return Err(format!("not an NCNN network: it starts with {magic:?}"));
    }
    lines.next().ok_or("the model's .param has no layer count")?;
    let mut out = Vec::new();
    for line in lines {
        let mut it = line.split_whitespace();
        let kind = it.next().ok_or("a line with no layer type")?.to_string();
        let name = it
            .next()
            .ok_or_else(|| format!("{kind} has no name"))?
            .to_string();
        let n_bottom = it
            .next()
            .and_then(|s| s.parse::<usize>().ok())
            .ok_or_else(|| format!("{name}: no bottom count"))?;
        let n_top = it
            .next()
            .and_then(|s| s.parse::<usize>().ok())
            .ok_or_else(|| format!("{name}: no top count"))?;
        let mut bottoms = Vec::with_capacity(n_bottom);
        for _ in 0..n_bottom {
            bottoms.push(
                it.next()
                    .ok_or_else(|| format!("{name}: fewer bottoms than it counts"))?
                    .to_string(),
            );
        }
        let mut tops = Vec::with_capacity(n_top);
        for _ in 0..n_top {
            tops.push(
                it.next()
                    .ok_or_else(|| format!("{name}: fewer tops than it counts"))?
                    .to_string(),
            );
        }
        let mut params = HashMap::new();
        for token in it {
            let Some((key, value)) = token.split_once('=') else {
                continue;
            };
            let Ok(key) = key.parse::<i32>() else {
                continue;
            };
            let list: Vec<f32> = value
                .split(',')
                .filter_map(|s| s.trim().parse::<f32>().ok())
                .collect();
            params.insert(key, list);
        }
        out.push(Layer {
            kind,
            name,
            bottoms,
            tops,
            params,
        });
    }
    Ok(out)
}

/// The `.bin`, read in the order the `.param` names the layers. Every
/// layer that keeps weights takes them here, one layer after another, so the
/// whole file is one queue.
///
/// A model may keep its weights as `f32` or as `f16`. The bucket's two
/// upscalers are `f16`: a hundred and forty megabytes of weights would not
/// do for a phone, so they are stored at half. An `f16` layer also carries a
/// four byte mark of its own before its weights, and its bias is still
/// `f32`. Which of the two a file is, is not written anywhere: it is the
/// reading that ends exactly at the end of the file.
pub struct Weights<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Weights<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Weights { bytes, at: 0 }
    }

    /// How many bytes are left.
    pub fn left(&self) -> usize {
        self.bytes.len().saturating_sub(self.at)
    }

    pub fn position(&self) -> usize {
        self.at
    }

    /// Skip `n` bytes.
    pub fn skip(&mut self, n: usize) -> Result<(), String> {
        let end = self.at.checked_add(n).ok_or("the weights overflow")?;
        if end > self.bytes.len() {
            return Err("the weights end early".to_string());
        }
        self.at = end;
        Ok(())
    }

    /// The next `n` `f32`s.
    pub fn floats(&mut self, n: usize) -> Result<Vec<f32>, String> {
        let need = n * 4;
        let end = self.at.checked_add(need).ok_or("the weights overflow")?;
        if end > self.bytes.len() {
            return Err(format!("the weights end early: {n} floats wanted"));
        }
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let o = self.at + i * 4;
            out.push(f32::from_le_bytes([
                self.bytes[o],
                self.bytes[o + 1],
                self.bytes[o + 2],
                self.bytes[o + 3],
            ]));
        }
        self.at = end;
        Ok(out)
    }

    /// The next `n` half precision numbers, as `f32`.
    pub fn halves(&mut self, n: usize) -> Result<Vec<f32>, String> {
        let need = n * 2;
        let end = self.at.checked_add(need).ok_or("the weights overflow")?;
        if end > self.bytes.len() {
            return Err(format!("the weights end early: {n} halves wanted"));
        }
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let o = self.at + i * 2;
            out.push(half_to_f32(u16::from_le_bytes([
                self.bytes[o],
                self.bytes[o + 1],
            ])));
        }
        self.at = end;
        Ok(out)
    }
}

/// A half precision number as a single one, by the bits.
pub fn half_to_f32(h: u16) -> f32 {
    let sign = ((h >> 15) as u32) << 31;
    let exp = ((h >> 10) & 0x1F) as u32;
    let man = (h & 0x3FF) as u32;
    let bits = match exp {
        0 if man == 0 => sign,
        0 => {
            // A subnormal half is a normal single, shifted up until the
            // leading one is where a normal number keeps it.
            let mut e = 127 - 15 + 1;
            let mut m = man;
            while m & 0x400 == 0 {
                m <<= 1;
                e -= 1;
            }
            sign | (e << 23) | ((m & 0x3FF) << 13)
        }
        31 => sign | (0xFF << 23) | (man << 13),
        _ => sign | ((exp + 127 - 15) << 23) | (man << 13),
    };
    f32::from_bits(bits)
}

/// How a model keeps its weights.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Layout {
    /// `f32`, one after another, which is what NCNN writes by default.
    Float,
    /// `f16`, with a four byte mark before each layer's weights.
    Half,
}

/// A convolution as the network holds it: its shape, its weights laid out
/// `[out][in][kh][kw]`, and the activation the layer carries.
#[derive(Debug, Clone, PartialEq)]
pub struct Conv {
    pub out_c: usize,
    pub in_c: usize,
    pub kh: usize,
    pub kw: usize,
    pub stride: usize,
    pub pad_h: usize,
    pub pad_w: usize,
    pub weight: Vec<f32>,
    pub bias: Option<Vec<f32>>,
    /// The leaky slope when the layer's activation is a ReLU; `Some(0.0)` is
    /// a plain ReLU, `None` no activation at all.
    pub leaky: Option<f32>,
}

/// What a layer keeps in the `.bin`.
#[derive(Debug, Clone, PartialEq)]
pub enum Weighed {
    Conv(Box<Conv>),
    /// A layer with no weights of its own (a split, a concat, a resize).
    None,
}

/// Read the weights of every layer, in the order the layers run.
///
/// Which layout the file is in is not written in it. It is told by the
/// reading that ends at the end of the file: half precision weights are
/// half the bytes, so a file read the wrong way either runs out or leaves
/// half of itself behind.
pub fn weigh(layers: &[Layer], bin: &[u8]) -> Result<Vec<Weighed>, String> {
    if let Ok(weights) = read_layout(layers, bin, Layout::Half) {
        return Ok(weights);
    }
    read_layout(layers, bin, Layout::Float).map_err(|e| format!("the weights cannot be read: {e}"))
}

fn read_layout(layers: &[Layer], bin: &[u8], layout: Layout) -> Result<Vec<Weighed>, String> {
    let mut w = Weights::new(bin);
    let mut out = Vec::with_capacity(layers.len());
    for layer in layers {
        if layer.kind != "Convolution" {
            out.push(Weighed::None);
            continue;
        }
        let out_c = layer.int(0, 0).max(0) as usize;
        let kw = layer.int(1, 1).max(1) as usize;
        let kh = layer.int(11, kw as i32).max(1) as usize;
        let stride = layer.int(3, 1).max(1) as usize;
        // NCNN takes one pad for all four sides unless the four are given.
        let pad_w = layer.int(4, 0).max(0) as usize;
        let pad_h = layer.int(14, pad_w as i32).max(0) as usize;
        let bias_term = layer.int(5, 0) != 0;
        let size = layer.int(6, 0).max(0) as usize;
        if out_c == 0 || size == 0 {
            return Err(format!("{}: a convolution with no weights", layer.name));
        }
        let per_out = kw * kh;
        if size % (out_c * per_out) != 0 {
            return Err(format!(
                "{}: {size} weights do not divide into {out_c} filters of {kh}x{kw}",
                layer.name
            ));
        }
        let in_c = size / (out_c * per_out);
        // A half precision layer carries a four byte mark of its own.
        if layout == Layout::Half {
            w.skip(4)?;
        }
        let weight = match layout {
            Layout::Half => w.halves(size)?,
            Layout::Float => w.floats(size)?,
        };
        // The bias is a single precision number either way.
        let bias = if bias_term {
            Some(w.floats(out_c)?)
        } else {
            None
        };
        // 2 is ReLU, whose parameter is the leaky slope.
        let leaky = match layer.int(9, 0) {
            2 => Some(layer.floats(-23310).first().copied().unwrap_or(0.0)),
            _ => None,
        };
        out.push(Weighed::Conv(Box::new(Conv {
            out_c,
            in_c,
            kh,
            kw,
            stride,
            pad_h,
            pad_w,
            weight,
            bias,
            leaky,
        })));
    }
    // A layout that fits is one that ends where the file does; a few bytes
    // of padding are forgiven.
    if w.left() > 8 {
        return Err(format!("{} bytes left over", w.left()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NET: &str = "7767517\n\
3 4\n\
Input                    data                     0 1 data\n\
Convolution              Conv_0                   1 1 data conv0 0=4 1=3 4=1 5=1 6=108\n\
Interp                   Resize_1                 1 1 conv0 out 0=1 1=2.000000e+00 2=2.000000e+00\n";

    #[test]
    fn a_param_reads_its_layers() {
        let layers = parse(NET).unwrap();
        assert_eq!(layers.len(), 3);
        assert_eq!(layers[0].kind, "Input");
        assert_eq!(layers[0].tops, vec!["data"]);
        assert_eq!(layers[1].kind, "Convolution");
        assert_eq!(layers[1].bottoms, vec!["data"]);
        assert_eq!(layers[1].tops, vec!["conv0"]);
        assert_eq!(layers[1].int(0, 0), 4);
        assert_eq!(layers[1].int(6, 0), 108);
        assert_eq!(layers[2].int(1, 0), 2);
    }

    #[test]
    fn a_negative_key_is_a_list() {
        let text = "7767517\n1 1\nConvolution c 0 1 a b 0=2 1=3 6=18 9=2 -23310=1,2.000000e-01\n";
        let layers = parse(text).unwrap();
        assert_eq!(layers[0].floats(-23310), &[1.0, 0.2]);
    }

    #[test]
    fn a_convolution_takes_its_shape_from_its_weight_count() {
        // 4 outputs of 3x3 over 3 inputs: 108 weights, then 4 biases.
        let layers = parse(NET).unwrap();
        let mut bin = Vec::new();
        for i in 0..108 {
            bin.extend_from_slice(&(i as f32).to_le_bytes());
        }
        for i in 0..4 {
            bin.extend_from_slice(&(i as f32).to_le_bytes());
        }
        let weighed = weigh(&layers, &bin).unwrap();
        let Weighed::Conv(c) = &weighed[1] else {
            panic!("the convolution is missing");
        };
        assert_eq!((c.out_c, c.in_c, c.kh, c.kw), (4, 3, 3, 3));
        assert_eq!(c.bias.as_ref().map(|b| b.len()), Some(4));
        assert_eq!(c.leaky, None);
        assert_eq!(weighed[2], Weighed::None);
    }

    #[test]
    fn a_short_bin_is_an_error_not_a_panic() {
        let layers = parse(NET).unwrap();
        let bin = vec![0u8; 16];
        assert!(weigh(&layers, &bin).is_err());
    }

    #[test]
    fn a_file_that_is_not_ncnn_is_refused() {
        assert!(parse("hello\n").is_err());
        assert!(parse("").is_err());
    }
}
