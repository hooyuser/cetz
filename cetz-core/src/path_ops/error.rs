//! Error type for path operations.

use std::fmt;

#[derive(Debug)]
pub enum PathOpsErr {
    InvalidOp(String),
    InvalidMode(String),
    InvalidFillRule(String),
    EmptyBatch,
    OpenSubpath,
    /// MalformedPath refers to a path element that appeared without a preceding `MoveTo`.
    MalformedPath,
    /// Wraps any failure (or panic) from inside `linesweeper`.
    LinesweeperFailed(String),
}

impl fmt::Display for PathOpsErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PathOpsErr::InvalidOp(op) => write!(f, "invalid boolean op: {op:?}"),
            PathOpsErr::InvalidMode(mode) => write!(f, "invalid clip mode: {mode:?}"),
            PathOpsErr::InvalidFillRule(rule) => write!(f, "invalid fill-rule: {rule:?}"),
            PathOpsErr::EmptyBatch => write!(f, "path operation wasm: batch cannot be empty"),
            PathOpsErr::OpenSubpath => {
                write!(f, "path operation wasm: every subpath should be closed")
            }
            PathOpsErr::MalformedPath => {
                write!(f, "path operation wasm: found a malformed path which has a segment without preceding move-to")
            }
            PathOpsErr::LinesweeperFailed(msg) => {
                write!(f, "path operation wasm: linesweeper failed: {msg}")
            }
        }
    }
}

impl std::error::Error for PathOpsErr {}
