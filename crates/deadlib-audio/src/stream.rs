//! Buffered playback of resolved audio paths on a decoder worker.
//!
//! The manager owns the output session and serializes stream commands. Decoder
//! workers reuse planar/SOLA buffers and the fixed music-block pool; the render
//! callback only consumes and recycles prepared blocks. Dropping every command
//! sender shuts down the output before the manager releases that pool.

mod editor_reader;
mod preview_fade;

pub mod lifecycle;
mod processing;
mod runtime;
mod stretch;

use deadlib_audio_core::{MusicBlockTiming, MusicBlockWriter, normalized_music_rate};
use deadlib_audio_decode as decode;
use deadlib_audio_decode::resample::{
    OUT_FRAMES_PER_CALL, PLANAR_INPUT_CAP_FRAMES, PlanarAccum, append_channel_mapped_i16,
    drop_front_samples, resampler_params,
    write_channel_mapped_i16, write_resampler_output,
};
#[cfg(windows)]
use deadlib_platform::windows_rt::{ThreadRole, boost_current_thread};
use log::{debug, error, warn};
use rubato::audioadapter_buffers::direct::SequentialSliceOfVecs;
use rubato::{Async, FixedAsync, Indexing, ResampleError, Resampler, ResamplerConstructionError};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use processing::MusicStages;
#[doc(inline)]
pub use runtime::{MusicStreamRuntime, StreamCommand, StreamManager, StreamReady, start};

/// Source start and output-time duration/fades for one playback segment.
#[derive(Clone, Copy, Debug)]
pub struct Cut {
    /// Source position in seconds; negative values prepend rate-scaled silence.
    pub start_sec: f64,
    /// Output duration in seconds, or infinity to decode until EOF.
    pub length_sec: f64,
    pub fade_in_sec: f64,
    pub fade_out_sec: f64,
}

impl Default for Cut {
    fn default() -> Self {
        Self {
            start_sec: 0.0,
            length_sec: f64::INFINITY,
            fade_in_sec: 0.0,
            fade_out_sec: 0.0,
        }
    }
}

const SILENCE_CHUNK_FRAMES: usize = 2048;
const MIN_MUSIC_RATE: f32 = 0.05;
const MAX_MUSIC_RATE: f32 = 8.0;
const MAX_PACKET_START_SNAP_SEC: f64 = 0.25;
const RESAMPLE_MAX_RELATIVE_RATIO: f64 = 64.0;
// Ignore implausibly large untrusted metadata instead of making one enormous
// speculative allocation. Eight million i16 samples cover more than a minute
// of stereo SFX at 48 kHz while keeping preload spikes bounded.
const MAX_SFX_PREALLOC_SAMPLES: usize = 8 * 1024 * 1024;
/// Threshold used everywhere to decide whether a music `rate` value is
/// "effectively 1.0". Picked to be smaller than any musically meaningful
/// rate change but well above the noise of `f32` round-tripping through
/// atomic storage. Used to gate SOLA activation, direct-audio passthrough,
/// and resampler/SOLA rebuilds, so all three stay in agreement (otherwise
/// a rate like 1.0001 could activate SOLA without triggering rebuild).
const RATE_EPS: f32 = 0.0005;

fn new_resampler(
    ratio: f64,
    max_relative_ratio: f64,
    channels: usize,
) -> Result<Async<f32>, ResamplerConstructionError> {
    Async::new_sinc(
        ratio,
        max_relative_ratio,
        &resampler_params(),
        OUT_FRAMES_PER_CALL,
        channels,
        FixedAsync::Output,
    )
}

// Both packet data and EOF padding already own planar vectors. The adapter
// borrows them directly; Indexing selects the live prefix without allocating
// per-channel slice handles, including for layouts wider than 7.1.
fn process_resampler(
    resampler: &mut Async<f32>,
    input: &[Vec<f32>],
    input_offset: usize,
    output: &mut [Vec<f32>],
) -> Result<(usize, usize), ResampleError> {
    let channels = resampler.nbr_channels();
    let input_frames = input_offset + resampler.input_frames_next();
    let input = SequentialSliceOfVecs::new(input, channels, input_frames)
        .expect("planar input holds every requested frame after the offset");
    let output_frames = resampler.output_frames_max();
    let mut output = SequentialSliceOfVecs::new_mut(output, channels, output_frames)
        .expect("resampler output allocation matches its channel and frame counts");
    let indexing = Indexing::new().input_offset(input_offset);
    resampler.process_into_buffer(&input, &mut output, Some(&indexing))
}

fn compat_lead_frames(ratio: f64) -> usize {
    // Rubato 0.16 obtained the sinc lookahead by requesting extra input before
    // its first fixed-output block. Rubato 4 emits that lookahead as leading
    // output instead, so trim the equivalent frames to preserve our alignment.
    let half_sinc = resampler_params().sinc_len / 2;
    (half_sinc.saturating_sub(1) as f64 * ratio).round() as usize
}

fn trim_resampler_lead(samples: &mut Vec<i16>, channels: usize, frames_left: &mut usize) {
    if channels == 0 || *frames_left == 0 {
        return;
    }
    let drop_frames = (*frames_left).min(samples.len() / channels);
    drop_front_samples(samples, drop_frames * channels);
    *frames_left -= drop_frames;
}

/// Sample rate and interleaved channel count accepted by the output transport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutputFormat {
    pub sample_rate_hz: u32,
    pub channels: usize,
}

/// Source options, output format and timeline generation fixed at decoder start.
#[derive(Debug)]
pub struct MusicDecodeContext {
    pub decode_options: decode::DecodeOptions,
    pub output: OutputFormat,
    pub generation: u64,
}

/// A handle to a streaming music track.
pub struct MusicStream {
    pub thread: thread::JoinHandle<MusicBlockWriter>,
    control: Arc<MusicControl>,
}
impl MusicStream {
    pub fn status_reader(&self) -> lifecycle::DecoderStatusReader { self.control.status.clone() }
}

#[derive(Clone, Copy)]
struct SeekRequest {
    follow_live_phase:bool,
    seconds: f64,
    generation: u64,
}

struct MusicControl {
    prepared: Option<Arc<decode::prepared::PreparedAudioSource>>,
    preview_phase: Option<(f64,u64)>,
    paused:AtomicBool,
    asset_epoch:u64,
    follow_live_phase:bool,
    failed: AtomicBool,
    status: lifecycle::DecoderStatusReader,
    stop_signal: AtomicBool,
    rate_bits: AtomicU32,
    preserve_pitch: AtomicBool,
    generation: AtomicU64,
    wake: DecoderWake,
    seek: Mutex<Option<SeekRequest>>,
    seek_serial: AtomicU64,
}

impl MusicControl {
    fn fail(&self, detail: &str) {
        // Publish the physical failure before reading the command generation.
        // A command racing this path either observes failed=true after installing
        // its generation, or this publisher sees that newly installed generation.
        self.failed.store(true, Ordering::SeqCst);
        self.status.publish(self.generation.load(Ordering::SeqCst), lifecycle::DecoderPhase::Failed, None, Some(detail),
        );
    }
}
impl MusicStream {
    fn wake_command(&self, generation: u64) {
        // Call only after Pending and the command's settings/generation have
        // been published. Never leave a new Pending generation on a dead worker.
        if self.control.failed.load(Ordering::SeqCst)
            || self.control.stop_signal.load(Ordering::Acquire)
            || self.thread.is_finished() {
            self.control.status.publish(generation, lifecycle::DecoderPhase::Failed, None,
                Some("Decoder worker is unavailable after failure"),
            );
        } else {
            self.control.wake.notify();
        }
    }
}

struct DecoderWake {
    sequence: AtomicU64,
    thread: OnceLock<thread::Thread>,
}

impl DecoderWake {
    const fn new() -> Self {
        Self {
            sequence: AtomicU64::new(0),
            thread: OnceLock::new(),
        }
    }

    fn register_current(&self) {
        assert!(
            self.thread.set(thread::current()).is_ok(),
            "music decoder wake handle registered more than once"
        );
    }

    fn notify(&self) {
        self.sequence.fetch_add(1, Ordering::Release);
        if let Some(thread) = self.thread.get() {
            thread.unpark();
        }
    }
}

struct MusicBackpressure {
    status:Option<lifecycle::DecoderStatusReader>,
    sample_rate_hz: u32,
    capacity_blocks: usize,
    low_blocks: usize,
    high_blocks: usize,
    transition_prefill_blocks: usize,
    generation: u64,
    transition: bool,
    await_render_drain: bool,
    transition_ready: bool,
    transition_published: usize,
}

// Keep enough already-decoded audio to cover the measured Ogg + rubato rebuild
// after a live rate change. The producer then refills in roughly 100 ms batches;
// the 700 ms pool still leaves about 200 ms of unpublished transition capacity.
const MUSIC_LOW_WATER_MS: usize = 400;
const MUSIC_REFILL_BATCH_MS: usize = 100;
const MUSIC_TRANSITION_PREFILL_MS: usize = 100;

fn duration_blocks(sample_rate_hz: u32, millis: usize) -> usize {
    (sample_rate_hz.max(1) as usize)
        .saturating_mul(millis)
        .div_ceil(1_000)
        .div_ceil(deadlib_audio_core::ring::MUSIC_BLOCK_FRAMES)
        .max(1)
}

fn block_duration(blocks: usize, sample_rate_hz: u32) -> Duration {
    let frames =
        (blocks as u128).saturating_mul(deadlib_audio_core::ring::MUSIC_BLOCK_FRAMES as u128);
    let nanos = frames
        .saturating_mul(1_000_000_000)
        .div_ceil(u128::from(sample_rate_hz.max(1)))
        .min(u128::from(u64::MAX));
    Duration::from_nanos(nanos as u64)
}

impl MusicBackpressure {
    fn new(sample_rate_hz: u32, capacity_blocks: usize, generation: u64) -> Self {
        let capacity_blocks = capacity_blocks.max(1);
        let low_blocks = duration_blocks(sample_rate_hz, MUSIC_LOW_WATER_MS)
            .min(capacity_blocks.saturating_sub(2).max(1));
        let high_blocks = low_blocks
            .saturating_add(duration_blocks(sample_rate_hz, MUSIC_REFILL_BATCH_MS))
            .min(capacity_blocks.saturating_sub(1).max(low_blocks));
        let transition_prefill_blocks =
            duration_blocks(sample_rate_hz, MUSIC_TRANSITION_PREFILL_MS)
                .min(capacity_blocks.saturating_sub(1).max(1));
        Self {
            status:None,
            sample_rate_hz,
            capacity_blocks,
            low_blocks,
            high_blocks,
            transition_prefill_blocks,
            generation,
            transition: true,
            await_render_drain: false,
            transition_ready: false,
            transition_published: 0,
        }
    }

    fn wait_before_push(
        &mut self,
        writer: &MusicBlockWriter,
        generation: u64,
        control: &MusicControl,
    ) -> bool {
        if generation != self.generation {
            self.generation = generation;
            self.transition = true;
            self.await_render_drain = true;
            self.transition_ready = false;
            self.transition_published = 0;
        }

        let outstanding = writer.outstanding_blocks();
        if self.transition
            && self.transition_ready
            && self.await_render_drain
            && writer.render_drained_generation() == generation
        {
            self.transition = false;
        }
        if self.transition {
            if outstanding < self.capacity_blocks {
                return false;
            }
            self.wait(control, generation, block_duration(1, self.sample_rate_hz));
            return true;
        }

        if outstanding < self.high_blocks {
            return false;
        }
        let blocks_to_low = outstanding.saturating_sub(self.low_blocks.saturating_add(1));
        self.wait(
            control,
            generation,
            block_duration(blocks_to_low, self.sample_rate_hz),
        );
        true
    }

