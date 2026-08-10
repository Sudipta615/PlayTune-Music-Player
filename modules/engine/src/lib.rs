pub mod analysis;
pub mod buffer;
pub mod commands;
pub mod decode;
pub mod dsp;
pub mod dsp_utils;
#[cfg(feature = "audio-output")]
pub mod engine;
#[cfg(feature = "audio-output")]
pub mod output;
pub mod playback_info;

// prelude, so `engine::AudioEngine` works directly (not just via prelude).
#[cfg(feature = "audio-output")]
pub use engine::AudioEngine;

pub use config::ResamplerQuality;
pub use decode::{extract_cover_art_to_cache, extract_track_metadata};

pub mod prelude {
    #[cfg(feature = "audio-output")]
    pub use crate::engine::AudioEngine;
    #[cfg(feature = "audio-output")]
    pub use crate::engine::PlaybackStream;
    pub use crate::{
        buffer::{
            AudioChunk, AudioFrame, BufferError, EngineCommand, FixedFrameBuffer, PlaybackInfo,
            PlaybackState, DEFAULT_SAMPLE_RATE,
        },
        decode::{extract_cover_art_to_cache, extract_track_metadata},
        dsp::pipeline::DspPipeline,
    };
    pub use config::ResamplerQuality;
}
