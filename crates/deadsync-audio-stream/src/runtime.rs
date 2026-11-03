mod editor;
pub use editor::EditorPreviewCursor;
pub use editor::{EditorLogicalPhase, EditorAudioGains, EditorAudioStatus, EditorTickKind, EditorOutputAck, EditorOutputEvent, EditorOutputUpdate};
#[cfg(target_os = "linux")]
use deadlib_audio::LinuxAudioBackend;
use deadlib_audio::{InitConfig, prepare_output};
use deadlib_audio_core::{
    OutputDeviceInfo, OutputTimingSnapshot, SfxPan, StutterDiagAudioEvent, normalized_music_rate,
};
use deadsync_audio_replaygain as replaygain;
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;

use crate::mix::{
    EFFECT_BUS, MINE_BUS, SCREEN_BUS, assist_tick_generation, init_controls, stop_assist_tick_bus,
    stop_mine_bus, stop_screen_bus,
};
use crate::music_map::clear_music_pos_map;
use crate::sfx_cache::SfxCache;
use crate::{MusicClock, SfxId};
use deadlib_audio::stream::{self, Cut, OutputFormat, StreamCommand};

mod output_reconfigure;
pub use output_reconfigure::{OutputRestoreReceipt, ReconfigureAck};
use output_reconfigure::OutputReconfigure;

static REPLAYGAIN_ENABLED: AtomicBool = AtomicBool::new(false);
static PRESERVE_PITCH_ENABLED: AtomicBool = AtomicBool::new(false);

/// Application-thread owner for audio commands and the sole SFX producer.
///
/// A successful initialization installs one engine. The no-audio value keeps
/// the same API but has no shared runtime state. `AudioControl` is deliberately
/// non-`Clone`: `App` is the only producer for the callback's bounded ring.
pub struct AudioControl {
    output_reconfigure: OutputReconfigure,
    engine: Option<AudioEngine>,
    music_request: Cell<Option<f32>>,
    editor: editor::Scope,
}

struct LiveOutput {
    manager: stream::StreamManager,
    plan: std::sync::Arc<deadlib_audio::OutputPlan>,
    config: InitConfig,
    controls: std::sync::Arc<deadlib_audio_core::MixControls>,
    buffers: deadlib_audio_core::OutputBufferState,
    backend: &'static str,
    device: String,
}

struct AudioEngine {
    live: Option<LiveOutput>,
    command_sender: Sender<StreamCommand>,
    decoder_status: stream::lifecycle::DecoderStatusMonitor,
    sfx_cache: SfxCache,
    output: OutputFormat,
    startup_output_devices: Vec<OutputDeviceInfo>,
}

#[inline(always)]
#[must_use]
pub fn timing_diag_last_callback_gap_ns() -> u64 {
    deadlib_audio_core::timing_diag_last_callback_gap_ns()
}

#[must_use]
pub fn stutter_diag_trigger_seq() -> u64 {
    deadlib_audio_core::stutter_diag_trigger_seq()
}

pub fn collect_stutter_diag_events(
    now_host_nanos: u64,
    window_ns: u64,
    out: &mut Vec<StutterDiagAudioEvent>,
) {
    deadlib_audio_core::collect_stutter_diag_events(now_host_nanos, window_ns, out);
}

/// Initialize audio with caller-resolved consolidated and legacy ReplayGain cache paths.
pub fn init(
    cfg: InitConfig,
    cache_file: PathBuf,
    legacy_cache_dir: PathBuf,
) -> Result<(AudioControl, MusicClock), String> {
    replaygain::init(replaygain::InitConfig {
        cache_file,
        legacy_cache_dir,
        result_callback: set_music_replaygain_if_matches,
    })
    .map_err(str::to_string)?;
    let controls = init_controls();
    let mut output_plan = prepare_output(&cfg, controls.clone());
    let startup_output_devices = output_plan.devices().to_vec();
    let stream::StreamReady {
        manager,
        buffers,
        command_sender,
        backend_ready: ready,
        sfx_sender,
        played_map,
        decoder_status,
    } = stream::start(output_plan.clone())?;
    output_plan.pin_backend(ready.backend_name);
    let music_clock = MusicClock::new(played_map, ready.device_sample_rate);
    Ok((
        AudioControl {
            output_reconfigure: OutputReconfigure::new(),
            music_request: Cell::new(None),
            editor: editor::Scope::new(),
            engine: Some(AudioEngine {
                live: Some(LiveOutput {
                    manager, plan: std::sync::Arc::new(output_plan), config: cfg,
                    controls: controls.clone(), buffers, backend: ready.backend_name,
                    device: ready.device_name.clone(),
                }),
                command_sender,
                decoder_status,
                sfx_cache: SfxCache::new(controls, sfx_sender),
                output: OutputFormat {
                    sample_rate_hz: ready.device_sample_rate,
                    channels: ready.device_channels,
                },
                startup_output_devices,
            }),
        },
        music_clock,
    ))
}

