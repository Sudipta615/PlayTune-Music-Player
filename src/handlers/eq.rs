use std::ffi::c_double;
use std::sync::atomic::Ordering;

use engine::buffer::EngineCommand;

use crate::app_state::{
    invalidate_shuffle_order, sync_shuffle_order, CURRENT_INDEX, CURRENT_TRACK_LIST, ENGINE_CMD_TX,
    EQ_BAND_FREQS, REPEAT_ENABLED, SHUFFLE_ENABLED,
};
use crate::ffi_safe;
use crate::ui_sync::refresh_up_next_queue;

pub extern "C" fn rust_eq_band(band_idx: i32, gain_db: c_double) {
    ffi_safe!({
        if !(0..10).contains(&band_idx) {
            log::warn!("rust_eq_band: out-of-range band_idx {band_idx}");
            return;
        }
        log::debug!("EQ Band {} adjusted to {:.1} dB", band_idx, gain_db);
        if let Some(tx) = ENGINE_CMD_TX.get() {
            let _ = tx.send(EngineCommand::SetEqBand {
                index: band_idx as usize,
                frequency: EQ_BAND_FREQS[band_idx as usize],
                gain_db: gain_db as f32,
                q: 1.0,
                enabled: true,
            });
        }
    });
}

pub extern "C" fn rust_eq_advanced_band(
    band_idx: i32,
    freq: c_double,
    gain_db: c_double,
    q: c_double,
    filter_type: i32,
) {
    ffi_safe!({
        if !(0..10).contains(&band_idx) {
            log::warn!("rust_eq_advanced_band: out-of-range band_idx {band_idx}");
            return;
        }
        use engine::dsp::equalizer::EqFilterType;
        let ftype = match filter_type {
            0 => EqFilterType::LowShelf,
            1 => EqFilterType::Peaking,
            2 => EqFilterType::HighShelf,
            3 => EqFilterType::LowPass,
            4 => EqFilterType::HighPass,
            5 => EqFilterType::Bandpass,
            6 => EqFilterType::Notch,
            _ => EqFilterType::Peaking,
        };
        log::debug!(
            "EQ Advanced Band {} adjusted to freq={:.1}Hz, gain={:.1}dB, Q={:.2}, type={:?}",
            band_idx,
            freq,
            gain_db,
            q,
            ftype
        );
        if let Some(tx) = ENGINE_CMD_TX.get() {
            let _ = tx.send(EngineCommand::SetEqBandParams {
                index: band_idx as usize,
                frequency: freq as f32,
                gain_db: gain_db as f32,
                q: q as f32,
                filter_type: ftype,
                enabled: true,
            });
        }
    });
}

pub extern "C" fn rust_set_resampler_quality(quality: i32) {
    ffi_safe!({
        let q = match quality {
            0 => engine::ResamplerQuality::Fast,
            2 => engine::ResamplerQuality::HighQuality,
            _ => engine::ResamplerQuality::Balanced,
        };
        log::info!("Resampler quality set to {:?}", q);
        if let Some(tx) = ENGINE_CMD_TX.get() {
            let _ = tx.send(EngineCommand::SetResamplerQuality(q));
        }
    });
}

pub extern "C" fn rust_set_output_backend(backend: i32) {
    ffi_safe!({
        let b = match backend {
            1 => config::AudioBackend::ExclusiveAlsa,
            2 => config::AudioBackend::ExclusiveWasapi,
            3 => config::AudioBackend::ExclusiveAsio,
            4 => config::AudioBackend::ExclusiveCoreAudioHog,
            _ => config::AudioBackend::Auto,
        };
        log::info!("Output backend changed via UI to {:?}", b);
        if let Some(tx) = ENGINE_CMD_TX.get() {
            let _ = tx.send(EngineCommand::SetOutputBackend(b));
        }
    });
}

pub extern "C" fn rust_set_output_device(device_name: *const std::ffi::c_char) {
    ffi_safe!({
        if device_name.is_null() {
            return;
        }
        let name = unsafe { std::ffi::CStr::from_ptr(device_name) }.to_string_lossy().to_string();
        log::info!("Output device changed via UI to {:?}", name);
        let device_opt =
            if name.is_empty() || name == "Default / Automatic" { None } else { Some(name) };
        if let Some(tx) = ENGINE_CMD_TX.get() {
            let _ = tx.send(EngineCommand::SetOutputDevice(device_opt));
        }
    });
}

pub extern "C" fn rust_eq_enabled(enabled: i32) {
    ffi_safe!({
        let is_enabled = enabled != 0;
        log::info!("Equalizer toggled. Active: {}", is_enabled);
        if let Some(tx) = ENGINE_CMD_TX.get() {
            let _ = tx.send(EngineCommand::SetEqEnabled(is_enabled));
        }
    });
}

pub extern "C" fn rust_preset_selected(preset_idx: i32) {
    ffi_safe!({
        rust_preset_selected_inner(preset_idx);
    });
}

