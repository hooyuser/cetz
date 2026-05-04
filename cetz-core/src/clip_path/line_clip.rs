use std::collections::HashMap;
use std::panic::AssertUnwindSafe;

use kurbo::{flatten, BezPath, PathEl, Point, Shape};
use linesweeper::sweep;
use linesweeper::{FillRule, SegIdx, Segments};

use crate::clip_path::ClipMode;
use crate::path_ops::convert::wire_to_closed_bez;
use crate::path_ops::error::PathOpsErr;
use crate::path_ops::fill::winding_inside;
use crate::path_ops::wire::{WirePath, WireSegment, WireSubpath};

#[derive(Debug, Clone)]
struct FlatSubpath {
    closed: bool,
    points: Vec<Point>,
}

#[derive(Debug, Clone, Copy)]
struct FlatLine {
    p0: Point,
    p1: Point,
}

fn close_enough(a: Point, b: Point, tol: f64) -> bool {
    a.distance_squared(b) <= tol * tol
}

fn lerp(a: Point, b: Point, t: f64) -> Point {
    Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

fn point_to_segment_distance_sq(p: Point, a: Point, b: Point) -> f64 {
    let ab = b - a;
    let len_sq = ab.hypot2();
    if len_sq == 0.0 {
        return p.distance_squared(a);
    }
    let ap = p - a;
    let t = (ap.dot(ab) / len_sq).clamp(0.0, 1.0);
    p.distance_squared(a + ab * t)
}

fn point_on_boundary(p: Point, boundary: &[FlatLine], tol: f64) -> bool {
    let tol_sq = tol * tol;
    boundary
        .iter()
        .any(|line| point_to_segment_distance_sq(p, line.p0, line.p1) <= tol_sq)
}

fn subpath_to_bez(subpath: &WireSubpath) -> BezPath {
    let mut bez = BezPath::new();
    bez.move_to(Point::new(subpath.origin[0], subpath.origin[1]));
    for seg in &subpath.segments {
        match seg {
            WireSegment::Line { to } => bez.line_to(Point::new(to[0], to[1])),
            WireSegment::Cubic { c1, c2, to } => bez.curve_to(
                Point::new(c1[0], c1[1]),
                Point::new(c2[0], c2[1]),
                Point::new(to[0], to[1]),
            ),
        }
    }
    if subpath.closed {
        bez.close_path();
    }
    bez
}

fn flatten_wire(path: &WirePath, tolerance: f64, tol: f64) -> Vec<FlatSubpath> {
    path.subpaths
        .iter()
        .filter_map(|subpath| {
            let bez = subpath_to_bez(subpath);
            let mut points = Vec::new();
            let origin = Point::new(subpath.origin[0], subpath.origin[1]);
            flatten(bez.iter(), tolerance, |el| match el {
                PathEl::MoveTo(p) => {
                    points.clear();
                    points.push(p);
                }
                PathEl::LineTo(p) => {
                    if points
                        .last()
                        .is_none_or(|last| !close_enough(*last, p, tol))
                    {
                        points.push(p);
                    }
                }
                PathEl::ClosePath => {
                    if points
                        .last()
                        .is_some_and(|last| !close_enough(*last, origin, tol))
                    {
                        points.push(origin);
                    }
                }
                PathEl::QuadTo(_, _) | PathEl::CurveTo(_, _, _) => unreachable!(),
            });
            if subpath.closed
                && points
                    .last()
                    .is_some_and(|last| !close_enough(*last, origin, tol))
            {
                points.push(origin);
            }
            (points.len() >= 2).then_some(FlatSubpath {
                closed: subpath.closed,
                points,
            })
        })
        .collect()
}

fn lines_for_subpath(subpath: &FlatSubpath) -> impl Iterator<Item = FlatLine> + '_ {
    subpath.points.windows(2).map(|pts| FlatLine {
        p0: pts[0],
        p1: pts[1],
    })
}

