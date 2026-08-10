#![allow(clippy::too_many_arguments)]

mod commands;
mod ffi;
pub mod types;

// Re-export the public API so callers use `bridge::foo()` the same way
// they did when everything was in a single bridge.rs file.
pub use commands::*;
pub use types::{Callbacks, FfiLoudnessWriteItem, FfiTagEditRequest, SongRowArg};