#[cfg(target_os = "linux")]
pub fn available_linux_backends() -> Vec<LinuxAudioBackend> {
    deadlib_audio::available_linux_backends()
}

pub fn set_replaygain_enabled(enabled: bool) {
    REPLAYGAIN_ENABLED.store(enabled, Ordering::Relaxed);
    if !enabled {
        deadlib_audio_core::reset_music_target_gain();
    }
}

#[inline(always)]
pub fn replaygain_enabled() -> bool {
    REPLAYGAIN_ENABLED.load(Ordering::Relaxed)
}

/// Sets the startup `RateModPreservesPitch` preference before an audio control
/// exists. Live application changes go through [`AudioControl`] so the current
/// track receives the update as well.
pub fn set_preserve_pitch_enabled(enabled: bool) {
    PRESERVE_PITCH_ENABLED.store(enabled, Ordering::Relaxed);
}

#[inline(always)]
pub fn preserve_pitch_enabled() -> bool {
    PRESERVE_PITCH_ENABLED.load(Ordering::Relaxed)
}

impl AudioControl {
    #[must_use]
    pub const fn without_audio() -> Self {
        Self {
            output_reconfigure: OutputReconfigure::new(),
            engine: None,
            music_request: Cell::new(None),
            editor: editor::Scope::new(),
        }
    }

    #[inline(always)]
    pub const fn is_available(&self) -> bool {
        self.engine.is_some()
    }

    pub fn startup_output_devices(&self) -> &[OutputDeviceInfo] {
        self.engine
            .as_ref()
            .map_or(&[], |engine| engine.startup_output_devices.as_slice())
    }

    /// Loads a sound for later playback; call during startup or screen/song preparation.
    ///
    /// Returns `None` when audio is unavailable or the sound cannot be loaded.
    /// Prepare an editor tick float clip outside render/update callbacks.
    pub fn editor_tick_output_format(&self) -> Option<OutputFormat> {
        self.engine.as_ref().map(|engine| engine.output)
    }
    pub fn prepare_editor_tick(&mut self, path: &Path) -> Option<SfxId> {
        let engine = self.engine.as_mut()?;
        engine.sfx_cache.prepare_editor_tick(path, engine.output)
    }
    pub fn prepare_sfx(&mut self, path: &Path) -> Option<SfxId> {
        let engine = self.engine.as_mut()?;
        engine.sfx_cache.prepare(path, engine.output)
    }

    /// Enqueues a prepared effect without filesystem access or decoding.
    pub fn play_sfx(&mut self, sound: &SfxId) {
        if let Some(engine) = self.engine.as_mut() {
            engine.sfx_cache.play(sound, EFFECT_BUS, 0);
        }
    }

    /// Stop and replay the current mine sound for one player (ITGmania parity).
    pub fn play_mine_sfx(&mut self, sound: &SfxId, player: u8, two_players: bool) {
        if let Some(engine) = self.engine.as_mut() {
            let pan = if two_players {
                if player == 0 {
                    SfxPan::Left
                } else {
                    SfxPan::Right
                }
            } else {
                SfxPan::Center
            };
            engine.sfx_cache.restart(sound, MINE_BUS, player, pan);
        }
    }

    pub fn stop_mine_sfx(&self) {
        stop_mine_bus();
    }

    /// Enqueues a prepared effect that stops when the owning screen exits.
    pub fn play_screen_sfx(&mut self, sound: &SfxId) {
        if let Some(engine) = self.engine.as_mut() {
            engine.sfx_cache.play(sound, SCREEN_BUS, 0);
        }
    }

    pub fn play_resolved_assist_tick(&mut self, sound: &SfxId) {
        if let Some(engine) = self.engine.as_mut() {
            engine.sfx_cache.play_assist_tick(sound, 0);
        }
    }

