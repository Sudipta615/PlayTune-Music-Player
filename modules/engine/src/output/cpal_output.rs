//! Audio output using cpal
//!
//! The output callback is designed to be zero-allocation, zero-blocking.

use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc,
};

use config::AudioBackend;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, SampleFormat, Stream, StreamConfig,
};
use thiserror::Error;

use crate::buffer::FixedFrameBuffer;

#[derive(Debug, Error)]
pub enum OutputError {
    #[error("No audio device available")]
    NoDevice,
    #[error("Failed to open stream: {0}")]
    StreamOpen(String),
    #[error("Unsupported sample format")]
    UnsupportedFormat,
    #[error("Buffer underrun")]
    Underrun,
    #[error("Stream error: {0}")]
    StreamError(String),
}

/// A thread-safe wrapper around CPAL's Stream.
///
/// CPAL's Stream does not implement Send/Sync by default to remain compatible
/// with some platforms (like Web/Emscripten). Since we only target desktop
/// platforms where it is safe to send streams across threads, we wrap it in
/// an unsafe Send + Sync implementation.
pub struct SendSyncStream(pub Stream);
unsafe impl Send for SendSyncStream {}
unsafe impl Sync for SendSyncStream {}

/// Audio output using cpal
pub struct CpalOutput {
    stream: Option<SendSyncStream>,
    device: Device,
    /// Resolved stream config (sample rate, channels, buffer size)
    stream_config: StreamConfig,
    /// Sample format for the output stream
    sample_format: SampleFormat,
    /// Shared buffer between DSP thread and output callback
    buffer: Arc<FixedFrameBuffer>,
    /// Flag to pause output
    paused: Arc<AtomicBool>,
    /// Flag indicating if the audio thread is inside the callback
    in_callback: Arc<AtomicBool>,
    /// Underrun counter
    underruns: Arc<AtomicU32>,
    /// Sample rate of the output device
    actual_sample_rate: u32,
    /// Flag indicating that a stream error has occurred and recovery
    /// may be needed.
    stream_error: Arc<AtomicBool>,
    visualizer_tap: Option<Arc<crate::analysis::FftVisualizerTap>>,
    backend: AudioBackend,
    target_device: Option<String>,
}

impl CpalOutput {
    /// Enumerate available output device names for a given audio backend
    pub fn enumerate_devices(backend: AudioBackend) -> Vec<String> {
        let host = match backend {
            #[cfg(target_os = "linux")]
            AudioBackend::ExclusiveAlsa => {
                cpal::host_from_id(cpal::HostId::Alsa).unwrap_or_else(|_| cpal::default_host())
            }
            #[cfg(target_os = "windows")]
            AudioBackend::ExclusiveWasapi => {
                cpal::host_from_id(cpal::HostId::Wasapi).unwrap_or_else(|_| cpal::default_host())
            }
            #[cfg(all(target_os = "windows", feature = "asio"))]
            AudioBackend::ExclusiveAsio => {
                cpal::host_from_id(cpal::HostId::Asio).unwrap_or_else(|_| cpal::default_host())
            }
            #[cfg(target_os = "macos")]
            AudioBackend::ExclusiveCoreAudioHog => cpal::default_host(),
            _ => cpal::default_host(),
        };

        let mut device_names = Vec::new();
        if let Ok(devices) = host.output_devices() {
            for d in devices {
                if let Ok(name) = d.name() {
                    if !device_names.contains(&name) {
                        device_names.push(name);
                    }
                }
            }
        }
        device_names
    }

    /// Create a new cpal output with automatic fallback
    pub fn new(
        buffer: Arc<FixedFrameBuffer>,
        backend: AudioBackend,
        target_device: Option<&str>,
        visualizer_tap: Option<Arc<crate::analysis::FftVisualizerTap>>,
    ) -> Result<Self, OutputError> {
        match Self::new_raw(buffer.clone(), backend, target_device, visualizer_tap.clone()) {
            Ok(output) => Ok(output),
            Err(e) => {
                let is_custom = backend != AudioBackend::Auto
                    || target_device.is_some_and(|d| !d.is_empty() && d != "Default / Automatic");
                if is_custom {
                    log::warn!(
                        "Audio output: Exclusive mode or target device {:?} failed during init ({}); falling back to default shared device (`Auto`).",
                        target_device,
                        e
                    );
                    Self::new_raw(buffer, AudioBackend::Auto, None, visualizer_tap)
                } else {
                    Err(e)
                }
            }
        }
    }