fn boundary_lines(flat_clip: &[FlatSubpath]) -> Vec<FlatLine> {
    flat_clip.iter().flat_map(lines_for_subpath).collect()
}

fn push_event_t(ts: &mut Vec<f64>, line: FlatLine, y: f64, x: f64) {
    let dx = line.p1.x - line.p0.x;
    let dy = line.p1.y - line.p0.y;
    let denom = if dx.abs() >= dy.abs() { dx } else { dy };
    if denom == 0.0 {
        return;
    }
    let t = if dx.abs() >= dy.abs() {
        (x - line.p0.x) / dx
    } else {
        (y - line.p0.y) / dy
    };
    if t.is_finite() && (-1e-9..=1.0 + 1e-9).contains(&t) {
        ts.push(t.clamp(0.0, 1.0));
    }
}

fn collect_split_ts(
    flat_clip: &[FlatSubpath],
    flat_body: &[FlatSubpath],
    eps: f64,
) -> Result<Vec<Vec<Vec<f64>>>, PathOpsErr> {
    let mut segments = Segments::default();
    for subpath in flat_clip {
        let mut points = subpath.points.clone();
        if subpath.closed
            && points.len() > 1
            && close_enough(*points.first().unwrap(), *points.last().unwrap(), eps)
        {
            points.pop();
        }
        if points.len() >= 2 {
            segments.add_closed_polyline(points);
        }
    }

    let mut split_ts: Vec<Vec<Vec<f64>>> = flat_body
        .iter()
        .map(|subpath| lines_for_subpath(subpath).map(|_| vec![0.0, 1.0]).collect())
        .collect();
    let mut saw_body_line = false;
    let mut body_seg_map: HashMap<SegIdx, (usize, usize)> = HashMap::new();

    for (subpath_idx, subpath) in flat_body.iter().enumerate() {
        for (line_idx, line) in lines_for_subpath(subpath).enumerate() {
            if close_enough(line.p0, line.p1, eps) {
                continue;
            }
            let before = segments.len();
            segments.add_points([line.p0, line.p1]);
            if segments.len() == before {
                continue;
            }
            for (pos, idx) in segments.indices().enumerate().skip(before) {
                if pos >= segments.len() {
                    break;
                }
                body_seg_map.insert(idx, (subpath_idx, line_idx));
            }
            saw_body_line = true;
        }
    }

    if !saw_body_line {
        return Ok(split_ts);
    }

    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        sweep::sweep(&segments, eps, |y, ev| {
            if let Some(&(subpath_idx, line_idx)) = body_seg_map.get(&ev.seg_idx) {
                let line = lines_for_subpath(&flat_body[subpath_idx])
                    .nth(line_idx)
                    .expect("body line index should still exist");
                let ts = &mut split_ts[subpath_idx][line_idx];
                push_event_t(ts, line, y, ev.x0);
                push_event_t(ts, line, y, ev.x1);
            }
        });
    }));

    match result {
        Ok(()) => Ok(split_ts),
        Err(_) => Err(PathOpsErr::LinesweeperFailed("linesweeper panicked".into())),
    }
}

fn normalize_ts(ts: &mut Vec<f64>, line: FlatLine, eps: f64) {
    ts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let length = (line.p1 - line.p0).hypot();
    let tol = if length > 0.0 {
        (eps / length).max(1e-10)
    } else {
        1e-10
    };
    ts.dedup_by(|a, b| (*a - *b).abs() <= tol);
}

fn classify_interval(
    clip_bez: &BezPath,
    boundary: &[FlatLine],
    fill_rule: FillRule,
    mode: ClipMode,
    p: Point,
    boundary_tol: f64,
) -> bool {
    let on_boundary = point_on_boundary(p, boundary, boundary_tol);
    let inside = winding_inside(clip_bez.winding(p), fill_rule);
    match mode {
        ClipMode::Include => inside || on_boundary,
        ClipMode::Exclude => !inside && !on_boundary,
    }
}