    /// Schedules a resolved assist tick on the callback's absolute stream
    /// timeline. A target of zero preserves immediate-play fallback behavior.
    pub fn play_scheduled_assist_tick(&mut self, sound: &SfxId, target_stream_frame: u64) {
        if let Some(engine) = self.engine.as_mut() {
            engine
                .sfx_cache
                .play_assist_tick(sound, target_stream_frame);
        }
    }

    pub fn play_actor_sfx(&mut self, owner: u64, sound: &SfxId) {
        if let Some(engine) = self.engine.as_mut() {
            engine.sfx_cache.play_owned(owner, sound, SCREEN_BUS);
        }
    }

    pub fn stop_actor_sfx(&mut self, owner: u64) {
        if let Some(engine) = self.engine.as_mut() {
            engine.sfx_cache.stop_owned(owner);
        }
    }

    pub fn pause_actor_sfx(&self, owner: u64, paused: bool) {
        if let Some(engine) = &self.engine {
            engine.sfx_cache.pause_owned(owner, paused);
        }
    }

    pub fn stop_screen_sfx(&mut self) {
        if let Some(engine) = self.engine.as_mut() {
            engine.sfx_cache.stop_owned_voices();
        }
        stop_screen_bus();
    }

    pub fn play_music(&self, path: PathBuf, cut: Cut, looping: bool, rate: f32) {
        self.release_editor_audio();
        self.music_request
            .set(self.engine.as_ref().map(|_| normalized_music_rate(rate)));
        play_music(self.engine.as_ref(), path, cut, looping, rate);
    }

    /// Latest acknowledgement for the current synchronous timeline command.
    /// Old decoder completions are ignored while its replacement is pending.
    /// This control-thread query does not indicate which samples are audible.
    pub fn decoder_status(&self) -> Option<stream::lifecycle::DecoderSnapshot> {
        let engine = self.engine.as_ref()?;
        let snapshot = engine.decoder_status.snapshot()?;
        (snapshot.command_generation == deadlib_audio_core::music_map_generation() && snapshot.asset_epoch == deadlib_audio_core::music_asset_epoch())
            .then_some(snapshot)
    }

    /// Synchronous token assigned before the current music command is queued.
    pub fn music_command_generation(&self) -> Option<u64> {
        self.engine.as_ref()?;
        self.music_request.get()?;
        Some(deadlib_audio_core::music_map_generation())
    }

    /// Seek the current decoder. The manager serializes this with pending Play/Stop.
    /// Invalid input and a disabled audio engine do not alter the shared clock.
    pub fn seek_music(&self, seconds: f64) {
        self.release_editor_audio();
        if let (Some(engine), Some(rate)) = (self.engine.as_ref(), self.music_request.get()) {
            seek_music(&engine.command_sender, seconds, rate);
        }
    }

    pub fn stop_music(&self) {
        self.release_editor_audio();
        self.music_request.set(None);
        stop_music(self.engine.as_ref());
    }

    pub fn set_music_rate(&self, rate: f32) {
        self.release_editor_audio();
        if self.music_request.get().is_some() {
            self.music_request.set(Some(normalized_music_rate(rate)));
        }
        set_music_rate(self.engine.as_ref(), rate);
    }

    pub fn set_preserve_pitch_enabled(&self, enabled: bool) {
        set_preserve_pitch_enabled(enabled);
        // The gameplay preference is retained for the next gameplay stream.
        // Source editor music uses rate resampling, independently of this option.
        if self.editor_music_owner().is_some() { return; }
        if let Some(engine) = self.engine.as_ref() {
            let generation = clear_music_pos_map();
            let _ = engine.command_sender.send(StreamCommand::SetPreservePitch {
                enabled,
                generation,
            });
        }
    }

    pub fn set_replaygain_enabled(&self, enabled: bool) {
        set_replaygain_enabled(enabled);
    }
}

#[inline(always)]
fn reset_music_stream_clock() -> u64 {
    // Reset immediately on the caller thread so async command handoff can't
    // leak the previous track's stream position into gameplay timing.
    deadlib_audio_core::reset_music_stream_clock_state();
    // Invalidate any assist ticks scheduled against the previous timeline; their
    // absolute target frames no longer correspond to the music position.
    stop_assist_tick_bus();
    clear_music_pos_map()
}

