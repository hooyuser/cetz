//! Path clipping operations.

mod line_clip;
mod op;
mod wire;

pub use op::clip_path;
pub use wire::ClipPathArgs;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClipMode {
    Include,
    Exclude,
}
