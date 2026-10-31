//! Nesting by no-fit polygon.
//!
//! Every piece is cut into convex parts. For two convex parts, the set of
//! translations that make them overlap is their no-fit polygon, which is the
//! Minkowski sum of one and the other mirrored: a convex polygon with the
//! edges of both, merged by angle. The forbidden region of a piece against
//! everything already placed is the union of those, one per pair of parts.
//!
//! The lowest free place is always a corner of the sheet or a vertex of that
//! union, so the search is: take those points, sort them by height and then
//! by distance from the left edge, and keep the first that is not inside any
//! no-fit polygon. That is the bottom left placement a sheet of cut parts
//! wants, and unlike packing boxes it lets a part sit in the hollow of
//! another one.
//!
//! Spacing is carried by the geometry rather than by a check: each part is
//! grown by half the spacing, as its Minkowski sum with a square of that
//! side, so two parts that touch after growing are exactly the spacing
//! apart.
//!
//! What is packed is the material: an outline with its holes taken out, each
//! hole cut open into the outline by a slit, so a part can be nested inside
//! the counter of an O. And the place a part is given is not just the first
//! free candidate, it is that candidate slid towards the sheet's top left
//! corner until it meets something, which is what puts a part exactly
//! against another rather than a hair away from it.

use tracedraw_core::geometry::Point;

/// A point, in the frame the packing works in.
type Pt = (f64, f64);

/// The sheet, and the rules.
pub struct Options {
    pub width: f64,
    pub height: f64,
    /// The least space left between two parts.
    pub gap: f64,
    /// The angles a piece may be turned to, in radians.
    pub angles: Vec<f64>,
}

/// Where a piece went: the sheet, the top left corner of the box it fills on
/// that sheet (y running down the sheet), the box itself, and the angle it
/// was turned to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    pub sheet: usize,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub angle: f64,
}

