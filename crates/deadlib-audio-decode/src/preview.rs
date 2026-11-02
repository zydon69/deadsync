//! Worker-only source editor resampling. Output is immutable dry float PCM.
use std::sync::atomic::{AtomicBool, Ordering};

use crate::hermite::interpolate as hermite;

/// Mirrors Fumen SoundManager.ResampleFloat's time grid and four-point
/// interpolation. Quantized decoder input is a separate format boundary.
pub fn resample(
    input: &[i16],
    channels: usize,
    input_hz: u32,
    output_hz: u32,
    cancelled: &AtomicBool,
) -> Result<Vec<f32>, String> {
    if channels == 0 || input_hz == 0 || output_hz == 0 || input.len() % channels != 0 {
        return Err("Invalid preview PCM format".into());
    }
    let frames = input.len() / channels;
    let count = ((frames as f64) * (f64::from(output_hz) / f64::from(input_hz))) as usize;
    let samples = count
        .checked_mul(channels)
        .ok_or("Preview PCM is too large")?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(samples)
        .map_err(|e| e.to_string())?;
    if input.is_empty() {
        return Ok(output);
    }
    let scale = 1.0f32 / 32768.;
    if input_hz == output_hz {
        for chunk in input.chunks(524288) {
            if cancelled.load(Ordering::Acquire) {
                return Err("Preview preparation cancelled".into());
            }
            output.extend(chunk.iter().map(|v| f32::from(*v) * scale));
        }
        return Ok(output);
    }
    let range = 1.0f32 / input_hz as f32;
    let maximum = (input.len() - channels) as i128;
    for frame in 0..count {
        if frame % 524288 == 0 && cancelled.load(Ordering::Acquire) {
            return Err("Preview preparation cancelled".into());
        }
        let time = frame as f64 / f64::from(output_hz);
        let first = (time * f64::from(input_hz)) as i128 - 1;
        let x1_time = (first + 1) as f64 / f64::from(input_hz);
        let t = ((time - x1_time) / f64::from(range)) as f32;
        for channel in 0..channels {
            let point = |n: i128| {
                let index =
                    ((first + n) * channels as i128 + channel as i128).clamp(0, maximum) as usize;
                f32::from(input[index]) * scale
            };
            output.push(hermite(point(0), point(1), point(2), point(3), t));
        }
    }
    Ok(output)
}

/// Shared Main/Preview preparation. Real editor assets carry the source float
/// view; legacy in-memory fixtures and explicit compatibility callers retain
/// their old i16 interpretation.
pub fn resample_prepared(
    source: &crate::prepared::PreparedAudioSource,
    output_hz: u32,
    cancelled: &AtomicBool,
) -> Result<Vec<f32>, String> {
    if let Some(pcm) = source.editor_samples() {
        crate::editor_ticks::resample_float(
            pcm,
            source.channels,
            source.sample_rate_hz,
            output_hz,
            cancelled,
        )
    } else {
        resample(
            source.samples(),
            source.channels,
            source.sample_rate_hz,
            output_hz,
            cancelled,
        )
    }
}