    fn note_pushed(&mut self, writer: &MusicBlockWriter, generation: u64) {
        if !self.transition || generation != self.generation {
            return;
        }
        self.transition_published = self.transition_published.saturating_add(1);
        if !self.transition_ready && self.transition_published >= self.transition_prefill_blocks {
            writer.publish_generation_ready(generation);
            if let Some(status)=&self.status{status.render_ready(generation);}
            self.transition_ready = true;
            if !self.await_render_drain {
                self.transition = false;
            }
        }
    }

    /// Releases a short initial/final stream that cannot reach the normal
    /// transition prefill. A live generation transition still waits for its
    /// render-side drain acknowledgement before entering steady pacing.
    fn finish_generation(&mut self, writer: &MusicBlockWriter, generation: u64) {
        if self.transition
            && !self.transition_ready
            && self.transition_published > 0
            && generation == self.generation
        {
            writer.publish_generation_ready(generation);
            if let Some(status)=&self.status{status.render_ready(generation);}
            self.transition_ready = true;
            if !self.await_render_drain {
                self.transition = false;
            }
        }
    }

    fn wait_for_credit(&self, control: &MusicControl, generation: u64) {
        self.wait(control, generation, block_duration(1, self.sample_rate_hz));
    }

    fn wait(&self, control: &MusicControl, generation: u64, duration: Duration) {
        let sequence = control.wake.sequence.load(Ordering::Acquire);
        if control.stop_signal.load(Ordering::Acquire)
            || control.generation.load(Ordering::Acquire) != generation
        {
            return;
        }
        if control.wake.sequence.load(Ordering::Acquire) != sequence {
            return;
        }
        thread::park_timeout(duration);
    }
}

#[must_use]
/// Snaps forward to a codec packet boundary when it is within 250 milliseconds.
/// Returns the requested start when probing fails or the boundary is too far away.
pub fn snap_music_start_sec(path: &Path, start_sec: f64) -> f64 {
    let Ok(Some(snapped)) = decode::snap_start_forward_to_packet(path, start_sec) else {
        return start_sec;
    };
    if snapped < start_sec || snapped - start_sec > MAX_PACKET_START_SNAP_SEC {
        return start_sec;
    }
    if (snapped - start_sec).abs() > f64::EPSILON {
        debug!("Snapped music cut start from {start_sec:.6}s to packet boundary {snapped:.6}s.");
    }
    snapped
}

fn wait_retained_pause(control:&MusicControl){
    while control.paused.load(Ordering::Acquire)&&!control.stop_signal.load(Ordering::Acquire){
        let sequence=control.wake.sequence.load(Ordering::Acquire);
        if !control.paused.load(Ordering::Acquire){break;}
        if control.wake.sequence.load(Ordering::Acquire)==sequence{thread::park_timeout(Duration::from_millis(100));}
    }
}
fn push_music_block(
    writer: &mut MusicBlockWriter,
    block: &[i16],
    out_channels: usize,
    timing: MusicBlockTiming,
    control: &MusicControl,
    backpressure: &mut MusicBackpressure,
) -> Result<f64, Box<dyn std::error::Error + Send + Sync>> {
    push_music_block_with_preview(writer,block,out_channels,timing,None,control,backpressure,
    )
}
fn push_music_block_with_preview(
    writer: &mut MusicBlockWriter,
    block: &[i16],
    out_channels: usize,
    timing: MusicBlockTiming,
    preview_frame:Option<u64>,
    control: &MusicControl,
    backpressure: &mut MusicBackpressure,
) -> Result<f64, Box<dyn std::error::Error + Send + Sync>> {
    if block.is_empty() || out_channels == 0 {
        return Ok(timing.music_start_sec);
    }
    debug_assert_eq!(writer.channels(), out_channels);
    let mut sample_offset = 0usize;
    let mut next_music_sec = timing.music_start_sec;
    while sample_offset < block.len() {
        wait_retained_pause(control);
        if control.stop_signal.load(Ordering::Acquire) {
            return Ok(next_music_sec);
        }
        if control.generation.load(Ordering::Acquire) != timing.generation {
            let remaining_frames = (block.len() - sample_offset) / out_channels;
            return Ok(
                (remaining_frames as f64).mul_add(timing.music_sec_per_frame, next_music_sec)
            );
        }
        if backpressure.wait_before_push(writer, timing.generation, control) {
            continue;
        }
        let pushed = writer.try_push_preview(
            &block[sample_offset..],
            MusicBlockTiming {
                asset_epoch: control.asset_epoch,
generation: timing.generation,
                music_start_sec: next_music_sec,
                music_sec_per_frame: timing.music_sec_per_frame,
            },
            preview_frame.map(|frame|frame.saturating_add((sample_offset/out_channels)as u64)),
        );
        if pushed == 0 {
            backpressure.wait_for_credit(control, timing.generation);
            continue;
        }
        backpressure.note_pushed(writer, timing.generation);
        let chunk_frames = pushed / out_channels;
        sample_offset += pushed;
        next_music_sec = (chunk_frames as f64).mul_add(timing.music_sec_per_frame, next_music_sec);
        control.status.publish(timing.generation, lifecycle::DecoderPhase::Producing, Some(next_music_sec), None,
        );
    }
    Ok(next_music_sec)
}

fn push_silence(
    writer: &mut MusicBlockWriter,
    silence_frames: usize,
    out_channels: usize,
    music_start_sec: f64,
    music_sec_per_frame: f64,
    control: &MusicControl,
    backpressure: &mut MusicBackpressure,
    seek_serial: u64,
) -> Result<f64, Box<dyn std::error::Error + Send + Sync>> {
    if silence_frames == 0 || out_channels == 0 {
        return Ok(music_start_sec);
    }
    let mut next_music_sec = music_start_sec;
    let silence_chunk = vec![0i16; SILENCE_CHUNK_FRAMES * out_channels];
    let mut frames_left = silence_frames;
    while frames_left > 0 {
        let chunk_frames = frames_left.min(SILENCE_CHUNK_FRAMES);
        let chunk_samples = chunk_frames * out_channels;
        let generation = control.generation.load(Ordering::Acquire);
        if control.stop_signal.load(Ordering::Acquire)
            || control.seek_serial.load(Ordering::Acquire) != seek_serial
        {
            break;
        }
        next_music_sec = push_music_block(
            writer,
            &silence_chunk[..chunk_samples],
            out_channels,
            MusicBlockTiming {
                asset_epoch: control.asset_epoch,
generation,
                music_start_sec: next_music_sec,
                music_sec_per_frame,
            },
            control,
            backpressure,
        )?;
        frames_left -= chunk_frames;
    }
    Ok(next_music_sec)
}

#[inline(always)]
fn lead_in_silence_timing(cut_start_sec: f64, rate: f32, sample_rate_hz: u32) -> (usize, f64) {
    let rate = normalized_music_rate(rate).clamp(MIN_MUSIC_RATE, MAX_MUSIC_RATE);
    let sample_rate = f64::from(sample_rate_hz.max(1));
    let real_seconds = (-cut_start_sec).max(0.0) / f64::from(rate);
    (
        (real_seconds * sample_rate).round() as usize,
        f64::from(rate) / sample_rate,
    )
}

/// Starts a decoder worker that returns its pooled block writer when joined.
/// Use [`MusicStreamRuntime`] to coordinate cancellation and live rate changes.
pub fn spawn_music_decoder_thread(
    path: PathBuf,
    cut: Cut,
    looping: bool,
    rate: f32,
    preserve_pitch: bool,
    writer: MusicBlockWriter,
    context: MusicDecodeContext,
) -> MusicStream {
    spawn_music_decoder_thread_with_retention(
        path,
        cut,
        looping,
        rate,
        preserve_pitch,
        writer,
        context,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
fn spawn_music_decoder_thread_with_retention(
    path: PathBuf,
    cut: Cut,
    looping: bool,
    rate: f32,
    preserve_pitch: bool,
    writer: MusicBlockWriter,
    context: MusicDecodeContext,
    retain_decoder: bool,
) -> MusicStream {
    spawn_music_decoder_thread_for_asset(path,cut,looping,rate,preserve_pitch,writer,context,retain_decoder,deadlib_audio_core::music_asset_epoch(),false,
    )
}
#[allow(clippy::too_many_arguments)]
fn spawn_music_decoder_thread_for_asset(path:PathBuf,cut:Cut,looping:bool,rate:f32,preserve_pitch:bool,writer:MusicBlockWriter,context:MusicDecodeContext,retain_decoder:bool,asset_epoch:u64,follow_live_phase:bool,
)->MusicStream {
    spawn_music_decoder_thread_with_source(path,cut,looping,rate,preserve_pitch,writer,context,retain_decoder,asset_epoch,follow_live_phase,None,None,
    )
}
#[allow(clippy::too_many_arguments)]
fn spawn_music_decoder_thread_with_source(path:PathBuf,cut:Cut,looping:bool,rate:f32,preserve_pitch:bool,writer:MusicBlockWriter,context:MusicDecodeContext,retain_decoder:bool,asset_epoch:u64,follow_live_phase:bool,prepared:Option<Arc<decode::prepared::PreparedAudioSource>>,preview_phase:Option<(f64,u64)>,
)->MusicStream {
    let control = Arc::new(MusicControl {
        prepared, preview_phase,
        paused:AtomicBool::new(deadlib_audio_core::music_track_paused()),
        asset_epoch,follow_live_phase,
        failed: AtomicBool::new(false),
        status: lifecycle::DecoderStatusReader::new_asset(context.generation, cut.start_sec, asset_epoch,
        ),
        stop_signal: AtomicBool::new(false),
        rate_bits: AtomicU32::new(rate.to_bits()),
        preserve_pitch: AtomicBool::new(preserve_pitch),
        generation: AtomicU64::new(context.generation),
        wake: DecoderWake::new(),
        seek: Mutex::new(None),
        seek_serial: AtomicU64::new(0),
    });
    let decoder_control = Arc::clone(&control);

    let thread = thread::spawn(move || {
        let mut writer = writer;
        decoder_control.wake.register_current();
        // Dev/test profiles unwind and can return the writer after a decoder
        // panic. Shipped profiles abort the process per the workspace policy.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            #[cfg(windows)]
            let _thread_policy = boost_current_thread(ThreadRole::AudioDecode);
            music_decoder_thread_loop(
                path,
                cut,
                looping,
                &decoder_control,
                &mut writer,
                context,
                retain_decoder,
            )
        }));
        match result {
            Ok(Ok(())) => {
                if retain_decoder && !decoder_control.stop_signal.load(Ordering::Acquire) {
                    decoder_control.fail("Retained decoder exited unexpectedly");
                }
            }
            Ok(Err(err)) => {
                decoder_control.fail(&err.to_string());
                error!("Music decoder thread failed: {err}");
            }
            Err(_) => {
                decoder_control.fail("Decoder worker panicked");
                error!("Music decoder thread panicked; transport writer recovered.");
            }
        }
        writer
    });

    MusicStream { thread, control }
}

