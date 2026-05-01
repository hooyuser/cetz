pub use super::super::error::PathBoolErr;
pub use super::super::op::path_bool;
pub use super::super::wire::{PathBoolArgs, WirePath, WireSegment, WireSubpath};

pub fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> WirePath {
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

pub fn empty_path() -> WirePath {
    WirePath { subpaths: Vec::new() }
}

pub fn run(a: WirePath, b: WirePath, op: &str) -> WirePath {
    run_with(a, b, op, "non-zero", "non-zero")
}

pub fn run_with(a: WirePath, b: WirePath, op: &str, fr_a: &str, fr_b: &str) -> WirePath {
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

pub fn bbox(path: &WirePath) -> Option<(f64, f64, f64, f64)> {
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
    if empty { None } else { Some((min_x, min_y, max_x, max_y)) }
}

pub fn approx_eq(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-3
}

pub fn approx_box(actual: (f64, f64, f64, f64), expected: (f64, f64, f64, f64)) -> bool {
    approx_eq(actual.0, expected.0)
        && approx_eq(actual.1, expected.1)
        && approx_eq(actual.2, expected.2)
        && approx_eq(actual.3, expected.3)
}

/// Shoelace area for a path made entirely of line segments.
/// Sums the absolute area of each subpath independently — correct for
/// simple connected regions but not for shapes with holes.
pub fn linear_area(path: &WirePath) -> f64 {
    path.subpaths
        .iter()
        .map(|sp| {
            let mut pts = vec![sp.origin];
            for seg in &sp.segments {
                match seg {
                    WireSegment::Line { to } => pts.push(*to),
                    WireSegment::Cubic { .. } => panic!("linear_area: non-linear segment"),
                }
            }
            let n = pts.len();
            let signed: f64 = (0..n)
                .map(|i| {
                    let [x1, y1] = pts[i];
                    let [x2, y2] = pts[(i + 1) % n];
                    x1 * y2 - x2 * y1
                })
                .sum();
            signed.abs() / 2.0
        })
        .sum()
}

/// Outer 4×4 square + inner same-winding 2×2 square.
/// fill_rule selects whether the inner region is filled (non-zero) or a hole
/// (even-odd).
pub fn doubly_wound_annulus() -> WirePath {
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
