//! Editor-only float clip preparation. Gameplay SFX decode/resampling is unchanged.
use crate::hermite::interpolate;
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub fn prepare(
    path: &Path,
    options: crate::DecodeOptions,
    output_hz: u32,
    output_channels: usize,
    cancelled: &AtomicBool,
) -> Result<Arc<[f32]>, Box<dyn std::error::Error + Send + Sync>> {
    if output_hz == 0 || output_channels == 0 {
        return Err("Invalid editor tick output format".into());
    }
    let prepared = crate::prepared::PreparedAudioSource::load_editor(path, options, cancelled)?;
    let channels = prepared.channels;
    let hz = prepared.sample_rate_hz;
    let input = prepared
        .editor_samples()
        .ok_or("Missing editor float PCM")?;
    let source = resample_float(input, channels, hz, output_hz, cancelled)?;
    if channels == output_channels {
        return Ok(source.into());
    }
    // The host output channel layout is an explicit adaptation. Preserve source
    // channel values; mono duplicates to missing channels; other missing channels are silence.
    let count = (source.len() / channels)
        .checked_mul(output_channels)
        .ok_or("Editor tick output size overflow")?;
    let mut mapped = Vec::new();
    mapped.try_reserve_exact(count)?;
    for frame in source.chunks_exact(channels) {
        for ch in 0..output_channels {
            mapped.push(
                frame
                    .get(ch)
                    .copied()
                    .unwrap_or_else(|| if channels == 1 { frame[0] } else { 0. }),
            );
        }
    }
    Ok(mapped.into())
}

/// Exact Fumen ResampleFloat time grid, Hermite expression and channel edge
/// clamping; takes float data without any i16 round trip.
pub fn resample_float(
    input: &[f32],
    channels: usize,
    input_hz: u32,
    output_hz: u32,
    cancelled: &AtomicBool,
) -> Result<Vec<f32>, String> {
    if channels == 0 || input_hz == 0 || output_hz == 0 || input.len() % channels != 0 {
        return Err("Invalid editor tick PCM format".into());
    }
    if cancelled.load(Ordering::Acquire) {
        return Err("Editor tick preparation cancelled".into());
    }
    let frames = input.len() / channels;
    let count = (frames as f64 * (f64::from(output_hz) / f64::from(input_hz))) as usize;
    let samples = count
        .checked_mul(channels)
        .ok_or("Editor tick size overflow")?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(samples)
        .map_err(|e| e.to_string())?;
    if input.is_empty() {
        return Ok(output);
    }
    if input_hz == output_hz {
        output.extend_from_slice(input);
        return Ok(output);
    }
    let range = 1.0f32 / input_hz as f32;
    let maximum = (input.len() - channels) as i128;
    for frame in 0..count {
        if frame % 524288 == 0 && cancelled.load(Ordering::Acquire) {
            return Err("Editor tick preparation cancelled".into());
        }
        let time = frame as f64 / f64::from(output_hz);
        let first = (time * f64::from(input_hz)) as i128 - 1;
        let x1_time = (first + 1) as f64 / f64::from(input_hz);
        let t = ((time - x1_time) / f64::from(range)) as f32;
        for channel in 0..channels {
            let point = |n: i128| {
                input[((first + n) * channels as i128 + channel as i128).clamp(0, maximum) as usize]
            };
            output.push(interpolate(point(0), point(1), point(2), point(3), t));
        }
    }
    Ok(output)
}
