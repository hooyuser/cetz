use crate::path_bool::wire::{PathBoolArgs, PathBoolOutput};
use crate::path_ops::boolean::{boolean_wire_paths, parse_bool_op};
use crate::path_ops::error::PathOpsErr;
use crate::path_ops::fill::parse_fill_rule;

pub fn path_bool(args: PathBoolArgs) -> Result<PathBoolOutput, PathOpsErr> {
    let op = parse_bool_op(&args.op)?;
    let fill_rule_a = parse_fill_rule(&args.fill_rule_a)?;
    let fill_rule_b = parse_fill_rule(&args.fill_rule_b)?;

    let path = boolean_wire_paths(&args.a, &args.b, op, fill_rule_a, fill_rule_b, args.eps)?;

    Ok(PathBoolOutput { path })
}