fn seek_music(sender: &Sender<StreamCommand>, seconds: f64, rate: f32) {
    if !seconds.is_finite() {
        return;
    }
    let generation = reset_music_stream_clock();
    deadlib_audio_core::activate_music_track();
    deadlib_audio_core::seed_music_stream_clock(seconds, rate);
    let _ = sender.send(StreamCommand::SeekMusic {
        seconds,
        generation,
    });
}

fn play_music(engine: Option<&AudioEngine>, path: PathBuf, cut: Cut, looping: bool, rate: f32) {
    play_music_with_pitch(engine,path,cut,looping,rate,preserve_pitch_enabled());
}

fn play_music_with_pitch(engine: Option<&AudioEngine>, path: PathBuf, cut: Cut, looping: bool, rate: f32, preserve_pitch: bool) {
    play_music_with_policy(engine,path,cut,looping,rate,preserve_pitch,ReplayGainPolicy::Gameplay);
}

#[derive(Clone,Copy,Eq,PartialEq)]
enum ReplayGainPolicy{Gameplay,Editor}

fn play_music_with_policy(engine:Option<&AudioEngine>,path:PathBuf,cut:Cut,looping:bool,rate:f32,preserve_pitch:bool,replaygain_policy:ReplayGainPolicy){
    let allow_replaygain=replaygain_policy==ReplayGainPolicy::Gameplay;
    let rate = normalized_music_rate(rate);
    let generation = reset_music_stream_clock();
    let asset_epoch=deadlib_audio_core::advance_music_asset_epoch();
    deadlib_audio_core::resume_music_track();
    deadlib_audio_core::seed_music_stream_clock(cut.start_sec, rate);

    let gain_guard=replaygain_track_guard();
    let track_id = deadlib_audio_core::next_music_track_id();
    if !allow_replaygain{EDITOR_REPLAYGAIN_EXCLUDED_TRACK.store(track_id,Ordering::Release);}
    let initial_gain = if allow_replaygain && replaygain_enabled() {
        replaygain::get_or_queue_gain_linear(&path, track_id).unwrap_or(1.0)
    } else {
        1.0
    };
    deadlib_audio_core::set_music_target_gain(initial_gain);
    // Snap to the new target at the track boundary so the previous track's
    // gain doesn't audibly bleed into the start of this one.
    deadlib_audio_core::snap_music_gain_generation();
    drop(gain_guard);

    if let Some(engine) = engine {
        let _ = engine.command_sender.send(StreamCommand::PlayMusic {
            asset_epoch,
            decode_options: crate::DECODE_OPTIONS,
            path,
            cut,
            looping,
            rate,
            preserve_pitch,
            generation,
        });
    }
}

/// Applies a `ReplayGain` result from the background analyzer, but only if it
/// still corresponds to the currently active music track. Called by
/// `deadsync_audio_replaygain` on its analysis thread. This control-plane API
/// serializes track changes and must not be called by an audio callback.
pub fn set_music_replaygain_if_matches(track_id: u64, gain_linear: f32) {
    let _guard=replaygain_track_guard();
    let active_id = deadlib_audio_core::active_music_track_id();
    if active_id != track_id || EDITOR_REPLAYGAIN_EXCLUDED_TRACK.load(Ordering::Acquire)==track_id {
        return;
    }
    if !deadlib_audio_core::music_track_active() {
        return;
    }
    deadlib_audio_core::set_music_target_gain(gain_linear);
}

fn stop_music(engine: Option<&AudioEngine>) {
    let asset_epoch=deadlib_audio_core::advance_music_asset_epoch();
    deadlib_audio_core::resume_music_track();
    reset_music_stream_clock();
    deadlib_audio_core::reset_music_target_gain();
    deadlib_audio_core::snap_music_gain_generation();
    if let Some(engine) = engine {
        let _ = engine.command_sender.send(StreamCommand::StopMusic{asset_epoch});
    }
}

fn set_music_rate(engine: Option<&AudioEngine>, rate: f32) {
    let rate = normalized_music_rate(rate);
    deadlib_audio_core::set_music_clock_rate(rate);
    let generation = clear_music_pos_map();
    if let Some(engine) = engine {
        let _ = engine
            .command_sender
            .send(StreamCommand::SetMusicRate { rate, generation });
    }
}

#[must_use]
pub fn assist_sfx_generation() -> u64 {
    assist_tick_generation()
}

#[must_use]
pub fn get_output_timing_snapshot() -> OutputTimingSnapshot {
    deadlib_audio_core::get_output_timing_snapshot()
}

