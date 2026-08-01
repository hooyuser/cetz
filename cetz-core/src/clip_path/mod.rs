//! Path clipping operations.

mod line_clip;
mod op;
mod wire;

pub use op::{clip_path, clip_path_batch};
pub use wire::{ClipPathArgs, ClipPathBatchArgs};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClipMode {
    Include,
    Exclude,
}
