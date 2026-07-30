//! Off-thread audio analysis for visualization and metadata
//!
//! This module re-exports analysis types from the `analysis` crate,
//! which is the canonical implementation.

// Re-export the canonical analysis types from analysis
pub use analysis::{
    BpmDetector, FftVisualizerTap, TrackAnalysis, WaveformGenerator as FileWaveformGenerator,
};