pub fn rust_preset_selected_inner(preset_idx: i32) {
    let presets = ["Flat", "Pop", "Rock", "Jazz", "Classical", "Electronic", "Hip Hop", "Custom"];
    let name = presets.get(preset_idx as usize).unwrap_or(&"Unknown");
    log::info!("EQ Preset selected: {} (Index: {})", name, preset_idx);
    let gains: [f32; 10] = match preset_idx {
        0 => [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], // Flat
        1 => [-1.5, -1.0, 0.0, 2.0, 4.0, 4.0, 2.0, 0.0, -1.0, -1.5], // Pop
        2 => [4.5, 3.5, 2.0, 0.5, -1.0, -1.5, 0.5, 2.5, 3.5, 4.5], // Rock
        3 => [3.0, 2.0, 1.0, 2.0, -1.5, -1.5, 0.0, 1.5, 2.5, 3.0], // Jazz
        4 => [4.5, 3.5, 3.0, 2.5, -1.5, -1.5, 0.0, 2.0, 3.0, 4.0], // Classical
        5 => [5.5, 4.5, 2.0, 0.0, -2.0, 2.0, 1.0, 2.0, 4.0, 5.0], // Electronic
        6 => [5.0, 4.0, 1.5, 3.0, -1.0, -1.0, 1.0, 2.0, 3.0, 4.0], // Hip Hop
        _ => return,
    };
    if let Some(tx) = ENGINE_CMD_TX.get() {
        for (idx, gain) in gains.iter().enumerate() {
            let _ = tx.send(EngineCommand::SetEqBand {
                index: idx,
                frequency: EQ_BAND_FREQS[idx],
                gain_db: *gain,
                q: 1.0,
                enabled: true,
            });
        }
    }
}

pub extern "C" fn rust_reset_eq() {
    ffi_safe!({
        rust_reset_eq_inner();
    });
}

pub fn rust_reset_eq_inner() {
    log::info!("EQ reset clicked");
    if let Some(tx) = ENGINE_CMD_TX.get() {
        for idx in 0..10 {
            let _ = tx.send(EngineCommand::SetEqBand {
                index: idx,
                frequency: EQ_BAND_FREQS[idx],
                gain_db: 0.0,
                q: 1.0,
                enabled: true,
            });
        }
    }
}

pub extern "C" fn rust_slider_param(param_idx: i32, value: c_double) {
    ffi_safe!({
        rust_slider_param_inner(param_idx, value);
    });
}

pub fn rust_slider_param_inner(param_idx: i32, value: c_double) {
    let params =
        ["Bass", "Treble", "Stereo Width", "Balance", "Preamp", "Repeat", "Shuffle", "Rate"];
    let name = params.get(param_idx as usize).unwrap_or(&"Unknown");
    if param_idx >= 5 {
        match param_idx {
            5 => {
                let enabled = value > 0.0;
                REPEAT_ENABLED.store(enabled, Ordering::SeqCst);
                log::info!("Repeat toggled: {}", enabled);
                if let Some(tx) = ENGINE_CMD_TX.get() {
                    let status = if enabled { "Track" } else { "None" };
                    let _ = tx.send(EngineCommand::SetLoopStatus(status.to_string()));
                }
            }
            6 => {
                let enabled = value > 0.0;
                SHUFFLE_ENABLED.store(enabled, Ordering::SeqCst);
                if enabled {
                    invalidate_shuffle_order();
                    let curr = *CURRENT_INDEX.lock();
                    let len =
                        CURRENT_TRACK_LIST.get().and_then(|l| l.try_lock()).map_or(0, |l| l.len());
                    if len > 0 {
                        sync_shuffle_order(curr, len);
                    }
                }
                refresh_up_next_queue();
                log::info!("Shuffle toggled: {}", enabled);
                if let Some(tx) = ENGINE_CMD_TX.get() {
                    let _ = tx.send(EngineCommand::SetShuffle(enabled));
                }
            }
            7 => {
                let rate = value as f32;
                if !rate.is_finite() || rate <= 0.0 {
                    log::warn!("SetRate ignored: non-finite or non-positive value {}", value);
                } else {
                    log::info!("Rate set to {:.3}", rate);
                    if let Some(tx) = ENGINE_CMD_TX.get() {
                        let _ = tx.send(EngineCommand::SetSpeed(rate));
                    }
                }
            }
            _ => {
                log::info!("Toggle clicked for param: {} (State: {})", name, value > 0.0);
            }
        }
    } else {
        log::debug!("Parametric slider: {} changed to {:.2}", name, value);
        if let Some(tx) = ENGINE_CMD_TX.get() {
            match param_idx {
                0 => {
                    let _ = tx.send(EngineCommand::SetBassShelf(value as f32));
                }
                1 => {
                    let _ = tx.send(EngineCommand::SetTrebleShelf(value as f32));
                }
                2 => {
                    let width_factor = if value > 2.0 { value / 100.0 } else { value };
                    let _ = tx.send(EngineCommand::SetStereoWidth(width_factor as f32));
                }
                3 => {
                    let _ = tx.send(EngineCommand::SetBalance(value as f32));
                }
                4 => {
                    let _ = tx.send(EngineCommand::SetPreamp(value as f32));
                }
                _ => {}
            }
        }
    }
}
