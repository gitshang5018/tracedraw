//! 智能群组: the objects that lie close to each other, each set a group of
//! its own. Two objects belong together when they meet once each is grown
//! by `gap`, so a row of shapes a little apart is one group and a shape
//! across the page is its own.

/// 智能群组: the boxes that belong together, each cluster a list of the
/// indices it holds, in the order they were given. Two boxes belong
/// together when they meet once each is grown by `gap`, so a cluster is
/// everything reachable that way — a row of shapes a little apart is one
/// group, and a shape across the page is its own.
pub fn clusters(boxes: &[(f64, f64, f64, f64)], gap: f64) -> Vec<Vec<usize>> {
    let n = boxes.len();
    if n == 0 {
        return Vec::new();
    }
    let gap = if gap.is_finite() { gap.max(0.0) } else { 0.0 };
    let mut parent: Vec<usize> = (0..n).collect();
    for i in 0..n {
        for j in i + 1..n {
            if meets(boxes[i], boxes[j], gap) {
                let (a, b) = (root(&mut parent, i), root(&mut parent, j));
                if a != b {
                    parent[a] = b;
                }
            }
        }
    }
    let mut out: Vec<Vec<usize>> = Vec::new();
    let mut seen: Vec<Option<usize>> = vec![None; n];
    for i in 0..n {
        let r = root(&mut parent, i);
        match seen[r] {
            Some(k) => out[k].push(i),
            None => {
                seen[r] = Some(out.len());
                out.push(vec![i]);
            }
        }
    }
    out
}

fn root(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

/// Whether two boxes touch once each is grown by `gap`.
fn meets(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64), gap: f64) -> bool {
    let (min_xa, max_xa) = (a.0.min(a.2), a.0.max(a.2));
    let (min_ya, max_ya) = (a.1.min(a.3), a.1.max(a.3));
    let (min_xb, max_xb) = (b.0.min(b.2), b.0.max(b.2));
    let (min_yb, max_yb) = (b.1.min(b.3), b.1.max(b.3));
    min_xa - gap <= max_xb && min_xb - gap <= max_xa && min_ya - gap <= max_yb && min_yb - gap <= max_ya
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_selected_makes_no_cluster() {
        assert!(clusters(&[], 2.0).is_empty());
    }

    #[test]
    fn shapes_a_little_apart_belong_together() {
        // Three in a row, 1 apart, with a gap of 2 between them allowed.
        let boxes = [
            (0.0, 0.0, 10.0, 10.0),
            (11.0, 0.0, 21.0, 10.0),
            (22.0, 0.0, 32.0, 10.0),
            (100.0, 0.0, 110.0, 10.0),
        ];
        let found = clusters(&boxes, 2.0);
        assert_eq!(found, vec![vec![0, 1, 2], vec![3]]);
    }

    #[test]
    fn a_gap_wider_than_the_room_splits_them() {
        let boxes = [(0.0, 0.0, 10.0, 10.0), (20.0, 0.0, 30.0, 10.0)];
        assert_eq!(clusters(&boxes, 2.0), vec![vec![0], vec![1]]);
        assert_eq!(clusters(&boxes, 10.0), vec![vec![0, 1]]);
    }

    #[test]
    fn overlapping_shapes_are_one_cluster() {
        let boxes = [(0.0, 0.0, 10.0, 10.0), (5.0, 5.0, 15.0, 15.0)];
        assert_eq!(clusters(&boxes, 0.0), vec![vec![0, 1]]);
    }
}
