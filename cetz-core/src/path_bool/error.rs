//! Error type for path boolean operations.

use std::fmt;

#[derive(Debug)]
pub enum BoolError {
    InvalidOp(String),
    InvalidFillRule(String),
    OpenSubpath,
    /// A path element appeared without a preceding `MoveTo`.
    MalformedPath,
    /// Wraps any failure (or panic) from inside `linesweeper`.
    LinesweeperFailed(String),
}

impl fmt::Display for BoolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BoolError::InvalidOp(op) => write!(f, "invalid path-bool op: {op:?}"),
            BoolError::InvalidFillRule(rule) => write!(f, "invalid fill-rule: {rule:?}"),
            BoolError::OpenSubpath => {
                write!(f, "path-bool requires every subpath to be closed")
            }
            BoolError::MalformedPath => {
                write!(f, "malformed path: segment without preceding move-to")
            }
            BoolError::LinesweeperFailed(msg) => write!(f, "linesweeper failed: {msg}"),
        }
    }
}

impl std::error::Error for BoolError {}
