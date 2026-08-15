use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc,
};

use crate::buffer::FixedFrameBuffer;

pub struct CallbackGuard<'a> {
    flag: &'a AtomicBool,
}

impl<'a> CallbackGuard<'a> {
    #[inline]
    pub fn new(flag: &'a AtomicBool) -> Self {
        flag.store(true, Ordering::Release);
        Self { flag }
    }
}

impl<'a> Drop for CallbackGuard<'a> {
    #[inline]
    fn drop(&mut self) {
        self.flag.store(false, Ordering::Release);
    }
}

pub fn audio_callback_f32(
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

    if channels == 2 {
        let got = buffer.pop_block_interleaved(data);
        if got < data.len() {
            data[got..].fill(0.0);
            underruns.fetch_add(1, Ordering::Relaxed);
        }
    } else {
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

#[allow(clippy::too_many_arguments)]
pub fn audio_callback_i16(
    data: &mut [i16],
    buffer: &FixedFrameBuffer,
    paused: &AtomicBool,
    in_callback: &AtomicBool,
    underruns: &AtomicU32,
    channels: usize,
    visualizer_tap: &Option<Arc<crate::analysis::FftVisualizerTap>>,
    scratch_buffer: &mut [f32],
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

    let mut underrun_flag = false;
    if channels == 2 {
        let total_samples = data.len();
        if scratch_buffer.len() < total_samples {
            // Panic or something? We're on audio thread, shouldn't allocate.
            // But if it happens, we can't do anything better.
            log::error!("Scratch buffer too small, audio glitch expected");
            data.fill(0);
            return;
        }
        let scratch = &mut scratch_buffer[..total_samples];
        let got = buffer.pop_block_interleaved(scratch);
        if got < total_samples {
            scratch[got..].fill(0.0);
            underrun_flag = true;
        }
        if let Some(ref tap) = visualizer_tap {
            tap.process_samples(scratch, channels);
        }
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

#[allow(clippy::too_many_arguments)]
pub fn audio_callback_u16(
    data: &mut [u16],
    buffer: &FixedFrameBuffer,
    paused: &AtomicBool,
    in_callback: &AtomicBool,
    underruns: &AtomicU32,
    channels: usize,
    visualizer_tap: &Option<Arc<crate::analysis::FftVisualizerTap>>,
    scratch_buffer: &mut [f32],
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

    let mut underrun_flag = false;
    if channels == 2 {
        let total_samples = data.len();
        if scratch_buffer.len() < total_samples {
            log::error!("Scratch buffer too small, audio glitch expected");
            data.fill(32768);
            return;
        }
        let scratch = &mut scratch_buffer[..total_samples];
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
                        *sample = (((clamped + 1.0) * 0.5 * 65535.0).round() as i64).clamp(0, 65535)
                            as u16;
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