/// Where each piece goes, in the order given. A piece is its rings: the
/// outlines and the holes they carry, which are told apart by what lies
/// inside what. The rings are in the drawing's own coordinates; the packing
/// turns them upside down into its own frame and answers in that frame,
/// which is the same frame [`crate::nesting`] uses.
pub fn nest(rings: &[Vec<Vec<Point>>], o: &Options) -> Vec<Spot> {
    let n = rings.len();
    if n == 0 {
        return Vec::new();
    }
    let half = (o.gap * 0.5).max(0.0);
    let board = (
        if o.width.is_finite() {
            o.width.max(1e-6)
        } else {
            1e-6
        },
        if o.height.is_finite() {
            o.height.max(1e-6)
        } else {
            1e-6
        },
    );
    // The frame the packing works in has y running down, so a piece's top
    // left corner is the smallest corner of its box.
    let base: Vec<(Vec<Vec<Pt>>, f64, f64)> = rings
        .iter()
        .map(|piece| {
            let flipped: Vec<Vec<Pt>> = piece
                .iter()
                .map(|ring| ring.iter().map(|p| (p.x, -p.y)).collect())
                .collect();
            local_parts(&flipped)
        })
        .collect();
    let mut spots = vec![
        Spot {
            sheet: 0,
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
            angle: 0.0,
        };
        n
    ];
    // The big parts decide the sheet, so they are placed while there is
    // still room for them.
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| {
        let (one, other) = (base[a].1 * base[a].2, base[b].1 * base[b].2);
        other
            .partial_cmp(&one)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut sheets: Vec<Vec<Vec<Pt>>> = vec![Vec::new()];
    for &i in &order {
        let (parts0, _, _) = &base[i];
        let current = sheets.len() - 1;
        let mut best: Option<(f64, f64, Spot, Vec<Vec<Pt>>)> = None;
        for &angle in &o.angles {
            let (parts, w, h) = at_angle(parts0, angle);
            if w <= 1e-9 || h <= 1e-9 {
                continue;
            }
            let fat: Vec<Vec<Pt>> = parts.iter().map(|p| inflate(p, half)).collect();
            let found = match sheets.get(current) {
                Some(placed) => place(&fat, w, h, placed, board),
                None => None,
            };
            let Some((x, y)) = found else {
                continue;
            };
            let better = match &best {
                None => true,
                Some((by, bx, _, _)) => {
                    y < by - 1e-9 || ((y - by).abs() <= 1e-9 && x < bx - 1e-9)
                }
            };
            if better {
                let moved: Vec<Vec<Pt>> = fat
                    .iter()
                    .map(|p| p.iter().map(|&(px, py)| (px + x, py + y)).collect())
                    .collect();
                best = Some((
                    y,
                    x,
                    Spot {
                        sheet: current,
                        x,
                        y,
                        w,
                        h,
                        angle,
                    },
                    moved,
                ));
            }
        }
        match best {
            Some((_, _, spot, moved)) => {
                if let Some(sheet) = sheets.get_mut(current) {
                    sheet.extend(moved);
                }
                spots[i] = spot;
            }
            None => {
                // A sheet of its own: the angle whose box is smallest, at the
                // corner.
                let mut pick: Option<(f64, f64, f64, f64, Vec<Vec<Pt>>)> = None;
                for &angle in &o.angles {
                    let (parts, w, h) = at_angle(parts0, angle);
                    if w <= 1e-9 || h <= 1e-9 {
                        continue;
                    }
                    let better = match &pick {
                        None => true,
                        Some((area, _, _, _, _)) => w * h < *area - 1e-9,
                    };
                    if better {
                        pick = Some((w * h, angle, w, h, parts));
                    }
                }
                if let Some((_, angle, w, h, parts)) = pick {
                    let fat: Vec<Vec<Pt>> = parts.iter().map(|p| inflate(p, half)).collect();
                    sheets.push(fat);
                    spots[i] = Spot {
                        sheet: sheets.len() - 1,
                        x: 0.0,
                        y: 0.0,
                        w,
                        h,
                        angle,
                    };
                }
            }
        }
    }
    spots
}

/// The material a piece is cut from, as convex parts, and the box it fills.
///
/// A piece is one or more outlines, each carrying its holes. What the sheet
/// has to hold is the material: an outline with its holes taken out, so a
/// part can be nested inside the counter of an O rather than only beside
/// it. Parts come back in the piece's own frame, the box's smallest corner
/// at the origin.
fn local_parts(rings: &[Vec<Pt>]) -> (Vec<Vec<Pt>>, f64, f64) {
    let clean: Vec<Vec<Pt>> = rings
        .iter()
        .map(|r| positive(r))
        .filter(|r| r.len() >= 3)
        .collect();
    if clean.is_empty() {
        return (Vec::new(), 0.0, 0.0);
    }
    let areas: Vec<f64> = clean.iter().map(|r| area2(r).abs() * 0.5).collect();
    let count = clean.len();
    // A ring inside a bigger one is that one's hole; a ring inside nothing
    // is an outline of its own, and a piece drawn as several separate parts
    // has several.
    let mut inside_of: Vec<Option<usize>> = vec![None; count];
    for i in 0..count {
        let mut best: Option<(f64, usize)> = None;
        for j in 0..count {
            if i == j || areas[j] <= areas[i] + 1e-9 {
                continue;
            }
            let Some(&probe) = clean[i].first() else {
                continue;
            };
            if !inside(probe, &clean[j]) {
                continue;
            }
            let better = match best {
                None => true,
                Some((had, _)) => areas[j] < had,
            };
            if better {
                best = Some((areas[j], j));
            }
        }
        inside_of[i] = best.map(|(_, j)| j);
    }
    let (mut x0, mut y0) = (f64::MAX, f64::MAX);
    let (mut x1, mut y1) = (f64::MIN, f64::MIN);
    let mut parts: Vec<Vec<Pt>> = Vec::new();
    for body in 0..count {
        if inside_of[body].is_some() {
            continue;
        }
        let holes: Vec<Vec<Pt>> = (0..count)
            .filter(|&i| inside_of[i] == Some(body))
            .map(|i| clean[i].clone())
            .collect();
        for &(x, y) in &clean[body] {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
        parts.extend(material_parts(&clean[body], &holes));
    }
    if parts.is_empty() || x1 <= x0 || y1 <= y0 {
        return (Vec::new(), 0.0, 0.0);
    }
    let moved: Vec<Vec<Pt>> = parts
        .iter()
        .map(|p| p.iter().map(|&(x, y)| (x - x0, y - y0)).collect())
        .collect();
    (moved, x1 - x0, y1 - y0)
}

/// The outline with its holes taken out, as convex parts.
///
/// Each hole is cut open towards the outline by a bridge, the way a polygon
/// with a hole is made into one ring, and the ring is then cut into convex
/// parts. The bridges are a slit of no width, which the ear walk can
/// stumble on, so the parts are only used when their area is the outline's
/// less the holes': otherwise the holes are filled in, which wastes the
/// space inside them but never packs a part into another part.
fn material_parts(outer: &[Pt], holes: &[Vec<Pt>]) -> Vec<Vec<Pt>> {
    if holes.is_empty() {
        return convex_parts(outer);
    }
    let mut want = area2(outer).abs() * 0.5;
    for h in holes {
        want -= area2(h).abs() * 0.5;
    }
    let mut ring: Vec<Pt> = outer.to_vec();
    let outline_way = area2(&ring) > 0.0;
    for hole in holes {
        // A hole is walked the other way round from its outline, which is
        // what makes the ring's area the outline's less the hole's.
        let mut h: Vec<Pt> = hole.clone();
        if (area2(&h) > 0.0) == outline_way {
            h.reverse();
        }
        match bridge(&ring, &h) {
            Some(next) => ring = next,
            None => return convex_parts(outer),
        }
    }
    let parts = convex_parts(&ring);
    let got: f64 = parts.iter().map(|p| area2(p).abs() * 0.5).sum();
    if (got - want).abs() > (want.abs() * 0.01).max(1e-6) {
        return convex_parts(outer);
    }
    parts
}

/// A ring with a hole cut open into it: the hole's rightmost vertex is
/// joined to the outline by a slit of no width, so the two become one ring
/// that runs round the outside and back round the inside.
fn bridge(ring: &[Pt], hole: &[Pt]) -> Option<Vec<Pt>> {
    let ring = positive(ring);
    if ring.len() < 3 || hole.len() < 3 {
        return None;
    }
    // The hole's rightmost vertex: a ray to the right from it leaves the
    // hole at once and meets the outline, and the slit along that ray cannot
    // cross the hole itself.
    let mut k = 0usize;
    for i in 1..hole.len() {
        if hole[i].0 > hole[k].0 {
            k = i;
        }
    }
    let m = hole[k];
    let mut hit: Option<(f64, usize)> = None;
    for i in 0..ring.len() {
        let a = ring[i];
        let b = ring[(i + 1) % ring.len()];
        if (a.1 > m.1) == (b.1 > m.1) {
            continue;
        }
        let t = (m.1 - a.1) / (b.1 - a.1);
        let x = a.0 + t * (b.0 - a.0);
        if x <= m.0 + 1e-9 {
            continue;
        }
        let better = match hit {
            None => true,
            Some((had, _)) => x < had - 1e-9,
        };
        if better {
            hit = Some((x, i));
        }
    }
    let (_, i) = hit?;
    let a = ring[i];
    let b = ring[(i + 1) % ring.len()];
    // The end of that edge further to the right is where the slit goes.
    let target = if a.0 >= b.0 { i } else { (i + 1) % ring.len() };
    let mut out: Vec<Pt> = Vec::with_capacity(ring.len() + hole.len() + 2);
    for j in 0..=target {
        out.push(ring[j]);
    }
    for j in 0..hole.len() {
        out.push(hole[(k + j) % hole.len()]);
    }
    out.push(hole[k]);
    for j in target..ring.len() {
        out.push(ring[j]);
    }
    Some(out)
}

/// The same parts turned by `angle`, with the box's smallest corner back at
/// the origin.
fn at_angle(parts: &[Vec<Pt>], angle: f64) -> (Vec<Vec<Pt>>, f64, f64) {
    let (sin, cos) = angle.sin_cos();
    let mut turned: Vec<Vec<Pt>> = Vec::with_capacity(parts.len());
    let (mut x0, mut y0) = (f64::MAX, f64::MAX);
    let (mut x1, mut y1) = (f64::MIN, f64::MIN);
    for part in parts {
        let p: Vec<Pt> = part
            .iter()
            .map(|&(x, y)| (x * cos - y * sin, x * sin + y * cos))
            .collect();
        for &(x, y) in &p {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
        turned.push(p);
    }
    if turned.is_empty() || x1 <= x0 || y1 <= y0 {
        return (Vec::new(), 0.0, 0.0);
    }
    let moved: Vec<Vec<Pt>> = turned
        .iter()
        .map(|p| p.iter().map(|&(x, y)| (x - x0, y - y0)).collect())
        .collect();
    (moved, x1 - x0, y1 - y0)
}

/// The lowest free place for a piece on one sheet, or nothing when the sheet
/// has no room for it.
fn place(
    prep: &[Vec<Pt>],
    w: f64,
    h: f64,
    placed: &[Vec<Pt>],
    board: (f64, f64),
) -> Option<(f64, f64)> {
    let (rx0, ry0) = (0.0, 0.0);
    let x1 = board.0 - w;
    let y1 = board.1 - h;
    if x1 < -1e-9 || y1 < -1e-9 {
        return None;
    }
    let (rx1, ry1) = (x1.max(0.0), y1.max(0.0));
    // Every translation that puts this piece on one already placed.
    let mut nfps: Vec<(Vec<Pt>, (f64, f64, f64, f64))> = Vec::new();
    for q in placed {
        for p in prep {
            // The translations that put this part on one already placed are
            // the placed part grown by the moving one turned round: `t` is
            // such a translation when `b + t = a` for some `a` of the part
            // placed and `b` of the one moving, so the sum is the placed
            // part with the moving one mirrored.
            let mirrored: Vec<Pt> = p.iter().map(|&(x, y)| (-x, -y)).collect();
            let poly = minkowski(q, &mirrored);
            if poly.len() < 3 {
                continue;
            }
            let b = bbox(&poly);
            if b.2 < rx0 - 1e-9 || b.0 > rx1 + 1e-9 || b.3 < ry0 - 1e-9 || b.1 > ry1 + 1e-9 {
                continue;
            }
            nfps.push((poly, b));
        }
    }
    if nfps.is_empty() {
        return Some((rx0, ry0));
    }
    // The corners of the sheet, and every vertex of the forbidden region
    // that lies on it: the lowest free place is always one of those.
    let mut cands: Vec<Pt> = vec![(rx0, ry0), (rx1, ry0), (rx0, ry1), (rx1, ry1)];
    for (poly, _) in &nfps {
        for v in clip(poly, rx0, ry0, rx1, ry1) {
            cands.push(v);
        }
    }
    cands.sort_by(|a, b| {
        a.1.partial_cmp(&b.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
    });
    let mut grid = Grid::new(rx0, ry0, rx1 - rx0, ry1 - ry0, 24);
    for (i, (_, b)) in nfps.iter().enumerate() {
        grid.insert(i, *b);
    }
    for c in cands {
        if c.0 < rx0 - 1e-9 || c.0 > rx1 + 1e-9 || c.1 < ry0 - 1e-9 || c.1 > ry1 + 1e-9 {
            continue;
        }
        let mut free = true;
        for &k in grid.at(c) {
            let Some((poly, b)) = nfps.get(k) else {
                continue;
            };
            if c.0 < b.0 - 1e-9 || c.0 > b.2 + 1e-9 || c.1 < b.1 - 1e-9 || c.1 > b.3 + 1e-9 {
                continue;
            }
            if inside(c, poly) {
                free = false;
                break;
            }
        }
        if free {
            return Some(rest(c, &nfps, rx0, ry0));
        }
    }
    // No vertex of the forbidden region is free: the free places are the
    // ones between them, a hole in the middle of a part for one. A coarse
    // walk of the sheet finds one, and the part is then slid into its
    // corner like any other.
    let steps = 16;
    for iy in 0..=steps {
        let y = ry0 + (ry1 - ry0) * iy as f64 / steps as f64;
        for ix in 0..=steps {
            let x = rx0 + (rx1 - rx0) * ix as f64 / steps as f64;
            if free((x, y), &nfps) {
                return Some(rest((x, y), &nfps, rx0, ry0));
            }
        }
    }
    None
}

/// The place a piece comes to rest in: slid towards the sheet's top left
/// corner until it touches, so it sits against what is above and beside it
/// rather than a hair away from it.
///
/// The point it starts from is already free, and the first contact along a
/// straight line is found exactly rather than stepped towards, so this lands
/// on the boundary. Three rounds, because sliding left can open the way up
/// again.
fn rest(pos: Pt, nfps: &[(Vec<Pt>, (f64, f64, f64, f64))], left_edge: f64, top_edge: f64) -> Pt {
    let mut p = pos;
    for _ in 0..3 {
        let up = travel(p, (0.0, -1.0), nfps, p.1 - top_edge);
        if up > 1e-9 && free((p.0, p.1 - up), nfps) {
            p.1 -= up;
        }
        let left = travel(p, (-1.0, 0.0), nfps, p.0 - left_edge);
        if left > 1e-9 && free((p.0 - left, p.1), nfps) {
            p.0 -= left;
        }
    }
    p
}

/// Whether a place is clear of every no-fit polygon. A slide that starts on
/// the edge of one goes into it at once, and the first contact of a ray is
/// then the point it starts from, which is no distance at all: the end of a
/// slide is tested rather than trusted.
fn free(p: Pt, nfps: &[(Vec<Pt>, (f64, f64, f64, f64))]) -> bool {
    for (poly, b) in nfps {
        if p.0 < b.0 - 1e-9 || p.0 > b.2 + 1e-9 || p.1 < b.1 - 1e-9 || p.1 > b.3 + 1e-9 {
            continue;
        }
        if inside(p, poly) {
            return false;
        }
    }
    true
}

/// How far a point can travel along `u` before it meets one of the rings, or
/// reaches `limit`.
fn travel(p: Pt, u: Pt, nfps: &[(Vec<Pt>, (f64, f64, f64, f64))], limit: f64) -> f64 {
    let mut best = if limit > 0.0 && limit.is_finite() {
        limit
    } else {
        0.0
    };
    for (poly, _) in nfps {
        let n = poly.len();
        for i in 0..n {
            if let Some(t) = ray_hit(p, u, poly[i], poly[(i + 1) % n]) {
                if t < best {
                    best = t;
                }
            }
        }
    }
    best.max(0.0)
}

/// Where a ray from `p` along `u` meets the line between two points, as a
/// distance along the ray, when it meets it between them.
fn ray_hit(p: Pt, u: Pt, a: Pt, b: Pt) -> Option<f64> {
    let e = sub(b, a);
    let den = cross(u, e);
    if den.abs() < 1e-12 {
        return None;
    }
    let ap = sub(a, p);
    let t = cross(ap, e) / den;
    let s = cross(ap, u) / den;
    if t > 1e-6 && (-1e-9..=1.0 + 1e-9).contains(&s) {
        Some(t)
    } else {
        None
    }
}

/// A coarse grid over the sheet, so a candidate is tested against the few
/// no-fit polygons near it rather than against all of them.
struct Grid {
    x0: f64,
    y0: f64,
    cw: f64,
    ch: f64,
    nx: usize,
    ny: usize,
    cells: Vec<Vec<usize>>,
}

impl Grid {
    fn new(x0: f64, y0: f64, w: f64, h: f64, n: usize) -> Grid {
        let nx = n.max(1);
        let ny = n.max(1);
        let cw = if w > 1e-9 { w / nx as f64 } else { 1.0 };
        let ch = if h > 1e-9 { h / ny as f64 } else { 1.0 };
        Grid {
            x0,
            y0,
            cw,
            ch,
            nx,
            ny,
            cells: vec![Vec::new(); nx * ny],
        }
    }

    fn ix(&self, x: f64) -> usize {
        let i = ((x - self.x0) / self.cw).floor();
        if i < 0.0 {
            0
        } else if i >= self.nx as f64 {
            self.nx - 1
        } else {
            i as usize
        }
    }

    fn iy(&self, y: f64) -> usize {
        let i = ((y - self.y0) / self.ch).floor();
        if i < 0.0 {
            0
        } else if i >= self.ny as f64 {
            self.ny - 1
        } else {
            i as usize
        }
    }

    fn insert(&mut self, i: usize, b: (f64, f64, f64, f64)) {
        let (ix0, ix1) = (self.ix(b.0), self.ix(b.2));
        let (iy0, iy1) = (self.iy(b.1), self.iy(b.3));
        for y in iy0..=iy1 {
            for x in ix0..=ix1 {
                if let Some(cell) = self.cells.get_mut(y * self.nx + x) {
                    cell.push(i);
                }
            }
        }
    }

    fn at(&self, p: Pt) -> &[usize] {
        match self.cells.get(self.iy(p.1) * self.nx + self.ix(p.0)) {
            Some(cell) => cell.as_slice(),
            None => &[],
        }
    }
}

// ----- The geometry the packing stands on ----------------------------------

fn sub(a: Pt, b: Pt) -> Pt {
    (a.0 - b.0, a.1 - b.1)
}

fn cross(a: Pt, b: Pt) -> f64 {
    a.0 * b.1 - a.1 * b.0
}

/// Twice the signed area: positive for a ring that runs one way, negative
/// for the other.
fn area2(p: &[Pt]) -> f64 {
    let n = p.len();
    if n < 3 {
        return 0.0;
    }
    let mut s = 0.0;
    for i in 0..n {
        let q = p[(i + 1) % n];
        s += p[i].0 * q.1 - q.0 * p[i].1;
    }
    s
}

fn positive(p: &[Pt]) -> Vec<Pt> {
    let mut v = p.to_vec();
    if area2(&v) < 0.0 {
        v.reverse();
    }
    v
}

fn bbox(p: &[Pt]) -> (f64, f64, f64, f64) {
    let (mut x0, mut y0) = (f64::MAX, f64::MAX);
    let (mut x1, mut y1) = (f64::MIN, f64::MIN);
    for &(x, y) in p {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    (x0, y0, x1, y1)
}

/// The convex parts of a ring: triangles cut off one at a time by the ears
/// of the polygon, then merged back together while the merge stays convex.
fn convex_parts(ring: &[Pt]) -> Vec<Vec<Pt>> {
    let ring = positive(ring);
    if ring.len() < 3 {
        return Vec::new();
    }
    if is_convex(&ring) {
        return vec![ring];
    }
    let tris = ear_clip(&ring);
    if tris.is_empty() {
        return vec![ring];
    }
    merge_convex(tris)
}

fn is_convex(p: &[Pt]) -> bool {
    let n = p.len();
    if n < 3 {
        return false;
    }
    for i in 0..n {
        let a = p[(i + n - 1) % n];
        let b = p[i];
        let c = p[(i + 1) % n];
        if cross(sub(b, a), sub(c, b)) < -1e-9 {
            return false;
        }
    }
    true
}

fn ear_clip(ring: &[Pt]) -> Vec<Vec<Pt>> {
    let mut r: Vec<Pt> = ring.to_vec();
    let mut out: Vec<Vec<Pt>> = Vec::new();
    let mut guard = 0usize;
    while r.len() > 3 && guard < 4000 {
        guard += 1;
        let n = r.len();
        let mut cut = false;
        for i in 0..n {
            let a = r[(i + n - 1) % n];
            let b = r[i];
            let c = r[(i + 1) % n];
            if cross(sub(b, a), sub(c, b)) <= 1e-12 {
                continue;
            }
            let mut ear = true;
            for j in 0..n {
                if j == i || j == (i + n - 1) % n || j == (i + 1) % n {
                    continue;
                }
                // The two sides of a slit are the same two points: a corner
                // on the ear's own edge is the ear itself, not a corner that
                // blocks it.
                if same(r[j], a) || same(r[j], b) || same(r[j], c) {
                    continue;
                }
                if in_tri(r[j], a, b, c) {
                    ear = false;
                    break;
                }
            }
            if ear {
                out.push(vec![a, b, c]);
                r.remove(i);
                cut = true;
                break;
            }
        }
        if !cut {
            // A ring with a hole cut open into it has a slit, and the two
            // sides of a slit lie on one line: no ear test will take any of
            // its corners. The flattest corner is cut away as a sliver and
            // the walk goes on, which is how a bridged ring comes apart.
            let n = r.len();
            let mut pick = 0usize;
            let mut flattest = f64::MAX;
            for i in 0..n {
                let a = r[(i + n - 1) % n];
                let b = r[i];
                let c = r[(i + 1) % n];
                let turn = cross(sub(b, a), sub(c, b)).abs();
                if turn < flattest {
                    flattest = turn;
                    pick = i;
                }
            }
            let a = r[(pick + n - 1) % n];
            let b = r[pick];
            let c = r[(pick + 1) % n];
            out.push(vec![a, b, c]);
            r.remove(pick);
        }
    }
    if r.len() == 3 {
        out.push(r);
    } else if r.len() > 3 {
        // A ring this walk could not ear down is cut into a fan rather than
        // dropped: these parts are what the packing measures against.
        for i in 1..r.len() - 1 {
            out.push(vec![r[0], r[i], r[i + 1]]);
        }
    }
    out
}

fn in_tri(p: Pt, a: Pt, b: Pt, c: Pt) -> bool {
    let d1 = cross(sub(b, a), sub(p, a));
    let d2 = cross(sub(c, b), sub(p, b));
    let d3 = cross(sub(a, c), sub(p, c));
    let below = d1 < -1e-12 || d2 < -1e-12 || d3 < -1e-12;
    let above = d1 > 1e-12 || d2 > 1e-12 || d3 > 1e-12;
    !(below && above)
}

fn merge_convex(mut parts: Vec<Vec<Pt>>) -> Vec<Vec<Pt>> {
    let mut again = true;
    while again {
        again = false;
        'pair: for i in 0..parts.len() {
            for j in (i + 1)..parts.len() {
                let joined = match join(&parts[i], &parts[j]) {
                    Some(u) if is_convex(&u) => u,
                    _ => continue,
                };
                parts[i] = joined;
                parts.remove(j);
                again = true;
                break 'pair;
            }
        }
    }
    parts
}

/// Two convex parts as one ring, when they share an edge: `a` runs the edge
/// one way and `b` the other, and the ring is `a` from the far end of that
/// edge round to its near end, then `b` round the same way.
fn join(a: &[Pt], b: &[Pt]) -> Option<Vec<Pt>> {
    let (na, nb) = (a.len(), b.len());
    if na < 3 || nb < 3 {
        return None;
    }
    for i in 0..na {
        let near = a[i];
        let far = a[(i + 1) % na];
        for j in 0..nb {
            if !same(b[j], far) || !same(b[(j + 1) % nb], near) {
                continue;
            }
            let mut out: Vec<Pt> = Vec::with_capacity(na + nb - 2);
            for k in 0..na {
                out.push(a[(i + 1 + k) % na]);
            }
            for k in 2..nb {
                out.push(b[(j + k) % nb]);
            }
            return Some(out);
        }
    }
    None
}

fn same(a: Pt, b: Pt) -> bool {
    (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
}

/// The Minkowski sum of two convex rings: start at the sum of their lowest
/// corners, then walk both rings' edges in order of angle, taking whichever
/// turns first, or both at once when they turn the same.
fn minkowski(a: &[Pt], b: &[Pt]) -> Vec<Pt> {
    let a = positive(a);
    let b = positive(b);
    let (na, nb) = (a.len(), b.len());
    if na < 3 || nb < 3 {
        return Vec::new();
    }
    let ia = lowest(&a);
    let ib = lowest(&b);
    let mut out: Vec<Pt> = vec![(a[ia].0 + b[ib].0, a[ia].1 + b[ib].1)];
    let (mut i, mut j) = (0usize, 0usize);
    let mut guard = 0usize;
    while (i < na || j < nb) && guard < na + nb + 4 {
        guard += 1;
        let ea = if i < na {
            sub(a[(ia + i + 1) % na], a[(ia + i) % na])
        } else {
            (0.0, 0.0)
        };
        let eb = if j < nb {
            sub(b[(ib + j + 1) % nb], b[(ib + j) % nb])
        } else {
            (0.0, 0.0)
        };
        let step = if i >= na {
            j += 1;
            eb
        } else if j >= nb {
            i += 1;
            ea
        } else {
            let turn = cross(ea, eb);
            if turn > 1e-12 {
                i += 1;
                ea
            } else if turn < -1e-12 {
                j += 1;
                eb
            } else {
                i += 1;
                j += 1;
                (ea.0 + eb.0, ea.1 + eb.1)
            }
        };
        let last = match out.last() {
            Some(&p) => p,
            None => (0.0, 0.0),
        };
        out.push((last.0 + step.0, last.1 + step.1));
    }
    // The walk comes back to where it started; that closing point repeats
    // the first one.
    if out.len() > 1 {
        let first = out[0];
        let last = out[out.len() - 1];
        if (last.0 - first.0).abs() < 1e-9 && (last.1 - first.1).abs() < 1e-9 {
            out.pop();
        }
    }
    out
}

fn lowest(p: &[Pt]) -> usize {
    let mut k = 0;
    for i in 1..p.len() {
        if p[i].1 < p[k].1 - 1e-12
            || ((p[i].1 - p[k].1).abs() <= 1e-12 && p[i].0 < p[k].0)
        {
            k = i;
        }
    }
    k
}

/// A convex ring grown by `half` in every direction: its Minkowski sum with a
/// square of that half side. A square rather than a disc, so the spacing it
/// leaves is at least `2 * half` in one of the two directions.
fn inflate(p: &[Pt], half: f64) -> Vec<Pt> {
    if half <= 1e-12 || p.len() < 3 {
        return p.to_vec();
    }
    let square = [(-half, -half), (half, -half), (half, half), (-half, half)];
    let sum = minkowski(p, &square);
    if sum.len() >= 3 {
        sum
    } else {
        p.to_vec()
    }
}

/// The part of a ring inside a box, by clipping it against the box's four
/// sides in turn. A convex ring stays convex, so its vertices are still the
/// only places a placement can be.
fn clip(poly: &[Pt], x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Pt> {
    let mut v: Vec<Pt> = poly.to_vec();
    for k in 0..4 {
        if v.len() < 3 {
            return Vec::new();
        }
        let mut out: Vec<Pt> = Vec::with_capacity(v.len() + 4);
        for i in 0..v.len() {
            let a = v[i];
            let b = v[(i + 1) % v.len()];
            let ia = side(a, k, x0, y0, x1, y1) >= 0.0;
            let ib = side(b, k, x0, y0, x1, y1) >= 0.0;
            if ia && ib {
                out.push(b);
            } else if ia != ib {
                let t = crossing(a, b, k, x0, y0, x1, y1);
                out.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
                if ib {
                    out.push(b);
                }
            }
        }
        v = out;
    }
    v
}

/// How far inside the k-th side of the box a point is: positive inside.
fn side(p: Pt, k: usize, x0: f64, y0: f64, x1: f64, y1: f64) -> f64 {
    match k {
        0 => p.0 - x0,
        1 => x1 - p.0,
        2 => p.1 - y0,
        _ => y1 - p.1,
    }
}

fn crossing(a: Pt, b: Pt, k: usize, x0: f64, y0: f64, x1: f64, y1: f64) -> f64 {
    let (da, db) = (side(a, k, x0, y0, x1, y1), side(b, k, x0, y0, x1, y1));
    let d = da - db;
    if d.abs() < 1e-12 {
        0.0
    } else {
        da / d
    }
}

/// Whether a point is inside a ring, by counting the crossings of a ray
/// going right from it. A point on the ring counts as outside, so a part
/// that only touches another one is free to sit there.
fn inside(p: Pt, poly: &[Pt]) -> bool {
    let n = poly.len();
    if n < 3 {
        return false;
    }
    // On the ring itself: touching is allowed, so it is not inside.
    for i in 0..n {
        if on_segment(p, poly[i], poly[(i + 1) % n]) {
            return false;
        }
    }
    let mut hit = false;
    let mut j = n - 1;
    for i in 0..n {
        let a = poly[i];
        let b = poly[j];
        if (a.1 > p.1) != (b.1 > p.1) {
            let x = a.0 + (p.1 - a.1) / (b.1 - a.1) * (b.0 - a.0);
            if p.0 < x {
                hit = !hit;
            }
        }
        j = i;
    }
    hit
}

/// Whether a point lies on the line between two others, near enough that it
/// is the same place.
fn on_segment(p: Pt, a: Pt, b: Pt) -> bool {
    let ab = sub(b, a);
    let ap = sub(p, a);
    let len = (ab.0 * ab.0 + ab.1 * ab.1).sqrt();
    if len < 1e-12 {
        return same(p, a);
    }
    if cross(ab, ap).abs() / len > 1e-7 {
        return false;
    }
    let t = (ap.0 * ab.0 + ap.1 * ab.1) / (len * len);
    (-1e-9..=1.0 + 1e-9).contains(&t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ring(points: &[(f64, f64)]) -> Vec<Point> {
        points.iter().map(|&(x, y)| Point::new(x, y)).collect()
    }

    fn options(w: f64, h: f64, gap: f64) -> Options {
        Options {
            width: w,
            height: h,
            gap,
            angles: vec![0.0],
        }
    }

    fn square(x: f64, y: f64, side: f64) -> Vec<(f64, f64)> {
        vec![(x, y), (x + side, y), (x + side, y + side), (x, y + side)]
    }

    #[test]
    fn two_squares_sum_to_a_square_of_the_sum() {
        let a = square(0.0, 0.0, 10.0);
        let b = square(0.0, 0.0, 10.0);
        let sum = minkowski(&a, &b);
        assert_eq!(sum.len(), 4, "{sum:?}");
        let b = bbox(&sum);
        assert!((b.0).abs() < 1e-9 && (b.1).abs() < 1e-9, "{b:?}");
        assert!((b.2 - 20.0).abs() < 1e-9 && (b.3 - 20.0).abs() < 1e-9, "{b:?}");
        // `area2` is twice the area, as the shoelace gives it.
        assert!((area2(&sum) - 800.0).abs() < 1e-6, "{}", area2(&sum));
    }

    #[test]
    fn a_square_and_its_mirror_are_a_square_of_the_sum() {
        let a = square(0.0, 0.0, 10.0);
        let mirrored: Vec<Pt> = square(0.0, 0.0, 10.0)
            .iter()
            .map(|&(x, y)| (-x, -y))
            .collect();
        let sum = minkowski(&a, &mirrored);
        let b = bbox(&sum);
        assert!((b.2 - b.0 - 20.0).abs() < 1e-9, "{b:?}");
        assert!((b.3 - b.1 - 20.0).abs() < 1e-9, "{b:?}");
    }

    #[test]
    fn an_l_shape_is_cut_into_parts_that_add_up_to_it() {
        let l = vec![
            (0.0, 0.0),
            (100.0, 0.0),
            (100.0, 40.0),
            (40.0, 40.0),
            (40.0, 100.0),
            (0.0, 100.0),
        ];
        let parts = convex_parts(&l);
        assert!(parts.len() >= 2, "{parts:?}");
        let total: f64 = parts.iter().map(|p| area2(p).abs()).sum();
        assert!((total - area2(&l).abs()).abs() < 1e-6, "{total}");
        for p in &parts {
            assert!(is_convex(p), "not convex: {p:?}");
        }
    }

    #[test]
    fn a_part_sits_in_the_hollow_of_another() {
        // An L that fills the sheet's box, and a square that only fits in
        // its notch. Packing boxes would put the square on another sheet.
        let l = ring(&[
            (0.0, 0.0),
            (100.0, 0.0),
            (100.0, 40.0),
            (40.0, 40.0),
            (40.0, 100.0),
            (0.0, 100.0),
        ]);
        let square_ring = ring(&square(0.0, 0.0, 40.0));
        let spots = nest(&[vec![l], vec![square_ring]], &options(100.0, 100.0, 0.0));
        assert_eq!(spots.len(), 2);
        assert_eq!(spots[1].sheet, 0, "it went to another sheet: {spots:?}");
        // In the packing's own frame the notch is the right hand side at the
        // top, so the square is to the right of the L's arm.
        assert!(spots[1].x >= 39.0, "not in the notch: {spots:?}");
        assert!(spots[1].y <= 61.0, "not in the notch: {spots:?}");
    }

    #[test]
    fn two_parts_on_a_sheet_do_not_overlap() {
        let a = ring(&square(0.0, 0.0, 50.0));
        let b = ring(&square(0.0, 0.0, 50.0));
        let spots = nest(&[vec![a], vec![b]], &options(100.0, 100.0, 0.0));
        assert_eq!(spots[0].sheet, 0);
        assert_eq!(spots[1].sheet, 0, "{spots:?}");
        let apart = spots[0].x + 50.0 <= spots[1].x + 1e-6
            || spots[1].x + 50.0 <= spots[0].x + 1e-6
            || spots[0].y + 50.0 <= spots[1].y + 1e-6
            || spots[1].y + 50.0 <= spots[0].y + 1e-6;
        assert!(apart, "{:?} and {:?} overlap", spots[0], spots[1]);
    }

    #[test]
    fn the_spacing_is_kept_between_two_squares() {
        let a = ring(&square(0.0, 0.0, 40.0));
        let b = ring(&square(0.0, 0.0, 40.0));
        let spots = nest(&[vec![a], vec![b]], &options(200.0, 200.0, 10.0));
        assert_eq!(spots[1].sheet, 0, "{spots:?}");
        let dx = (spots[0].x - spots[1].x).abs();
        let dy = (spots[0].y - spots[1].y).abs();
        assert!(
            dx >= 49.0 || dy >= 49.0,
            "a gap of ten is not kept: {:?} {:?}",
            spots[0],
            spots[1]
        );
    }

    #[test]
    fn a_second_sheet_when_the_first_is_full() {
        let mut rings: Vec<Vec<Vec<Point>>> = Vec::new();
        for _ in 0..3 {
            rings.push(vec![ring(&square(0.0, 0.0, 90.0))]);
        }
        let spots = nest(&rings, &options(100.0, 100.0, 0.0));
        let sheets = spots.iter().map(|s| s.sheet + 1).max().unwrap_or(0);
        assert_eq!(sheets, 3, "{spots:?}");
    }

    #[test]
    fn a_hole_is_carried_by_its_outline() {
        let outer = square(0.0, 0.0, 100.0);
        let hole = square(30.0, 30.0, 40.0);
        let (parts, w, h) = local_parts(&[outer, hole]);
        assert!(
            (w - 100.0).abs() < 1e-9 && (h - 100.0).abs() < 1e-9,
            "{w} by {h}"
        );
        let total: f64 = parts.iter().map(|p| area2(p).abs() * 0.5).sum();
        assert!(
            (total - 8400.0).abs() < 1.0,
            "the material comes to {total}, not 8400"
        );
    }

    #[test]
    fn two_ls_keep_the_spacing_asked_for() {
        // Not a box: the corners of one L end up in the hollow of the other,
        // and the spacing has to hold between the outlines, not the boxes.
        let l = vec![
            (0.0, 0.0),
            (60.0, 0.0),
            (60.0, 20.0),
            (20.0, 20.0),
            (20.0, 60.0),
            (0.0, 60.0),
        ];
        let a = ring(&l);
        let b = ring(&l);
        let spots = nest(
            &[vec![a.clone()], vec![b.clone()]],
            &options(400.0, 400.0, 10.0),
        );
        assert_eq!(spots[1].sheet, 0, "{spots:?}");
        let placed = |pts: &[Point], s: &Spot| -> Vec<(f64, f64)> {
            pts.iter().map(|p| (p.x + s.x, p.y - s.y)).collect()
        };
        let pa = placed(&a, &spots[0]);
        let pb = placed(&b, &spots[1]);
        let mut closest = f64::MAX;
        for i in 0..pa.len() {
            for j in 0..pb.len() {
                let d = ((pa[i].0 - pb[j].0).powi(2) + (pa[i].1 - pb[j].1).powi(2)).sqrt();
                closest = closest.min(d);
            }
        }
        assert!(closest >= 9.9, "the spacing came out {closest}");
    }

    #[test]
    fn a_part_goes_inside_a_hole() {
        // A square with a hole through it, on a sheet exactly its own size:
        // the only room left is the hole.
        let outer = ring(&square(0.0, 0.0, 100.0));
        let hole = ring(&square(30.0, 30.0, 40.0));
        let small = ring(&square(0.0, 0.0, 20.0));
        let spots = nest(
            &[vec![outer, hole], vec![small]],
            &options(100.0, 100.0, 0.0),
        );
        assert_eq!(spots[1].sheet, 0, "{spots:?}");
        assert!(
            spots[1].x >= 29.0
                && spots[1].x <= 51.0
                && spots[1].y >= 29.0
                && spots[1].y <= 51.0,
            "it is not in the hole: {spots:?}"
        );
    }

    #[test]
    fn a_point_on_a_ring_is_not_inside_it() {
        let s = square(0.0, 0.0, 10.0);
        assert!(inside((5.0, 5.0), &s));
        assert!(!inside((0.0, 5.0), &s));
        assert!(!inside((15.0, 5.0), &s));
    }

    #[test]
    fn clipping_a_ring_to_a_box_keeps_its_corners() {
        let s = square(0.0, 0.0, 10.0);
        let c = clip(&s, -5.0, -5.0, 5.0, 5.0);
        assert_eq!(c.len(), 4, "{c:?}");
        let b = bbox(&c);
        assert!((b.0).abs() < 1e-9 && (b.1).abs() < 1e-9, "{b:?}");
        assert!((b.2 - 5.0).abs() < 1e-9 && (b.3 - 5.0).abs() < 1e-9, "{b:?}");
    }
}
