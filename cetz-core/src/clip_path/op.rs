use kurbo::BezPath;

use crate::clip_path::line_clip::PreparedLineClip;
use crate::clip_path::wire::{
    ClipPathArgs, ClipPathBatchArgs, ClipPathBatchBody, ClipPathBatchOutput, ClipPathOutput,
};
use crate::clip_path::ClipMode;
use crate::path_ops::boolean::{boolean_wire_path_with_clip_bez, BoolOp};
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
    clip_bez: &BezPath,
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
    boolean_wire_path_with_clip_bez(
        &body_closed,
        clip_bez,
        area_op_for_mode(mode),
        body_fill_rule,
        clip_fill_rule,
        eps,
    )
}

pub fn clip_path(args: ClipPathArgs) -> Result<ClipPathOutput, PathOpsErr> {
    let batch = clip_path_batch(ClipPathBatchArgs {
        clip_region: args.clip_region,
        bodies: vec![ClipPathBatchBody {
            body: args.body,
            body_fill_rule: args.body_fill_rule,
            need_line: args.need_line,
            need_area: args.need_area,
        }],
        mode: args.mode,
        clip_fill_rule: args.clip_fill_rule,
        eps: args.eps,
    })?;

    batch
        .outputs
        .into_iter()
        .next()
        .ok_or(PathOpsErr::EmptyBatch)
}

