//! Packing a set of objects onto sheets of one size.
//!
//! The boxes are placed by maximal free rectangles: a sheet starts as one
//! free rectangle, each object takes the one a heuristic likes best, and
//! every free rectangle the object overlaps is split around it. Four
//! heuristics and three orders are all tried, and the run that fills the
//! fewest sheets wins: the same parts come out well under one order and
//! badly under another, and which is which depends on the parts.
//!
//! With irregular packing on, a piece is turned to the angle that makes its
//! box smallest, and after the boxes are placed every piece is slid down
//! and left until its own outline touches something: the boxes are a
//! starting point, the outlines are what the sheet ends up holding.

use tracedraw_core::geometry::Point;

/// Where a piece went: the sheet it is on, and the top left corner of the
/// box it fills on that sheet. Measured from the sheet's top left corner,
/// with y running down the sheet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    pub sheet: usize,
    pub x: f64,
    pub y: f64,
    /// Whether the packer turned it a quarter turn, which it may do when
    /// turning is allowed and the angle was not chosen already.
    pub turned: bool,
}

/// The sheet, and the rules of the packing.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// The sheet's width and height, in the drawing's units.
    pub width: f64,
    pub height: f64,
    /// The least space left between two pieces, and between a piece and the
    /// sheet's edge.
    pub gap: f64,
    /// Whether a piece may be turned a quarter turn to make it fit.
    pub rotate: bool,
}

/// A piece to place: the outline it is cut by, as the points of its convex
/// hull, in the drawing's own coordinates.
#[derive(Clone, Debug, Default)]
pub struct Piece {
    pub hull: Vec<Point>,
}

/// A piece turned to an angle, with the box it fills at that angle and its
/// outline in the coordinates the packing uses: from the box's top left
/// corner, y running down.
#[derive(Clone, Debug)]
pub struct Turn {
    pub angle: f64,
    pub w: f64,
    pub h: f64,
    pub hull: Vec<(f64, f64)>,
}

impl Turn {
    /// The box this turn fills, which a quarter turn of the packer swaps.
    pub fn box_of(&self, turned: bool) -> (f64, f64) {
        if turned {
            (self.h, self.w)
        } else {
            (self.w, self.h)
        }
    }

    /// The outline as it lies on the sheet: `turned` on top of the angle the
    /// turn was chosen at.
    pub fn hull_at(&self, turned: bool) -> Vec<(f64, f64)> {
        if !turned {
            return self.hull.clone();
        }
        // A quarter turn about the box's centre. In a frame where y runs
        // down, the turn that looks clockwise sends (x, y) to (-y, x) about
        // the origin, which in a box whose top left corner is the origin is
        // (h - y, x); the box comes out h wide and w tall.
        let h = self.h;
        self.hull.iter().map(|&(x, y)| (h - y, x)).collect()
    }
}

/// The angle a piece is placed at, and the box it fills there.
///
/// Turning off is the piece as drawn. Turning on with plain boxes is the
/// quarter turn the packer decides, so nothing is turned here. Turning on
/// with irregular packing looks for the angle whose box is smallest, which
/// is what a shape with a long thin part of it gains from.
pub fn turn_of(piece: &Piece, rotate: bool, irregular: bool) -> Turn {
    let angles: &[f64] = if !rotate || !irregular {
        &[0.0]
    } else {
        // Fifteen degrees apart, a half turn being enough: the other half
        // repeats it.
        &[
            0.0, 0.261_799, 0.523_599, 0.785_398, 1.047_198, 1.308_997, 1.570_796, 1.832_596,
            2.094_395, 2.356_194, 2.617_994, 2.879_793,
        ]
    };
    let mut best: Option<Turn> = None;
    for &angle in angles {
        let turn = at_angle(piece, angle);
        if turn.w <= 0.0 || turn.h <= 0.0 {
            continue;
        }
        let better = match &best {
            None => true,
            Some(had) => {
                let (mine, theirs) = (turn.w * turn.h, had.w * had.h);
                mine < theirs - 1e-9
                    || ((mine - theirs).abs() <= 1e-9
                        && turn.w.max(turn.h) < had.w.max(had.h))
            }
        };
        if better {
            best = Some(turn);
        }
    }
    best.unwrap_or_else(|| Turn {
        angle: 0.0,
        w: 0.0,
        h: 0.0,
        hull: Vec::new(),
    })
}

