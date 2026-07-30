pub mod symphonia_decoder;

pub use symphonia_decoder::{
    extract_cover_art_to_cache, extract_loudness_metadata, extract_track_metadata, DecodeError,
    DecodeInfo, DecodedChunk, SymphoniaDecoder,
};