fn push_chunk(chunks: &mut Vec<Vec<Point>>, start: Point, end: Point, tol: f64) {
    if close_enough(start, end, tol) {
        return;
    }
    if let Some(chunk) = chunks.last_mut() {
        if chunk
            .last()
            .is_some_and(|last| close_enough(*last, start, tol))
        {
            if chunk
                .last()
                .is_none_or(|last| !close_enough(*last, end, tol))
            {
                chunk.push(end);
            }
            return;
        }
    }
    chunks.push(vec![start, end]);
}

fn merge_closed_wraparound(chunks: &mut Vec<Vec<Point>>, tol: f64) {
    if chunks.len() < 2 {
        return;
    }
    let first_starts_at = chunks.first().and_then(|c| c.first()).copied();
    let last_ends_at = chunks.last().and_then(|c| c.last()).copied();
    if let (Some(first_start), Some(last_end)) = (first_starts_at, last_ends_at) {
        if close_enough(first_start, last_end, tol) {
            let first = chunks.remove(0);
            let mut last = chunks.pop().unwrap();
            last.extend(first.into_iter().skip(1));
            chunks.insert(0, last);
        }
    }
}

fn chunks_to_wire(
    mut chunks: Vec<Vec<Point>>,
    original_closed: bool,
    tol: f64,
) -> Vec<WireSubpath> {
    if original_closed {
        merge_closed_wraparound(&mut chunks, tol);
    }

    chunks
        .into_iter()
        .filter_map(|mut chunk| {
            if chunk.len() < 2 {
                return None;
            }
            let closed = original_closed
                && chunk.len() > 2
                && close_enough(*chunk.first().unwrap(), *chunk.last().unwrap(), tol);
            if closed {
                chunk.pop();
            }
            if chunk.len() < 2 {
                return None;
            }
            let origin = chunk[0];
            let segments = chunk
                .iter()
                .skip(1)
                .map(|p| WireSegment::Line { to: [p.x, p.y] })
                .collect();
            Some(WireSubpath {
                origin: [origin.x, origin.y],
                closed,
                segments,
            })
        })
        .collect()
}