/// The piece at `angle`, its outline moved into a box that starts at the
/// origin with y running down.
fn at_angle(piece: &Piece, angle: f64) -> Turn {
    let (sin, cos) = angle.sin_cos();
    let turned: Vec<(f64, f64)> = piece
        .hull
        .iter()
        .map(|p| (p.x * cos - p.y * sin, p.x * sin + p.y * cos))
        .collect();
    if turned.is_empty() {
        return Turn {
            angle,
            w: 0.0,
            h: 0.0,
            hull: Vec::new(),
        };
    }
    let (mut x0, mut y0) = (f64::MAX, f64::MAX);
    let (mut x1, mut y1) = (f64::MIN, f64::MIN);
    for &(x, y) in &turned {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    Turn {
        angle,
        w: (x1 - x0).max(0.0),
        h: (y1 - y0).max(0.0),
        hull: turned.iter().map(|&(x, y)| (x - x0, y1 - y)).collect(),
    }
}

/// The convex hull of some points, by the monotone chain: sorted along x,
/// the lower and upper sides are walked and every point that turns the
/// wrong way is dropped. A sheet of paper is the case this exists for, and
/// a rectangle comes back as its four corners.
pub fn hull(points: &[Point]) -> Vec<Point> {
    let mut pts: Vec<Point> = points.to_vec();
    if pts.len() < 3 {
        return pts;
    }
    pts.sort_by(|a, b| {
        a.x.partial_cmp(&b.x)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
    });
    let cross = |o: Point, a: Point, b: Point| (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
    let mut lower: Vec<Point> = Vec::with_capacity(pts.len());
    for &p in &pts {
        while lower.len() >= 2 {
            let n = lower.len();
            if cross(lower[n - 2], lower[n - 1], p) <= 1e-12 {
                lower.pop();
            } else {
                break;
            }
        }
        lower.push(p);
    }
    let mut upper: Vec<Point> = Vec::with_capacity(pts.len());
    for &p in pts.iter().rev() {
        while upper.len() >= 2 {
            let n = upper.len();
            if cross(upper[n - 2], upper[n - 1], p) <= 1e-12 {
                upper.pop();
            } else {
                break;
            }
        }
        upper.push(p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

/// Where each box goes, in the order given. A box that no sheet can hold
/// still gets one, at its corner: a piece too big to fit is a mistake in
/// the numbers, not a reason for it to vanish.
pub fn pack(sizes: &[(f64, f64)], o: &Options) -> Vec<Spot> {
    if sizes.is_empty() {
        return Vec::new();
    }
    let mut best: Option<(usize, f64, Vec<Spot>)> = None;
    for order in 0..3u8 {
        let idx = order_of(sizes, order);
        for heuristic in 0..4u8 {
            let (spots, depth) = one_run(&idx, sizes, o, heuristic);
            let sheets = spots.iter().map(|s| s.sheet + 1).max().unwrap_or(0);
            let better = match &best {
                None => true,
                Some((bs, bd, _)) => sheets < *bs || (sheets == *bs && depth < *bd - 1e-9),
            };
            if better {
                best = Some((sheets, depth, spots));
            }
        }
    }
    match best {
        Some((_, _, spots)) => spots,
        None => vec![
            Spot {
                sheet: 0,
                x: 0.0,
                y: 0.0,
                turned: false,
            };
            sizes.len()
        ],
    }
}

/// The order the boxes are offered to the sheet in.
fn order_of(sizes: &[(f64, f64)], kind: u8) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..sizes.len()).collect();
    let key = |s: (f64, f64)| match kind {
        0 => s.0 * s.1,
        1 => s.0.max(s.1),
        _ => s.0,
    };
    idx.sort_by(|&a, &b| {
        key(sizes[b])
            .partial_cmp(&key(sizes[a]))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    idx
}

#[derive(Clone, Copy, Debug)]
struct Free {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// One pass: the boxes in that order, placed by that heuristic. Answers the
/// spots and how deep the last sheet was filled, which is what tells two
/// runs of the same sheet count apart.
fn one_run(idx: &[usize], sizes: &[(f64, f64)], o: &Options, heuristic: u8) -> (Vec<Spot>, f64) {
    let gap = if o.gap.is_finite() { o.gap.max(0.0) } else { 0.0 };
    let board_w = if o.width.is_finite() { o.width.max(1e-6) } else { 1e-6 };
    let board_h = if o.height.is_finite() {
        o.height.max(1e-6)
    } else {
        1e-6
    };
    let whole = Free {
        x: 0.0,
        y: 0.0,
        w: board_w,
        h: board_h,
    };
    let mut spots: Vec<Spot> = vec![
        Spot {
            sheet: 0,
            x: 0.0,
            y: 0.0,
            turned: false,
        };
        sizes.len()
    ];
    let mut sheet = 0usize;
    let mut free: Vec<Free> = vec![whole];
    let mut depth = 0.0f64;
    for &i in idx {
        let (w0, h0) = sizes[i];
        // A piece takes the gap along with it, so the next one keeps its
        // distance without the packer having to remember anything.
        let need_w = w0.max(0.0) + gap;
        let need_h = h0.max(0.0) + gap;
        let mut fresh = true;
        loop {
            if let Some((x, y, turned)) = best_fit(&free, need_w, need_h, o.rotate, heuristic) {
                let (fw, fh) = if turned {
                    (need_h, need_w)
                } else {
                    (need_w, need_h)
                };
                occupy(&mut free, x, y, fw, fh);
                depth = depth.max(y + fh);
                spots[i] = Spot {
                    sheet,
                    x,
                    y,
                    turned,
                };
                break;
            }
            if fresh {
                // An empty sheet, and it still does not fit: it goes at the
                // corner, turned only if that is what brings it closer.
                let turned = o.rotate && need_w > board_w && need_h <= board_w;
                let (fw, fh) = if turned {
                    (need_h, need_w)
                } else {
                    (need_w, need_h)
                };
                occupy(&mut free, 0.0, 0.0, fw, fh);
                depth = depth.max(fh);
                spots[i] = Spot {
                    sheet,
                    x: 0.0,
                    y: 0.0,
                    turned,
                };
                break;
            }
            sheet += 1;
            free.clear();
            free.push(whole);
            depth = 0.0;
            fresh = true;
        }
    }
    (spots, depth)
}

/// The free rectangle a heuristic likes best, and whether the piece goes in
/// it turned.
fn best_fit(
    free: &[Free],
    w: f64,
    h: f64,
    rotate: bool,
    heuristic: u8,
) -> Option<(f64, f64, bool)> {
    let mut best: Option<(f64, f64, f64, f64, bool)> = None;
    for f in free {
        for turned in [false, true] {
            if turned && !rotate {
                continue;
            }
            let (pw, ph) = if turned { (h, w) } else { (w, h) };
            if pw > f.w + 1e-9 || ph > f.h + 1e-9 {
                continue;
            }
            let dw = f.w - pw;
            let dh = f.h - ph;
            let (first, second) = match heuristic {
                // The short side left over, which leaves room for a piece
                // that is narrow on one side.
                0 => (dw.min(dh), dw.max(dh)),
                // The long side left over, which keeps whole rows free.
                1 => (dw.max(dh), dw.min(dh)),
                // The free area left over, which keeps big gaps whole.
                2 => (f.w * f.h - pw * ph, dw.min(dh)),
                // The lowest, then the leftmost: it fills the bottom of the
                // sheet first and leaves one edge clean.
                _ => (f.y + ph, f.x),
            };
            let better = match best {
                None => true,
                Some((b1, b2, _, _, _)) => {
                    first < b1 - 1e-9
                        || ((first - b1).abs() <= 1e-9 && second < b2 - 1e-9)
                }
            };
            if better {
                best = Some((first, second, f.x, f.y, turned));
            }
        }
    }
    best.map(|(_, _, x, y, turned)| (x, y, turned))
}

/// Take `w` by `h` out of the free rectangles, splitting every one it
/// overlaps around it and dropping the pieces of them another already holds.
fn occupy(free: &mut Vec<Free>, x: f64, y: f64, w: f64, h: f64) {
    let mut out: Vec<Free> = Vec::with_capacity(free.len() * 2);
    for f in free.iter().copied() {
        let clear = x >= f.x + f.w - 1e-9
            || x + w <= f.x + 1e-9
            || y >= f.y + f.h - 1e-9
            || y + h <= f.y + 1e-9;
        if clear {
            out.push(f);
            continue;
        }
        if x > f.x + 1e-9 {
            out.push(Free {
                x: f.x,
                y: f.y,
                w: x - f.x,
                h: f.h,
            });
        }
        if x + w < f.x + f.w - 1e-9 {
            out.push(Free {
                x: x + w,
                y: f.y,
                w: f.x + f.w - (x + w),
                h: f.h,
            });
        }
        if y > f.y + 1e-9 {
            out.push(Free {
                x: f.x,
                y: f.y,
                w: f.w,
                h: y - f.y,
            });
        }
        if y + h < f.y + f.h - 1e-9 {
            out.push(Free {
                x: f.x,
                y: y + h,
                w: f.w,
                h: f.y + f.h - (y + h),
            });
        }
    }
    out.retain(|f| f.w > 1e-9 && f.h > 1e-9);
    let mut kept: Vec<Free> = Vec::with_capacity(out.len());
    for (i, a) in out.iter().enumerate() {
        let mut inside = false;
        for (j, b) in out.iter().enumerate() {
            if i == j {
                continue;
            }
            if holds(b, a) && (i > j || !holds(a, b)) {
                inside = true;
                break;
            }
        }
        if !inside {
            kept.push(*a);
        }
    }
    *free = kept;
}

fn holds(b: &Free, a: &Free) -> bool {
    a.x >= b.x - 1e-9
        && a.y >= b.y - 1e-9
        && a.x + a.w <= b.x + b.w + 1e-9
        && a.y + a.h <= b.y + b.h + 1e-9
}

/// Slide every piece down and then left until its own outline meets
/// something, which is what makes irregular packing tighter than the boxes
/// it started from: a piece ends up in the hollow of the ones below it.
///
/// The overlap of two convex outlines while one slides in a straight line is
/// one stretch of travel, so the longest free slide is found by halving the
/// range rather than by stepping a millimetre at a time.
pub fn settle(spots: &mut [Spot], turns: &[Turn], o: &Options) {
    if spots.len() < 2 || spots.len() != turns.len() {
        return;
    }
    let board_w = o.width;
    let board_h = o.height;
    let gap = if o.gap.is_finite() { o.gap.max(0.0) } else { 0.0 };
    // The outlines as they lie on their sheets, worked out once: the
    // settling below moves them about, it does not turn them.
    let hulls: Vec<Vec<(f64, f64)>> = spots
        .iter()
        .zip(turns.iter())
        .map(|(s, t)| t.hull_at(s.turned))
        .collect();
    for _pass in 0..6 {
        let mut moved = false;
        // The last placed are on top of the pile: they settle first, and the
        // ones under them do not have to be moved out of the way again.
        for i in (0..spots.len()).rev() {
            let down = longest(spots, &hulls, i, 0.0, 1.0, board_w, board_h, gap);
            if down > 1e-6 {
                spots[i].y += down;
                moved = true;
            }
            let left = longest(spots, &hulls, i, -1.0, 0.0, board_w, board_h, gap);
            if left > 1e-6 {
                spots[i].x -= left;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
}

/// How far piece `i` can travel in `(dx, dy)` with its outline clear of the
/// sheet's edge and of every other piece, found by halving.
#[allow(clippy::too_many_arguments)]
fn longest(
    spots: &[Spot],
    hulls: &[Vec<(f64, f64)>],
    i: usize,
    dx: f64,
    dy: f64,
    board_w: f64,
    board_h: f64,
    gap: f64,
) -> f64 {
    let mut high = if dx != 0.0 { board_w } else { board_h };
    if high <= 0.0 || !high.is_finite() {
        return 0.0;
    }
    if clear(spots, hulls, i, dx * high, dy * high, board_w, board_h, gap) {
        return high;
    }
    let mut low = 0.0;
    for _ in 0..24 {
        let mid = (low + high) * 0.5;
        if clear(spots, hulls, i, dx * mid, dy * mid, board_w, board_h, gap) {
            low = mid;
        } else {
            high = mid;
        }
        if high - low < 0.05 {
            break;
        }
    }
    low
}

/// Whether piece `i`, moved by `(dx, dy)`, keeps its outline on the sheet
/// and `gap` away from every other piece's.
#[allow(clippy::too_many_arguments)]
fn clear(
    spots: &[Spot],
    hulls: &[Vec<(f64, f64)>],
    i: usize,
    dx: f64,
    dy: f64,
    board_w: f64,
    board_h: f64,
    gap: f64,
) -> bool {
    let s = spots[i];
    let here = &hulls[i];
    if here.len() < 3 {
        return false;
    }
    let at = (s.x + dx, s.y + dy);
    for &(x, y) in here {
        let (x, y) = (x + at.0, y + at.1);
        if x < -1e-6 || y < -1e-6 || x > board_w + 1e-6 || y > board_h + 1e-6 {
            return false;
        }
    }
    for j in 0..spots.len() {
        if j == i {
            continue;
        }
        let other = spots[j];
        if other.sheet != s.sheet || hulls[j].len() < 3 {
            continue;
        }
        if !apart(here, at, &hulls[j], (other.x, other.y), gap) {
            return false;
        }
    }
    true
}

/// Whether two convex outlines, each at its own offset, are further apart
/// than `gap`, by the axes of their own edges: a gap along any one of them
/// means the two cannot touch.
fn apart(
    a: &[(f64, f64)],
    ao: (f64, f64),
    b: &[(f64, f64)],
    bo: (f64, f64),
    gap: f64,
) -> bool {
    for poly in [a, b] {
        let n = poly.len();
        for i in 0..n {
            let (px, py) = poly[i];
            let (qx, qy) = poly[(i + 1) % n];
            let (nx, ny) = (-(qy - py), qx - px);
            let len = (nx * nx + ny * ny).sqrt();
            if len < 1e-12 {
                continue;
            }
            let (nx, ny) = (nx / len, ny / len);
            let (a0, a1) = span(a, ao, nx, ny);
            let (b0, b1) = span(b, bo, nx, ny);
            if a1 <= b0 + gap - 1e-9 || b1 <= a0 + gap - 1e-9 {
                return true;
            }
        }
    }
    false
}

fn span(poly: &[(f64, f64)], off: (f64, f64), nx: f64, ny: f64) -> (f64, f64) {
    let mut lo = f64::MAX;
    let mut hi = f64::MIN;
    for &(x, y) in poly {
        let d = (x + off.0) * nx + (y + off.1) * ny;
        lo = lo.min(d);
        hi = hi.max(d);
    }
    (lo, hi)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(w: f64, h: f64, gap: f64, rotate: bool) -> Options {
        Options {
            width: w,
            height: h,
            gap,
            rotate,
        }
    }

    fn square(at: (f64, f64), size: f64) -> Piece {
        let (x, y) = at;
        Piece {
            hull: vec![
                Point::new(x, y),
                Point::new(x + size, y),
                Point::new(x + size, y + size),
                Point::new(x, y + size),
            ],
        }
    }

    #[test]
    fn four_boxes_of_one_sheet() {
        let sizes = [(100.0, 100.0); 4];
        let spots = pack(&sizes, &options(200.0, 200.0, 0.0, false));
        assert_eq!(spots.len(), 4);
        assert!(spots.iter().all(|s| s.sheet == 0), "{spots:?}");
        for (i, a) in spots.iter().enumerate() {
            for b in spots.iter().skip(i + 1) {
                let apart = a.x + 100.0 <= b.x + 1e-9
                    || b.x + 100.0 <= a.x + 1e-9
                    || a.y + 100.0 <= b.y + 1e-9
                    || b.y + 100.0 <= a.y + 1e-9;
                assert!(apart, "{a:?} and {b:?} overlap");
            }
        }
    }

    #[test]
    fn a_piece_too_wide_for_the_sheet_is_turned() {
        let sizes = [(200.0, 100.0)];
        let spots = pack(&sizes, &options(150.0, 250.0, 0.0, true));
        assert!(spots[0].turned, "{spots:?}");
        let spots = pack(&sizes, &options(150.0, 250.0, 0.0, false));
        assert!(!spots[0].turned, "nothing may be turned: {spots:?}");
    }

    #[test]
    fn the_gap_is_kept_between_pieces() {
        let sizes = [(100.0, 100.0); 2];
        let spots = pack(&sizes, &options(230.0, 100.0, 10.0, false));
        assert_eq!(spots[0].sheet, 0);
        assert_eq!(spots[1].sheet, 0, "{spots:?}");
        let (a, b) = (spots[0], spots[1]);
        let dx = (a.x - b.x).abs();
        let dy = (a.y - b.y).abs();
        assert!(
            dx >= 109.0 || dy >= 109.0,
            "a gap of ten is not kept: {a:?} {b:?}"
        );
    }

    #[test]
    fn more_than_one_sheet_when_they_do_not_fit() {
        let sizes = [(100.0, 100.0); 5];
        let spots = pack(&sizes, &options(200.0, 200.0, 0.0, false));
        let sheets = spots.iter().map(|s| s.sheet + 1).max().unwrap_or(0);
        assert_eq!(sheets, 2, "{spots:?}");
    }

    #[test]
    fn a_box_bigger_than_the_sheet_still_gets_a_place() {
        let sizes = [(500.0, 100.0)];
        let spots = pack(&sizes, &options(200.0, 200.0, 0.0, false));
        assert_eq!(spots.len(), 1);
        assert_eq!((spots[0].x, spots[0].y), (0.0, 0.0));
    }

    #[test]
    fn a_rectangle_comes_back_as_its_four_corners() {
        // The four corners, and points along the edges and inside, which a
        // flattened path is full of.
        let mut pts = vec![
            Point::new(0.0, 0.0),
            Point::new(40.0, 0.0),
            Point::new(40.0, 20.0),
            Point::new(0.0, 20.0),
        ];
        for i in 1..10 {
            let t = i as f64 * 4.0;
            pts.push(Point::new(t, 0.0));
            pts.push(Point::new(40.0, t));
            pts.push(Point::new(t, 20.0));
            pts.push(Point::new(0.0, t));
            pts.push(Point::new(t, 10.0));
        }
        let h = hull(&pts);
        assert_eq!(h.len(), 4, "{h:?}");
    }

    #[test]
    fn a_round_outline_keeps_only_its_rim() {
        // Points on a circle are all on its hull, so what matters is that
        // none from inside it survive.
        let mut pts: Vec<Point> = (0..48)
            .map(|i| {
                let t = i as f64 * 0.13;
                Point::new(t.cos() * 50.0, t.sin() * 50.0)
            })
            .collect();
        pts.push(Point::new(0.0, 0.0));
        pts.push(Point::new(10.0, 10.0));
        let h = hull(&pts);
        assert!(h.len() >= 6, "{} points: {h:?}", h.len());
        for p in &h {
            let r = (p.x * p.x + p.y * p.y).sqrt();
            assert!((r - 50.0).abs() < 1e-6, "{p:?} is not on the rim");
        }
    }

    #[test]
    fn a_quarter_turn_keeps_the_outline_inside_its_box() {
        let piece = Piece {
            hull: vec![
                Point::new(0.0, 0.0),
                Point::new(40.0, 0.0),
                Point::new(40.0, 10.0),
                Point::new(0.0, 10.0),
            ],
        };
        let turn = turn_of(&piece, true, false);
        let (w, h) = turn.box_of(true);
        assert!((w - 10.0).abs() < 1e-9 && (h - 40.0).abs() < 1e-9, "{w} {h}");
        for (x, y) in turn.hull_at(true) {
            assert!(
                (-1e-6..=w + 1e-6).contains(&x) && (-1e-6..=h + 1e-6).contains(&y),
                "({x}, {y}) is outside {w} by {h}"
            );
        }
    }

    #[test]
    fn irregular_packing_turns_a_long_thin_piece_to_its_smallest_box() {
        // A diagonal bar: 45 degrees makes its box 40 by 40, square on it
        // makes it 56 by 56.
        let piece = Piece {
            hull: vec![
                Point::new(0.0, 0.0),
                Point::new(40.0, 40.0),
                Point::new(30.0, 50.0),
                Point::new(-10.0, 10.0),
            ],
        };
        let straight = turn_of(&piece, true, false);
        let turned = turn_of(&piece, true, true);
        assert!(
            turned.w * turned.h < straight.w * straight.h,
            "{} is not smaller than {}",
            turned.w * turned.h,
            straight.w * straight.h
        );
    }

    #[test]
    fn settling_moves_a_piece_into_the_hollow_below_it() {
        let sizes = [(100.0, 100.0); 2];
        let mut spots = pack(&sizes, &options(400.0, 400.0, 0.0, false));
        let turns: Vec<Turn> = sizes
            .iter()
            .map(|_| {
                turn_of(
                    &Piece {
                        hull: vec![
                            Point::new(0.0, 0.0),
                            Point::new(100.0, 0.0),
                            Point::new(100.0, 100.0),
                            Point::new(0.0, 100.0),
                        ],
                    },
                    false,
                    false,
                )
            })
            .collect();
        spots[0].x = 0.0;
        spots[0].y = 0.0;
        spots[1].x = 200.0;
        spots[1].y = 200.0;
        settle(&mut spots, &turns, &options(400.0, 400.0, 0.0, false));
        assert!(spots[1].x < 200.0 || spots[1].y < 200.0, "{spots:?}");
        // Nothing overlaps: the two stay a box apart in one direction.
        let (a, b) = (spots[0], spots[1]);
        let apart = a.x + 100.0 <= b.x + 1e-6
            || b.x + 100.0 <= a.x + 1e-6
            || a.y + 100.0 <= b.y + 1e-6
            || b.y + 100.0 <= a.y + 1e-6;
        assert!(apart, "{a:?} {b:?}");
    }
}
