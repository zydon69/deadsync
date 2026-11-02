//! Bounded onset/tempo proposal on the same decoded timeline as the game.
//! This is a signal-processing estimate, not a guarantee of musical meter.
use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

const HOP_SECONDS: f64 = 0.01;
const MAX_SECONDS: usize = 30 * 60;

#[derive(Clone, Debug)]
pub struct Rhythm {
    pub duration_seconds: f64,
    pub bpm: f64,
    pub beat_offset_seconds: f64,
    pub confidence: f64,
    pub onsets: Vec<(f64, f64)>,
    /// Five-second windows: start, end, relative RMS energy, onset count.
    pub sections: Vec<(f64, f64, f64, usize)>,
}

pub fn analyze_file(path: &Path, cancelled: &AtomicBool) -> Result<Rhythm, String> {
    let opened = deadlib_audio_decode::open_file(
        path,
        deadlib_audio_decode::DecodeOptions::STEP_MANIA_TIMELINE,
    )
    .map_err(|e| e.to_string())?;
    let channels = opened.channels;
    let rate = opened.sample_rate_hz as usize;
    if channels == 0 || channels > 8 || rate == 0 || rate > 384_000 {
        return Err("Unsupported audio channel count or sample rate".into());
    }
    let hop = (rate as f64 * HOP_SECONDS).round().max(1.0) as usize;
    let mut reader = opened.reader;
    let mut packet = Vec::new();
    let mut energy = Vec::new();
    let mut accumulated = 0.0;
    let mut frames = 0usize;
    let mut total = 0usize;
    loop {
        check(cancelled)?;
        packet.clear();
        if !reader
            .read_dec_packet_into(&mut packet)
            .map_err(|e| e.to_string())?
        {
            break;
        }
        if packet.len() % channels != 0 {
            return Err("Audio decoder returned a partial frame".into());
        }
        for frame in packet.chunks_exact(channels) {
            accumulated += frame
                .iter()
                .map(|&v| {
                    let x = f64::from(v) / 32768.0;
                    x * x
                })
                .sum::<f64>()
                / channels as f64;
            frames += 1;
            total += 1;
            if frames == hop {
                energy.push((accumulated / hop as f64 * 10_000.0).ln_1p());
                frames = 0;
                accumulated = 0.0;
            }
            if total > rate * MAX_SECONDS {
                return Err("Audio analysis is limited to 30 minutes".into());
            }
        }
    }
    if energy.len() < 200 || energy.iter().copied().fold(0.0, f64::max) < 0.01 {
        return Err("Audio is silent or too short for a tempo proposal".into());
    }
    let step = hop as f64 / rate as f64;
    let flux: Vec<f64> = energy.windows(2).map(|e| (e[1] - e[0]).max(0.0)).collect();
    let mut envelope = vec![0.0; flux.len()];
    let mut onsets = Vec::new();
    for i in 1..flux.len() - 1 {
        if i % 1024 == 0 {
            check(cancelled)?;
        }
        let left = i.saturating_sub(15);
        let right = (i + 16).min(flux.len());
        let mean = flux[left..right].iter().sum::<f64>() / (right - left) as f64;
        if flux[i] > mean * 1.5 + 0.015 && flux[i] >= flux[i - 1] && flux[i] > flux[i + 1] {
            envelope[i] = flux[i];
            onsets.push(((i + 1) as f64 * step, flux[i]));
        }
    }
    if onsets.len() < 4 {
        return Err("Not enough rhythmic transients to estimate tempo".into());
    }
    let lags = tempo_lags(step, envelope.len());
    let norm = envelope
        .iter()
        .map(|v| v * v)
        .sum::<f64>()
        .max(f64::EPSILON);
    let mut scores = Vec::new();
    for lag in lags {
        check(cancelled)?;
        let correlation = envelope[lag..]
            .iter()
            .zip(&envelope[..envelope.len() - lag])
            .map(|(a, b)| a * b)
            .sum::<f64>()
            / norm;
        scores.push((lag, correlation));
    }
    let &(lag, score) = scores
        .iter()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .ok_or("No tempo candidate")?;
    let mut phases = vec![0.0; lag];
    for (i, value) in envelope.iter().enumerate() {
        phases[i % lag] += value;
    }
    let phase = phases
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map(|(i, _)| i)
        .unwrap_or(0);
    let duration = total as f64 / rate as f64;
    let window = (5.0 / step).round().max(1.0) as usize;
    let levels: Vec<f64> = energy
        .chunks(window)
        .map(|chunk| (chunk.iter().map(|e| e.exp_m1()).sum::<f64>() / chunk.len() as f64).sqrt())
        .collect();
    let peak = levels.iter().copied().fold(0.0, f64::max).max(f64::EPSILON);
    let sections = levels
        .iter()
        .enumerate()
        .map(|(i, level)| {
            let start = i as f64 * window as f64 * step;
            let end = ((i + 1) as f64 * window as f64 * step).min(duration);
            (
                start,
                end,
                level / peak,
                onsets
                    .iter()
                    .filter(|(second, _)| *second >= start && *second < end)
                    .count(),
            )
        })
        .collect();
    Ok(Rhythm {
        duration_seconds: total as f64 / rate as f64,
        bpm: 60.0 / (lag as f64 * step),
        beat_offset_seconds: (phase + 1) as f64 * step,
        confidence: score.clamp(0.0, 1.0),
        onsets,
        sections,
    })
}
fn check(cancelled: &AtomicBool) -> Result<(), String> {
    if cancelled.load(Ordering::Acquire) {
        Err("Audio analysis cancelled".into())
    } else {
        Ok(())
    }
}

fn tempo_lags(step: f64, envelope_len: usize) -> std::ops::RangeInclusive<usize> {
    let min = (60.0 / 300.0 / step).ceil() as usize;
    let max = ((60.0 / 40.0 / step).floor() as usize).min(envelope_len / 2);
    min.max(1)..=max
}
#[cfg(test)]
mod tests {
    #[test]
    fn every_tempo_candidate_stays_within_the_tool_schema_for_fractional_hops() {
        for hz in [22050., 44100., 48000., 384000.] {
            let hop = (hz * super::HOP_SECONDS).round().max(1.);
            let step = hop / hz;
            let lags = super::tempo_lags(step, 10000);
            assert!(!lags.is_empty());
            for lag in lags {
                let bpm = 60. / (lag as f64 * step);
                assert!(
                    (40.0..=300.0).contains(&bpm),
                    "hz={hz}, lag={lag}, bpm={bpm}"
                );
            }
        }
    }
}