pub(crate) fn clip_line_path(
    clip_region: &WirePath,
    body: &WirePath,
    fill_rule: FillRule,
    mode: ClipMode,
    eps: f64,
) -> Result<WirePath, PathOpsErr> {
    if clip_region.is_empty() {
        return Ok(match mode {
            ClipMode::Include => WirePath::empty(),
            ClipMode::Exclude => body.clone(),
        });
    }

    let clip_bez = wire_to_closed_bez(clip_region)?;
    let flatten_tol = eps.max(1e-5);
    let point_tol = eps.max(1e-9);
    let boundary_tol = (eps.max(flatten_tol)) * 8.0;
    let flat_clip = flatten_wire(clip_region, flatten_tol, point_tol);
    if flat_clip.is_empty() {
        return Ok(match mode {
            ClipMode::Include => WirePath::empty(),
            ClipMode::Exclude => body.clone(),
        });
    }
    let flat_body = flatten_wire(body, flatten_tol, point_tol);
    if flat_body.is_empty() {
        return Ok(WirePath::empty());
    }

    let boundary = boundary_lines(&flat_clip);
    let mut split_ts = collect_split_ts(&flat_clip, &flat_body, eps)?;
    let mut output = WirePath::empty();

    for (subpath_idx, subpath) in flat_body.iter().enumerate() {
        let lines: Vec<_> = lines_for_subpath(subpath).collect();
        let mut chunks: Vec<Vec<Point>> = Vec::new();
        for (line_idx, line) in lines.iter().copied().enumerate() {
            let ts = &mut split_ts[subpath_idx][line_idx];
            normalize_ts(ts, line, eps);
            for pair in ts.windows(2) {
                let t0 = pair[0];
                let t1 = pair[1];
                if t1 <= t0 {
                    continue;
                }
                let mid = lerp(line.p0, line.p1, (t0 + t1) * 0.5);
                if classify_interval(&clip_bez, &boundary, fill_rule, mode, mid, boundary_tol) {
                    push_chunk(
                        &mut chunks,
                        lerp(line.p0, line.p1, t0),
                        lerp(line.p0, line.p1, t1),
                        point_tol,
                    );
                }
            }
        }
        output
            .subpaths
            .extend(chunks_to_wire(chunks, subpath.closed, point_tol));
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect_wire(min: (f64, f64), max: (f64, f64)) -> WirePath {
        WirePath {
            subpaths: vec![WireSubpath {
                origin: [min.0, min.1],
                closed: true,
                segments: vec![
                    WireSegment::Line { to: [max.0, min.1] },
                    WireSegment::Line { to: [max.0, max.1] },
                    WireSegment::Line { to: [min.0, max.1] },
                ],
            }],
        }
    }

    fn line_wire(a: (f64, f64), b: (f64, f64)) -> WirePath {
        WirePath {
            subpaths: vec![WireSubpath {
                origin: [a.0, a.1],
                closed: false,
                segments: vec![WireSegment::Line { to: [b.0, b.1] }],
            }],
        }
    }

    #[test]
    fn include_open_line_through_rect() {
        let out = clip_line_path(
            &rect_wire((0.0, 0.0), (1.0, 1.0)),
            &line_wire((-1.0, 0.5), (2.0, 0.5)),
            FillRule::NonZero,
            ClipMode::Include,
            1e-6,
        )
        .unwrap();
        assert_eq!(out.subpaths.len(), 1);
        assert_eq!(out.subpaths[0].origin, [0.0, 0.5]);
        assert_eq!(
            out.subpaths[0].segments,
            vec![WireSegment::Line { to: [1.0, 0.5] }]
        );
    }

    #[test]
    fn exclude_open_line_through_rect() {
        let out = clip_line_path(
            &rect_wire((0.0, 0.0), (1.0, 1.0)),
            &line_wire((-1.0, 0.5), (2.0, 0.5)),
            FillRule::NonZero,
            ClipMode::Exclude,
            1e-6,
        )
        .unwrap();
        assert_eq!(out.subpaths.len(), 2);
        assert_eq!(out.subpaths[0].origin, [-1.0, 0.5]);
        assert_eq!(out.subpaths[1].origin, [1.0, 0.5]);
    }

    #[test]
    fn boundary_line_include_keeps_exclude_drops() {
        let clip = rect_wire((0.0, 0.0), (1.0, 1.0));
        let body = line_wire((0.0, 0.0), (1.0, 0.0));
        let include =
            clip_line_path(&clip, &body, FillRule::NonZero, ClipMode::Include, 1e-6).unwrap();
        let exclude =
            clip_line_path(&clip, &body, FillRule::NonZero, ClipMode::Exclude, 1e-6).unwrap();
        assert_eq!(include.subpaths.len(), 1);
        assert!(exclude.subpaths.is_empty());
    }

    #[test]
    fn clipped_closed_outline_becomes_open_piece() {
        let out = clip_line_path(
            &rect_wire((0.0, 0.0), (1.0, 1.0)),
            &rect_wire((-0.5, 0.25), (0.5, 0.75)),
            FillRule::NonZero,
            ClipMode::Include,
            1e-6,
        )
        .unwrap();
        assert_eq!(out.subpaths.len(), 1);
        assert!(!out.subpaths[0].closed);
        assert_eq!(out.subpaths[0].origin, [0.0, 0.25]);
    }

    #[test]
    fn empty_clip_exclude_preserves_original_body() {
        let body = line_wire((0.0, 0.0), (1.0, 0.0));
        let out = clip_line_path(
            &WirePath::empty(),
            &body,
            FillRule::NonZero,
            ClipMode::Exclude,
            1e-6,
        )
        .unwrap();
        assert_eq!(out, body);
    }
}
