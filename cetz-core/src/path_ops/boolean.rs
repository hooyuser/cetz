use std::panic::AssertUnwindSafe;

use kurbo::BezPath;
use linesweeper::topology::{BinaryWindingNumber, Topology};
use linesweeper::FillRule;

use crate::path_ops::convert::{bez_to_wire, wire_to_closed_bez};
use crate::path_ops::eps::auto_eps;
use crate::path_ops::error::PathOpsErr;
use crate::path_ops::fill::winding_inside;
use crate::path_ops::wire::WirePath;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BoolOp {
    Union,
    Intersection,
    Difference,
    Xor,
}

pub(crate) fn parse_bool_op(op: &str) -> Result<BoolOp, PathOpsErr> {
    match op {
        "union" => Ok(BoolOp::Union),
        "intersection" => Ok(BoolOp::Intersection),
        "difference" => Ok(BoolOp::Difference),
        "xor" => Ok(BoolOp::Xor),
        _ => Err(PathOpsErr::InvalidOp(op.to_string())),
    }
}

pub(crate) fn boolean_wire_paths(
    a: &WirePath,
    b: &WirePath,
    op: BoolOp,
    fill_rule_a: FillRule,
    fill_rule_b: FillRule,
    eps: Option<f64>,
) -> Result<WirePath, PathOpsErr> {
    let a = wire_to_closed_bez(a)?;
    let b = wire_to_closed_bez(b)?;

    let eps = match eps {
        Some(eps) => eps,
        None => auto_eps(&[&a, &b])?,
    };

    boolean_bez_paths(&a, &b, op, fill_rule_a, fill_rule_b, eps)
}

pub(crate) fn boolean_wire_path_with_clip_bez(
    a: &WirePath,
    clip_bez: &BezPath,
    op: BoolOp,
    fill_rule_a: FillRule,
    fill_rule_clip: FillRule,
    eps: f64,
) -> Result<WirePath, PathOpsErr> {
    let a = wire_to_closed_bez(a)?;
    boolean_bez_paths(&a, clip_bez, op, fill_rule_a, fill_rule_clip, eps)
}

fn boolean_bez_paths(
    a: &BezPath,
    b: &BezPath,
    op: BoolOp,
    fill_rule_a: FillRule,
    fill_rule_b: FillRule,
    eps: f64,
) -> Result<WirePath, PathOpsErr> {
    // catch_unwind so a panic inside linesweeper turns into a recoverable
    // error rather than aborting the WASM module.
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        // We drive `Topology` directly instead of `linesweeper::binary_op` because
        // the latter accepts only a single global `FillRule`; we need one per operand.
        let topology = Topology::from_paths_binary(&a, &b, eps).map_err(|e| e.to_string())?;
        let inside = |w: &BinaryWindingNumber| {
            let ia = winding_inside(w.shape_a, fill_rule_a);
            let ib = winding_inside(w.shape_b, fill_rule_b);
            match op {
                BoolOp::Union => ia || ib,
                BoolOp::Intersection => ia && ib,
                BoolOp::Xor => ia != ib,
                BoolOp::Difference => ia && !ib,
            }
        };
        Ok::<_, String>(topology.contours(inside))
    }));

    let contours = match result {
        Ok(Ok(c)) => c,
        Ok(Err(msg)) => return Err(PathOpsErr::LinesweeperFailed(msg)),
        Err(_) => {
            return Err(PathOpsErr::LinesweeperFailed("linesweeper panicked".into()));
        }
    };

    let mut combined = WirePath::empty();
    for contour in contours.contours() {
        let mut wire = bez_to_wire(&contour.path)?;
        combined.subpaths.append(&mut wire.subpaths);
    }
    Ok(combined)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_op_all_valid() {
        assert_eq!(parse_bool_op("union").unwrap(), BoolOp::Union);
        assert_eq!(parse_bool_op("intersection").unwrap(), BoolOp::Intersection);
        assert_eq!(parse_bool_op("difference").unwrap(), BoolOp::Difference);
        assert_eq!(parse_bool_op("xor").unwrap(), BoolOp::Xor);
    }

    #[test]
    fn parse_op_invalid() {
        assert!(matches!(
            parse_bool_op("subtract"),
            Err(PathOpsErr::InvalidOp(_))
        ));
        assert!(matches!(parse_bool_op(""), Err(PathOpsErr::InvalidOp(_))));
    }
}
