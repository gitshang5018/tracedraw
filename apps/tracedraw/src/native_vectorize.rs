//! Native High-Precision Vectorizer.
//!
//! Provides perceptually accurate vectorization of raster graphics into
//! minimal-node, gap-free, geometric-aligned Bezier paths using OKLab color
//! space quantization, sub-pixel contour tracing, 42-degree corner detection,
//! straight-line and orthogonal snapping, and Schneider cubic Bezier curve fitting.

use image::RgbaImage;
use tracedraw_core::{
    geometry::{BezPath, CubicBez, ParamCurve, ParamCurveDeriv, PathEl, Point, Vec2},
    Color,
};

/// Configuration options for the proprietary native vectorizer.
#[derive(Debug, Clone)]
pub struct VectorizeOptions {
    /// Maximum number of quantized colors (2..32, default 16).
    pub max_colors: usize,
    /// Detail fitting tolerance in pixels (default 0.35 px).
    pub tolerance: f64,
    /// Corner angle threshold in radians (default 42.0 degrees).
    pub corner_threshold_rad: f64,
    /// Collinear straight-line snapping tolerance in pixels (default 0.25 px).
    pub line_snap_tolerance: f64,
    /// Outward dilation offset in pixels for base/underlap layers (default 0.6 px).
    pub dilation_px: f64,
    /// Minimum island pixel count to keep (default 4).
    pub min_speck_area: usize,
    /// Whether to ignore fully transparent background pixels (default true).
    pub ignore_transparent: bool,
}

impl Default for VectorizeOptions {
    fn default() -> Self {
        Self {
            max_colors: 16,
            tolerance: 0.35,
            corner_threshold_rad: 42.0f64.to_radians(),
            line_snap_tolerance: 0.25,
            dilation_px: 0.6,
            min_speck_area: 4,
            ignore_transparent: true,
        }
    }
}

// ---------------------------------------------------------------------------
// 1. OKLab Perceptual Color Space Conversion
// ---------------------------------------------------------------------------

#[inline]
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

#[inline]
fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Convert standard sRGB (0..255) to OKLab (L, a, b).
pub fn rgb_to_oklab(r: u8, g: u8, b: u8) -> [f32; 3] {
    let r_lin = srgb_to_linear(r as f32 / 255.0);
    let g_lin = srgb_to_linear(g as f32 / 255.0);
    let b_lin = srgb_to_linear(b as f32 / 255.0);

    let l = 0.4122214708 * r_lin + 0.5363325363 * g_lin + 0.0514459929 * b_lin;
    let m = 0.2119034982 * r_lin + 0.6806995451 * g_lin + 0.1073969566 * b_lin;
    let s = 0.0883024619 * r_lin + 0.2817188376 * g_lin + 0.6299787005 * b_lin;

    let l_ = l.cbrt();
    let m_ = m.cbrt();
    let s_ = s.cbrt();

    let big_l = 0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_;
    let a = 1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_;
    let b = 0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_;

    [big_l, a, b]
}

/// Convert OKLab (L, a, b) back to standard sRGB (0..255).
pub fn oklab_to_rgb(lab: [f32; 3]) -> [u8; 3] {
    let big_l = lab[0];
    let a = lab[1];
    let b = lab[2];

    let l_ = big_l + 0.3963377774 * a + 0.2158037573 * b;
    let m_ = big_l - 0.1055613458 * a - 0.0638541728 * b;
    let s_ = big_l - 0.0894841775 * a - 1.2914855480 * b;

    let l = l_ * l_ * l_;
    let m = m_ * m_ * m_;
    let s = s_ * s_ * s_;

    let r_lin = 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s;
    let g_lin = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s;
    let b_lin = -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s;

    let r = (linear_to_srgb(r_lin.clamp(0.0, 1.0)) * 255.0)
        .round()
        .clamp(0.0, 255.0) as u8;
    let g = (linear_to_srgb(g_lin.clamp(0.0, 1.0)) * 255.0)
        .round()
        .clamp(0.0, 255.0) as u8;
    let b = (linear_to_srgb(b_lin.clamp(0.0, 1.0)) * 255.0)
        .round()
        .clamp(0.0, 255.0) as u8;

    [r, g, b]
}

