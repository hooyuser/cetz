//! Top-level `path_bool` entry point: parse args, build a linesweeper
//! topology, classify each region with per-operand fill rules, and convert
//! the result back to the wire format.
//!
//! linesweeper exposes `binary_op` / `binary_op_with_eps`, but those accept
//! only a single global `FillRule`. We need each operand to have its own
//! fill rule, so we drive the lower-level `topology::Topology` API directly:
//! `Topology::from_paths_binary` already separates winding numbers per shape
//! (`BinaryWindingNumber { shape_a, shape_b }`), and `Topology::contours`
//! takes a closure that classifies each region — exactly the seam we need.

use std::panic::AssertUnwindSafe;

use kurbo::{BezPath, Shape};
use linesweeper::topology::{BinaryWindingNumber, Topology};
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

/// Mirror of the auto-eps formula used inside `linesweeper::binary_op`. We
/// replicate it here because we call the lower-level topology API directly
/// and `binary_op`'s wrapper isn't on our path.
fn auto_eps(a: &BezPath, b: &BezPath) -> Result<f64, BoolError> {
    let bbox = a.bounding_box().union(b.bounding_box());
    let min = bbox.min_x().min(bbox.min_y());
    let max = bbox.max_x().max(bbox.max_y());
    if min.is_nan() || max.is_nan() {
        return Err(BoolError::LinesweeperFailed("NaN coordinate in input".into()));
    }
    if min.is_infinite() || max.is_infinite() {
        return Err(BoolError::LinesweeperFailed(
            "infinite coordinate in input".into(),
        ));
    }
    let m = min.abs().max(max.abs());
    let eps = (m * (f64::EPSILON * 64.0)).max(1e-6);
    debug_assert!(eps.is_finite());
    Ok(eps)
}

fn winding_inside(winding: i32, fill_rule: FillRule) -> bool {
    match fill_rule {
        FillRule::EvenOdd => winding % 2 != 0,
        FillRule::NonZero => winding != 0,
    }
}

