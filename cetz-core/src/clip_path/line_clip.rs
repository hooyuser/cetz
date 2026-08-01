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
    original_idx: usize,
    closed: bool,
    points: Vec<Point>,
}

#[derive(Debug, Clone, Copy)]
struct FlatLine {
    p0: Point,
    p1: Point,
}

#[derive(Debug, Clone, Copy)]
struct LineBounds {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

#[derive(Debug, Clone, Copy)]
struct BoundaryEntry {
    line: FlatLine,
    bounds: LineBounds,
}

#[derive(Debug, Clone)]
struct BoundaryIndex {
    entries: Vec<BoundaryEntry>,
}

impl LineBounds {
    fn from_line(line: FlatLine) -> Self {
        Self {
            min_x: line.p0.x.min(line.p1.x),
            min_y: line.p0.y.min(line.p1.y),
            max_x: line.p0.x.max(line.p1.x),
            max_y: line.p0.y.max(line.p1.y),
        }
    }

    fn from_point(p: Point, tol: f64) -> Self {
        Self {
            min_x: p.x - tol,
            min_y: p.y - tol,
            max_x: p.x + tol,
            max_y: p.y + tol,
        }
    }

    fn expand(self, tol: f64) -> Self {
        Self {
            min_x: self.min_x - tol,
            min_y: self.min_y - tol,
            max_x: self.max_x + tol,
            max_y: self.max_y + tol,
        }
    }

    fn union(self, other: Self) -> Self {
        Self {
            min_x: self.min_x.min(other.min_x),
            min_y: self.min_y.min(other.min_y),
            max_x: self.max_x.max(other.max_x),
            max_y: self.max_y.max(other.max_y),
        }
    }

    fn overlaps(self, other: Self) -> bool {
        self.min_x <= other.max_x
            && self.max_x >= other.min_x
            && self.min_y <= other.max_y
            && self.max_y >= other.min_y
    }
}

impl BoundaryIndex {
    fn new(lines: Vec<FlatLine>) -> Self {
        let mut entries: Vec<_> = lines
            .into_iter()
            .map(|line| BoundaryEntry {
                line,
                bounds: LineBounds::from_line(line),
            })
            .collect();
        entries.sort_by(|a, b| a.bounds.min_y.partial_cmp(&b.bounds.min_y).unwrap());
        Self { entries }
    }

    fn any_overlapping(&self, bounds: LineBounds) -> bool {
        for entry in &self.entries {
            if entry.bounds.min_y > bounds.max_y {
                break;
            }
            if entry.bounds.overlaps(bounds) {
                return true;
            }
        }
        false
    }

    fn line_may_touch_boundary(&self, line: FlatLine, tol: f64) -> bool {
        self.any_overlapping(LineBounds::from_line(line).expand(tol))
    }