#[inline]
pub fn oklab_dist_sq(c1: [f32; 3], c2: [f32; 3]) -> f32 {
    let dl = c1[0] - c2[0];
    let da = c1[1] - c2[1];
    let db = c1[2] - c2[2];
    dl * dl + da * da + db * db
}

// ---------------------------------------------------------------------------
// 2. OKLab K-Means++ Clustering & Quantization
// ---------------------------------------------------------------------------

struct QuantizedImage {
    palette_rgb: Vec<[u8; 3]>,
    labels: Vec<Option<usize>>,
}

fn quantize_oklab(
    img: &RgbaImage,
    max_colors: usize,
    ignore_transparent: bool,
) -> QuantizedImage {
    let w = img.width() as usize;
    let h = img.height() as usize;
    let total = w * h;

    let mut opaque_indices = Vec::new();
    let mut oklab_pixels = Vec::with_capacity(total);

    for (i, p) in img.pixels().enumerate() {
        let is_opaque = !ignore_transparent || p[3] >= 32;
        if is_opaque {
            opaque_indices.push(i);
        }
        oklab_pixels.push(rgb_to_oklab(p[0], p[1], p[2]));
    }

    if opaque_indices.is_empty() {
        return QuantizedImage {
            palette_rgb: Vec::new(),
            labels: vec![None; total],
        };
    }

    let k = max_colors.clamp(2, 32).min(opaque_indices.len());

    // Representative sample for initial centers (up to 1024 points)
    let step = (opaque_indices.len() / 1024).max(1);
    let sample: Vec<[f32; 3]> = opaque_indices
        .iter()
        .step_by(step)
        .map(|&idx| oklab_pixels[idx])
        .collect();

    // K-Means++ initialization
    let mut centers: Vec<[f32; 3]> = Vec::with_capacity(k);
    if let Some(&first) = sample.first() {
        centers.push(first);
    }

    while centers.len() < k {
        let mut best_dist = -1.0f32;
        let mut best_point = sample[0];

        for &p in &sample {
            let mut min_d = f32::MAX;
            for &c in &centers {
                let d = oklab_dist_sq(p, c);
                if d < min_d {
                    min_d = d;
                }
            }
            if min_d > best_dist {
                best_dist = min_d;
                best_point = p;
            }
        }
        centers.push(best_point);
    }

    // Lloyd iterations (up to 8 passes)
    for _ in 0..8 {
        let mut sums = vec![[0.0f32; 3]; k];
        let mut counts = vec![0usize; k];

        for &idx in &opaque_indices {
            let p = oklab_pixels[idx];
            let mut min_d = f32::MAX;
            let mut best_c = 0;
            for (ci, &c) in centers.iter().enumerate() {
                let d = oklab_dist_sq(p, c);
                if d < min_d {
                    min_d = d;
                    best_c = ci;
                }
            }
            sums[best_c][0] += p[0];
            sums[best_c][1] += p[1];
            sums[best_c][2] += p[2];
            counts[best_c] += 1;
        }

        let mut max_shift = 0.0f32;
        for i in 0..k {
            if counts[i] > 0 {
                let new_c = [
                    sums[i][0] / counts[i] as f32,
                    sums[i][1] / counts[i] as f32,
                    sums[i][2] / counts[i] as f32,
                ];
                let shift = oklab_dist_sq(centers[i], new_c);
                if shift > max_shift {
                    max_shift = shift;
                }
                centers[i] = new_c;
            }
        }
        if max_shift < 1e-5 {
            break;
        }
    }

    // Assign final labels
    let mut labels = vec![None; total];
    for &idx in &opaque_indices {
        let p = oklab_pixels[idx];
        let mut min_d = f32::MAX;
        let mut best_c = 0;
        for (ci, &c) in centers.iter().enumerate() {
            let d = oklab_dist_sq(p, c);
            if d < min_d {
                min_d = d;
                best_c = ci;
            }
        }
        labels[idx] = Some(best_c);
    }

    let palette_rgb: Vec<[u8; 3]> = centers.iter().map(|&lab| oklab_to_rgb(lab)).collect();

    QuantizedImage {
        palette_rgb,
        labels,
    }
}