    fn new_raw(
        buffer: Arc<FixedFrameBuffer>,
        backend: AudioBackend,
        target_device: Option<&str>,
        visualizer_tap: Option<Arc<crate::analysis::FftVisualizerTap>>,
    ) -> Result<Self, OutputError> {
        let host = match backend {
            #[cfg(target_os = "linux")]
            AudioBackend::ExclusiveAlsa => {
                log::info!("Audio output: Requesting exclusive ALSA host");
                cpal::host_from_id(cpal::HostId::Alsa).unwrap_or_else(|_| cpal::default_host())
            }
            #[cfg(target_os = "windows")]
            AudioBackend::ExclusiveWasapi => {
                log::info!("Audio output: Requesting exclusive WASAPI host");
                cpal::host_from_id(cpal::HostId::Wasapi).unwrap_or_else(|_| cpal::default_host())
            }
            #[cfg(all(target_os = "windows", feature = "asio"))]
            AudioBackend::ExclusiveAsio => {
                log::info!("Audio output: Requesting exclusive ASIO host");
                cpal::host_from_id(cpal::HostId::Asio).unwrap_or_else(|_| cpal::default_host())
            }
            #[cfg(all(target_os = "windows", not(feature = "asio")))]
            AudioBackend::ExclusiveAsio => {
                log::warn!(
                    "Audio output: ASIO support not compiled in; falling back to default host"
                );
                cpal::default_host()
            }
            #[cfg(target_os = "macos")]
            AudioBackend::ExclusiveCoreAudioHog => {
                log::info!("Audio output: Requesting CoreAudio Hog Mode");
                cpal::default_host() // CoreAudio is the default on macOS
            }
            _ => cpal::default_host(),
        };

        #[allow(unused_mut)]
        let mut device = None;

        if let Some(target_name) = target_device {
            if !target_name.is_empty() && target_name != "Default / Automatic" {
                if let Ok(devices) = host.output_devices() {
                    for d in devices {
                        if let Ok(name) = d.name() {
                            if name == target_name || name.contains(target_name) {
                                log::info!("Audio output: Selected target device: {}", name);
                                device = Some(d);
                                break;
                            }
                        }
                    }
                }
                if device.is_none() {
                    log::warn!(
                        "Target audio device '{}' not found on host; falling back to automatic device selection",
                        target_name
                    );
                }
            }
        }

        // If ALSA was requested and no specific target device found/selected, try to find a hardware device rather than 'default'
        if device.is_none() && backend == AudioBackend::ExclusiveAlsa {
            #[cfg(target_os = "linux")]
            if let Ok(devices) = host.output_devices() {
                let mut valid_devices: Vec<_> = devices
                    .filter(|d| {
                        let name = d.name().unwrap_or_default().to_lowercase();
                        name != "default"
                            && !name.starts_with("sysdefault")
                            && !name.contains("pulse")
                            && !name.contains("pipewire")
                            && !name.contains("dmix")
                    })
                    .collect();

                valid_devices.sort_by_key(|d| {
                    if d.name().unwrap_or_default().to_lowercase().contains("analog") {
                        0
                    } else {
                        1
                    }
                });

                if let Some(hw_dev) = valid_devices.into_iter().next() {
                    log::info!(
                        "Audio output: Selected exclusive hardware device: {}",
                        hw_dev.name().unwrap_or_default()
                    );
                    device = Some(hw_dev);
                }
            }
        }

        let device =
            device.or_else(|| host.default_output_device()).ok_or(OutputError::NoDevice)?;

        // Use the device's default config instead of max-sample-rate.
        let default_config = device
            .default_output_config()
            .map_err(|e| OutputError::StreamOpen(format!("Cannot get default config: {}", e)))?;

        let target_sample_rate = default_config.sample_rate().0;

        let supported = device
            .supported_output_configs()
            .map_err(|e| OutputError::StreamOpen(format!("Cannot query configs: {}", e)))?;
        let supported_configs: Vec<_> = supported.collect();

        let config = supported_configs
            .iter()
            .find(|c| {
                c.sample_format() == SampleFormat::F32
                    && c.min_sample_rate().0 <= target_sample_rate
                    && c.max_sample_rate().0 >= target_sample_rate
            })
            .map(|c| c.with_sample_rate(cpal::SampleRate(target_sample_rate)))
            .or_else(|| {
                supported_configs.iter().find(|c| c.sample_format() == SampleFormat::F32).map(|c| {
                    let rate =
                        target_sample_rate.clamp(c.min_sample_rate().0, c.max_sample_rate().0);
                    c.with_sample_rate(cpal::SampleRate(rate))
                })
            })
            .or_else(|| {
                supported_configs
                    .iter()
                    .find(|c| {
                        (c.sample_format() == SampleFormat::I16
                            || c.sample_format() == SampleFormat::U16)
                            && c.min_sample_rate().0 <= target_sample_rate
                            && c.max_sample_rate().0 >= target_sample_rate
                    })
                    .map(|c| c.with_sample_rate(cpal::SampleRate(target_sample_rate)))
            })
            .or_else(|| {
                supported_configs
                    .iter()
                    .find(|c| {
                        c.sample_format() == SampleFormat::I16
                            || c.sample_format() == SampleFormat::U16
                    })
                    .map(|c| {
                        let rate =
                            target_sample_rate.clamp(c.min_sample_rate().0, c.max_sample_rate().0);
                        c.with_sample_rate(cpal::SampleRate(rate))
                    })
            })
            .ok_or(OutputError::UnsupportedFormat)?;

        let actual_sample_rate = config.sample_rate().0;
        let channels = config.channels();
        let sample_format = config.sample_format();

        let buffer_size = match config.buffer_size() {
            cpal::SupportedBufferSize::Range { min, max } => {
                cpal::BufferSize::Fixed(2048.clamp(*min, *max))
            }
            cpal::SupportedBufferSize::Unknown => cpal::BufferSize::Default,
        };

        let stream_config = StreamConfig {
            channels,
            sample_rate: cpal::SampleRate(actual_sample_rate),
            buffer_size,
        };

        log::info!(
            "Audio output: {} Hz, {} ch, {:?}, buffer size: {:?}",
            actual_sample_rate,
            channels,
            sample_format,
            buffer_size
        );

        Ok(Self {
            stream: None,
            device,
            stream_config,
            sample_format,
            buffer,
            paused: Arc::new(AtomicBool::new(false)),
            in_callback: Arc::new(AtomicBool::new(false)),
            underruns: Arc::new(AtomicU32::new(0)),
            actual_sample_rate,
            stream_error: Arc::new(AtomicBool::new(false)),
            visualizer_tap,
            backend,
            target_device: target_device.map(|s| s.to_string()),
        })
    }