#[inline]
const fn seek_preroll_in_frames(seek_ok: bool, start_frame: u64, seek_start_frame: u64) -> u64 {
    if seek_ok {
        start_frame.saturating_sub(seek_start_frame)
    } else {
        0
    }
}

#[inline]
fn music_output_start_sec(
    seek_ok: bool,
    seek_start_frame: u64,
    cut_start_sec: f64,
    sample_rate_hz: u32,
) -> f64 {
    if seek_ok {
        seek_start_frame as f64 / f64::from(sample_rate_hz.max(1))
    } else {
        cut_start_sec.max(0.0)
    }
}

fn sfx_output_capacity(frames_total_hint: Option<u64>, in_hz: u32, output: OutputFormat) -> usize {
    let Some(input_frames) = frames_total_hint else {
        return 0;
    };
    if in_hz == 0 || output.sample_rate_hz == 0 || output.channels == 0 {
        return 0;
    }
    let numerator = u128::from(input_frames) * u128::from(output.sample_rate_hz);
    let output_frames = numerator.div_ceil(u128::from(in_hz));
    let samples = output_frames.saturating_mul(output.channels as u128);
    usize::try_from(samples)
        .ok()
        .filter(|&samples| samples <= MAX_SFX_PREALLOC_SAMPLES)
        .unwrap_or(0)
}

fn live_asset_music_seconds(hz:u32)->Option<f64>{
    let elapsed=deadlib_audio_core::music_total_frames().saturating_sub(deadlib_audio_core::music_track_start_frame())as f64/f64::from(hz.max(1));
    deadlib_audio_core::seeded_music_position(elapsed).map(|(seconds,_)|seconds)
}
#[allow(clippy::too_many_arguments)]
fn music_decoder_thread_loop(
    path: PathBuf,
    cut: Cut,
    looping: bool,
    control: &MusicControl,
    writer: &mut MusicBlockWriter,
    context: MusicDecodeContext,
    retain_decoder: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let MusicControl {
        prepared: _, preview_phase: _,
        paused: _,
        asset_epoch: _,follow_live_phase: _,
        failed: _,
        status: _,
        stop_signal: stop,
        rate_bits,
        preserve_pitch,
        generation,
        wake: _,
        seek: _,
        seek_serial: _,
    } = control;
    #[cfg(test)] if control.follow_live_phase{wait_asset_open_gate(control.asset_epoch); }
    let opened = editor_reader::EditorDecodeReader::open(&path, context.decode_options, control.prepared.as_ref(),
    )?;
    control.status.opened_codec(context.generation, opened.reader.codec(), &path);
    let mut reader = opened.reader;
    let in_ch = opened.channels;
    let in_hz = opened.sample_rate_hz;

    let out_ch = context.output.channels;
    let out_hz = context.output.sample_rate_hz;
    let mut backpressure =
        MusicBackpressure::new(out_hz, writer.capacity_blocks(), context.generation);
    backpressure.status=Some(control.status.clone());
    let is_ogg_stream = reader.is_streaming_vorbis();

    debug!(
        "Music decode start: {:?} ({} ch @ {} Hz) -> output {} ch @ {} Hz (rate x{}, preserve_pitch={}).",
        path,
        in_ch,
        in_hz,
        out_ch,
        out_hz,
        f32::from_bits(rate_bits.load(Ordering::Relaxed)),
        preserve_pitch.load(Ordering::Relaxed),
    );

    let mut stages = MusicStages::new(in_ch, in_hz, out_hz);
    let mut out_tmp = Vec::with_capacity(OUT_FRAMES_PER_CALL * out_ch);
    let mut pkt_buf = Vec::new();

    let loop_cut = cut;
    let mut cut = cut;
    let mut restore_preview_loop = false;
    if let Some((phase,frame))=control.preview_phase {
        let length=if cut.length_sec.is_finite(){cut.length_sec}else{control.prepared.as_ref().map_or(0.,|source| {
                source.frames()as f64/f64::from(source.sample_rate_hz)
            })};
        if !length.is_finite()||length<=0.{return Err("Preview source has no playable frames".into());}
        let elapsed=deadlib_audio_core::music_total_frames().saturating_sub(frame)as f64/f64::from(context.output.sample_rate_hz.max(1));
        let offset=(phase+elapsed-cut.start_sec).rem_euclid(length);
        cut.start_sec+=offset;cut.length_sec=length-offset;
        restore_preview_loop=true;
    }
    if control.follow_live_phase {
        let stream=deadlib_audio_core::music_total_frames().saturating_sub(deadlib_audio_core::music_track_start_frame()) as f64 / f64::from(context.output.sample_rate_hz.max(1));
        if let Some((now,_))=deadlib_audio_core::seeded_music_position(stream){cut.start_sec=now;}
    }
    let mut applied_seek = 0;
    let mut lead_pending = true;
    let mut output_generation;
    'main_loop: loop {
        wait_retained_pause(control);
        if stop.load(Ordering::Acquire) {
            break;
        }
        let request = control
            .seek
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        let repositioned = if let Some(request) = request {
            cut.start_sec = if request.follow_live_phase{live_asset_music_seconds(context.output.sample_rate_hz).unwrap_or(request.seconds)}else{request.seconds};
            applied_seek = request.generation;
            lead_pending = true;
            true
        } else {
            false
        };
        // Load the generation before the settings it tags. Observing a newly
        // published generation therefore also observes the preceding setting
        // stores; new settings paired with an old generation are discarded.
        output_generation = generation.load(Ordering::Acquire);
        if control.seek_serial.load(Ordering::Acquire) != applied_seek {
            continue 'main_loop;
        }
        let mut current_rate_f32 = f32::from_bits(rate_bits.load(Ordering::Acquire));
        if !current_rate_f32.is_finite() || current_rate_f32 <= 0.0 {
            current_rate_f32 = 1.0;
        } else {
            current_rate_f32 = current_rate_f32.clamp(MIN_MUSIC_RATE, MAX_MUSIC_RATE);
        }
        let mut current_pp = preserve_pitch.load(Ordering::Acquire);
        if lead_pending && cut.start_sec < 0.0 {
            let rate = f32::from_bits(rate_bits.load(Ordering::Relaxed));
            let (silence_frames, music_sec_per_frame) =
                lead_in_silence_timing(cut.start_sec, rate, out_hz);
            if silence_frames > 0 {
                let _ = push_silence(
                    writer,
                    silence_frames,
                    out_ch,
                    cut.start_sec,
                    music_sec_per_frame,
                    control,
                    &mut backpressure,
                    applied_seek,
                )?;
            }
        }

        // A normal loop repeats source audio, not the initial silent lead-in.
        // An explicit new seek re-arms that lead-in above.
        lead_pending = false;
        stages.set_rate(current_rate_f32, current_pp)?;
        stages.reset();
        // SOLA emits its initial window at source spacing, so seek preroll
        // only scales with sample-rate conversion while stretching is active.
        let output_ratio = f64::from(out_hz)
            / f64::from(in_hz)
            / if current_pp && (current_rate_f32 - 1.0).abs() > RATE_EPS {
                1.0
            } else {
                f64::from(current_rate_f32)
            };

        let start_frame_f = (cut.start_sec * f64::from(in_hz)).max(0.0);
        let start_floor = start_frame_f.floor() as u64;
        // Lewton can fail on seeks that land inside the first few OGG pages.
        // Decoding and dropping <1s from the start is cheap and avoids those
        // false preview failures.
        let bypass_seek = is_ogg_stream && start_floor < u64::from(in_hz);
        if repositioned && (start_floor == 0 || bypass_seek) {
            // The reader is retained; rewind even when the requested seek would
            // normally use linear decoding from a freshly opened reader.
            if let Err(error) = reader.seek_frame(0) {
                if looping {
                    // A preview owns this opened asset for every iteration.
                    // Reopening the pathname could silently substitute a new file.
                    return Err(error.into());
                }
                warn!("Music rewind failed for {path:?}; reopening: {error}");
                reader = editor_reader::EditorDecodeReader::open(&path, context.decode_options, control.prepared.as_ref(),
                )?.reader;
            }
        }
        let mut seek_ok = false;
        let mut seek_start_frame = 0u64;
        if start_floor > 0 && !bypass_seek {
            let seek_frame =
                start_floor.saturating_sub(deadlib_audio_core::ring::PREROLL_IN_FRAMES);
            match reader.seek_frame(seek_frame) {
                Ok(()) => {
                    seek_ok = true;
                    seek_start_frame = reader.current_frame();
                }
                Err(e) => {
                    warn!(
                        "Music seek failed for {path:?} at frame {seek_frame}; restarting from start: {e}"
                    );
                    if looping {
                        // Linear decode from the same asset is a safe fallback
                        // when its codec cannot seek directly to the cut.
                        reader.seek_frame(0)?;
                    } else {
                        let reopened = editor_reader::EditorDecodeReader::open(&path, context.decode_options, control.prepared.as_ref(),
                        )?;
                        debug_assert_eq!(reopened.channels, in_ch);
                        debug_assert_eq!(reopened.sample_rate_hz, in_hz);
                        reader = reopened.reader;
                    }
                }
            }
        }

    control.status.opened_codec(output_generation, reader.codec(), &path);
        let preroll_in_frames = seek_preroll_in_frames(seek_ok, start_floor, seek_start_frame);
        let mut preroll_out_frames: u64 = if preroll_in_frames > 0 {
            (preroll_in_frames as f64 * output_ratio).ceil() as u64
        } else {
            0
        };
        let mut to_drop_in: u64 = if seek_ok { 0 } else { start_floor };
        let fade_in_frames = preview_fade::frames(cut.fade_in_sec, out_hz);
        let fade_out_frames = preview_fade::frames(cut.fade_out_sec, out_hz);
        let mut frames_left_out: Option<u64> = if cut.length_sec.is_finite() {
            Some(if looping { preview_fade::frames(cut.length_sec, out_hz) } else { (cut.length_sec * f64::from(out_hz)).round().max(0.0) as u64 })
        } else if looping {
            // Dedicated source preview uses the entire immutable PCM cache.
            // It still has a real endpoint for the outgoing envelope.
            control.prepared.as_ref().map(|source| {
                let duration=(source.frames() as f64/f64::from(in_hz)-cut.start_sec.max(0.)).max(0.)/f64::from(current_rate_f32);
                preview_fade::frames(duration,out_hz)
            })
        } else { None };
        let preview_fade_offset=if restore_preview_loop{
            preview_fade::frames(cut.start_sec,out_hz).saturating_sub(preview_fade::frames(loop_cut.start_sec,out_hz))
        }else{0};
        if restore_preview_loop {
            let full_length=if loop_cut.length_sec.is_finite(){Some(preview_fade::frames(loop_cut.length_sec,out_hz))}else{control.prepared.as_ref().map(|source| {
                    preview_fade::frames(source.frames() as f64/f64::from(in_hz),out_hz)
                })};
            if let Some(full_length)=full_length{frames_left_out=Some(full_length.saturating_sub(preview_fade_offset));}
        }
        let total_frames_target = frames_left_out.map(|frames|frames.saturating_add(preview_fade_offset));
        let dynamic_preview=control.prepared.is_some() && !control.follow_live_phase && deadlib_audio_core::editor_preview_envelope_enabled(control.asset_epoch);
        let fade_spec = if dynamic_preview {None} else {preview_fade::PreviewFade::new(fade_in_frames, fade_out_frames, total_frames_target)};

        let mut frames_emitted_total: u64 = preview_fade_offset;
        let mut next_music_output_sec =
            music_output_start_sec(seek_ok, seek_start_frame, cut.start_sec, in_hz);

        #[inline(always)]
        fn cap_out_frames(
            out_tmp: &mut Vec<i16>,
            out_ch: usize,
            frames_left_out: &mut Option<u64>,
        ) -> bool {
            if let Some(left) = frames_left_out {
                let frames = out_tmp.len() / out_ch;
                if *left == 0 {
                    out_tmp.clear();
                    return true;
                }
                if (frames as u64) > *left {
                    out_tmp.truncate((*left as usize) * out_ch);
                    *left = 0;
                    return true;
                }
                *left -= frames as u64;
            }
            false
        }

        out_tmp.clear();
        pkt_buf.clear();

        loop {
            if stop.load(Ordering::Acquire) {
                break 'main_loop;
            }
            if control.seek_serial.load(Ordering::Acquire) != applied_seek {
                continue 'main_loop;
            }
            let eof = !reader.read_dec_packet_into(&mut pkt_buf)?;
            if eof {
                pkt_buf.clear();
                stages.finish();
            } else if pkt_buf.is_empty() {
                continue;
            }
            let mut slice = &pkt_buf[..];
            if !eof && to_drop_in > 0 {
                let pkt_frames = (pkt_buf.len() / in_ch) as u64;
                if to_drop_in >= pkt_frames {
                    to_drop_in -= pkt_frames;
                    continue;
                }
                let drop_samples = (to_drop_in as usize) * in_ch;
                slice = &pkt_buf[drop_samples..];
                to_drop_in = 0;
            }
            if !eof {
                // Load the generation before the settings it tags — see note above.
                output_generation = generation.load(Ordering::Acquire);
                // A seek publishes its serial before its generation. Observing
                // that generation can never relabel old PCM as the new position.
                if control.seek_serial.load(Ordering::Acquire) != applied_seek {
                    continue 'main_loop;
                }
                let desired_rate = f32::from_bits(rate_bits.load(Ordering::Acquire));
                let mut desired_rate = if desired_rate.is_finite() && desired_rate > 0.0 {
                    desired_rate
                } else {
                    1.0
                };
                desired_rate = desired_rate.clamp(MIN_MUSIC_RATE, MAX_MUSIC_RATE);
                let desired_pp = preserve_pitch.load(Ordering::Acquire);
                if (desired_rate - current_rate_f32).abs() > RATE_EPS || desired_pp != current_pp {
                    current_rate_f32 = desired_rate;
                    current_pp = desired_pp;
                    stages.set_rate(current_rate_f32, current_pp)?;
                }
            }
            if stages.is_direct() {
                if eof {
                    break;
                }
                let music_sec_per_frame = 1.0 / f64::from(out_hz.max(1));
                let mut direct = slice;
                if preroll_out_frames > 0 {
                    let frames = direct.len() / in_ch;
                    let drop_frames = (preroll_out_frames as usize).min(frames);
                    let drop_samples = drop_frames * in_ch;
                    if drop_samples > 0 {
                        direct = &direct[drop_samples..];
                        preroll_out_frames = preroll_out_frames.saturating_sub(drop_frames as u64);
                        next_music_output_sec = (drop_frames as f64)
                            .mul_add(music_sec_per_frame, next_music_output_sec);
                    }
                }
                let mut finished = false;
                let frames = direct.len() / in_ch;
                if let Some(left) = &mut frames_left_out {
                    if *left == 0 {
                        direct = &[];
                        finished = true;
                    } else if (frames as u64) > *left {
                        direct = &direct[..(*left as usize) * in_ch];
                        *left = 0;
                        finished = true;
                    } else {
                        *left -= frames as u64;
                    }
                }
                if !direct.is_empty() {
                    let frames = (direct.len() / in_ch) as u64;
                    if in_ch == out_ch && fade_spec.is_none() {
                        frames_emitted_total = frames_emitted_total.saturating_add(frames);
                        next_music_output_sec = push_music_block_with_preview(
                            writer,
                            direct,
                            out_ch,
                            MusicBlockTiming {
                                asset_epoch: control.asset_epoch,
generation: output_generation,
                                music_start_sec: next_music_output_sec,
                                music_sec_per_frame,
                            },
                            dynamic_preview.then_some(frames_emitted_total.saturating_sub(frames)),
                            control,
                            &mut backpressure,
                        )?;
                    } else {
                        if in_ch == out_ch {
                            out_tmp.clear();
                            out_tmp.extend_from_slice(direct);
                        } else {
                            write_channel_mapped_i16(direct, in_ch, out_ch, &mut out_tmp);
                        }
                        if let Some(fade) = fade_spec {
                            fade.apply(&mut out_tmp, out_ch, frames_emitted_total);
                        }
                        frames_emitted_total = frames_emitted_total.saturating_add(frames);
                        next_music_output_sec = push_music_block_with_preview(
                            writer,
                            &out_tmp,
                            out_ch,
                            MusicBlockTiming {
                                asset_epoch: control.asset_epoch,
generation: output_generation,
                                music_start_sec: next_music_output_sec,
                                music_sec_per_frame,
                            },
                            dynamic_preview.then_some(frames_emitted_total.saturating_sub(frames)),
                            control,
                            &mut backpressure,
                        )?;
                    }
                }
                if finished {
                    break;
                }
                continue;
            }
            if !eof {
                stages.push(slice);
            }
            while let Some(music_sec_per_frame) = stages.pull(&mut out_tmp, out_ch)? {
                let produced_frames = out_tmp.len() / out_ch;
                if preroll_out_frames > 0 {
                    let drop_frames = (preroll_out_frames as usize).min(produced_frames);
                    let drop_samples = drop_frames * out_ch;
                    if drop_samples > 0 {
                        drop_front_samples(&mut out_tmp, drop_samples);
                        preroll_out_frames = preroll_out_frames.saturating_sub(drop_frames as u64);
                        next_music_output_sec = (drop_frames as f64)
                            .mul_add(music_sec_per_frame, next_music_output_sec);
                    }
                }
                let finished = cap_out_frames(&mut out_tmp, out_ch, &mut frames_left_out);
                if !out_tmp.is_empty() {
                    if let Some(fade) = fade_spec {
                        fade.apply(&mut out_tmp, out_ch, frames_emitted_total);
                    }
                    frames_emitted_total =
                        frames_emitted_total.saturating_add((out_tmp.len() / out_ch) as u64);
                    next_music_output_sec = push_music_block_with_preview(
                        writer,
                        &out_tmp,
                        out_ch,
                        MusicBlockTiming {
                            asset_epoch: control.asset_epoch,
generation: output_generation,
                            music_start_sec: next_music_output_sec,
                            music_sec_per_frame,
                        },
                        dynamic_preview.then_some(frames_emitted_total.saturating_sub((out_tmp.len()/out_ch)as u64),
                        ),
                        control,
                        &mut backpressure,
                    )?;
                }
                if finished {
                    break;
                }
            }
            if eof || matches!(frames_left_out, Some(0)) {
                break;
            }
        }

        if stop.load(Ordering::Acquire) {
            break 'main_loop;
        }
        if control.seek_serial.load(Ordering::Acquire) != applied_seek {
            continue 'main_loop;
        }
        if !looping {
            backpressure.finish_generation(writer, output_generation);
            control.status.publish(output_generation, lifecycle::DecoderPhase::Exhausted, None, None,
            );
            if !retain_decoder {
                break 'main_loop;
            }
            // EOF is not cancellation: keep the open decoder available for a
            // later seek, without polling or retaining unrecycled audio blocks.
            loop {
                let sequence = control.wake.sequence.load(Ordering::Acquire);
                let latest_generation = generation.load(Ordering::Acquire);
                if control.seek_serial.load(Ordering::Acquire) == applied_seek {
                    control.status.publish(latest_generation, lifecycle::DecoderPhase::Exhausted, None, None,
                    );
                }
                if stop.load(Ordering::Acquire) {
                    break 'main_loop;
                }
                if control.seek_serial.load(Ordering::Acquire) != applied_seek {
                    continue 'main_loop;
                }
                if control.wake.sequence.load(Ordering::Acquire) == sequence {
                    thread::park();
                }
            }
        }
        // Rewind the retained asset, including zero cuts and OGG cuts below
        // one second which intentionally bypass direct seeking above. The next
        // iteration resets DSP stages and reapplies the cut/preroll normally.
        // A failed rewind is explicit: never replace this asset from its path.
        reader.seek_frame(0)?;
        if restore_preview_loop {cut=loop_cut;restore_preview_loop=false;}
        debug!("Looping music: rewound retained asset {path:?}");
    }
    if !stop.load(Ordering::Acquire) {
        backpressure.finish_generation(writer, generation.load(Ordering::Acquire));
    }
    Ok(())
}

