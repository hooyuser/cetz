//! Top-level `path_bool` entry point: parse args, run linesweeper, convert
//! back to the wire format.

use std::panic::AssertUnwindSafe;

use linesweeper::{BinaryOp, FillRule};

use crate::path_bool::convert::{bez_to_wire, wire_to_bez};
use crate::path_bool::error::BoolError;
use crate::path_bool::wire::{PathBoolArgs, PathBoolOutput, WirePath};

fn parse_op(op: &str) -> Result<BinaryOp, BoolError> {
    match op {
        "union" => Ok(BinaryOp::Union),
        "intersection" => Ok(BinaryOp::Intersection),
        "difference" => Ok(BinaryOp::Difference),
        "xor" => Ok(BinaryOp::Xor),
        _ => Err(BoolError::InvalidOp(op.to_string())),
    }
}

fn parse_fill_rule(rule: &str) -> Result<FillRule, BoolError> {
    match rule {
        "non-zero" => Ok(FillRule::NonZero),
        "even-odd" => Ok(FillRule::EvenOdd),
        _ => Err(BoolError::InvalidFillRule(rule.to_string())),
    }
}

pub fn path_bool(args: PathBoolArgs) -> Result<PathBoolOutput, BoolError> {
    let op = parse_op(&args.op)?;
    let fill_rule = parse_fill_rule(&args.fill_rule)?;
    let a = wire_to_bez(&args.a)?;
    let b = wire_to_bez(&args.b)?;

    // catch_unwind so a panic inside linesweeper turns into a recoverable
    // error rather than aborting the WASM module.
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| match args.eps {
        Some(eps) => linesweeper::binary_op_with_eps(&a, &b, fill_rule, op, eps),
        None => linesweeper::binary_op(&a, &b, fill_rule, op),
    }));

    let contours = match result {
        Ok(Ok(c)) => c,
        Ok(Err(e)) => return Err(BoolError::LinesweeperFailed(e.to_string())),
        Err(_) => {
            return Err(BoolError::LinesweeperFailed(
                "linesweeper panicked".into(),
            ));
        }
    };

    let mut combined = WirePath {
        subpaths: Vec::new(),
    };
    for contour in contours.contours() {
        let mut wire = bez_to_wire(&contour.path)?;
        combined.subpaths.append(&mut wire.subpaths);
    }
    Ok(PathBoolOutput { path: combined })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path_bool::wire::{WireSegment, WireSubpath};
    use kurbo::Shape;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> WirePath {
        WirePath {
            subpaths: vec![WireSubpath {
                origin: [x0, y0],
                closed: true,
                segments: vec![
                    WireSegment::Line { to: [x1, y0] },
                    WireSegment::Line { to: [x1, y1] },
                    WireSegment::Line { to: [x0, y1] },
                ],
            }],
        }
    }

    fn empty_path() -> WirePath {
        WirePath {
            subpaths: Vec::new(),
        }
    }

    fn run(a: WirePath, b: WirePath, op: &str) -> WirePath {
        path_bool(PathBoolArgs {
            a,
            b,
            op: op.into(),
            fill_rule: "non-zero".into(),
            eps: None,
        })
        .expect("path_bool failed")
        .path
    }

    fn bbox(path: &WirePath) -> Option<(f64, f64, f64, f64)> {
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        let mut empty = true;
        for sp in &path.subpaths {
            empty = false;
            let mut update = |p: [f64; 2]| {
                min_x = min_x.min(p[0]);
                min_y = min_y.min(p[1]);
                max_x = max_x.max(p[0]);
                max_y = max_y.max(p[1]);
            };
            update(sp.origin);
            for seg in &sp.segments {
                match seg {
                    WireSegment::Line { to } => update(*to),
                    WireSegment::Cubic { c1, c2, to } => {
                        update(*c1);
                        update(*c2);
                        update(*to);
                    }
                }
            }
        }
        if empty {
            None
        } else {
            Some((min_x, min_y, max_x, max_y))
        }
    }

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-3
    }

    fn approx_box(actual: (f64, f64, f64, f64), expected: (f64, f64, f64, f64)) -> bool {
        approx_eq(actual.0, expected.0)
            && approx_eq(actual.1, expected.1)
            && approx_eq(actual.2, expected.2)
            && approx_eq(actual.3, expected.3)
    }

    // ---- Step A4: four ops on overlapping unit squares ----

    #[test]
    fn union_of_overlapping_squares() {
        // a = [0,0] -> [1,1]; b = [0.5,0.5] -> [1.5,1.5]
        let r = run(rect(0.0, 0.0, 1.0, 1.0), rect(0.5, 0.5, 1.5, 1.5), "union");
        assert_eq!(r.subpaths.len(), 1, "union should be one connected contour");
        let bb = bbox(&r).expect("non-empty result");
        assert!(
            approx_box(bb, (0.0, 0.0, 1.5, 1.5)),
            "union bbox mismatch: {bb:?}"
        );
    }

    #[test]
    fn intersection_of_overlapping_squares() {
        let r = run(
            rect(0.0, 0.0, 1.0, 1.0),
            rect(0.5, 0.5, 1.5, 1.5),
            "intersection",
        );
        assert_eq!(r.subpaths.len(), 1);
        let bb = bbox(&r).unwrap();
        assert!(
            approx_box(bb, (0.5, 0.5, 1.0, 1.0)),
            "intersection bbox: {bb:?}"
        );
    }

    #[test]
    fn difference_of_overlapping_squares() {
        // a - b: removes the upper-right corner of a
        let r = run(
            rect(0.0, 0.0, 1.0, 1.0),
            rect(0.5, 0.5, 1.5, 1.5),
            "difference",
        );
        assert_eq!(r.subpaths.len(), 1);
        let bb = bbox(&r).unwrap();
        assert!(approx_box(bb, (0.0, 0.0, 1.0, 1.0)), "diff bbox: {bb:?}");
    }

    #[test]
    fn xor_of_overlapping_squares() {
        let r = run(rect(0.0, 0.0, 1.0, 1.0), rect(0.5, 0.5, 1.5, 1.5), "xor");
        // xor of two corner-overlapping squares: two L-shaped pieces sharing
        // the inner corner. linesweeper may yield 1 connected contour or 2.
        assert!(r.subpaths.len() >= 1);
        let bb = bbox(&r).unwrap();
        assert!(approx_box(bb, (0.0, 0.0, 1.5, 1.5)), "xor bbox: {bb:?}");
    }

    #[test]
    fn disjoint_union_yields_two_subpaths() {
        let r = run(
            rect(0.0, 0.0, 1.0, 1.0),
            rect(2.0, 2.0, 3.0, 3.0),
            "union",
        );
        assert_eq!(r.subpaths.len(), 2);
    }

    #[test]
    fn difference_with_inner_hole() {
        // a fully contains b; a - b should yield outer + inner ring
        let r = run(
            rect(0.0, 0.0, 4.0, 4.0),
            rect(1.0, 1.0, 3.0, 3.0),
            "difference",
        );
        assert!(
            r.subpaths.len() >= 2,
            "expected outer + inner ring, got {}",
            r.subpaths.len()
        );
    }

    #[test]
    fn invalid_op_returns_error() {
        let err = path_bool(PathBoolArgs {
            a: rect(0.0, 0.0, 1.0, 1.0),
            b: rect(0.0, 0.0, 1.0, 1.0),
            op: "subtract".into(),
            fill_rule: "non-zero".into(),
            eps: None,
        })
        .unwrap_err();
        assert!(matches!(err, BoolError::InvalidOp(_)));
    }

    #[test]
    fn invalid_fill_rule_returns_error() {
        let err = path_bool(PathBoolArgs {
            a: rect(0.0, 0.0, 1.0, 1.0),
            b: rect(0.0, 0.0, 1.0, 1.0),
            op: "union".into(),
            fill_rule: "evenodd".into(),
            eps: None,
        })
        .unwrap_err();
        assert!(matches!(err, BoolError::InvalidFillRule(_)));
    }

    // ---- Step A5: edge cases ----

    #[test]
    fn empty_b_truth_table() {
        // a is a unit square, b is empty
        // union(a, ∅) = a
        let u = run(rect(0.0, 0.0, 1.0, 1.0), empty_path(), "union");
        assert_eq!(u.subpaths.len(), 1);
        // intersection(a, ∅) = ∅
        let i = run(rect(0.0, 0.0, 1.0, 1.0), empty_path(), "intersection");
        assert_eq!(i.subpaths.len(), 0);
        // difference(a, ∅) = a
        let d = run(rect(0.0, 0.0, 1.0, 1.0), empty_path(), "difference");
        assert_eq!(d.subpaths.len(), 1);
        // xor(a, ∅) = a
        let x = run(rect(0.0, 0.0, 1.0, 1.0), empty_path(), "xor");
        assert_eq!(x.subpaths.len(), 1);
    }

    #[test]
    fn empty_both_inputs() {
        for op in &["union", "intersection", "difference", "xor"] {
            let r = run(empty_path(), empty_path(), op);
            assert!(
                r.subpaths.is_empty(),
                "{op} of empty/empty should be empty, got {} subpaths",
                r.subpaths.len()
            );
        }
    }

    #[test]
    fn shared_edge() {
        // Two squares sharing the edge x=1
        let r = run(rect(0.0, 0.0, 1.0, 1.0), rect(1.0, 0.0, 2.0, 1.0), "union");
        // Should fuse into a single 2x1 rectangle
        assert_eq!(r.subpaths.len(), 1);
        let bb = bbox(&r).unwrap();
        assert!(approx_box(bb, (0.0, 0.0, 2.0, 1.0)));
    }

    #[test]
    fn fully_contained_intersection() {
        // a fully contains b => intersection = b
        let r = run(
            rect(0.0, 0.0, 4.0, 4.0),
            rect(1.0, 1.0, 3.0, 3.0),
            "intersection",
        );
        assert_eq!(r.subpaths.len(), 1);
        let bb = bbox(&r).unwrap();
        assert!(approx_box(bb, (1.0, 1.0, 3.0, 3.0)));
    }

    #[test]
    fn open_subpath_input_errors() {
        let mut a = rect(0.0, 0.0, 1.0, 1.0);
        a.subpaths[0].closed = false;
        let err = path_bool(PathBoolArgs {
            a,
            b: rect(0.5, 0.5, 1.5, 1.5),
            op: "union".into(),
            fill_rule: "non-zero".into(),
            eps: None,
        })
        .unwrap_err();
        assert!(matches!(err, BoolError::OpenSubpath));
    }

    #[test]
    fn cubic_input_circle_via_kurbo() {
        // Use kurbo to build a circle (approximation of cubics) and union it
        // with itself. Result should still be one connected contour.
        let circ = kurbo::Circle::new((0.0, 0.0), 1.0).to_path(0.01);
        let wire_circle = bez_to_wire(&circ).unwrap();
        let r = run(wire_circle.clone(), wire_circle, "union");
        assert_eq!(r.subpaths.len(), 1);
    }

    #[test]
    fn auto_eps_matches_with_eps_choice() {
        // Sanity: passing an explicit eps close to the auto choice should
        // produce comparable bbox results (loose tolerance).
        let auto = run(rect(0.0, 0.0, 1.0, 1.0), rect(0.5, 0.5, 1.5, 1.5), "union");
        let with = path_bool(PathBoolArgs {
            a: rect(0.0, 0.0, 1.0, 1.0),
            b: rect(0.5, 0.5, 1.5, 1.5),
            op: "union".into(),
            fill_rule: "non-zero".into(),
            eps: Some(1e-6),
        })
        .unwrap()
        .path;
        assert_eq!(auto.subpaths.len(), with.subpaths.len());
    }
}