pub fn clip_path_batch(args: ClipPathBatchArgs) -> Result<ClipPathBatchOutput, PathOpsErr> {
    if args.bodies.is_empty() {
        return Err(PathOpsErr::EmptyBatch);
    }

    let mode = parse_mode(&args.mode)?;
    let clip_fill_rule = parse_fill_rule(&args.clip_fill_rule)?;
    let body_fill_rules = args
        .bodies
        .iter()
        .map(|body| parse_fill_rule(&body.body_fill_rule))
        .collect::<Result<Vec<_>, _>>()?;

    // Validate and convert inputs up front so batch-wide `eps: auto` is
    // deterministic and every body observes the same tolerance.
    let clip_bez = wire_to_closed_bez(&args.clip_region)?;
    let body_bezs = args
        .bodies
        .iter()
        .map(|body| wire_to_bez_any(&body.body))
        .collect::<Result<Vec<_>, _>>()?;
    let eps = match args.eps {
        Some(eps) => eps,
        None => {
            let mut eps_paths = Vec::with_capacity(body_bezs.len() + 1);
            eps_paths.push(&clip_bez);
            eps_paths.extend(body_bezs.iter());
            auto_eps(&eps_paths)?
        }
    };

    let needs_line = args.bodies.iter().any(|body| body.need_line);
    let prepared_line = if needs_line {
        Some(PreparedLineClip::new(
            &args.clip_region,
            &clip_bez,
            clip_fill_rule,
            mode,
            eps,
        )?)
    } else {
        None
    };

    let mut outputs = Vec::with_capacity(args.bodies.len());
    for (body, body_fill_rule) in args.bodies.iter().zip(body_fill_rules) {
        let area_path = if body.need_area {
            Some(area_clip(
                &args.clip_region,
                &clip_bez,
                &body.body,
                mode,
                clip_fill_rule,
                body_fill_rule,
                eps,
            )?)
        } else {
            None
        };

        let line_path = if body.need_line {
            Some(
                prepared_line
                    .as_ref()
                    .expect("prepared line clip should exist when any body needs line clipping")
                    .clip_body(&body.body)?,
            )
        } else {
            None
        };

        outputs.push(ClipPathOutput {
            line_path,
            area_path,
        });
    }

    Ok(ClipPathBatchOutput { outputs })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path_ops::boolean::boolean_wire_paths;
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

    fn line_wire(a: (f64, f64), b: (f64, f64)) -> WirePath {
        WirePath {
            subpaths: vec![WireSubpath {
                origin: [a.0, a.1],
                closed: false,
                segments: vec![WireSegment::Line { to: [b.0, b.1] }],
            }],
        }
    }

    fn clip_args(
        clip_region: WirePath,
        body: WirePath,
        mode: &str,
        eps: Option<f64>,
        need_line: bool,
        need_area: bool,
    ) -> ClipPathArgs {
        ClipPathArgs {
            clip_region,
            body,
            mode: mode.into(),
            clip_fill_rule: "non-zero".into(),
            body_fill_rule: "non-zero".into(),
            eps,
            need_line,
            need_area,
        }
    }

    fn batch_body(body: WirePath, need_line: bool, need_area: bool) -> ClipPathBatchBody {
        ClipPathBatchBody {
            body,
            body_fill_rule: "non-zero".into(),
            need_line,
            need_area,
        }
    }

    #[test]
    fn area_include_matches_boolean_intersection() {
        let clip = rect_wire((0.0, 0.0), (1.0, 1.0));
        let clip_bez = wire_to_closed_bez(&clip).unwrap();
        let body = rect_wire((-0.5, -0.5), (0.5, 0.5));
        let area = area_clip(
            &clip,
            &clip_bez,
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
        let clip_bez = wire_to_closed_bez(&clip).unwrap();
        let body = rect_wire((-0.5, -0.5), (0.5, 0.5));
        let area = area_clip(
            &clip,
            &clip_bez,
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
    fn batch_single_body_matches_single_api() {
        let clip = rect_wire((0.0, 0.0), (1.0, 1.0));
        let body = line_wire((-1.0, 0.5), (2.0, 0.5));
        let single = clip_path(clip_args(
            clip.clone(),
            body.clone(),
            "include",
            Some(1e-6),
            true,
            false,
        ))
        .unwrap();
        let batch = clip_path_batch(ClipPathBatchArgs {
            clip_region: clip,
            bodies: vec![batch_body(body, true, false)],
            mode: "include".into(),
            clip_fill_rule: "non-zero".into(),
            eps: Some(1e-6),
        })
        .unwrap();
        assert_eq!(batch.outputs, vec![single]);
    }

    #[test]
    fn batch_lines_match_individual_calls_with_explicit_eps() {
        let clip = rect_wire((0.0, 0.0), (1.0, 1.0));
        let bodies = vec![
            line_wire((-1.0, 0.5), (2.0, 0.5)),
            line_wire((0.0, 0.0), (1.0, 0.0)),
            line_wire((-1.0, 1.5), (2.0, 1.5)),
        ];
        let batch = clip_path_batch(ClipPathBatchArgs {
            clip_region: clip.clone(),
            bodies: bodies
                .iter()
                .cloned()
                .map(|body| batch_body(body, true, false))
                .collect(),
            mode: "exclude".into(),
            clip_fill_rule: "non-zero".into(),
            eps: Some(1e-6),
        })
        .unwrap();
        let expected = bodies
            .into_iter()
            .map(|body| {
                clip_path(clip_args(
                    clip.clone(),
                    body,
                    "exclude",
                    Some(1e-6),
                    true,
                    false,
                ))
                .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(batch.outputs, expected);
    }

    #[test]
    fn batch_areas_match_individual_calls_with_explicit_eps() {
        let clip = rect_wire((0.0, 0.0), (1.0, 1.0));
        let bodies = vec![
            rect_wire((-0.5, -0.5), (0.5, 0.5)),
            rect_wire((0.25, 0.25), (1.25, 1.25)),
        ];
        let batch = clip_path_batch(ClipPathBatchArgs {
            clip_region: clip.clone(),
            bodies: bodies
                .iter()
                .cloned()
                .map(|body| batch_body(body, false, true))
                .collect(),
            mode: "include".into(),
            clip_fill_rule: "non-zero".into(),
            eps: Some(1e-6),
        })
        .unwrap();
        let expected = bodies
            .into_iter()
            .map(|body| {
                clip_path(clip_args(
                    clip.clone(),
                    body,
                    "include",
                    Some(1e-6),
                    false,
                    true,
                ))
                .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(batch.outputs, expected);
    }

    #[test]
    fn batch_auto_eps_matches_individual_calls_with_batch_eps() {
        let clip = rect_wire((0.0, 0.0), (100.0, 100.0));
        let bodies = vec![
            line_wire((-10.0, 25.0), (110.0, 25.0)),
            rect_wire((50.0, 50.0), (120.0, 120.0)),
        ];
        let clip_bez = wire_to_closed_bez(&clip).unwrap();
        let body_bezs = bodies
            .iter()
            .map(wire_to_bez_any)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let mut eps_paths = vec![&clip_bez];
        eps_paths.extend(body_bezs.iter());
        let batch_eps = auto_eps(&eps_paths).unwrap();

        let batch = clip_path_batch(ClipPathBatchArgs {
            clip_region: clip.clone(),
            bodies: vec![
                batch_body(bodies[0].clone(), true, false),
                batch_body(bodies[1].clone(), false, true),
            ],
            mode: "include".into(),
            clip_fill_rule: "non-zero".into(),
            eps: None,
        })
        .unwrap();
        let expected = vec![
            clip_path(clip_args(
                clip.clone(),
                bodies[0].clone(),
                "include",
                Some(batch_eps),
                true,
                false,
            ))
            .unwrap(),
            clip_path(clip_args(
                clip,
                bodies[1].clone(),
                "include",
                Some(batch_eps),
                false,
                true,
            ))
            .unwrap(),
        ];
        assert_eq!(batch.outputs, expected);
    }

    #[test]
    fn empty_batch_is_rejected() {
        let args = ClipPathBatchArgs {
            clip_region: rect_wire((0.0, 0.0), (1.0, 1.0)),
            bodies: Vec::new(),
            mode: "include".into(),
            clip_fill_rule: "non-zero".into(),
            eps: Some(1e-6),
        };
        assert!(matches!(clip_path_batch(args), Err(PathOpsErr::EmptyBatch)));
    }

    #[test]
    fn batch_invalid_fill_rule_is_rejected() {
        let args = ClipPathBatchArgs {
            clip_region: rect_wire((0.0, 0.0), (1.0, 1.0)),
            bodies: vec![ClipPathBatchBody {
                body: line_wire((0.0, 0.0), (1.0, 0.0)),
                body_fill_rule: "bad-rule".into(),
                need_line: true,
                need_area: false,
            }],
            mode: "include".into(),
            clip_fill_rule: "non-zero".into(),
            eps: Some(1e-6),
        };
        assert!(matches!(
            clip_path_batch(args),
            Err(PathOpsErr::InvalidFillRule(_))
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