/// Decodes a resolved sound into retained samples matching the output format.
///
/// Call during preparation: this function performs I/O and allocates buffers.
/// Returns an error if decoding or resampling fails.
///
/// # Panics
///
/// Panics if an internal state invariant is violated.
pub fn load_and_resample_sfx(
    path: &Path,
    output: OutputFormat,
    options: decode::DecodeOptions,
) -> Result<Arc<[i16]>, Box<dyn std::error::Error + Send + Sync>> {
    let opened = decode::open_file(path, options)?;
    let mut reader = opened.reader;
    let in_ch = opened.channels;
    let in_hz = opened.sample_rate_hz;
    let frames_total_hint = opened.frames_total_hint;
    let out_ch = output.channels;
    let out_hz = output.sample_rate_hz;
    let output_capacity = sfx_output_capacity(frames_total_hint, in_hz, output);

    if in_hz == out_hz {
        let mut pkt_buf = Vec::new();
        let mut decoded_data = Vec::with_capacity(output_capacity);
        while reader.read_dec_packet_into(&mut pkt_buf)? {
            if !pkt_buf.is_empty() {
                if in_ch == out_ch {
                    decoded_data.extend_from_slice(&pkt_buf);
                } else {
                    append_channel_mapped_i16(&pkt_buf, in_ch, out_ch, &mut decoded_data);
                }
            }
        }
        return Ok(Arc::from(decoded_data));
    }

    let ratio = f64::from(out_hz) / f64::from(in_hz);
    let mut resampler = new_resampler(ratio, 1.0, in_ch)?;

    let mut in_planar = PlanarAccum::new(in_ch, PLANAR_INPUT_CAP_FRAMES);
    let mut resample_out = vec![vec![0.0; resampler.output_frames_max()]; in_ch];
    let mut out_tmp = Vec::with_capacity(OUT_FRAMES_PER_CALL * out_ch);
    let mut pkt_buf = Vec::new();
    let mut resampled_data = Vec::with_capacity(output_capacity);
    let mut resampler_lead_frames = compat_lead_frames(ratio);

    while reader.read_dec_packet_into(&mut pkt_buf)? {
        if pkt_buf.is_empty() {
            continue;
        }
        in_planar.push_i16_interleaved(&pkt_buf, in_ch);
        loop {
            let need = resampler.input_frames_next();
            if in_planar.available_frames() < need {
                break;
            }
            let produced_frames = process_resampler(
                &mut resampler,
                &in_planar.channels,
                in_planar.start_frame,
                &mut resample_out,
            )?
            .1;
            in_planar.consume_frames(need);
            if produced_frames == 0 {
                break;
            }
            write_resampler_output(&resample_out, produced_frames, out_ch, &mut out_tmp);
            trim_resampler_lead(&mut out_tmp, out_ch, &mut resampler_lead_frames);
            resampled_data.extend_from_slice(&out_tmp);
        }
    }

    if !in_planar.is_empty() {
        let need = resampler.input_frames_next();
        let start = in_planar.start_frame;
        for channel in &mut in_planar.channels {
            channel.resize(start + need, 0.0);
        }
        let produced_frames = process_resampler(
            &mut resampler,
            &in_planar.channels,
            start,
            &mut resample_out,
        )?
        .1;
        if produced_frames > 0 {
            write_resampler_output(&resample_out, produced_frames, out_ch, &mut out_tmp);
            trim_resampler_lead(&mut out_tmp, out_ch, &mut resampler_lead_frames);
            resampled_data.extend_from_slice(&out_tmp);
        }
        in_planar.clear();
    }

    let need = resampler.input_frames_next();
    for channel in &mut in_planar.channels {
        channel.resize(need, 0.0);
    }
    let produced_frames =
        process_resampler(&mut resampler, &in_planar.channels, 0, &mut resample_out)?.1;
    if produced_frames > 0 {
        write_resampler_output(&resample_out, produced_frames, out_ch, &mut out_tmp);
        trim_resampler_lead(&mut out_tmp, out_ch, &mut resampler_lead_frames);
        resampled_data.extend_from_slice(&out_tmp);
    }
    Ok(Arc::from(resampled_data))
}