// ---------------------------------------------------------------------------
// 3. Despeckling / Island Filter
// ---------------------------------------------------------------------------

fn despeckle_labels(
    labels: &mut [Option<usize>],
    w: usize,
    h: usize,
    min_area: usize,
) {
    if min_area <= 1 {
        return;
    }
    let total = w * h;
    let mut visited = vec![false; total];
    let mut queue = Vec::new();
    let mut comp = Vec::new();

    for start in 0..total {
        if visited[start] {
            continue;
        }
        visited[start] = true;
        let Some(lab) = labels[start] else {
            continue;
        };

        comp.clear();
        comp.push(start);
        queue.clear();
        queue.push(start);

        while let Some(curr) = queue.pop() {
            let x = curr % w;
            let y = curr / w;

            let neighbors = [
                if x > 0 { Some(curr - 1) } else { None },
                if x + 1 < w { Some(curr + 1) } else { None },
                if y > 0 { Some(curr - w) } else { None },
                if y + 1 < h { Some(curr + w) } else { None },
            ];

            for n_idx in neighbors.into_iter().flatten() {
                if !visited[n_idx] && labels[n_idx] == Some(lab) {
                    visited[n_idx] = true;
                    comp.push(n_idx);
                    queue.push(n_idx);
                }
            }
        }

        if comp.len() < min_area {
            let mut neighbor_counts = std::collections::HashMap::new();
            for &idx in &comp {
                let x = idx % w;
                let y = idx / w;
                let neighbors = [
                    if x > 0 { Some(idx - 1) } else { None },
                    if x + 1 < w { Some(idx + 1) } else { None },
                    if y > 0 { Some(idx - w) } else { None },
                    if y + 1 < h { Some(idx + w) } else { None },
                ];
                for n_idx in neighbors.into_iter().flatten() {
                    if let Some(n_lab) = labels[n_idx] {
                        if n_lab != lab {
                            *neighbor_counts.entry(n_lab).or_insert(0usize) += 1;
                        }
                    }
                }
            }

            if let Some((&best_neighbor, _)) = neighbor_counts.iter().max_by_key(|&(_, c)| *c) {
                for &idx in &comp {
                    labels[idx] = Some(best_neighbor);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 4. Polygon Normal Dilation
// ---------------------------------------------------------------------------

fn signed_area(pts: &[Point]) -> f64 {
    let n = pts.len();
    if n < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    for i in 0..n {
        let p1 = pts[i];
        let p2 = pts[(i + 1) % n];
        sum += p1.x * p2.y - p2.x * p1.y;
    }
    sum * 0.5
}

/// Outward normal dilation by `offset_px` to prevent white gaps (T-junctions).
fn dilate_ring(ring: &[Point], offset_px: f64) -> Vec<Point> {
    let n = ring.len();
    if n < 3 || offset_px.abs() < 1e-4 {
        return ring.to_vec();
    }
    let area = signed_area(ring);
    if area.abs() < 1e-6 {
        return ring.to_vec();
    }

    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let prev = ring[(i + n - 1) % n];
        let curr = ring[i];
        let next = ring[(i + 1) % n];

        let v1 = curr - prev;
        let v2 = next - curr;
        let l1 = v1.hypot();
        let l2 = v2.hypot();
        if l1 < 1e-9 || l2 < 1e-9 {
            out.push(curr);
            continue;
        }
        let u1 = v1 / l1;
        let u2 = v2 / l2;

        let (n1, n2) = if area > 0.0 {
            (Vec2::new(u1.y, -u1.x), Vec2::new(u2.y, -u2.x))
        } else {
            (Vec2::new(-u1.y, u1.x), Vec2::new(-u2.y, u2.x))
        };

        let n_avg = n1 + n2;
        let l_avg = n_avg.hypot();
        if l_avg < 1e-9 {
            out.push(curr);
            continue;
        }
        let n_unit = n_avg / l_avg;
        let cos_half = (n_unit.x * n1.x + n_unit.y * n1.y).max(0.2);
        let miter = (1.0 / cos_half).clamp(0.8, 1.3);
        out.push(curr + n_unit * (offset_px * miter));
    }
    out
}

// ---------------------------------------------------------------------------
// 5. Straight Line Check & Schneider Bezier Curve Fitting
// ---------------------------------------------------------------------------

fn unit(v: Vec2) -> Vec2 {
    let l = v.hypot();
    if l > 1e-12 && l.is_finite() {
        v / l
    } else {
        Vec2::ZERO
    }
}

const TANGENT_REACH: usize = 3;

fn is_collinear_segment(pts: &[Point], tolerance: f64) -> bool {
    if pts.len() <= 2 {
        return true;
    }
    let p0 = pts[0];
    let p1 = pts[pts.len() - 1];
    let v = p1 - p0;
    let len = v.hypot();
    if len < 1e-6 {
        return true;
    }
    let n = Vec2::new(-v.y / len, v.x / len);
    for &p in &pts[1..pts.len() - 1] {
        let dist = ((p.x - p0.x) * n.x + (p.y - p0.y) * n.y).abs();
        if dist > tolerance {
            return false;
        }
    }
    true
}

fn fit_cubics(
    pts: &[Point],
    t1: Vec2,
    t2: Vec2,
    tolerance: f64,
    out: &mut BezPath,
) {
    fit_cubics_rec(pts, t1, t2, tolerance, out, 0);
}

fn fit_cubics_rec(
    pts: &[Point],
    t1: Vec2,
    t2: Vec2,
    tolerance: f64,
    out: &mut BezPath,
    depth: usize,
) {
    let n = pts.len();
    if n < 2 {
        return;
    }
    let (p0, p3) = (pts[0], pts[n - 1]);
    let dist = (p3 - p0).hypot();
    if n == 2 || depth > 24 {
        let h = (dist / 3.0).min(dist * 0.5);
        out.curve_to(p0 + t1 * h, p3 + t2 * h, p3);
        return;
    }

    let mut u = vec![0.0; n];
    for i in 1..n {
        u[i] = u[i - 1] + (pts[i] - pts[i - 1]).hypot();
    }
    let total = u[n - 1];
    if total < 1e-12 {
        out.line_to(p3);
        return;
    }
    for v in u.iter_mut() {
        *v /= total;
    }

    let generate = |u: &[f64]| -> CubicBez {
        let (mut c00, mut c01, mut c11, mut x0, mut x1) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for (p, &t) in pts.iter().zip(u) {
            let mt = 1.0 - t;
            let b0 = mt * mt * mt;
            let b1 = 3.0 * t * mt * mt;
            let b2 = 3.0 * t * t * mt;
            let b3 = t * t * t;
            let a1 = t1 * b1;
            let a2 = t2 * b2;
            c00 += a1.dot(a1);
            c01 += a1.dot(a2);
            c11 += a2.dot(a2);
            let tmp = p.to_vec2() - (p0.to_vec2() * (b0 + b1) + p3.to_vec2() * (b2 + b3));
            x0 += a1.dot(tmp);
            x1 += a2.dot(tmp);
        }
        let det = c00 * c11 - c01 * c01;
        let eps = 1e-6 * dist;
        let max_len = dist * 1.2;

        let (l1, l2) = if det.abs() > 1e-12 {
            let raw_l1 = (x0 * c11 - x1 * c01) / det;
            let raw_l2 = (c00 * x1 - c01 * x0) / det;
            if raw_l1.is_finite()
                && raw_l2.is_finite()
                && raw_l1 >= eps
                && raw_l2 >= eps
                && raw_l1 <= max_len
                && raw_l2 <= max_len
            {
                (raw_l1, raw_l2)
            } else {
                (dist / 3.0, dist / 3.0)
            }
        } else {
            (dist / 3.0, dist / 3.0)
        };

        CubicBez::new(p0, p0 + t1 * l1, p3 + t2 * l2, p3)
    };

    let worst = |c: &CubicBez, u: &[f64]| -> (f64, usize) {
        let mut best = (0.0, n / 2);
        for i in 1..n - 1 {
            let d = (c.eval(u[i]) - pts[i]).hypot2();
            if d > best.0 {
                best = (d, i);
            }
        }
        best
    };

    let mut curve = generate(&u);
    let (mut err, mut split) = worst(&curve, &u);
    let tol2 = tolerance * tolerance;
    if err <= tol2 {
        out.curve_to(curve.p1, curve.p2, curve.p3);
        return;
    }

    if err < tol2 * 4.0 {
        for _ in 0..4 {
            let d1 = curve.deriv();
            let d2 = d1.deriv();
            for i in 0..n {
                let t = u[i];
                let diff = curve.eval(t) - pts[i];
                let q1 = d1.eval(t).to_vec2();
                let q2 = d2.eval(t).to_vec2();
                let den = q1.dot(q1) + diff.dot(q2);
                if den.abs() > 1e-12 {
                    u[i] = (t - diff.dot(q1) / den).clamp(0.0, 1.0);
                }
            }
            curve = generate(&u);
            (err, split) = worst(&curve, &u);
            if err <= tol2 {
                out.curve_to(curve.p1, curve.p2, curve.p3);
                return;
            }
        }
    }

    let split = split.clamp(1, n - 2);
    let reach = TANGENT_REACH.min(split).min(n - 1 - split).max(1);
    let centre = unit(pts[split - reach] - pts[split + reach]);
    let centre = if centre == Vec2::ZERO {
        unit(pts[split - 1] - pts[split])
    } else {
        centre
    };
    fit_cubics_rec(&pts[..=split], t1, centre, tolerance, out, depth + 1);
    fit_cubics_rec(&pts[split..], -centre, t2, tolerance, out, depth + 1);
}

fn polyline_closed(pts: &[Point]) -> BezPath {
    let mut p = BezPath::new();
    if let Some(&f) = pts.first() {
        p.move_to(f);
        for &q in &pts[1..] {
            p.line_to(q);
        }
        p.close_path();
    }
    p
}

/// Fit a closed ring using unit-step resampling, corner detection, line snapping, and Schneider Beziers.
fn fit_ring_precision(ring: &[Point], options: &VectorizeOptions) -> BezPath {
    let mut pts: Vec<Point> = Vec::new();
    let mut starts: Vec<Point> = Vec::new();
    let n = ring.len();
    if n < 3 {
        return BezPath::new();
    }

    // Step 1: Unit-step edge resampling to eliminate sparse-vertex artifacts
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        let steps = (b - a).hypot().round().max(1.0) as usize;
        for k in 0..steps {
            starts.push(a.lerp(b, k as f64 / steps as f64));
            pts.push(a.lerp(b, (k as f64 + 0.5) / steps as f64));
        }
    }

    let n = pts.len();
    if n < 4 {
        return polyline_closed(&pts);
    }

    // Step 2: Feature corner detection
    let reach = 3usize.min(n / 4).max(1);
    let threshold = options.corner_threshold_rad;
    let at = |i: isize| pts[i.rem_euclid(n as isize) as usize];
    let turn: Vec<f64> = (0..n as isize)
        .map(|i| {
            let (a, b, c) = (at(i - reach as isize), at(i), at(i + reach as isize));
            let (u, v) = (b - a, c - b);
            let cross = u.x * v.y - u.y * v.x;
            let dot = u.x * v.x + u.y * v.y;
            cross.atan2(dot).abs()
        })
        .collect();

    let corners: Vec<usize> = (0..n)
        .filter(|&i| {
            turn[i] > threshold
                && (1..=reach as isize).all(|d| {
                    let before = turn[(i as isize - d).rem_euclid(n as isize) as usize];
                    let after = turn[(i as isize + d).rem_euclid(n as isize) as usize];
                    turn[i] >= before && turn[i] > after
                })
        })
        .collect();

    // Step 3: Anchor corners to pixel corners
    let mut is_corner = vec![false; n];
    for &c in &corners {
        is_corner[c] = true;
        let mid = pts[(c + n - 1) % n].midpoint(pts[(c + 1) % n]);
        let (a, b) = (starts[c], starts[(c + 1) % n]);
        pts[c] = if (a - mid).hypot() > (b - mid).hypot() {
            a
        } else {
            b
        };
    }

    // Step 4: Gentle smoothing passes between corners to eliminate grid stairs
    for _ in 0..4 {
        let prev = pts.clone();
        for i in 0..n {
            if is_corner[i] {
                continue;
            }
            let a = prev[(i + n - 1) % n];
            let c = prev[(i + 1) % n];
            pts[i] = Point::new(
                (a.x + 2.0 * prev[i].x + c.x) / 4.0,
                (a.y + 2.0 * prev[i].y + c.y) / 4.0,
            );
        }
    }

    let mut out = BezPath::new();

    // If no distinct corners or only 1 corner, fit 4 quarter curves meeting smoothly
    if corners.len() < 2 {
        let cuts: Vec<usize> = (0..4).map(|k| k * n / 4).collect();
        let tangent = |i: usize| unit(pts[(i + reach) % n] - pts[(i + n - reach) % n]);
        out.move_to(pts[0]);
        for k in 0..4 {
            let (a, b) = (cuts[k], cuts[(k + 1) % 4]);
            let mut piece: Vec<Point> = if b > a {
                pts[a..=b].to_vec()
            } else {
                pts[a..].to_vec()
            };
            if b <= a {
                piece.push(pts[0]);
            }
            fit_cubics(&piece, tangent(a), -tangent(b), options.tolerance, &mut out);
        }
        out.close_path();
        return out;
    }

    // Segment chaining between corners with strict G0 continuity
    let mut sorted_corners = corners;
    sorted_corners.sort_unstable();
    sorted_corners.dedup();

    out.move_to(pts[sorted_corners[0]]);

    for k in 0..sorted_corners.len() {
        let a = sorted_corners[k];
        let b = sorted_corners[(k + 1) % sorted_corners.len()];
        let mut piece: Vec<Point> = Vec::new();
        let mut i = a;
        loop {
            piece.push(pts[i]);
            if i == b && piece.len() > 1 {
                break;
            }
            i = (i + 1) % n;
            if piece.len() > n + 1 {
                break;
            }
        }

        if piece.len() < 2 {
            continue;
        }

        let end_pt = pts[b];
        if is_collinear_segment(&piece, options.line_snap_tolerance) {
            out.line_to(end_pt);
        } else {
            let m = piece.len();
            let far = TANGENT_REACH.min(m - 1);
            let t1 = unit(piece[far] - piece[0]);
            let t2 = unit(piece[m - 1 - far] - piece[m - 1]);
            fit_cubics(&piece, t1, t2, options.tolerance, &mut out);
        }
    }

    out.close_path();
    out
}

// ---------------------------------------------------------------------------
// 6. Full Vectorization Pipeline
// ---------------------------------------------------------------------------

/// Vectorize an RGBA image into high-precision, gap-free vector shapes.
pub fn vectorize(img: &RgbaImage, options: &VectorizeOptions) -> Vec<crate::trace::Traced> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    if w == 0 || h == 0 {
        return Vec::new();
    }

    // Step 1: OKLab quantization
    let mut quantized = quantize_oklab(img, options.max_colors, options.ignore_transparent);
    if quantized.palette_rgb.is_empty() {
        return Vec::new();
    }

    // Step 2: Despeckle / island removal
    despeckle_labels(&mut quantized.labels, w, h, options.min_speck_area);

    // Step 3: Extract contours for each color and measure areas
    let num_colors = quantized.palette_rgb.len();
    let mut color_rings: Vec<Vec<Vec<Point>>> = vec![Vec::new(); num_colors];
    let mut color_areas: Vec<f64> = vec![0.0; num_colors];

    for ci in 0..num_colors {
        let mask: Vec<bool> = quantized
            .labels
            .iter()
            .map(|&l| l == Some(ci))
            .collect();

        if !mask.iter().any(|&m| m) {
            continue;
        }

        let rings = crate::trace::contours(&mask, w, h);
        let mut total_area = 0.0f64;
        for r in &rings {
            total_area += signed_area(r).abs();
        }
        color_areas[ci] = total_area;
        color_rings[ci] = rings;
    }

    // Step 4: Topological stacking (largest area at bottom)
    let mut order: Vec<usize> = (0..num_colors).collect();
    order.sort_by(|&a, &b| {
        color_areas[b]
            .partial_cmp(&color_areas[a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let max_area = color_areas.iter().cloned().fold(0.0f64, f64::max);
    let mut out = Vec::new();

    for &ci in &order {
        if color_rings[ci].is_empty() {
            continue;
        }
        let is_base_layer = color_areas[ci] >= max_area * 0.15;
        let mut path = BezPath::new();

        for ring in &color_rings[ci] {
            if ring.len() < 3 {
                continue;
            }

            // Normal dilation on base layers to ensure physical gap-free rendering
            let processed_ring = if is_base_layer && options.dilation_px > 0.0 {
                dilate_ring(ring, options.dilation_px)
            } else {
                ring.clone()
            };

            let fitted = fit_ring_precision(&processed_ring, options);
            if !fitted.elements().is_empty() {
                path.extend(fitted);
            }
        }

        // Clamp any out-of-bounds coordinates to image range + 1.0 margin as a safety guarantee
        let mut sanitized = BezPath::new();
        let max_x = w as f64 + 1.0;
        let max_y = h as f64 + 1.0;
        let clamp_pt = |p: Point| Point::new(p.x.clamp(-1.0, max_x), p.y.clamp(-1.0, max_y));

        for el in path.elements() {
            match el {
                PathEl::MoveTo(p) => sanitized.move_to(clamp_pt(*p)),
                PathEl::LineTo(p) => sanitized.line_to(clamp_pt(*p)),
                PathEl::QuadTo(c, p) => sanitized.quad_to(clamp_pt(*c), clamp_pt(*p)),
                PathEl::CurveTo(c1, c2, p) => {
                    sanitized.curve_to(clamp_pt(*c1), clamp_pt(*c2), clamp_pt(*p))
                }
                PathEl::ClosePath => sanitized.close_path(),
            }
        }

        if !sanitized.elements().is_empty() {
            let rgb = quantized.palette_rgb[ci];
            out.push(crate::trace::Traced {
                path: sanitized,
                color: Color::rgb8(rgb[0], rgb[1], rgb[2]),
                closed: true,
            });
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oklab_round_trip() {
        for (r, g, b) in [
            (255, 0, 0),
            (0, 255, 0),
            (0, 0, 255),
            (255, 255, 255),
            (0, 0, 0),
            (128, 128, 128),
            (240, 180, 50),
        ] {
            let lab = rgb_to_oklab(r, g, b);
            let back = oklab_to_rgb(lab);
            assert!(
                (r as i32 - back[0] as i32).abs() <= 1,
                "R mismatch for ({r},{g},{b}): got {:?}",
                back
            );
            assert!(
                (g as i32 - back[1] as i32).abs() <= 1,
                "G mismatch for ({r},{g},{b}): got {:?}",
                back
            );
            assert!(
                (b as i32 - back[2] as i32).abs() <= 1,
                "B mismatch for ({r},{g},{b}): got {:?}",
                back
            );
        }
    }

    #[test]
    fn straight_segment_collinear_check() {
        let pts = vec![
            Point::new(10.0, 10.0),
            Point::new(20.0, 10.0),
            Point::new(30.0, 10.0),
            Point::new(40.0, 10.0),
        ];
        assert!(is_collinear_segment(&pts, 0.25));
    }
}