    /// Start the audio output stream with automatic fallback on failure
    pub fn start(&mut self) -> Result<(), OutputError> {
        match self.start_raw() {
            Ok(()) => Ok(()),
            Err(e) => {
                let is_custom = self.backend != AudioBackend::Auto
                    || self
                        .target_device
                        .as_ref()
                        .is_some_and(|d| !d.is_empty() && d != "Default / Automatic");
                if is_custom {
                    log::warn!(
                        "Audio output: Failed to start stream in exclusive mode / target device {:?} ({}); falling back to default shared device (`Auto`).",
                        self.target_device,
                        e
                    );
                    let mut fallback = Self::new_raw(
                        self.buffer.clone(),
                        AudioBackend::Auto,
                        None,
                        self.visualizer_tap.clone(),
                    )?;
                    fallback.start_raw()?;
                    *self = fallback;
                    Ok(())
                } else {
                    Err(e)
                }
            }
        }
    }

    fn start_raw(&mut self) -> Result<(), OutputError> {
        let buffer = Arc::clone(&self.buffer);
        let paused = Arc::clone(&self.paused);
        let in_callback = Arc::clone(&self.in_callback);
        let underruns = Arc::clone(&self.underruns);
        let stream_error = Arc::clone(&self.stream_error);
        let channels = self.stream_config.channels as usize;
        let visualizer_tap = self.visualizer_tap.clone();
        let callback_initialized = Arc::new(AtomicBool::new(false));

        // Error callback: instead of just logging, set the stream_error
        // flag so the engine can detect device disconnections and attempt
        // recovery. Common errors include device removal (USB unplug),
        // Bluetooth disconnection, and sample rate changes when the OS
        // switches the default audio device.
        let error_callback = move |err: cpal::StreamError| {
            log::error!("Audio output error: {}", err);
            stream_error.store(true, Ordering::Release);
        };

        let stream = match self.sample_format {
            SampleFormat::F32 => {
                let in_callback = Arc::clone(&in_callback);
                let visualizer_tap = visualizer_tap.clone();
                let callback_initialized = Arc::clone(&callback_initialized);
                self.device
                    .build_output_stream(
                        &self.stream_config,
                        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                            escalate_callback_thread_priority(&callback_initialized);
                            Self::audio_callback(
                                data,
                                &buffer,
                                &paused,
                                &in_callback,
                                &underruns,
                                channels,
                                &visualizer_tap,
                            );
                        },
                        error_callback,
                        None,
                    )
                    .map_err(|e| OutputError::StreamOpen(format!("{}", e)))?
            }
            SampleFormat::I16 => {
                let in_callback = Arc::clone(&in_callback);
                let visualizer_tap = visualizer_tap.clone();
                let callback_initialized = Arc::clone(&callback_initialized);
                self.device
                    .build_output_stream(
                        &self.stream_config,
                        move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                            escalate_callback_thread_priority(&callback_initialized);
                            Self::audio_callback_i16(
                                data,
                                &buffer,
                                &paused,
                                &in_callback,
                                &underruns,
                                channels,
                                &visualizer_tap,
                            );
                        },
                        error_callback,
                        None,
                    )
                    .map_err(|e| OutputError::StreamOpen(format!("{}", e)))?
            }
            SampleFormat::U16 => {
                let in_callback = Arc::clone(&in_callback);
                let visualizer_tap = visualizer_tap.clone();
                let callback_initialized = Arc::clone(&callback_initialized);
                self.device
                    .build_output_stream(
                        &self.stream_config,
                        move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                            escalate_callback_thread_priority(&callback_initialized);
                            Self::audio_callback_u16(
                                data,
                                &buffer,
                                &paused,
                                &in_callback,
                                &underruns,
                                channels,
                                &visualizer_tap,
                            );
                        },
                        error_callback,
                        None,
                    )
                    .map_err(|e| OutputError::StreamOpen(format!("{}", e)))?
            }
            _ => {
                return Err(OutputError::UnsupportedFormat);
            }
        };

        stream.play().map_err(|e| OutputError::StreamOpen(format!("Play failed: {}", e)))?;
        self.stream = Some(SendSyncStream(stream));

        log::info!("Audio output stream started successfully");
        Ok(())
    }

    /// Audio callback for F32 output - ZERO ALLOCATION, ZERO BLOCKING
    #[inline]
    fn audio_callback(
        data: &mut [f32],
        buffer: &FixedFrameBuffer,
        paused: &AtomicBool,
        in_callback: &AtomicBool,
        underruns: &AtomicU32,
        channels: usize,
        visualizer_tap: &Option<Arc<crate::analysis::FftVisualizerTap>>,
    ) {
        let _guard = CallbackGuard::new(in_callback);
        if paused.load(Ordering::Acquire) {
            data.fill(0.0);
            return;
        }
        if channels == 0 {
            data.fill(0.0);
            return;
        }

        // Fast path: stereo output matches the stereo PCM ring 1:1.
        // The decode loop always pushes interleaved L,R pairs, so we can
        // pop the entire callback buffer in one shot.
        if channels == 2 {
            let got = buffer.pop_block_interleaved(data);
            if got < data.len() {
                // Underrun: fill the remainder with zeros and bump the
                // counter. This still counts as a single underrun event
                // even if multiple frames were missing.
                data[got..].fill(0.0);
                underruns.fetch_add(1, Ordering::Relaxed);
            }
        } else {
            // Non-stereo output (mono or >2 channels). Fall back to the
            // per-frame path. This is rare — most output devices are
            // stereo — so the perf cost is acceptable here.
            let mut underrun_flag = false;
            for frame in data.chunks_mut(channels) {
                match buffer.pop() {
                    Some(audio_frame) => {
                        for (ch, sample) in frame.iter_mut().enumerate() {
                            *sample = if ch < audio_frame.num_channels as usize {
                                audio_frame.channels[ch]
                            } else {
                                0.0
                            };
                        }
                    }
                    None => {
                        frame.fill(0.0);
                        underrun_flag = true;
                    }
                }
            }
            if underrun_flag {
                underruns.fetch_add(1, Ordering::Relaxed);
            }
        }
        for sample in data.iter_mut() {
            *sample = if sample.is_finite() { sample.clamp(-1.0, 1.0) } else { 0.0 };
        }

        if let Some(ref tap) = visualizer_tap {
            tap.process_samples(data, channels);
        }
    }

    /// Audio callback for I16 output
    ///
    /// Same hot-path optimization as `audio_callback`: bulk-pop the entire
    /// callback buffer in one shot (for stereo), then convert in place.
    /// The previous per-frame `buffer.pop()` loop was the #1 CPU hotspot
    /// on the audio thread.
    #[inline]
    fn audio_callback_i16(
        data: &mut [i16],
        buffer: &FixedFrameBuffer,
        paused: &AtomicBool,
        in_callback: &AtomicBool,
        underruns: &AtomicU32,
        channels: usize,
        visualizer_tap: &Option<Arc<crate::analysis::FftVisualizerTap>>,
    ) {
        let _guard = CallbackGuard::new(in_callback);
        if paused.load(Ordering::Acquire) {
            data.fill(0);
            return;
        }
        if channels == 0 {
            data.fill(0);
            return;
        }

        // Reusable scratch buffer for the F32→I16 conversion. Allocated on
        // the stack for the typical callback size; falls back to a Vec only
        // for pathological 8192+ sample callbacks (which don't happen in
        // practice — CPAL's buffer_size is fixed at 2048 in new_raw()).
        const SCRATCH_CAP: usize = 4096;
        let mut stack_scratch = [0.0f32; SCRATCH_CAP];

        let mut underrun_flag = false;
        if channels == 2 {
            // Bulk pop stereo interleaved f32 into the scratch buffer, then
            // convert to i16 in place.
            let total_samples = data.len();
            let scratch: &mut [f32] = if total_samples <= SCRATCH_CAP {
                &mut stack_scratch[..total_samples]
            } else {
                // Extremely rare: callback asked for more than 4096 samples.
                // Fall back to a Vec allocation. This is acceptable because
                // it only happens once per callback, not per sample.
                &mut vec![0.0f32; total_samples]
            };
            let got = buffer.pop_block_interleaved(scratch);
            if got < total_samples {
                scratch[got..].fill(0.0);
                underrun_flag = true;
            }
            if let Some(ref tap) = visualizer_tap {
                tap.process_samples(scratch, channels);
            }
            // Convert f32 → i16 with clamping. The compiler auto-vectorizes
            // this loop with SSE2 on x86-64 (4 f32s per iteration).
            for (dst, &src) in data.iter_mut().zip(scratch.iter()) {
                let scaled = (src.clamp(-1.0, 1.0) * 32768.0).clamp(-32768.0, 32767.0);
                *dst = scaled as i16;
            }
        } else {
            for frame in data.chunks_mut(channels) {
                match buffer.pop() {
                    Some(audio_frame) => {
                        for (ch, sample) in frame.iter_mut().enumerate() {
                            let val = if ch < audio_frame.num_channels as usize {
                                audio_frame.channels[ch]
                            } else {
                                0.0
                            };
                            let scaled = (val.clamp(-1.0, 1.0) * 32768.0).clamp(-32768.0, 32767.0);
                            *sample = scaled as i16;
                        }
                    }
                    None => {
                        frame.fill(0);
                        underrun_flag = true;
                    }
                }
            }
        }
        if underrun_flag {
            underruns.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Audio callback for U16 output
    ///
    /// Same hot-path optimization as the F32 and I16 callbacks.
    #[inline]
    fn audio_callback_u16(
        data: &mut [u16],
        buffer: &FixedFrameBuffer,
        paused: &AtomicBool,
        in_callback: &AtomicBool,
        underruns: &AtomicU32,
        channels: usize,
        visualizer_tap: &Option<Arc<crate::analysis::FftVisualizerTap>>,
    ) {
        let _guard = CallbackGuard::new(in_callback);
        if paused.load(Ordering::Acquire) {
            data.fill(32768);
            return;
        }
        if channels == 0 {
            data.fill(32768);
            return;
        }

        const SCRATCH_CAP: usize = 4096;
        let mut stack_scratch = [0.0f32; SCRATCH_CAP];

        let mut underrun_flag = false;
        if channels == 2 {
            let total_samples = data.len();
            let scratch: &mut [f32] = if total_samples <= SCRATCH_CAP {
                &mut stack_scratch[..total_samples]
            } else {
                &mut vec![0.0f32; total_samples]
            };
            let got = buffer.pop_block_interleaved(scratch);
            if got < total_samples {
                scratch[got..].fill(0.0);
                underrun_flag = true;
            }
            if let Some(ref tap) = visualizer_tap {
                tap.process_samples(scratch, channels);
            }
            for (dst, &src) in data.iter_mut().zip(scratch.iter()) {
                let clamped = src.clamp(-1.0, 1.0);
                *dst = (((clamped + 1.0) * 0.5 * 65535.0).round() as i64).clamp(0, 65535) as u16;
            }
        } else {
            for frame in data.chunks_mut(channels) {
                match buffer.pop() {
                    Some(audio_frame) => {
                        for (ch, sample) in frame.iter_mut().enumerate() {
                            let val = if ch < audio_frame.num_channels as usize {
                                audio_frame.channels[ch]
                            } else {
                                0.0
                            };
                            let clamped = val.clamp(-1.0, 1.0);
                            *sample = (((clamped + 1.0) * 0.5 * 65535.0).round() as i64)
                                .clamp(0, 65535) as u16;
                        }
                    }
                    None => {
                        frame.fill(32768);
                        underrun_flag = true;
                    }
                }
            }
        }
        if underrun_flag {
            underruns.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Pause the output
    pub fn pause(&self) {
        self.paused.store(true, Ordering::Release);
        if let Some(ref stream) = self.stream {
            let _ = stream.0.pause();
        }
        // Always wait for any in-flight callback to exit before returning.
        // The caller (typically `reset_buffer` or `stop`) is about to
        // mutate shared state (ring indices, pending frames, decoder
        // state) that the audio callback also touches; without this
        // synchronisation, the callback could read stale or corrupt state.
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(50);
        while self.in_callback.load(Ordering::Acquire) {
            if std::time::Instant::now() >= deadline {
                log::warn!(
                    "CpalOutput::pause(): callback did not exit within 50ms; \
                     proceeding to avoid deadlock"
                );
                break;
            }
            std::thread::sleep(std::time::Duration::from_micros(100));
        }
    }

    /// Resume the output
    pub fn resume(&self) {
        self.paused.store(false, Ordering::Release);
        if let Some(ref stream) = self.stream {
            let _ = stream.0.play();
        }
    }

    /// Reset the output buffer safely.
    pub fn reset_buffer(&self) {
        self.pause();
        self.buffer.reset();
        self.resume();
    }

    /// Get the number of underruns since last check
    pub fn take_underruns(&self) -> u32 {
        self.underruns.swap(0, Ordering::Relaxed)
    }

    /// Get the actual sample rate
    pub fn sample_rate(&self) -> u32 {
        self.actual_sample_rate
    }

    /// Check if a stream error has been reported (e.g., device disconnection).
    /// The error flag is cleared after reading.
    pub fn take_stream_error(&self) -> bool {
        self.stream_error.swap(false, Ordering::AcqRel)
    }

    /// Stop the output stream
    pub fn stop(&mut self) {
        self.pause();
        if let Some(stream) = self.stream.take() {
            struct SendPtr(*mut SendSyncStream);
            unsafe impl Send for SendPtr {}

            let stream_box = Box::new(stream);
            let send_ptr = SendPtr(Box::into_raw(stream_box));
            match std::thread::Builder::new().name("tc-cpal-stream-drop".into()).spawn(move || {
                let send_ptr = send_ptr;
                // Safety: send_ptr.0 was created by Box::into_raw above
                // and is uniquely owned by this closure. We reconstruct
                // the Box and then drop it, which drops the inner Stream
                // on this background thread (the only safe place to drop
                // it without deadlocking the audio callback join).
                let boxed: Box<SendSyncStream> = unsafe { Box::from_raw(send_ptr.0) };
                drop(boxed);
            }) {
                Ok(_) => {}
                Err(e) => {
                    log::warn!(
                        "Failed to spawn stream-drop thread ({}); leaking stream to avoid deadlock",
                        e
                    );
                    // Safety: send_ptr.0 was created by Box::into_raw above
                    // and the closure never ran (so the box is still
                    // uniquely owned by us via the raw pointer). We
                    // intentionally leak it — the OS reclaims the memory
                    // and the underlying CPAL stream on process exit. We
                    // do NOT reconstruct the Box here, because dropping it
                    // would re-introduce the synchronous-drop deadlock.
                    // (No-op: just let send_ptr drop, which drops the raw
                    // pointer — raw pointers have no Drop side effect, so
                    // the heap allocation is leaked as intended.)
                }
            }
        }
    }

    /// Get the current device name for diagnostic purposes.
    pub fn device_name(&self) -> String {
        self.device.name().unwrap_or_else(|_| "unknown".to_string())
    }
}

/// Escalate the current thread (the CPAL audio callback thread) to real-time
/// priority. Runs only once; subsequent calls return immediately.
///
/// On Linux this requires rtkit permissions, `ulimit -r`, or CAP_SYS_NICE.
/// Failure is logged once and playback continues with default scheduling.
fn escalate_callback_thread_priority(initialized: &AtomicBool) {
    if initialized.swap(true, Ordering::Relaxed) {
        return;
    }
    // Enable FTZ + DAZ on the audio callback thread.
    let _ = crate::buffer::enable_flush_zero_denormals_on_current_thread();

    #[cfg(feature = "thread-priority")]
    {
        match thread_priority::set_current_thread_priority(thread_priority::ThreadPriority::Max) {
            Ok(()) => {
                log::info!("Audio callback thread escalated to real-time priority");
            }
            Err(e) => {
                log::warn!(
                    "Failed to set real-time priority for audio callback thread: {}. \
                     Audio will continue with default scheduling. \
                     On Linux, ensure rtkit permissions or ulimit -r is configured.",
                    e
                );
            }
        }
    }
}

struct CallbackGuard<'a> {
    flag: &'a AtomicBool,
}

impl<'a> CallbackGuard<'a> {
    fn new(flag: &'a AtomicBool) -> Self {
        flag.store(true, Ordering::Release);
        Self { flag }
    }
}

impl<'a> Drop for CallbackGuard<'a> {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::Release);
    }
}