#[cfg(test)]
mod tests {
    use super::{
        DecoderWake, MAX_SFX_PREALLOC_SAMPLES, MusicBackpressure, MusicControl, OutputFormat,
        lead_in_silence_timing, lifecycle, music_output_start_sec, push_music_block, seek_preroll_in_frames,
        sfx_output_capacity,
    };
    use deadlib_audio_core::{MusicBlockTiming, music_transport};
    use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    const SAMPLE_RATE: u32 = 48_000;

    fn control(generation: u64) -> MusicControl {
        MusicControl {
            prepared: None,
            preview_phase: None,
            paused:AtomicBool::new(false),
            asset_epoch:deadlib_audio_core::music_asset_epoch(),follow_live_phase:false,
            failed: AtomicBool::new(false),
            status: lifecycle::DecoderStatusReader::new(generation, 0.0),
            stop_signal: AtomicBool::new(false),
            rate_bits: AtomicU32::new(1.0f32.to_bits()),
            preserve_pitch: AtomicBool::new(false),
            generation: AtomicU64::new(generation),
            wake: DecoderWake::new(),
            seek: Mutex::new(None),
            seek_serial: AtomicU64::new(0),
        }
    }

    #[test]
    fn lifecycle_failure_racing_a_new_command_never_leaves_pending() {
        let _audio_guard = crate::test_audio_guard();
        use std::sync::atomic::Ordering;
        use std::sync::{Arc, Barrier};
        for _ in 0..64 {
            let control = Arc::new(control(1));
            let barrier = Arc::new(Barrier::new(2));
            let worker_control = control.clone();
            let worker_barrier = barrier.clone();
            let (transport, _render) = music_transport(SAMPLE_RATE, 2);
            let thread = std::thread::spawn(move || {
                worker_barrier.wait();
                worker_control.fail("decoder failed concurrently");
                transport.writer
            });
            let stream = super::MusicStream { thread, control };
            barrier.wait();
            stream.control.status.begin(2, Some(10.0));
            stream.control.generation.store(2, Ordering::SeqCst);
            stream.wake_command(2);
            let status = stream.status_reader();
            let _writer = stream.thread.join().unwrap();
            let snapshot = status.snapshot();
            assert_eq!(snapshot.command_generation, 2);
            assert_eq!(snapshot.phase, lifecycle::DecoderPhase::Failed);
        }
    }

    #[test]
    fn seeked_map_starts_at_decoder_frame() {
        let _audio_guard = crate::test_audio_guard();
        let sec = music_output_start_sec(true, 44_092, 1.0, 44_100);

        assert!((sec - (44_092.0 / 44_100.0)).abs() <= 1e-12);
    }

    #[test]
    fn preroll_drop_uses_actual_seek_distance() {
        let _audio_guard = crate::test_audio_guard();
        assert_eq!(seek_preroll_in_frames(true, 44_100, 44_092), 8);
        assert_eq!(seek_preroll_in_frames(true, 44_100, 44_120), 0);
        assert_eq!(seek_preroll_in_frames(false, 44_100, 0), 0);
    }

    #[test]
    fn negative_lead_in_silence_scales_with_music_rate() {
        let _audio_guard = crate::test_audio_guard();
        let (frames, music_sec_per_frame) = lead_in_silence_timing(-9.0, 1.5, SAMPLE_RATE);

        assert_eq!(frames, 6 * SAMPLE_RATE as usize);
        assert!((music_sec_per_frame - 1.5 / f64::from(SAMPLE_RATE)).abs() <= f64::EPSILON);
        assert!((frames as f64).mul_add(music_sec_per_frame, -9.0).abs() <= 1e-12);
    }

    #[test]
    fn sfx_capacity_scales_frame_hint_to_output_format() {
        let _audio_guard = crate::test_audio_guard();
        assert_eq!(
            sfx_output_capacity(
                Some(44_100),
                44_100,
                OutputFormat {
                    sample_rate_hz: 48_000,
                    channels: 2,
                }
            ),
            96_000
        );
        assert_eq!(
            sfx_output_capacity(
                Some(1),
                44_100,
                OutputFormat {
                    sample_rate_hz: 48_000,
                    channels: 2,
                }
            ),
            4
        );
    }

    #[test]
    fn sfx_capacity_rejects_missing_or_unbounded_hints() {
        let _audio_guard = crate::test_audio_guard();
        let output = OutputFormat {
            sample_rate_hz: 48_000,
            channels: 2,
        };
        assert_eq!(sfx_output_capacity(None, 44_100, output), 0);
        assert_eq!(
            sfx_output_capacity(Some(MAX_SFX_PREALLOC_SAMPLES as u64), 1, output),
            0
        );
    }

    #[test]
    fn generation_change_cancels_a_backpressured_block() {
        let _audio_guard = crate::test_audio_guard();
        const CHANNELS: usize = 2;
        const GENERATION: u64 = 7;
        const SEC_PER_FRAME: f64 = 0.125;
        const TIMEOUT: Duration = Duration::from_secs(1);

        let (mut stream, _render) = music_transport(SAMPLE_RATE, CHANNELS);
        let full_block = vec![1; 256 * CHANNELS];
        let timing = MusicBlockTiming {
            asset_epoch: deadlib_audio_core::music_asset_epoch(),
generation: GENERATION,
            music_start_sec: 0.0,
            music_sec_per_frame: SEC_PER_FRAME,
        };
        let mut filled = 0;
        while stream.writer.try_push(&full_block, timing) != 0 {
            filled += 1;
        }
        assert!(filled > 0);

        let control = Arc::new(control(GENERATION));
        let entered = Arc::new(AtomicBool::new(false));
        let control_thread = Arc::clone(&control);
        let entered_thread = entered.clone();
        let mut writer = stream.writer;
        let thread = std::thread::spawn(move || {
            control_thread.wake.register_current();
            let mut backpressure =
                MusicBackpressure::new(SAMPLE_RATE, writer.capacity_blocks(), GENERATION);
            entered_thread.store(true, Ordering::Release);
            let result = push_music_block(
                &mut writer,
                &[7, 8],
                CHANNELS,
                MusicBlockTiming {
                    asset_epoch: deadlib_audio_core::music_asset_epoch(),
generation: GENERATION,
                    music_start_sec: 3.25,
                    music_sec_per_frame: SEC_PER_FRAME,
                },
                &control_thread,
                &mut backpressure,
            );
            (writer, result)
        });

        while !entered.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
        std::thread::sleep(Duration::from_millis(2));
        control.generation.store(GENERATION + 1, Ordering::Release);
        control.wake.notify();
        let deadline = Instant::now() + TIMEOUT;
        while !thread.is_finished() && Instant::now() < deadline {
            std::thread::yield_now();
        }
        if !thread.is_finished() {
            control.stop_signal.store(true, Ordering::Release);
            control.wake.notify();
        }
        let (_writer, result) = thread.join().expect("producer did not panic");
        assert!(
            Instant::now() < deadline,
            "generation reset did not unblock producer"
        );
        let next_sec = result.expect("canceling a stale block succeeds");
        assert_eq!(next_sec.to_bits(), (3.25 + SEC_PER_FRAME).to_bits());
    }

    #[test]
    fn generation_transition_can_use_reserved_pool_blocks() {
        let _audio_guard = crate::test_audio_guard();
        const CHANNELS: usize = 2;
        const OLD_GENERATION: u64 = 11;
        const NEW_GENERATION: u64 = 12;
        let (mut stream, _render) = music_transport(SAMPLE_RATE, CHANNELS);
        let mut backpressure =
            MusicBackpressure::new(SAMPLE_RATE, stream.writer.capacity_blocks(), OLD_GENERATION);
        let control = control(NEW_GENERATION);
        let samples = vec![1; deadlib_audio_core::ring::MUSIC_BLOCK_FRAMES * CHANNELS];
        let old_timing = MusicBlockTiming {
            asset_epoch: deadlib_audio_core::music_asset_epoch(),
generation: OLD_GENERATION,
            music_start_sec: 0.0,
            music_sec_per_frame: 1.0 / f64::from(SAMPLE_RATE),
        };
        for _ in 0..backpressure.high_blocks {
            assert_ne!(stream.writer.try_push(&samples, old_timing), 0);
        }
        assert_eq!(stream.writer.outstanding_blocks(), backpressure.high_blocks);

        let started = Instant::now();
        let next = push_music_block(
            &mut stream.writer,
            &samples,
            CHANNELS,
            MusicBlockTiming {
                asset_epoch: deadlib_audio_core::music_asset_epoch(),
generation: NEW_GENERATION,
                music_start_sec: 1.0,
                music_sec_per_frame: 1.0 / f64::from(SAMPLE_RATE),
            },
            &control,
            &mut backpressure,
        )
        .expect("transition push succeeds");

        assert!(started.elapsed() < Duration::from_millis(20));
        assert_eq!(
            stream.writer.outstanding_blocks(),
            backpressure.high_blocks + 1
        );
        assert!(next > 1.0);
    }

    #[test]
    fn short_initial_stream_publishes_its_partial_prefill_at_eof() {
        let _audio_guard = crate::test_audio_guard();
        const CHANNELS: usize = 2;
        const GENERATION: u64 = 21;
        let (mut stream, _render) = music_transport(SAMPLE_RATE, CHANNELS);
        let control = control(GENERATION);
        let mut backpressure =
            MusicBackpressure::new(SAMPLE_RATE, stream.writer.capacity_blocks(), GENERATION);
        let samples = [1, -1];
        push_music_block(
            &mut stream.writer,
            &samples,
            CHANNELS,
            MusicBlockTiming {
                asset_epoch: deadlib_audio_core::music_asset_epoch(),
generation: GENERATION,
                music_start_sec: 0.0,
                music_sec_per_frame: 1.0 / f64::from(SAMPLE_RATE),
            },
            &control,
            &mut backpressure,
        )
        .expect("short stream block publishes");

        assert!(backpressure.transition);
        assert!(!backpressure.transition_ready);
        backpressure.finish_generation(&stream.writer, GENERATION);
        assert!(backpressure.transition_ready);
        assert!(!backpressure.transition);
    }

