// Re-export the enums at the crate root for direct access (`config::FilterType`,
// `config::CrossfeedProfile`, etc.). The previous `types::enums` shim module
// re-exported `crate::*` which made the path self-referential and polluted the
// public API. Callers should use the root path or `config::prelude::*`.
//
// Backwards-compatibility shim: keep `config::types::enums::Foo` working for
// any external code (and for `buffer.rs` which references
// `config::types::enums::CrossfeedProfile`) by re-exporting the specific
// enum types instead of the entire crate.
pub mod types {
    pub mod enums {
        pub use crate::{
            AudioBackend, CrossfadeConfig, CrossfeedProfile, FilterType, LoudnessMode,
            PerformanceMode, ResamplerQuality,
        };
    }
}

mod dsp;
mod enums;
mod file;
mod library;

// Public API — re-export everything from sub-modules at the crate root so
// existing callers like `config::EngineConfig` keep working unchanged.
pub use dsp::*;
pub use enums::*;
pub use file::*;
pub use library::*;
