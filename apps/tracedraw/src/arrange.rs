//! 智能排布：把选中的对象摆成紧凑的几行。
//!
//! 自己的算法，与排料（nesting）不同：只挪整块的外框，不旋转、不嵌合，
//! 所以结果可预测，也不会把一个圆形挤进另一个的凹处。
//!
//! 做法是「按行装箱」：对象按高度从高到低依次入行，放不下就换一行，行高
//! 取该行最高的那个。行宽不写死，而是试一排候选值，取摆出来最方正的那一
//! 种（最大边最小，同分时取更窄的），于是两个对象不会被拉成一条长线，十
//! 个也不会叠成一根柱子。

/// 一行的宽度候选值有多少个：够密，又不至于慢。
const TRIES: usize = 32;

/// The places the items take, in a box whose top-left corner is `(0, 0)`
/// and whose y grows downward, with the box's width and height. `gap` is
/// the room kept between neighbours, in the same units.
pub fn shelf(sizes: &[(f64, f64)], gap: f64) -> (Vec<(f64, f64)>, f64, f64) {
    if sizes.is_empty() {
        return (Vec::new(), 0.0, 0.0);
    }
    let gap = if gap.is_finite() { gap.max(0.0) } else { 0.0 };
    let widest = sizes.iter().fold(0.0f64, |w, s| w.max(size(s.0)));
    let total: f64 = sizes.iter().map(|s| size(s.0) + gap).sum();
    // A candidate of the widest item puts every one of them on a row of its
    // own; one of the whole line puts them all on one. The best is between.
    let mut best: Option<(Vec<(f64, f64)>, f64, f64, f64)> = None;
    for step in 0..=TRIES {
        let target = widest + (total - widest) * step as f64 / TRIES as f64;
        let (spots, w, h) = pack(sizes, gap, target);
        let score = w.max(h);
        let better = match &best {
            None => true,
            Some((_, bw, _, bs)) => {
                score < *bs - 1e-9 || ((score - *bs).abs() <= 1e-9 && w < *bw - 1e-9)
            }
        };
        if better {
            best = Some((spots, w, h, score));
        }
    }
    match best {
        Some((spots, w, h, _)) => (spots, w, h),
        None => (Vec::new(), 0.0, 0.0),
    }
}

/// One arrangement: rows filled left to right at `target`, the tallest
/// items first, so the rows stay level.
fn pack(sizes: &[(f64, f64)], gap: f64, target: f64) -> (Vec<(f64, f64)>, f64, f64) {
    let mut order: Vec<usize> = (0..sizes.len()).collect();
    order.sort_by(|a, b| {
        size(sizes[*b].1)
            .partial_cmp(&size(sizes[*a].1))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut spots = vec![(0.0, 0.0); sizes.len()];
    let (mut x, mut y, mut row_h, mut wide) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for i in order {
        let (w, h) = (size(sizes[i].0), size(sizes[i].1));
        if x > 0.0 && x + w > target + 1e-9 {
            y += row_h + gap;
            x = 0.0;
            row_h = 0.0;
        }
        spots[i] = (x, y);
        x += w + gap;
        row_h = row_h.max(h);
        wide = wide.max(x - gap);
    }
    (spots, wide.max(0.0), y + row_h)
}

/// A size that is not a number or is negative counts as nothing.
fn size(v: f64) -> f64 {
    if v.is_finite() && v > 0.0 {
        v
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_to_place_takes_no_room() {
        let (spots, w, h) = shelf(&[], 5.0);
        assert!(spots.is_empty());
        assert_eq!((w, h), (0.0, 0.0));
    }

    #[test]
    fn one_object_stays_where_it_is() {
        let (spots, w, h) = shelf(&[(30.0, 20.0)], 5.0);
        assert_eq!(spots, vec![(0.0, 0.0)]);
        assert_eq!((w, h), (30.0, 20.0));
    }

    #[test]
    fn four_squares_come_out_square() {
        let (spots, w, h) = shelf(&[(10.0, 10.0); 4], 0.0);
        assert_eq!((w, h), (20.0, 20.0));
        let mut seen = spots.clone();
        seen.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(a.1.partial_cmp(&b.1).unwrap()));
        assert_eq!(seen, vec![(0.0, 0.0), (0.0, 10.0), (10.0, 0.0), (10.0, 10.0)]);
    }

    #[test]
    fn the_gap_is_kept_between_neighbours() {
        let (spots, w, h) = shelf(&[(10.0, 10.0); 4], 4.0);
        for (i, a) in spots.iter().enumerate() {
            for b in &spots[i + 1..] {
                let apart = (a.0 - b.0).abs() >= 10.0 + 4.0 - 1e-9
                    || (a.1 - b.1).abs() >= 10.0 + 4.0 - 1e-9;
                assert!(apart, "{a:?} and {b:?} are too close");
            }
        }
        assert!(w <= 24.0 + 1e-9 && h <= 24.0 + 1e-9, "{w} x {h}");
    }

    #[test]
    fn a_tall_and_a_wide_one_do_not_overlap() {
        let (spots, w, h) = shelf(&[(40.0, 6.0), (6.0, 40.0)], 2.0);
        assert_eq!(spots.len(), 2);
        assert!(w > 0.0 && h > 0.0);
        // The tall one goes first, and the wide one beside or below it.
        let (a, b) = (spots[0], spots[1]);
        assert!(
            (a.0 - b.0).abs() >= 40.0 - 1e-9
                || (a.1 - b.1).abs() >= 40.0 - 1e-9
                || (a.0 - b.0).abs() >= 6.0 + 2.0 - 1e-9
                || (a.1 - b.1).abs() >= 6.0 + 2.0 - 1e-9
        );
    }

    #[test]
    fn sizes_that_are_not_numbers_count_as_nothing() {
        let (spots, w, h) = shelf(&[(f64::NAN, 10.0), (10.0, f64::INFINITY)], 1.0);
        assert_eq!(spots.len(), 2);
        assert!(w.is_finite() && h.is_finite());
    }
}
