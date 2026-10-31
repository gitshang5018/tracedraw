//! Packing a set of boxes onto sheets of one size.
//!
//! The boxes are placed by maximal free rectangles: a sheet starts as one
//! free rectangle, each box takes the one a heuristic likes best, and every
//! free rectangle the box overlaps is split around it. Four heuristics and
//! three orders are all tried, and the run that fills the fewest sheets
//! wins: the same parts come out well under one order and badly under
//! another, and which is which depends on the parts.
//!
//! Outlines that have to interlock are not this module's business: see
//! [`crate::nfp`], which packs the shapes themselves.

/// Where a box went: the sheet it is on, and the top left corner of the box
/// it fills on that sheet. Measured from the sheet's top left corner, with
/// y running down the sheet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    pub sheet: usize,
    pub x: f64,
    pub y: f64,
    /// Whether the packer turned it a quarter turn, which it may do when
    /// turning is allowed.
    pub turned: bool,
}

/// The sheet, and the rules of the packing.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// The sheet's width and height, in the drawing's units.
    pub width: f64,
    pub height: f64,
    /// The least space left between two boxes.
    pub gap: f64,
    /// Whether a box may be turned a quarter turn to make it fit.
    pub rotate: bool,
}

/// Where each box goes, in the order given. A box that no sheet can hold
/// still gets one, at its corner: a part too big to fit is a mistake in the
/// numbers, not a reason for it to vanish.
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
    let board_w = if o.width.is_finite() {
        o.width.max(1e-6)
    } else {
        1e-6
    };
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
        // A box takes the gap along with it, so the next one keeps its
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

/// The free rectangle a heuristic likes best, and whether the box goes in it
/// turned.
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
                // The short side left over, which leaves room for a part
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
                    first < b1 - 1e-9 || ((first - b1).abs() <= 1e-9 && second < b2 - 1e-9)
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
    fn a_box_too_wide_for_the_sheet_is_turned() {
        let sizes = [(200.0, 100.0)];
        let spots = pack(&sizes, &options(150.0, 250.0, 0.0, true));
        assert!(spots[0].turned, "{spots:?}");
        let spots = pack(&sizes, &options(150.0, 250.0, 0.0, false));
        assert!(!spots[0].turned, "nothing may be turned: {spots:?}");
    }

    #[test]
    fn the_gap_is_kept_between_boxes() {
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
}