#[cfg(test)]
mod seek_tests {
    use super::*;
    #[test]
    fn decoder_acknowledgements_reject_prior_generations_without_an_audio_device() {
        let _guard = crate::CLOCK_TEST_LOCK.lock().unwrap();
        let output = OutputFormat { sample_rate_hz: 48_000, channels: 2 };
        let (transport, _render) = deadlib_audio_core::music_transport(48_000, 2);
        let mut runtime = stream::MusicStreamRuntime::new(transport.writer, output);
        let monitor = runtime.decoder_status();
        let (sender, receiver) = std::sync::mpsc::channel();
        let (sfx_sender, _sfx_receiver) = deadlib_audio_core::sfx_transport(8);
        let audio = AudioControl {
            output_reconfigure: OutputReconfigure::new(),
            engine: Some(AudioEngine {
                live: None,
                command_sender: sender,
                decoder_status: monitor,
                sfx_cache: SfxCache::new(std::sync::Arc::new(deadlib_audio_core::MixControls::new()), sfx_sender),
                output,
                startup_output_devices: Vec::new(),
            }),
            music_request: Cell::new(None),
            editor: editor::Scope::new(),
        };
        assert!(audio.decoder_status().is_none());
        assert!(AudioControl::without_audio().decoder_status().is_none());
        assert!(audio.music_command_generation().is_none());
        let missing = std::env::temp_dir().join(format!("deadsync-status-missing-{}.wav", std::process::id()));
        assert!(!missing.exists());
        audio.play_music(missing, Cut::default(), false, 1.0);
        let generation = audio.music_command_generation().unwrap();
        runtime.handle(receiver.try_recv().unwrap());
        // Stop joins the worker, avoiding a sleep or race with its open failure.
        runtime.handle(StreamCommand::StopMusic{asset_epoch:deadlib_audio_core::music_asset_epoch()});
        let status = audio.decoder_status().unwrap();
        assert_eq!(status.command_generation, generation);
        assert_eq!(status.phase, stream::lifecycle::DecoderPhase::Stopped);
        assert!(!status.opened_successfully);
        audio.stop_music();
        assert!(audio.music_command_generation().is_none());
        assert!(audio.decoder_status().is_none());
    }

    #[test]
    fn seek_keeps_rate_gain_and_track_identity_and_rejects_invalid_or_disabled_requests() {
        let _guard = crate::CLOCK_TEST_LOCK.lock().unwrap();
        use deadlib_audio_core::*;
        let (tx, rx) = std::sync::mpsc::channel();
        for rate in [0.8, 1.2] {
            for seconds in [-0.125, 0.0, 0.123456789] {
                seed_music_stream_clock(9.0, rate);
                activate_music_track();
                let track = next_music_track_id();
                set_music_target_gain(0.5);
                let gain_generation = music_gain_snap_generation();
                let previous = music_map_generation();
                seek_music(&tx, seconds, rate);
                match rx.try_recv().unwrap() {
                    StreamCommand::SeekMusic {
                        seconds: actual,
                        generation,
                    } => {
                        assert_eq!(actual, seconds);
                        assert_ne!(generation, previous);
                        assert_eq!(generation, music_map_generation());
                    }
                    other => panic!("unexpected {other:?}"),
                }
                assert_eq!(
                    seeded_music_position(0.0),
                    if seconds == 0.0 {
                        None
                    } else {
                        Some((seconds, rate))
                    }
                );
                assert_eq!(active_music_track_id(), track);
                assert_eq!(music_target_gain(), 0.5);
                assert_eq!(music_gain_snap_generation(), gain_generation);
                assert!(music_track_active());
            }
        }
        let generation = music_map_generation();
        let seed = seeded_music_position(0.0);
        for seconds in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            seek_music(&tx, seconds, 1.2);
        }
        AudioControl::without_audio().seek_music(1.0);
        assert_eq!(music_map_generation(), generation);
        assert_eq!(seeded_music_position(0.0), seed);
        assert!(rx.try_recv().is_err());
        let (sfx_sender, _sfx_receiver) = deadlib_audio_core::sfx_transport(8);
        let audio = AudioControl {
            output_reconfigure: OutputReconfigure::new(),
            engine: Some(AudioEngine {
                live: None,
                command_sender: tx,
                decoder_status: Default::default(),
                sfx_cache: SfxCache::new(std::sync::Arc::new(MixControls::new()), sfx_sender),
                output: OutputFormat {
                    sample_rate_hz: 48_000,
                    channels: 2,
                },
                startup_output_devices: Vec::new(),
            }),
            music_request: Cell::new(None),
            editor: editor::Scope::new(),
        };
        audio.seek_music(1.0);
        assert_eq!(music_map_generation(), generation);
        assert!(rx.try_recv().is_err());
        audio.play_music(
            PathBuf::from("command-only.wav"),
            Cut::default(),
            false,
            0.8,
        );
        assert!(matches!(
            rx.try_recv().unwrap(),
            StreamCommand::PlayMusic { rate: 0.8, .. }
        ));
        audio.seek_music(-0.125);
        assert!(matches!(
            rx.try_recv().unwrap(),
            StreamCommand::SeekMusic {
                seconds: -0.125,
                ..
            }
        ));
        assert_eq!(seeded_music_position(0.0), Some((-0.125, 0.8)));
        audio.set_music_rate(1.2);
        assert!(matches!(
            rx.try_recv().unwrap(),
            StreamCommand::SetMusicRate { rate: 1.2, .. }
        ));
        audio.seek_music(0.25);
        assert!(matches!(
            rx.try_recv().unwrap(),
            StreamCommand::SeekMusic { seconds: 0.25, .. }
        ));
        assert_eq!(seeded_music_position(0.0), Some((0.25, 1.2)));
        audio.stop_music();
        assert!(matches!(rx.try_recv().unwrap(), StreamCommand::StopMusic{..}));
        let stopped = music_map_generation();
        audio.seek_music(1.0);
        assert_eq!(music_map_generation(), stopped);
        assert!(rx.try_recv().is_err());
        stop_music_track();
        reset_music_stream_clock_state();
        reset_music_target_gain();
    }
}