    #[test]
    fn live_transition_waits_for_render_drain_before_steady_pacing() {
        let _audio_guard = crate::test_audio_guard();
        const CHANNELS: usize = 2;
        const OLD_GENERATION: u64 = 31;
        const NEW_GENERATION: u64 = 32;
        let (mut stream, _render) = music_transport(SAMPLE_RATE, CHANNELS);
        let control = control(NEW_GENERATION);
        let mut backpressure =
            MusicBackpressure::new(SAMPLE_RATE, stream.writer.capacity_blocks(), OLD_GENERATION);
        let samples = vec![
            1;
            backpressure.transition_prefill_blocks
                * deadlib_audio_core::ring::MUSIC_BLOCK_FRAMES
                * CHANNELS
        ];

        push_music_block(
            &mut stream.writer,
            &samples,
            CHANNELS,
            MusicBlockTiming {
                asset_epoch: deadlib_audio_core::music_asset_epoch(),
generation: NEW_GENERATION,
                music_start_sec: 0.0,
                music_sec_per_frame: 1.0 / f64::from(SAMPLE_RATE),
            },
            &control,
            &mut backpressure,
        )
        .expect("transition prefill publishes");

        assert!(backpressure.transition_ready);
        assert!(backpressure.await_render_drain);
        assert!(backpressure.transition);
        assert!(stream.writer.outstanding_blocks() < backpressure.high_blocks);
        assert_ne!(stream.writer.render_drained_generation(), NEW_GENERATION);
    }

    #[test]
    fn watermarks_leave_at_least_190_ms_transition_reserve() {
        let _audio_guard = crate::test_audio_guard();
        for sample_rate_hz in [44_100, 48_000, 96_000] {
            let (stream, _render) = music_transport(sample_rate_hz, 8);
            let backpressure =
                MusicBackpressure::new(sample_rate_hz, stream.writer.capacity_blocks(), 1);
            let reserve_frames = (backpressure.capacity_blocks - backpressure.high_blocks)
                * deadlib_audio_core::ring::MUSIC_BLOCK_FRAMES;
            assert!(
                reserve_frames * 1_000 >= sample_rate_hz as usize * 190,
                "{sample_rate_hz} Hz reserve was only {reserve_frames} frames"
            );
        }
    }
}

#[cfg(test)]
mod perf_tests;

#[cfg(test)]
static ASSET_OPEN_GATE:std::sync::Mutex<Option<(u64,std::sync::mpsc::Sender<()>,std::sync::mpsc::Receiver<()>,
    )>,
