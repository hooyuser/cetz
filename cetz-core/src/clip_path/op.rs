use crate::clip_path::line_clip::clip_line_path;
use crate::clip_path::wire::{ClipPathArgs, ClipPathOutput};
use crate::clip_path::ClipMode;
use crate::path_ops::boolean::{boolean_wire_paths, BoolOp};
use crate::path_ops::convert::{closed_subpaths, wire_to_bez_any, wire_to_closed_bez};
use crate::path_ops::eps::auto_eps;
use crate::path_ops::error::PathOpsErr;
use crate::path_ops::fill::parse_fill_rule;
use crate::path_ops::wire::WirePath;

fn parse_mode(mode: &str) -> Result<ClipMode, PathOpsErr> {
    match mode {
        "include" => Ok(ClipMode::Include),
        "exclude" => Ok(ClipMode::Exclude),
        _ => Err(PathOpsErr::InvalidMode(mode.to_string())),
    }
}

fn area_op_for_mode(mode: ClipMode) -> BoolOp {
    match mode {
        ClipMode::Include => BoolOp::Intersection,
        ClipMode::Exclude => BoolOp::Difference,
    }
}

fn area_clip(
    clip_region: &WirePath,
    body: &WirePath,
    mode: ClipMode,
    clip_fill_rule: linesweeper::FillRule,
    body_fill_rule: linesweeper::FillRule,
    eps: f64,
) -> Result<WirePath, PathOpsErr> {
    let body_closed = closed_subpaths(body);
    if body_closed.is_empty() {
        return Ok(WirePath::empty());
    }
    if clip_region.is_empty() {
        return Ok(match mode {
            ClipMode::Include => WirePath::empty(),
            ClipMode::Exclude => body_closed,
        });
    }
    boolean_wire_paths(
        &body_closed,
        clip_region,
        area_op_for_mode(mode),
        body_fill_rule,
        clip_fill_rule,
        Some(eps),
    )
}

pub fn clip_path(args: ClipPathArgs) -> Result<ClipPathOutput, PathOpsErr> {
    let mode = parse_mode(&args.mode)?;
    let clip_fill_rule = parse_fill_rule(&args.clip_fill_rule)?;
    let body_fill_rule = parse_fill_rule(&args.body_fill_rule)?;

    // Validate clip-region closedness up front, even if only line clipping is requested.
    let clip_bez = wire_to_closed_bez(&args.clip_region)?;
    let body_bez = wire_to_bez_any(&args.body)?;
    let eps = match args.eps {
        Some(eps) => eps,
        None => auto_eps(&[&clip_bez, &body_bez])?,
    };

    let area_path = if args.need_area {
        Some(area_clip(
            &args.clip_region,
            &args.body,
            mode,
            clip_fill_rule,
            body_fill_rule,
            eps,
        )?)
    } else {
        None
    };

    let line_path = if args.need_line {
        Some(clip_line_path(
            &args.clip_region,
            &args.body,
            clip_fill_rule,
            mode,
            eps,
        )?)
    } else {
        None
    };

    Ok(ClipPathOutput {
        line_path,
        area_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path_ops::wire::{WireSegment, WireSubpath};

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

    #[test]
    fn area_include_matches_boolean_intersection() {
        let clip = rect_wire((0.0, 0.0), (1.0, 1.0));
        let body = rect_wire((-0.5, -0.5), (0.5, 0.5));
        let area = area_clip(
            &clip,
            &body,
            ClipMode::Include,
            linesweeper::FillRule::NonZero,
            linesweeper::FillRule::NonZero,
            1e-6,
        )
        .unwrap();
        let expected = boolean_wire_paths(
            &body,
            &clip,
            BoolOp::Intersection,
            linesweeper::FillRule::NonZero,
            linesweeper::FillRule::NonZero,
            Some(1e-6),
        )
        .unwrap();
        assert_eq!(area, expected);
    }

    #[test]
    fn area_exclude_matches_boolean_difference() {
        let clip = rect_wire((0.0, 0.0), (1.0, 1.0));
        let body = rect_wire((-0.5, -0.5), (0.5, 0.5));
        let area = area_clip(
            &clip,
            &body,
            ClipMode::Exclude,
            linesweeper::FillRule::NonZero,
            linesweeper::FillRule::NonZero,
            1e-6,
        )
        .unwrap();
        let expected = boolean_wire_paths(
            &body,
            &clip,
            BoolOp::Difference,
            linesweeper::FillRule::NonZero,
            linesweeper::FillRule::NonZero,
            Some(1e-6),
        )
        .unwrap();
        assert_eq!(area, expected);
    }

    #[test]
    fn invalid_mode_is_rejected() {
        assert!(matches!(
            parse_mode("inside"),
            Err(PathOpsErr::InvalidMode(_))
        ));
    }

    #[test]
    fn open_clip_region_is_rejected() {
        let mut clip = rect_wire((0.0, 0.0), (1.0, 1.0));
        clip.subpaths[0].closed = false;
        let args = ClipPathArgs {
            clip_region: clip,
            body: rect_wire((0.0, 0.0), (1.0, 1.0)),
            mode: "include".into(),
            clip_fill_rule: "non-zero".into(),
            body_fill_rule: "non-zero".into(),
            eps: Some(1e-6),
            need_line: true,
            need_area: true,
        };
        assert!(matches!(clip_path(args), Err(PathOpsErr::OpenSubpath)));
    }
}