// Immutable track IDs prevent late background gameplay analysis from applying
// to editor playback. A new gameplay ID remains eligible without global reset.
static EDITOR_REPLAYGAIN_EXCLUDED_TRACK:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(0);
fn begin_editor_unity_replaygain(){
    let _guard=replaygain_track_guard();
    let track_id=deadlib_audio_core::next_music_track_id();
    EDITOR_REPLAYGAIN_EXCLUDED_TRACK.store(track_id,Ordering::Release);
    deadlib_audio_core::set_music_target_gain(1.);
    deadlib_audio_core::snap_music_gain_generation();
}

// Control/analysis threads only. Audio callbacks never acquire this guard.
// Serialize ID replacement+unity with tagged analysis so an analysis which read
// an old ID immediately before editor acceptance cannot write gain afterward.
static REPLAYGAIN_TRACK_GUARD:std::sync::Mutex<()>=std::sync::Mutex::new(());
fn replaygain_track_guard()->std::sync::MutexGuard<'static,()>{REPLAYGAIN_TRACK_GUARD.lock().unwrap_or_else(|poison|poison.into_inner())}

#[cfg(test)]
mod editor_replaygain_policy_tests {
    use super::*;
    #[test]
    fn legacy_editor_track_rejects_tagged_gain_and_next_gameplay_track_remains_eligible(){
        let _serial=crate::CLOCK_TEST_LOCK.lock().unwrap();
        let enabled=replaygain_enabled();set_replaygain_enabled(false);
        deadlib_audio_core::activate_music_track();
        play_music_with_policy(None,PathBuf::from("editor.wav"),Cut{start_sec:0.25,length_sec:0.75,..Default::default()},true,1.35,false,ReplayGainPolicy::Editor);
        deadlib_audio_core::activate_music_track();let editor=deadlib_audio_core::active_music_track_id();set_music_replaygain_if_matches(editor,0.125);assert_eq!(deadlib_audio_core::music_target_gain(),1.);
        play_music(None,PathBuf::from("gameplay.wav"),Cut::default(),false,1.);
        deadlib_audio_core::activate_music_track();let gameplay=deadlib_audio_core::active_music_track_id();assert_ne!(gameplay,editor);set_music_replaygain_if_matches(gameplay,0.375);assert_eq!(deadlib_audio_core::music_target_gain(),0.375);set_music_replaygain_if_matches(editor,2.);assert_eq!(deadlib_audio_core::music_target_gain(),0.375);
        set_replaygain_enabled(enabled);deadlib_audio_core::reset_music_target_gain();
    }
}