>=std::sync::Mutex::new(None);
#[cfg(test)]
fn wait_asset_open_gate(epoch:u64){let gate={let mut gate=ASSET_OPEN_GATE.lock().unwrap();if gate.as_ref().is_some_and(|value|value.0==epoch){gate.take()}else{None}};if let Some((_,entered,release))=gate{let _=entered.send(());let _=release.recv_timeout(Duration::from_secs(5));}}
#[cfg(test)]
mod live_asset_decode_tests{
    use super::*;
    use deadlib_audio_core::{CallbackClockSource,CallbackInfo, MixControls, OutputBufferMut,RenderState,
    };
    fn wav(path:&Path,ramp:bool){let frames=96_000u32;let mut bytes=Vec::new();bytes.extend_from_slice(b"RIFF");bytes.extend_from_slice(&(36+frames*4).to_le_bytes());bytes.extend_from_slice(b"WAVEfmt ");bytes.extend_from_slice(&16u32.to_le_bytes());bytes.extend_from_slice(&1u16.to_le_bytes());bytes.extend_from_slice(&2u16.to_le_bytes());bytes.extend_from_slice(&48_000u32.to_le_bytes());bytes.extend_from_slice(&192_000u32.to_le_bytes());bytes.extend_from_slice(&4u16.to_le_bytes());bytes.extend_from_slice(&16u16.to_le_bytes());bytes.extend_from_slice(b"data");bytes.extend_from_slice(&(frames*4).to_le_bytes());for frame in 0..frames{let value=if ramp{(1000+frame/64)as i16}else{-3000i16};bytes.extend_from_slice(&value.to_le_bytes());bytes.extend_from_slice(&value.to_le_bytes());}std::fs::write(path,bytes).unwrap();}
    fn render(renderer:&mut RenderState,frames:usize)->Vec<i16>{let mut output=vec![0;frames*2];let total=deadlib_audio_core::music_total_frames();renderer.render(OutputBufferMut::I16(&mut output),CallbackInfo{anchor_nanos:1_000_000_000+total*1_000_000_000/48_000,clock:CallbackClockSource::Instant,
            },std::iter::empty(),
        );output}
    #[test]
    fn delayed_asset_open_silences_old_pcm_and_starts_new_ramp_at_current_dsp_phase(){
        let _audio_guard = crate::test_audio_guard();
        let directory=std::env::temp_dir().join(format!("deadlib-live-asset-{}",std::process::id()));std::fs::create_dir_all(&directory).unwrap();let a=directory.join("a.wav");let b=directory.join("b.wav");wav(&a,false);wav(&b,true);
        deadlib_audio_core::reset_music_stream_clock_state();deadlib_audio_core::reset_music_target_gain();let generation=deadlib_audio_core::bump_music_map_generation();deadlib_audio_core::seed_music_asset_clock(0.,1.);
        let(stream,handle)=deadlib_audio_core::music_transport(48_000,2);let mut played=stream.played_map;let mut renderer=RenderState::new(handle,Arc::new(MixControls::new()),2);let mut runtime=MusicStreamRuntime::new(stream.writer,OutputFormat{sample_rate_hz:48_000,channels:2,
            },
        );runtime.handle(StreamCommand::PlayMusic{asset_epoch:deadlib_audio_core::music_asset_epoch(),
decode_options:decode::DecodeOptions::STEP_MANIA_TIMELINE,path:a,cut:Cut::default(),looping:false,rate:1.,preserve_pitch:false,generation,
        });let deadline=std::time::Instant::now()+Duration::from_secs(3);while !runtime.decoder_status().snapshot().is_some_and(|s|s.final_music_seconds.is_some_and(|time|time>0.2)){assert!(std::time::Instant::now()<deadline);thread::yield_now();}assert!(render(&mut renderer,120).iter().any(|sample|*sample<0));let track_start=deadlib_audio_core::music_track_start_frame();
        let epoch=deadlib_audio_core::advance_music_asset_epoch();let(entered_tx,entered_rx)=std::sync::mpsc::channel();let(release_tx,release_rx)=std::sync::mpsc::channel();*ASSET_OPEN_GATE.lock().unwrap()=Some((epoch,entered_tx,release_rx));runtime.handle(StreamCommand::SwapMusicAsset{decode_options:decode::DecodeOptions::STEP_MANIA_TIMELINE,path:b,rate:1.,preserve_pitch:false,generation,asset_epoch:epoch,
        });entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        // Exactly 500ms of acknowledged callback time passes while opening.
        for _ in 0..100{assert!(render(&mut renderer,240).iter().all(|sample|*sample==0));}
        assert_eq!(deadlib_audio_core::music_map_generation(),generation);assert_eq!(deadlib_audio_core::music_track_start_frame(),track_start);release_tx.send(()).unwrap();
        let deadline=std::time::Instant::now()+Duration::from_secs(3);while !runtime.decoder_status().snapshot().is_some_and(|s| {
            s.asset_epoch==epoch&&s.final_music_seconds.is_some_and(|time|time>0.70)
        }){assert!(std::time::Instant::now()<deadline);thread::yield_now();}
        let deadline=std::time::Instant::now()+Duration::from_secs(3);let mut found=false;
        while std::time::Instant::now()<deadline{let total=deadlib_audio_core::music_total_frames();let output=render(&mut renderer,240);assert!(output.iter().all(|sample|*sample>=0));let mut mapping=None;while let Some((tag,asset,segment))=played.pop(){if tag==generation&&asset==epoch{mapping=Some(segment);}}
            if let Some(segment)=mapping{let actual_frame=(segment.music_start_sec*48_000.).round()as u64;assert!(actual_frame>=24_000,"late open rewound to focus-request time");assert!(actual_frame.abs_diff(total-track_start)<=256,"PCM phase must match current DSP time");let first=output.iter().copied().find(|v|*v>0).unwrap();let expected=1000+(actual_frame/64)as i16;assert!((first-expected).abs()<=8,"new PCM sample {first} does not match current phase {expected}");found=true;break;}thread::yield_now();}
        assert!(found,"new decoder never reached a current-phase render block");drop(runtime);let _=std::fs::remove_dir_all(directory);
    }
    #[test]
    fn retained_pause_resumes_open_wav_after_path_removal(){
        let _audio_guard = crate::test_audio_guard();
        let directory=std::env::temp_dir().join(format!("deadlib-retained-pause-{}",std::process::id()));std::fs::create_dir_all(&directory).unwrap();let path=directory.join("retained.wav");wav(&path,true);
        deadlib_audio_core::resume_music_track();deadlib_audio_core::reset_music_stream_clock_state();deadlib_audio_core::reset_music_target_gain();let generation=deadlib_audio_core::bump_music_map_generation();let epoch=deadlib_audio_core::advance_music_asset_epoch();deadlib_audio_core::seed_music_asset_clock(0.,1.);
        let(stream,handle)=deadlib_audio_core::music_transport(48_000,2);let mut played=stream.played_map;let mut renderer=RenderState::new(handle,Arc::new(MixControls::new()),2);let mut runtime=MusicStreamRuntime::new(stream.writer,OutputFormat{sample_rate_hz:48_000,channels:2,
            },
        );runtime.handle(StreamCommand::PlayMusic{asset_epoch:epoch,decode_options:decode::DecodeOptions::STEP_MANIA_TIMELINE,path:path.clone(),cut:Cut::default(),looping:false,rate:1.,preserve_pitch:false,generation,
        });
        let deadline=std::time::Instant::now()+Duration::from_secs(3);while !runtime.decoder_status().snapshot().is_some_and(|s|s.render_ready_generation==Some(generation)){assert!(std::time::Instant::now()<deadline);thread::yield_now();}assert!(render(&mut renderer,240).iter().any(|v|*v>0));while played.pop().is_some(){}
        deadlib_audio_core::pause_music_track();let paused_generation=deadlib_audio_core::bump_music_map_generation();runtime.handle(StreamCommand::PauseMusic{generation:paused_generation,asset_epoch:epoch,
        });std::fs::remove_file(&path).unwrap();for _ in 0..20{assert!(render(&mut renderer,240).iter().all(|v|*v==0));}assert_eq!(deadlib_audio_core::music_asset_epoch(),epoch);
        deadlib_audio_core::reset_music_stream_clock_state();let resumed=deadlib_audio_core::bump_music_map_generation();deadlib_audio_core::activate_music_track();deadlib_audio_core::seed_music_asset_clock(1.,1.);deadlib_audio_core::mark_music_track_started(deadlib_audio_core::music_track_start_frame());deadlib_audio_core::resume_music_track();runtime.handle(StreamCommand::ResumeMusic{seconds:1.,rate:1.,preserve_pitch:false,generation:resumed,asset_epoch:epoch,
        });
        let deadline=std::time::Instant::now()+Duration::from_secs(3);while !runtime.decoder_status().snapshot().is_some_and(|s|s.render_ready_generation==Some(resumed)){assert!(std::time::Instant::now()<deadline);thread::yield_now();}let pcm=render(&mut renderer,240);assert!(pcm.iter().all(|v|*v>=1750),"retained reader should decode ramp at frame48000 despite deleted pathname");let mut found=false;while let Some((tag,asset,map))=played.pop(){if tag==resumed{assert_eq!(asset,epoch);assert!((map.music_start_sec-1.).abs()<1./48_000.);found=true;}}assert!(found);drop(runtime);deadlib_audio_core::resume_music_track();let _=std::fs::remove_dir_all(directory);
    }
    #[test]
    fn pause_swap_resume_during_blocked_open_starts_replacement_at_current_phase(){
        let _audio_guard = crate::test_audio_guard();
        let directory=std::env::temp_dir().join(format!("deadlib-paused-asset-{}",std::process::id()));std::fs::create_dir_all(&directory).unwrap();let a=directory.join("a.wav");let b=directory.join("b.wav");wav(&a,false);wav(&b,true);deadlib_audio_core::resume_music_track();deadlib_audio_core::reset_music_stream_clock_state();deadlib_audio_core::reset_music_target_gain();let generation=deadlib_audio_core::bump_music_map_generation();let old_epoch=deadlib_audio_core::advance_music_asset_epoch();deadlib_audio_core::seed_music_asset_clock(0.,1.);
        let(stream,handle)=deadlib_audio_core::music_transport(48_000,2);let mut played=stream.played_map;let mut renderer=RenderState::new(handle,Arc::new(MixControls::new()),2);let mut runtime=MusicStreamRuntime::new(stream.writer,OutputFormat{sample_rate_hz:48_000,channels:2,
            },
        );runtime.handle(StreamCommand::PlayMusic{asset_epoch:old_epoch,decode_options:decode::DecodeOptions::STEP_MANIA_TIMELINE,path:a,cut:Cut::default(),looping:false,rate:1.,preserve_pitch:false,generation,
        });let deadline=std::time::Instant::now()+Duration::from_secs(3);while !runtime.decoder_status().snapshot().is_some_and(|s|s.render_ready_generation==Some(generation)){assert!(std::time::Instant::now()<deadline);thread::yield_now();}assert!(render(&mut renderer,240).iter().any(|v|*v<0));while played.pop().is_some(){}
        deadlib_audio_core::pause_music_track();let paused=deadlib_audio_core::bump_music_map_generation();runtime.handle(StreamCommand::PauseMusic{generation:paused,asset_epoch:old_epoch,
        });let epoch=deadlib_audio_core::advance_music_asset_epoch();let(entered_tx,entered_rx)=std::sync::mpsc::channel();let(release_tx,release_rx)=std::sync::mpsc::channel();*ASSET_OPEN_GATE.lock().unwrap()=Some((epoch,entered_tx,release_rx));runtime.handle(StreamCommand::SwapMusicAsset{decode_options:decode::DecodeOptions::STEP_MANIA_TIMELINE,path:b,rate:1.,preserve_pitch:false,generation:paused,asset_epoch:epoch,
        });entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        deadlib_audio_core::reset_music_stream_clock_state();let resumed=deadlib_audio_core::bump_music_map_generation();deadlib_audio_core::activate_music_track();deadlib_audio_core::seed_music_asset_clock(1.,1.);deadlib_audio_core::mark_music_track_started(deadlib_audio_core::music_track_start_frame());deadlib_audio_core::resume_music_track();runtime.handle(StreamCommand::ResumeMusic{seconds:1.,rate:1.,preserve_pitch:false,generation:resumed,asset_epoch:epoch,
        });for _ in 0..20{assert!(render(&mut renderer,240).iter().all(|v|*v==0));}release_tx.send(()).unwrap();let deadline=std::time::Instant::now()+Duration::from_secs(3);while !runtime.decoder_status().snapshot().is_some_and(|s|s.render_ready_generation==Some(resumed)){assert!(std::time::Instant::now()<deadline);thread::yield_now();}let pcm=render(&mut renderer,240);assert!(pcm.iter().all(|v|*v>=1825),"replacement ramp should start at1.1s after100ms pending");let mut found=false;while let Some((tag,asset,map))=played.pop(){if tag==resumed{assert_eq!(asset,epoch);assert!((map.music_start_sec-1.1).abs()<1./48_000.);found=true;}}assert!(found);drop(runtime);deadlib_audio_core::resume_music_track();let _=std::fs::remove_dir_all(directory);
    }
    #[test]
    fn stale_stop_cannot_dispose_the_current_asset_decoder() {
        let _audio_guard = crate::test_audio_guard();
        let directory=std::env::temp_dir().join(format!("deadlib-stale-stop-{}",std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path=directory.join("current.wav");wav(&path,true);
        deadlib_audio_core::resume_music_track();
        deadlib_audio_core::reset_music_stream_clock_state();
        deadlib_audio_core::reset_music_target_gain();
        let generation=deadlib_audio_core::bump_music_map_generation();
        let stale_epoch=deadlib_audio_core::advance_music_asset_epoch();
        let current_epoch=deadlib_audio_core::advance_music_asset_epoch();
        let(stream,handle)=deadlib_audio_core::music_transport(48_000,2);
        let mut renderer=RenderState::new(handle,Arc::new(MixControls::new()),2);
        let mut runtime=MusicStreamRuntime::new(stream.writer,OutputFormat{sample_rate_hz:48_000,channels:2,
            },
        );
        runtime.handle(StreamCommand::PlayMusic{asset_epoch:current_epoch,
            decode_options:decode::DecodeOptions::STEP_MANIA_TIMELINE,path,
            cut:Cut::default(),looping:false,rate:1.,preserve_pitch:false,generation,
        });
        let deadline=std::time::Instant::now()+Duration::from_secs(3);
        while !runtime.decoder_status().snapshot().is_some_and(|status| {
            status.asset_epoch==current_epoch&&status.render_ready_generation==Some(generation)
        }) {
            assert!(std::time::Instant::now()<deadline);thread::yield_now();
        }
        runtime.handle(StreamCommand::StopMusic{asset_epoch:stale_epoch,
        });
        let status=runtime.decoder_status().snapshot().unwrap();
        assert_ne!(status.phase,lifecycle::DecoderPhase::Stopped);
        assert_eq!(status.asset_epoch,current_epoch);
        assert!(deadlib_audio_core::music_track_active());
        assert!(render(&mut renderer,240).iter().any(|sample|*sample>0));
        drop(runtime);let _=std::fs::remove_dir_all(directory);
    }
    #[test]
    fn initial_source_start_blocked_open_uses_current_dsp_phase_without_open_ack(){
        let _audio_guard = crate::test_audio_guard();
        let directory=std::env::temp_dir().join(format!("deadlib-source-start-{}",std::process::id()));std::fs::create_dir_all(&directory).unwrap();let path=directory.join("initial.wav");wav(&path,true);deadlib_audio_core::resume_music_track();deadlib_audio_core::reset_music_stream_clock_state();deadlib_audio_core::reset_music_target_gain();let(stream,handle)=deadlib_audio_core::music_transport(48_000,2);let mut played=stream.played_map;let mut renderer=RenderState::new(handle,Arc::new(MixControls::new()),2);let mut runtime=MusicStreamRuntime::new(stream.writer,OutputFormat{sample_rate_hz:48_000,channels:2,
            },
        );
        let generation=deadlib_audio_core::bump_music_map_generation();let epoch=deadlib_audio_core::advance_music_asset_epoch();deadlib_audio_core::activate_music_track();deadlib_audio_core::seed_music_asset_clock(0.,1.);deadlib_audio_core::mark_music_track_started(deadlib_audio_core::music_track_start_frame());let origin=deadlib_audio_core::music_track_start_frame();let(entered_tx,entered_rx)=std::sync::mpsc::channel();let(release_tx,release_rx)=std::sync::mpsc::channel();*ASSET_OPEN_GATE.lock().unwrap()=Some((epoch,entered_tx,release_rx));runtime.handle(StreamCommand::StartMusic{path:Some(path),seconds:0.,rate:1.,preserve_pitch:false,decode_options:decode::DecodeOptions::STEP_MANIA_TIMELINE,generation,asset_epoch:epoch,
        });entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();let pending=runtime.decoder_status().snapshot().unwrap();assert_eq!(pending.phase,lifecycle::DecoderPhase::Pending);assert!(!pending.opened_successfully);assert!(pending.codec.is_none());assert_eq!(pending.render_ready_generation,None);
        for _ in 0..100{assert!(render(&mut renderer,240).iter().all(|sample|*sample==0));}assert_eq!(deadlib_audio_core::music_track_start_frame(),origin);assert!(played.pop().is_none());assert_eq!(deadlib_audio_core::seeded_music_position(0.5).unwrap().0,0.5);release_tx.send(()).unwrap();let deadline=std::time::Instant::now()+Duration::from_secs(3);while !runtime.decoder_status().snapshot().is_some_and(|s|s.render_ready_generation==Some(generation)){assert!(std::time::Instant::now()<deadline);thread::yield_now();}let actual=runtime.decoder_status().snapshot().unwrap();assert!(actual.opened_successfully);assert_eq!(actual.codec,Some(lifecycle::DecodedCodec::Wav));assert_eq!(deadlib_audio_core::music_track_start_frame(),origin);let output=render(&mut renderer,240);assert!(output.iter().all(|v|*v>=1375),"initial PCM must start at0.5s, not request0");let mut mapped=false;while let Some((tag,asset,map))=played.pop(){assert_eq!(tag,generation);assert_eq!(asset,epoch);assert!((map.music_start_sec-0.5).abs()<1./48_000.);mapped=true;}assert!(mapped);drop(runtime);let _=std::fs::remove_dir_all(directory);
    }
    fn fresh_rate_reference(path:&Path,start:f64,rate:f32,frames:usize,
    )->(Vec<i16>,Vec<deadlib_audio_core::MusicMapSeg>){
        let generation=deadlib_audio_core::music_map_generation();let epoch=deadlib_audio_core::music_asset_epoch();
        let(stream,handle)=deadlib_audio_core::music_transport(48_000,2);let mut played=stream.played_map;let mut renderer=RenderState::new(handle,Arc::new(MixControls::new()),2);let mut runtime=MusicStreamRuntime::new(stream.writer,OutputFormat{sample_rate_hz:48_000,channels:2,
            },
        );
        runtime.handle(StreamCommand::PlayMusic{asset_epoch:epoch,decode_options:decode::DecodeOptions::STEP_MANIA_TIMELINE,path:path.to_path_buf(),cut:Cut{start_sec:start,..Default::default()},looping:false,rate,preserve_pitch:false,generation,
        });
        let deadline=std::time::Instant::now()+Duration::from_secs(3);while !runtime.decoder_status().snapshot().is_some_and(|status| {
            status.render_ready_generation==Some(generation)&&matches!(status.phase,lifecycle::DecoderPhase::Producing|lifecycle::DecoderPhase::Exhausted)
        }){assert!(std::time::Instant::now()<deadline);thread::yield_now();}
        let output=render(&mut renderer,frames);let mut maps=Vec::new();while let Some((tag,asset,map))=played.pop(){if tag==generation&&asset==epoch{maps.push(map);}}
        assert!(!maps.is_empty());drop(runtime);(output,maps)
    }
    #[test]
    fn queued_swap_then_rate_never_plays_old_rate_pcm_under_replacement_authority(){
        let _audio_guard = crate::test_audio_guard();
        let directory=std::env::temp_dir().join(format!("deadlib-live-rate-asset-{}",std::process::id()));std::fs::create_dir_all(&directory).unwrap();let path=directory.join("b.wav");wav(&path,true);deadlib_audio_core::reset_music_stream_clock_state();deadlib_audio_core::reset_music_target_gain();let old_generation=deadlib_audio_core::bump_music_map_generation();deadlib_audio_core::activate_music_track();deadlib_audio_core::mark_music_track_started(deadlib_audio_core::music_track_start_frame());deadlib_audio_core::seed_music_asset_clock(0.,1.);let(stream,handle)=deadlib_audio_core::music_transport(48_000,2);let mut played=stream.played_map;let mut renderer=RenderState::new(handle,Arc::new(MixControls::new()),2);let mut runtime=MusicStreamRuntime::new(stream.writer,OutputFormat{sample_rate_hz:48_000,channels:2,
            },
        );
        let epoch=deadlib_audio_core::advance_music_asset_epoch();
        // The UI accepts Rate before the manager handles the earlier Swap.
        deadlib_audio_core::set_music_clock_rate(2.);let new_generation=deadlib_audio_core::bump_music_map_generation();runtime.handle(StreamCommand::SwapMusicAsset{decode_options:decode::DecodeOptions::STEP_MANIA_TIMELINE,path:path.clone(),rate:1.,preserve_pitch:false,generation:old_generation,asset_epoch:epoch,
        });let deadline=std::time::Instant::now()+Duration::from_secs(3);while !runtime.decoder_status().snapshot().is_some_and(|s|s.final_music_seconds.is_some_and(|time|time>0.2)){assert!(std::time::Instant::now()<deadline);thread::yield_now();}
        assert!(render(&mut renderer,240).iter().all(|v|*v==0),"old captured Rate PCM must wait for queued Rate generation");runtime.handle(StreamCommand::SetMusicRateAt{rate:2.,seconds:0.,generation:new_generation,asset_epoch:epoch,
        });let pending=runtime.decoder_status().snapshot().unwrap();assert_eq!(pending.command_generation,new_generation);if pending.phase==lifecycle::DecoderPhase::Pending{assert_eq!(pending.render_ready_generation,None);assert_eq!(pending.final_music_seconds,None);}let deadline=std::time::Instant::now()+Duration::from_secs(3);while !runtime.decoder_status().snapshot().is_some_and(|s| {
            s.command_generation==new_generation&&s.render_ready_generation==Some(new_generation)&&matches!(s.phase,lifecycle::DecoderPhase::Producing|lifecycle::DecoderPhase::Exhausted)
        }){assert!(std::time::Instant::now()<deadline);thread::yield_now();}
        let first_now=deadlib_audio_core::seeded_music_position(deadlib_audio_core::music_total_frames().saturating_sub(deadlib_audio_core::music_track_start_frame())as f64/48_000.,
        ).unwrap().0;
        let actual_pcm=render(&mut renderer,240);let mut actual_maps=Vec::new();while let Some((generation,asset,map))=played.pop(){if generation==new_generation&&asset==epoch{actual_maps.push(map);}}
        assert!(!actual_maps.is_empty(),"ready replacement must be audible in its first callback, not wait for the old decoded cursor");
        assert!((actual_maps[0].music_start_sec-first_now).abs()<=256.*2./48_000.,"replacement starts at decoded advance instead of DSP NOW");
        drop(runtime);
        // The actual DSP intentionally reports consumed-input/output ratio.
        // Rubato's first batch consumes 513 source frames for 256 outputs at
        // 2x before compatibility lead trimming; its slope is not nominal 2/hz.
        // A fresh real decoder at the same cut supplies an independent control
        // path, including sinc startup, preroll and every emitted PCM sample.
        let(reference_pcm,reference_maps)=fresh_rate_reference(&path,first_now,2.,240);
        assert_eq!(actual_pcm,reference_pcm,"retargeted PCM must equal a fresh real 2x decode at DSP NOW");
        assert_eq!(actual_maps.len(),reference_maps.len());
        for(actual,reference)in actual_maps.iter().zip(&reference_maps){assert_eq!(actual.frames,reference.frames);assert_eq!(actual.music_start_sec.to_bits(),reference.music_start_sec.to_bits());assert_eq!(actual.music_sec_per_frame.to_bits(),reference.music_sec_per_frame.to_bits());}
        let _=std::fs::remove_dir_all(directory);
    }
}

#[cfg(test)]
#[path = "stream/retained_loop_tests.rs"]
mod retained_loop_tests;

#[cfg(test)]
#[path="stream/preview_fade_tests.rs"]
mod preview_fade_tests;

#[cfg(test)]
#[path="stream/preview_fade_physical_tests.rs"]
mod preview_fade_physical_tests;


/// Editor Main and preview use immutable output-rate float PCM. Worker owns conversion,
/// command publication and reclamation; callback owns only the actual cursor.
fn spawn_prepared_preview_thread(source:Arc<decode::prepared::PreparedAudioSource>,path:PathBuf,cut:Cut,looping:bool,writer:MusicBlockWriter,context:MusicDecodeContext,asset_epoch:u64,preview_phase:Option<(f64,u64)>,editor_main:bool,
)->MusicStream{
    let control=Arc::new(MusicControl{prepared:Some(source.clone()),preview_phase,paused:AtomicBool::new(deadlib_audio_core::music_track_paused()),asset_epoch,follow_live_phase:false,failed:AtomicBool::new(false),status:lifecycle::DecoderStatusReader::new_asset(context.generation,cut.start_sec,asset_epoch,
        ),stop_signal:AtomicBool::new(false),rate_bits:AtomicU32::new(1f32.to_bits()),preserve_pitch:AtomicBool::new(false),generation:AtomicU64::new(context.generation),wake:DecoderWake::new(),seek:Mutex::new(None),seek_serial:AtomicU64::new(0),
    });
    let worker=control.clone();
    let thread=thread::spawn(move||{
        let mut writer=writer;worker.wake.register_current();
        let result=(||->Result<(),Box<dyn std::error::Error+Send+Sync>>{
            let output_hz=context.output.sample_rate_hz.max(1);let out_channels=context.output.channels.max(1);
            let opened=editor_reader::EditorDecodeReader::open(&path,context.decode_options,Some(&source),
            )?;
            worker.status.opened_codec(context.generation,opened.reader.codec(),&path);
            let converted=decode::preview::resample_prepared(&source,output_hz,&worker.stop_signal).map_err(|message|std::io::Error::other(message))?;
            let frames=converted.len()/source.channels;
            let mut output=Vec::new();output.try_reserve_exact(frames.checked_mul(out_channels).ok_or("Preview source is too large")?,
            )?;
            for (index,input)in converted.chunks_exact(source.channels).enumerate(){
                if index%524288==0&&worker.stop_signal.load(Ordering::Acquire){return Ok(());}
                for channel in 0..out_channels{
                    // Normal host channel mapping is prepared here, not mixed
                    // ad hoc inside the callback. Mono remains audible in both ears.
                    output.push(if source.channels==1{input[0]}else{input.get(channel).copied().unwrap_or(0.)});
                }
            }
            let initial_frame=if let Some((phase,physical))=preview_phase{
                let base=(phase*f64::from(output_hz))as i64;
                let elapsed=if deadlib_audio_core::music_track_paused(){0}else{deadlib_audio_core::music_total_frames().saturating_sub(physical)};
                let frame=i128::from(base)+i128::from(elapsed);
                let start=(cut.start_sec*f64::from(output_hz))as i64;let length=(cut.length_sec*f64::from(output_hz))as i64;
                if looping&&length>0&&frame>=i128::from(start)+i128::from(length){(i128::from(start)+(frame-i128::from(start)).rem_euclid(i128::from(length)))as i64}else{frame.min(i128::from(i64::MAX))as i64}
            }else{((cut.start_sec*f64::from(output_hz))as i64).max(0)};
            let mut descriptor=deadlib_audio_core::PreviewSource{editor_main,samples:output.into(),channels:out_channels,sample_rate_hz:output_hz,generation:context.generation,asset_epoch,initial_frame,looping,
            };
            loop{
                if worker.stop_signal.load(Ordering::Acquire)||asset_epoch!=deadlib_audio_core::music_asset_epoch()||(!editor_main&&context.generation!=deadlib_audio_core::music_map_generation()){return Ok(());}
                match writer.try_publish_preview_source(descriptor){Ok(())=>break,Err(returned)=>descriptor=returned,
                }
                thread::park_timeout(std::time::Duration::from_millis(1));
            }
            let ready_generation=if editor_main{worker.generation.load(Ordering::SeqCst)}else{context.generation};
            writer.publish_preview_generation_ready(ready_generation,asset_epoch);
            worker.status.publish(ready_generation,lifecycle::DecoderPhase::Producing,Some(initial_frame as f64/f64::from(output_hz)),None,
            );
            worker.status.render_ready(ready_generation);
            let mut exhausted=false;
            while !worker.stop_signal.load(Ordering::Acquire){
                writer.reclaim_preview_sources();
                if editor_main{
                    let generation=worker.generation.load(Ordering::SeqCst);
                    if {let status=worker.status.snapshot();status.command_generation!=generation||status.render_ready_generation!=Some(generation)}{
                        worker.status.opened_codec(generation,opened.reader.codec(),&path);
                        writer.publish_preview_generation_ready(generation,asset_epoch);
                        worker.status.publish(generation,lifecycle::DecoderPhase::Producing,None,None,
                        );worker.status.render_ready(generation);
                    }
                    let _=worker.seek.lock().unwrap_or_else(|error|error.into_inner()).take();
                }
                if !editor_main&&!looping&&!exhausted{
                    // A replacement can start exactly at EOF and render only
                    // silence. Its actual callback still acknowledges the
                    // endpoint, even though it published no audible PCM cursor.
                    // Accepted initial clocks are deliberately not evidence.
                    let endpoint =deadlib_audio_core::preview_callback_cursor().filter(|cursor| {
                            cursor.asset_epoch==asset_epoch&&cursor.generation==context.generation
                        })
                        .map(|cursor| cursor.next_music_seconds)
                        .or_else(|| {
                            deadlib_audio_core::rendered_music_cursor()
                                .filter(|cursor| {
                                    cursor.asset_epoch == asset_epoch
                                        && cursor.generation == context.generation
                                })
                                .map(|cursor| cursor.next_music_seconds)
                        });
                    if let Some(endpoint) =
                        endpoint.filter(|seconds| *seconds >=cut.start_sec+cut.length_sec)
                    {worker.status.publish(context.generation,lifecycle::DecoderPhase::Exhausted,Some(endpoint),None,
                        );exhausted=true;}
                    }
                thread::park_timeout(std::time::Duration::from_millis(10));
            }
            writer.reclaim_preview_sources();Ok(())
        })();
        if let Err(error)=result{if !worker.stop_signal.load(Ordering::Acquire){worker.fail(&error.to_string());}}
        writer
    });
    MusicStream{thread,control}
}