pub fn path_bool(args: PathBoolArgs) -> Result<PathBoolOutput, BoolError> {
    let op = parse_op(&args.op)?;
    let fill_rule_a = parse_fill_rule(&args.fill_rule_a)?;
    let fill_rule_b = parse_fill_rule(&args.fill_rule_b)?;
    let a = wire_to_bez(&args.a)?;
    let b = wire_to_bez(&args.b)?;

    let eps = match args.eps {
        Some(eps) => eps,
        None => auto_eps(&a, &b)?,
    };

    // catch_unwind so a panic inside linesweeper turns into a recoverable
    // error rather than aborting the WASM module. linesweeper's
    // `NonClosedPath` lives in a private module, so we stringify it inside
    // the closure to keep our error type nameable here.
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let top = Topology::from_paths_binary(&a, &b, eps).map_err(|e| e.to_string())?;
        let inside = |w: &BinaryWindingNumber| {
            let ia = winding_inside(w.shape_a, fill_rule_a);
            let ib = winding_inside(w.shape_b, fill_rule_b);
            match op {
                BinaryOp::Union => ia || ib,
                BinaryOp::Intersection => ia && ib,
                BinaryOp::Xor => ia != ib,
                BinaryOp::Difference => ia && !ib,
            }
        };
        Ok::<_, String>(top.contours(inside))
    }));

    let contours = match result {
        Ok(Ok(c)) => c,
        Ok(Err(msg)) => return Err(BoolError::LinesweeperFailed(msg)),
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
        run_with(a, b, op, "non-zero", "non-zero")
    }

    fn run_with(a: WirePath, b: WirePath, op: &str, fr_a: &str, fr_b: &str) -> WirePath {
        path_bool(PathBoolArgs {
            a,
            b,
            op: op.into(),
            fill_rule_a: fr_a.into(),
            fill_rule_b: fr_b.into(),
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
            fill_rule_a: "non-zero".into(),
            fill_rule_b: "non-zero".into(),
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
            fill_rule_a: "evenodd".into(),
            fill_rule_b: "non-zero".into(),
            eps: None,
        })
        .unwrap_err();
        assert!(matches!(err, BoolError::InvalidFillRule(_)));

        let err = path_bool(PathBoolArgs {
            a: rect(0.0, 0.0, 1.0, 1.0),
            b: rect(0.0, 0.0, 1.0, 1.0),
            op: "union".into(),
            fill_rule_a: "non-zero".into(),
            fill_rule_b: "evenodd".into(),
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
            fill_rule_a: "non-zero".into(),
            fill_rule_b: "non-zero".into(),
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
            fill_rule_a: "non-zero".into(),
            fill_rule_b: "non-zero".into(),
            eps: Some(1e-6),
        })
        .unwrap()
        .path;
        assert_eq!(auto.subpaths.len(), with.subpaths.len());
    }

    // ---- Per-operand fill rules ----

    /// A rectangle that contains a smaller, oppositely-wound inner rectangle.
    /// Under non-zero this is a square with a square hole; under even-odd
    /// (which ignores winding direction) it's the same; the difference
    /// surfaces only when the inner rectangle has the *same* direction as
    /// the outer one — that's a self-overlapping "doubly-wound" shape, which
    /// non-zero treats as fully filled and even-odd treats as an annulus.
    fn doubly_wound_annulus() -> WirePath {
        // outer CCW, inner CCW (same winding) — fill_rule decides interpretation
        WirePath {
            subpaths: vec![
                WireSubpath {
                    origin: [0.0, 0.0],
                    closed: true,
                    segments: vec![
                        WireSegment::Line { to: [4.0, 0.0] },
                        WireSegment::Line { to: [4.0, 4.0] },
                        WireSegment::Line { to: [0.0, 4.0] },
                    ],
                },
                WireSubpath {
                    origin: [1.0, 1.0],
                    closed: true,
                    segments: vec![
                        WireSegment::Line { to: [3.0, 1.0] },
                        WireSegment::Line { to: [3.0, 3.0] },
                        WireSegment::Line { to: [1.0, 3.0] },
                    ],
                },
            ],
        }
    }

    #[test]
    fn per_operand_fill_rule_changes_result() {
        // probe = a small rect that sits entirely inside the inner hole region.
        // Under fill_rule_a = "non-zero" the doubly-wound shape is fully
        // filled, so intersection(a, probe) = probe.
        // Under fill_rule_a = "even-odd" the inner area is a hole, so
        // intersection(a, probe) is empty.
        let a = doubly_wound_annulus();
        let probe = rect(1.5, 1.5, 2.5, 2.5);

        let nz = run_with(a.clone(), probe.clone(), "intersection", "non-zero", "non-zero");
        let eo = run_with(a, probe, "intersection", "even-odd", "non-zero");

        let nz_bb = bbox(&nz).expect("non-zero result should fill the probe");
        assert!(
            approx_box(nz_bb, (1.5, 1.5, 2.5, 2.5)),
            "non-zero intersection should equal the probe; got bbox {nz_bb:?}"
        );
        assert!(
            eo.subpaths.is_empty(),
            "even-odd interprets the doubly-wound shape as an annulus, so the probe in the hole should yield empty; got {} subpaths",
            eo.subpaths.len()
        );
    }

    #[test]
    fn per_operand_fill_rule_is_independent() {
        // Symmetric to the test above, but the doubly-wound shape is operand
        // b. Verifies that fill_rule_b is wired up independently of fill_rule_a.
        let probe = rect(1.5, 1.5, 2.5, 2.5);
        let b = doubly_wound_annulus();

        let nz = run_with(probe.clone(), b.clone(), "intersection", "non-zero", "non-zero");
        let eo = run_with(probe, b, "intersection", "non-zero", "even-odd");

        assert!(
            !nz.subpaths.is_empty(),
            "non-zero on b should fill the inner area"
        );
        assert!(
            eo.subpaths.is_empty(),
            "even-odd on b makes inner area a hole; got {} subpaths",
            eo.subpaths.len()
        );
    }
}
