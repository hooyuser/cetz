//! Path boolean operations (union, intersection, difference, xor).
//!
//! Wraps the [`linesweeper`](https://crates.io/crates/linesweeper) crate
//! behind a CBOR wire format suitable for the Typst <-> WASM boundary.

// Until Step B1 wires the WASM export through `path_bool_func`, the items
// below appear unused to the wasm32 build. The `cargo test` build does
// reach them via the in-crate test modules.
#[allow(dead_code)]
mod convert;
#[allow(dead_code)]
mod error;
#[allow(dead_code)]
mod op;
#[allow(dead_code)]
mod wire;
