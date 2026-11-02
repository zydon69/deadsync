#![cfg(target_os = "linux")]

use deadlib_audio_core::{
    AudioOutputMode, CallbackClockSource, CallbackInfo, OutputBackendReady, OutputBufferMut,
    OutputTelemetryClock, OutputTimingQuality, RenderState, SfxReceiver,
    note_output_clock_fallback, publish_output_timing, publish_output_timing_quality,
    report_audio_render_callback,
};
use deadlib_platform::host_time::now_nanos;
use libloading::Library;
use log::{info, warn};
use std::ffi::c_int;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::thread::{self, JoinHandle};

const PULSE_FALLBACK_BUFFER_FRAMES: u32 = 2048;
#[repr(C)]
#[derive(Clone, Copy)]
struct PaSampleSpec {
    format: c_int,
    rate: u32,
    channels: u8,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct PaBufferAttr {
    maxlength: u32,
    tlength: u32,
    prebuf: u32,
    minreq: u32,
    fragsize: u32,
}
mod async_connection;
use async_connection::PulseConnection;
pub fn is_available() -> bool {
    async_connection::available()
}

fn load_library(names: &[&str]) -> Result<Library, String> {
    let mut last_err = None;
    for name in names {
        // SAFETY: loading a shared object is the intended `libloading` API here;
        // we keep the returned handle alive for the full lifetime of any symbols
        // resolved from it.
        match unsafe { Library::new(*name) } {
            Ok(lib) => return Ok(lib),
            Err(err) => last_err = Some(format!("{name}: {err}")),
        }
    }
    Err(last_err.unwrap_or_else(|| "no candidate library names were provided".to_string()))
}

// SAFETY: The caller must choose `T` to match the actual symbol signature exported by `lib`.
unsafe fn load_symbol<T: Copy>(lib: &Library, name: &[u8]) -> Result<T, String> {
    // SAFETY: the caller chooses `T` to match the actual symbol signature, and
    // `lib` remains alive after the copied function pointer is returned.
    unsafe { lib.get::<T>(name) }
        .map(|sym| *sym)
        .map_err(|err| {
            format!(
                "{}: {err}",
                String::from_utf8_lossy(name).trim_end_matches('\0')
            )
        })
}

pub struct PulseOutputPrep {
    device_name: String,
    sample_rate_hz: u32,
    channels: usize,
    period_frames: u32,
    buffer_frames: u32,
}

impl PulseOutputPrep {
    pub fn ready(&self) -> OutputBackendReady {
        OutputBackendReady {
            device_sample_rate: self.sample_rate_hz,
            device_channels: self.channels,
            device_name: self.device_name.clone(),
            backend_name: "pulse-shared",
            requested_output_mode: AudioOutputMode::Shared,
            fallback_from_native: false,
            timing_clock: OutputTelemetryClock::Monotonic,
            timing_quality: OutputTimingQuality::Trusted,
        }
    }
}

pub struct PulseOutputStream {
    negotiated: deadlib_audio_core::BufferNegotiation,
    stop_flag: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl PulseOutputStream {
    pub fn buffer_negotiation(&self) -> deadlib_audio_core::BufferNegotiation {
        self.negotiated
    }
    pub fn shutdown(&mut self) -> Result<(), String> {
        self.stop_flag.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .map_err(|_| "Pulse render thread panicked during shutdown".to_string())?;
        }
        Ok(())
    }
}

impl Drop for PulseOutputStream {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

pub fn prepare(
    requested_device_name: Option<String>,
    sample_rate_hz: u32,
    channels: usize,
) -> Result<PulseOutputPrep, String> {
    prepare_with_buffer(requested_device_name, sample_rate_hz, channels, None)
}

pub fn prepare_with_buffer(
    requested_device_name: Option<String>,
    sample_rate_hz: u32,
    channels: usize,
    requested_period: Option<u32>,
) -> Result<PulseOutputPrep, String> {
    if requested_period.is_some_and(|value| !(32..=8192).contains(&value)) {
        return Err("invalid Pulse buffer-frame request".into());
    }
    let sample_rate_hz = sample_rate_hz.max(1);
    let channels = channels.clamp(1, 32);
    let period_frames = requested_period.unwrap_or_else(|| suggested_period_frames(sample_rate_hz));
    let buffer_frames = period_frames.saturating_mul(4).max(period_frames);
    let device_name = match requested_device_name {
        Some(name) if !name.is_empty() => {
            format!("PulseAudio default sink (requested '{name}' unsupported)")
        }
        _ => "PulseAudio default sink".to_string(),
    };
    Ok(PulseOutputPrep {
        device_name,
        sample_rate_hz,
        channels,
        period_frames,
        buffer_frames,
    })
}

pub fn start(
    prep: PulseOutputPrep,
    render: RenderState,
    sfx_receiver: SfxReceiver,
) -> Result<PulseOutputStream, String> {
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_thread = stop_flag.clone();
    let (ready_tx, ready_rx) = channel::<Result<deadlib_audio_core::BufferNegotiation, String>>();
    let thread = thread::Builder::new()
        .name("pulse_out".to_string())
        .spawn(move || render_thread(prep, render, sfx_receiver, stop_flag_thread, ready_tx))
        .map_err(|e| format!("failed to spawn PulseAudio render thread: {e}"))?;
    match ready_rx.recv() {
        Ok(Ok(negotiated)) => Ok(PulseOutputStream {
            negotiated,
            stop_flag,
            thread: Some(thread),
        }),
        Ok(Err(err)) => {
            let _ = thread.join();
            Err(err)
        }
        Err(_) => {
            let _ = thread.join();
            Err("PulseAudio render thread exited during startup".to_string())
        }
    }
}

#[derive(Clone, Copy)]
struct PulseTiming {
    playback_host_nanos: u64,
    latency_frames: u32,
    latency_ns: u64,
    timing_quality: OutputTimingQuality,
}

struct PulseClockHealth {
    warned_fallback: bool,
}

impl PulseClockHealth {
    #[inline(always)]
    const fn new() -> Self {
        Self {
            warned_fallback: false,
        }
    }
}

fn render_thread(
    prep: PulseOutputPrep,
    render: RenderState,
    sfx_receiver: SfxReceiver,
    stop_flag: Arc<AtomicBool>,
    ready_tx: Sender<Result<deadlib_audio_core::BufferNegotiation, String>>,
) {
    if let Err(err) = render_thread_inner(prep, render, sfx_receiver, &stop_flag, &ready_tx) {
        let _ = ready_tx.send(Err(err));
    }
}

fn render_thread_inner(
    mut prep: PulseOutputPrep,
    mut render: RenderState,
    mut sfx_receiver: SfxReceiver,
    stop_flag: &AtomicBool,
    ready_tx: &Sender<Result<deadlib_audio_core::BufferNegotiation, String>>,
) -> Result<(), String> {
    let stream = PulseConnection::open(&prep, stop_flag)?;
    prep.period_frames = stream.period_frames;
    prep.buffer_frames = stream.buffer_frames;
    let period_ns = frames_to_nanos(prep.sample_rate_hz, prep.period_frames);
    let buffer_ns = frames_to_nanos(prep.sample_rate_hz, prep.buffer_frames);
    info!("PulseAudio '{}' using shared output.", prep.device_name);
    publish_output_timing(
        prep.sample_rate_hz,
        period_ns,
        buffer_ns,
        prep.buffer_frames,
        prep.buffer_frames,
        prep.buffer_frames,
        buffer_ns,
    );
    // minreq is the negotiated client write quantum, not hardware period or
    // number of native buffers. Render exactly that verified quantum.
    let negotiated = deadlib_audio_core::BufferNegotiation {
        period_frames: Some(prep.period_frames),
        num_buffers: None,
    };
    if ready_tx.send(Ok(negotiated)).is_err() {
        return Ok(());
    }

    let mut mix = vec![0i16; prep.period_frames as usize * prep.channels];
    let mut clock_health = PulseClockHealth::new();
    while !stop_flag.load(Ordering::Relaxed) {
        let timing_before = playback_timing(&stream, prep.sample_rate_hz, &mut clock_health);
        let result = render.render(
            OutputBufferMut::I16(&mut mix),
            CallbackInfo {
                anchor_nanos: timing_before.playback_host_nanos,
                clock: CallbackClockSource::Instant,
            },
            sfx_receiver.try_iter(),
        );
        report_audio_render_callback(result, log::log_enabled!(log::Level::Trace), now_nanos);
        stream.write_i16(&mix, stop_flag)?;
        let timing_after = playback_timing(&stream, prep.sample_rate_hz, &mut clock_health);
        publish_output_timing_quality(worst_quality(
            timing_before.timing_quality,
            timing_after.timing_quality,
        ));
        publish_output_timing(
            prep.sample_rate_hz,
            period_ns,
            buffer_ns,
            prep.buffer_frames,
            timing_after.latency_frames,
            timing_after.latency_frames,
            timing_after.latency_ns,
        );
    }
    Ok(())
}

#[inline(always)]
fn playback_timing(
    stream: &PulseConnection,
    sample_rate_hz: u32,
    clock_health: &mut PulseClockHealth,
) -> PulseTiming {
    let now = now_nanos();
    let Ok(latency_ns) = stream.latency_nanos() else {
        note_output_clock_fallback(now_nanos(), log::log_enabled!(log::Level::Trace));
        if !clock_health.warned_fallback {
            warn!("PulseAudio latency query failed; falling back to host-time anchors.");
            clock_health.warned_fallback = true;
        }
        return PulseTiming {
            playback_host_nanos: now,
            latency_frames: 0,
            latency_ns: 0,
            timing_quality: OutputTimingQuality::Fallback,
        };
    };
    clock_health.warned_fallback = false;
    PulseTiming {
        playback_host_nanos: now.saturating_add(latency_ns),
        latency_frames: nanos_to_frames(sample_rate_hz, latency_ns),
        latency_ns,
        timing_quality: OutputTimingQuality::Trusted,
    }
}

#[inline(always)]
fn frames_to_bytes(frames: u32, channels: usize) -> u32 {
    let bytes = u64::from(frames)
        .saturating_mul(channels.max(1) as u64)
        .saturating_mul(std::mem::size_of::<i16>() as u64);
    bytes.min((u32::MAX - 1) as u64) as u32
}

#[inline(always)]
fn frames_to_nanos(sample_rate_hz: u32, frames: u32) -> u64 {
    if sample_rate_hz == 0 || frames == 0 {
        return 0;
    }
    (u64::from(frames) * 1_000_000_000) / u64::from(sample_rate_hz)
}

#[inline(always)]
fn nanos_to_frames(sample_rate_hz: u32, nanos: u64) -> u32 {
    if sample_rate_hz == 0 || nanos == 0 {
        return 0;
    }
    ((u128::from(sample_rate_hz) * u128::from(nanos)) / 1_000_000_000u128)
        .min((u32::MAX - 1) as u128) as u32
}

#[inline(always)]
const fn worst_quality(a: OutputTimingQuality, b: OutputTimingQuality) -> OutputTimingQuality {
    if (a as u8) >= (b as u8) { a } else { b }
}

#[inline(always)]
fn suggested_period_frames(sample_rate_hz: u32) -> u32 {
    let frames = sample_rate_hz.max(1) / 200;
    frames.clamp(128, PULSE_FALLBACK_BUFFER_FRAMES)
}