    fn point_on_boundary(&self, p: Point, tol: f64) -> bool {
        let tol_sq = tol * tol;
        let bounds = LineBounds::from_point(p, tol);
        for entry in &self.entries {
            if entry.bounds.min_y > bounds.max_y {
                break;
            }
            if entry.bounds.overlaps(bounds)
                && point_to_segment_distance_sq(p, entry.line.p0, entry.line.p1) <= tol_sq
            {
                return true;
            }
        }
        false
    }
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
        .enumerate()
        .filter_map(|(original_idx, subpath)| {
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
                original_idx,
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

fn add_clip_runs(segments: &mut Segments, flat_clip: &[FlatSubpath], bounds: LineBounds, eps: f64) {
    for subpath in flat_clip {
        let lines: Vec<_> = lines_for_subpath(subpath).collect();
        let mut run_start: Option<usize> = None;
        for (line_idx, line) in lines.iter().copied().enumerate() {
            let overlaps = !close_enough(line.p0, line.p1, eps)
                && LineBounds::from_line(line).expand(eps).overlaps(bounds);
            match (run_start, overlaps) {
                (None, true) => run_start = Some(line_idx),
                (Some(start), false) => {
                    add_open_line_run(segments, &lines, start, line_idx);
                    run_start = None;
                }
                _ => {}
            }
        }
        if let Some(start) = run_start {
            add_open_line_run(segments, &lines, start, lines.len());
        }
    }
}

fn add_open_line_run(segments: &mut Segments, lines: &[FlatLine], start: usize, end: usize) {
    if start >= end {
        return;
    }
    let mut points = Vec::with_capacity(end - start + 1);
    points.push(lines[start].p0);
    points.extend(lines[start..end].iter().map(|line| line.p1));
    segments.add_points(points);
}

fn collect_split_ts(
    flat_clip: &[FlatSubpath],
    body_lines: &[Vec<FlatLine>],
    touch_candidates: &[Vec<bool>],
    boundary_tol: f64,
    eps: f64,
) -> Result<Vec<Vec<Vec<f64>>>, PathOpsErr> {
    let mut split_ts: Vec<Vec<Vec<f64>>> = body_lines
        .iter()
        .map(|lines| lines.iter().map(|_| vec![0.0, 1.0]).collect())
        .collect();

    let mut candidate_bounds: Option<LineBounds> = None;
    for (lines, candidates) in body_lines.iter().zip(touch_candidates) {
        for (line, candidate) in lines.iter().zip(candidates) {
            if !candidate {
                continue;
            }
            let bounds = LineBounds::from_line(*line).expand(boundary_tol);
            candidate_bounds = Some(match candidate_bounds {
                Some(acc) => acc.union(bounds),
                None => bounds,
            });
        }
    }

    let Some(candidate_bounds) = candidate_bounds else {
        return Ok(split_ts);
    };

    let mut segments = Segments::default();
    add_clip_runs(&mut segments, flat_clip, candidate_bounds, eps);

    let mut body_seg_map: HashMap<SegIdx, (usize, usize)> = HashMap::new();

    for (subpath_idx, (lines, candidates)) in body_lines.iter().zip(touch_candidates).enumerate() {
        let mut run_start: Option<usize> = None;
        for (line_idx, candidate) in candidates.iter().copied().enumerate() {
            match (run_start, candidate) {
                (None, true) => run_start = Some(line_idx),
                (Some(start), false) => {
                    add_body_run(
                        &mut segments,
                        &mut body_seg_map,
                        lines,
                        subpath_idx,
                        start,
                        line_idx,
                    );
                    run_start = None;
                }
                _ => {}
            }
        }
        if let Some(start) = run_start {
            add_body_run(
                &mut segments,
                &mut body_seg_map,
                lines,
                subpath_idx,
                start,
                lines.len(),
            );
        }
    }

    if body_seg_map.is_empty() {
        return Ok(split_ts);
    }

    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        sweep::sweep(&segments, eps, |y, ev| {
            if let Some(&(subpath_idx, line_idx)) = body_seg_map.get(&ev.seg_idx) {
                let line = body_lines[subpath_idx][line_idx];
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

fn body_touch_candidates(
    body_lines: &[Vec<FlatLine>],
    boundary: &BoundaryIndex,
    boundary_tol: f64,
    eps: f64,
) -> Vec<Vec<bool>> {
    body_lines
        .iter()
        .map(|lines| {
            lines
                .iter()
                .map(|line| {
                    !close_enough(line.p0, line.p1, eps)
                        && boundary.line_may_touch_boundary(*line, boundary_tol)
                })
                .collect()
        })
        .collect()
}

fn add_body_run(
    segments: &mut Segments,
    body_seg_map: &mut HashMap<SegIdx, (usize, usize)>,
    lines: &[FlatLine],
    subpath_idx: usize,
    start: usize,
    end: usize,
) {
    if start >= end {
        return;
    }
    let before = segments.len();
    add_open_line_run(segments, lines, start, end);
    for (offset, idx) in segments
        .indices()
        .enumerate()
        .skip(before)
        .take(end - start)
        .map(|(pos, idx)| (pos - before, idx))
    {
        body_seg_map.insert(idx, (subpath_idx, start + offset));
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
    boundary: &BoundaryIndex,
    fill_rule: FillRule,
    mode: ClipMode,
    p: Point,
    boundary_tol: f64,
) -> bool {
    let on_boundary = boundary.point_on_boundary(p, boundary_tol);
    if on_boundary {
        return mode == ClipMode::Include;
    }
    let inside = winding_inside(clip_bez.winding(p), fill_rule);
    match mode {
        ClipMode::Include => inside,
        ClipMode::Exclude => !inside,
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

pub(crate) struct PreparedLineClip<'a> {
    clip_bez: &'a BezPath,
    flat_clip: Vec<FlatSubpath>,
    boundary: BoundaryIndex,
    fill_rule: FillRule,
    mode: ClipMode,
    eps: f64,
    flatten_tol: f64,
    point_tol: f64,
    boundary_tol: f64,
    empty_clip: bool,
}

impl<'a> PreparedLineClip<'a> {
    pub(crate) fn new(
        clip_region: &WirePath,
        clip_bez: &'a BezPath,
        fill_rule: FillRule,
        mode: ClipMode,
        eps: f64,
    ) -> Result<Self, PathOpsErr> {
        let flatten_tol = eps.max(1e-5);
        let point_tol = eps.max(1e-9);
        let boundary_tol = (eps.max(flatten_tol)) * 8.0;
        let flat_clip = if clip_region.is_empty() {
            Vec::new()
        } else {
            flatten_wire(clip_region, flatten_tol, point_tol)
        };
        let empty_clip = clip_region.is_empty() || flat_clip.is_empty();
        let boundary = BoundaryIndex::new(boundary_lines(&flat_clip));

        Ok(Self {
            clip_bez,
            flat_clip,
            boundary,
            fill_rule,
            mode,
            eps,
            flatten_tol,
            point_tol,
            boundary_tol,
            empty_clip,
        })
    }

    pub(crate) fn clip_body(&self, body: &WirePath) -> Result<WirePath, PathOpsErr> {
        if self.empty_clip {
            return Ok(match self.mode {
                ClipMode::Include => WirePath::empty(),
                ClipMode::Exclude => body.clone(),
            });
        }

        let flat_body = flatten_wire(body, self.flatten_tol, self.point_tol);
        if flat_body.is_empty() {
            return Ok(WirePath::empty());
        }

        let body_lines: Vec<Vec<FlatLine>> = flat_body
            .iter()
            .map(|sp| lines_for_subpath(sp).collect())
            .collect();
        let touch_candidates =
            body_touch_candidates(&body_lines, &self.boundary, self.boundary_tol, self.eps);
        let mut split_ts = collect_split_ts(
            &self.flat_clip,
            &body_lines,
            &touch_candidates,
            self.boundary_tol,
            self.eps,
        )?;
        let mut output = WirePath::empty();

        for (subpath_idx, subpath) in flat_body.iter().enumerate() {
            let lines = &body_lines[subpath_idx];
            let may_touch_boundary = touch_candidates[subpath_idx]
                .iter()
                .any(|candidate| *candidate);
            if !may_touch_boundary {
                let p = lerp(lines[0].p0, lines[0].p1, 0.5);
                if classify_interval(
                    self.clip_bez,
                    &self.boundary,
                    self.fill_rule,
                    self.mode,
                    p,
                    self.boundary_tol,
                ) {
                    output
                        .subpaths
                        .push(body.subpaths[subpath.original_idx].clone());
                }
                continue;
            }

            let mut chunks: Vec<Vec<Point>> = Vec::new();
            let mut all_kept = true;
            let mut any_kept = false;
            for (line_idx, line) in lines.iter().copied().enumerate() {
                let ts = &mut split_ts[subpath_idx][line_idx];
                normalize_ts(ts, line, self.eps);
                for pair in ts.windows(2) {
                    let t0 = pair[0];
                    let t1 = pair[1];
                    if t1 <= t0 {
                        continue;
                    }
                    let mid = lerp(line.p0, line.p1, (t0 + t1) * 0.5);
                    if classify_interval(
                        self.clip_bez,
                        &self.boundary,
                        self.fill_rule,
                        self.mode,
                        mid,
                        self.boundary_tol,
                    ) {
                        any_kept = true;
                        push_chunk(
                            &mut chunks,
                            lerp(line.p0, line.p1, t0),
                            lerp(line.p0, line.p1, t1),
                            self.point_tol,
                        );
                    } else {
                        all_kept = false;
                    }
                }
            }
            if all_kept && any_kept {
                output
                    .subpaths
                    .push(body.subpaths[subpath.original_idx].clone());
            } else if any_kept {
                output
                    .subpaths
                    .extend(chunks_to_wire(chunks, subpath.closed, self.point_tol));
            }
        }

        Ok(output)
    }
}

#[allow(dead_code)]
pub(crate) fn clip_line_path(
    clip_region: &WirePath,
    body: &WirePath,
    fill_rule: FillRule,
    mode: ClipMode,
    eps: f64,
) -> Result<WirePath, PathOpsErr> {
    let clip_bez = wire_to_closed_bez(clip_region)?;
    PreparedLineClip::new(clip_region, &clip_bez, fill_rule, mode, eps)?.clip_body(body)
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

    fn cubic_wire(start: (f64, f64), c1: (f64, f64), c2: (f64, f64), end: (f64, f64)) -> WirePath {
        WirePath {
            subpaths: vec![WireSubpath {
                origin: [start.0, start.1],
                closed: false,
                segments: vec![WireSegment::Cubic {
                    c1: [c1.0, c1.1],
                    c2: [c2.0, c2.1],
                    to: [end.0, end.1],
                }],
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
    fn include_curve_inside_rect_preserves_original_curve() {
        let body = cubic_wire((0.2, 0.2), (0.3, 0.9), (0.7, 0.1), (0.8, 0.8));
        let out = clip_line_path(
            &rect_wire((0.0, 0.0), (1.0, 1.0)),
            &body,
            FillRule::NonZero,
            ClipMode::Include,
            1e-6,
        )
        .unwrap();
        assert_eq!(out, body);
    }

    #[test]
    fn exclude_curve_outside_rect_preserves_original_curve() {
        let body = cubic_wire((2.0, 0.2), (2.3, 0.9), (2.7, 0.1), (2.8, 0.8));
        let out = clip_line_path(
            &rect_wire((0.0, 0.0), (1.0, 1.0)),
            &body,
            FillRule::NonZero,
            ClipMode::Exclude,
            1e-6,
        )
        .unwrap();
        assert_eq!(out, body);
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
