//! Editor-scoped transport and gain ownership, isolated from gameplay levels.
mod assets;
mod output;
use super::*;
pub use assets::EditorPreviewCursor;
use deadlib_audio_core::{MixBus, SfxSchedule};
use std::sync::Arc;
fn editor_source_rate(rate: f64) -> f64 {
    if rate.is_finite() {
        rate.clamp(0.25, 4.)
    } else {
        1.
    }
}
pub use output::{EditorOutputAck, EditorOutputEvent, EditorOutputUpdate};

const ASSIST: MixBus = MixBus::new(4);
const BEAT: MixBus = MixBus::new(5);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditorAudioGains {
    pub main: f32,
    pub music: f32,
    pub assist: f32,
    pub beat: f32,
}
impl EditorAudioGains {
    fn core(self) -> deadlib_audio_core::EditorMixGains {
        deadlib_audio_core::EditorMixGains {
            main: self.main,
            music: self.music,
            assist: self.assist,
            beat: self.beat,
        }
    }
    fn valid(self) -> bool {
        [self.main, self.music, self.assist, self.beat]
            .into_iter()
            .all(|x| x.is_finite() && (0.0..=1.0).contains(&x))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorTickKind {
    Assist,
    Beat,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct EditorAudioStatus {
    pub owner: u64,
    pub tick_generation: u64,
    pub sample_rate: u32,
}
#[derive(Clone, Copy)]
struct State {
    owner: u64,
    generation: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorLogicalPhase {
    pub owner: u64,
    pub generation: u64,
    pub asset_epoch: u64,
}
pub(super) struct Scope {
    tick_voices: std::cell::RefCell<Option<[Arc<deadlib_audio_core::SfxScheduleVoice>; 2]>>,
    assets: std::cell::RefCell<assets::Assets>,
    paused_position: Cell<Option<f64>>,
    music_path: std::cell::RefCell<Option<PathBuf>>,
    logical_music: Cell<bool>,
    pending_asset: Cell<Option<u64>>,
    music_owned: Cell<bool>,
    state: Cell<State>,
    output: output::Output,
    controls: std::cell::RefCell<Option<std::sync::Arc<deadlib_audio_core::MixControls>>>,
}
impl Scope {
    pub(super) const fn new() -> Self {
        Self {
            tick_voices: std::cell::RefCell::new(None),
            assets: std::cell::RefCell::new(assets::Assets::new()),
            paused_position: Cell::new(None),
            music_path: std::cell::RefCell::new(None),
            logical_music: Cell::new(false),
            pending_asset: Cell::new(None),
            music_owned: Cell::new(false),
            state: Cell::new(State {
                owner: 0,
                generation: 1,
            }),
            output: output::Output::new(),
            controls: std::cell::RefCell::new(None),
        }
    }
}
impl AudioControl {
    pub fn begin_editor_audio(&self, owner: u64, gains: EditorAudioGains) -> bool {
        if owner == 0 || owner == u64::MAX || !gains.valid() {
            return false;
        }
        if self.editor.state.get().owner != owner && self.editor.state.get().generation == u64::MAX
        {
            return false;
        }
        let old = self.editor.state.get().owner;
        if let Some(engine) = self.engine.as_ref() {
            let controls = engine.sfx_cache.controls();
            if (old == owner && !controls.can_update_editor_gain_override(owner, gains.core()))
                || (old == 0 && !controls.can_begin_editor_gain_override(owner, gains.core()))
                || old != 0 && old != owner
            {
                return false;
            }
        }
        if !self.begin_editor_asset_owner(owner) || self.engine.is_none() {
            return false;
        }
        if old == owner {
            return self.update_editor_audio(owner, gains);
        }
        if old != 0 {
            return false;
        }
        let controls = self
            .engine
            .as_ref()
            .expect("available engine")
            .sfx_cache
            .controls();
        if !controls.begin_editor_gain_override(owner, gains.core()) {
            return false;
        }
        controls.set_bus_gain(ASSIST, gains.assist * gains.main);
        controls.set_bus_gain(BEAT, gains.beat * gains.main);
        self.editor.controls.replace(Some(controls.clone()));
        self.editor
            .tick_voices
            .replace(Some(std::array::from_fn(|_| {
                Arc::new(deadlib_audio_core::SfxScheduleVoice::new())
            })));
        self.editor.music_owned.set(false);
        self.editor.logical_music.set(false);
        self.editor.paused_position.set(None);
        self.editor.music_path.borrow_mut().take();
        self.editor.state.set(State {
            owner,
            generation: self.editor.state.get().generation.saturating_add(1),
        });
        controls.stop_bus(ASSIST);
        controls.stop_bus(BEAT);
        true
    }
    pub fn update_editor_audio(&self, owner: u64, gains: EditorAudioGains) -> bool {
        if owner == 0 || self.editor.state.get().owner != owner || !gains.valid() {
            return false;
        }
        let held = self.editor.controls.borrow();
        let Some(controls) = held.as_ref() else {
            return false;
        };
        if !controls.update_editor_gain_override(owner, gains.core()) {
            return false;
        }
        controls.set_bus_gain(ASSIST, gains.assist * gains.main);
        controls.set_bus_gain(BEAT, gains.beat * gains.main);
        true
    }
    /// Release only the owned override. Global levels changed during editing
    /// are immediately visible; no captured level overwrites them on release.
    pub fn end_editor_audio(&self, owner: u64) -> bool {
        let state = self.editor.state.get();
        let released_assets = if state.owner == 0 || state.owner == owner {
            self.end_editor_asset_owner(owner)
        } else {
            false
        };
        if owner == 0 || state.owner != owner {
            return released_assets;
        }
        self.release_editor_assets();
        self.silence_editor_ticks_terminal();
        let mut state = self.editor.state.get();
        state.owner = 0;
        self.editor.state.set(state);
        self.editor.music_owned.set(false);
        self.editor.logical_music.set(false);
        self.editor.paused_position.set(None);
        self.editor.music_path.borrow_mut().take();
        if let Some(controls) = self.editor_terminal_controls() {
            controls.end_stream_gain_override(owner);
        }
        self.music_request.set(None);
        stop_music(self.engine.as_ref());
        true
    }
    /// Only terminal Stop/release may saturate: no transport can subsequently
    /// acquire another window at MAX, while existing voices must be silenced.
    fn editor_terminal_controls(&self) -> Option<Arc<deadlib_audio_core::MixControls>> {
        self.editor.controls.borrow().clone().or_else(|| {
            self.engine
                .as_ref()
                .map(|engine| engine.sfx_cache.controls())
        })
    }
    fn silence_editor_ticks_terminal(&self) {
        if let Some(voices) = self.editor.tick_voices.borrow().as_ref() {
            for voice in voices {
                voice.reset();
            }
        }
        if let Some(controls) = self.editor_terminal_controls() {
            controls.stop_bus(ASSIST);
            controls.stop_bus(BEAT);
        }
        let mut state = self.editor.state.get();
        state.generation = state.generation.saturating_add(1);
        self.editor.state.set(state);
    }
    pub(super) fn release_editor_audio(&self) {
        let owner = self.editor.state.get().owner;
        if owner != 0 {
            self.end_editor_audio(owner);
        } else {
            self.release_editor_assets();
        }
    }
    pub fn editor_audio_status(&self) -> EditorAudioStatus {
        let state = self.editor.state.get();
        EditorAudioStatus {
            owner: state.owner,
            tick_generation: state.generation,
            sample_rate: self
                .engine
                .as_ref()
                .map_or(0, |engine| engine.output.sample_rate_hz),
        }
    }
    /// Preference changes revoke future windows while the callback keeps its
    /// actual already-playing tick tail. Transport boundaries still stop buses.
    pub fn retarget_editor_ticks(&self, owner: u64) -> bool {
        let mut state = self.editor.state.get();
        if owner == 0 || state.owner != owner {
            return false;
        }
        let Some(next) = state.generation.checked_add(1) else {
            return false;
        };
        let voices = self.editor.tick_voices.borrow();
        let Some(voices) = voices.as_ref() else {
            return false;
        };
        for voice in voices {
            voice.cancel_future();
        }
        state.generation = next;
        self.editor.state.set(state);
        true
    }
    pub fn invalidate_editor_ticks(&self, owner: u64) -> bool {
        let mut state = self.editor.state.get();
        if owner == 0 || state.owner != owner {
            return false;
        }
        let Some(next) = state.generation.checked_add(1) else {
            return false;
        };
        let held = self.editor.controls.borrow();
        let Some(controls) = held.as_ref() else {
            return false;
        };
        if let Some(voices) = self.editor.tick_voices.borrow().as_ref() {
            for voice in voices {
                voice.reset();
            }
        }
        controls.stop_bus(ASSIST);
        controls.stop_bus(BEAT);
        state.generation = next;
        self.editor.state.set(state);
        true
    }
    /// Source StartPlayback owns a logical DSP phase independently of decoding.
    /// A missing selection is a silent music clock, not a fabricated file error.
    pub fn editor_start_music(
        &self,
        owner: u64,
        path: Option<PathBuf>,
        seconds: f64,
        rate: f64,
    ) -> bool {
        if !deadlib_audio_core::editor_main_can_configure() {
            return false;
        }
        if owner == 0
            || self.editor.state.get().owner != owner
            || !seconds.is_finite()
            || !rate.is_finite()
        {
            return false;
        }
        let Some(engine) = self.engine.as_ref() else {
            return false;
        };
        if !self.invalidate_editor_ticks(owner) {
            return false;
        }
        let _ = self.freeze_editor_main_phase();
        let rate = editor_source_rate(rate);
        self.music_request.set(Some(rate as f32));
        let generation = reset_music_stream_clock();
        let asset_epoch = deadlib_audio_core::advance_music_asset_epoch();
        deadlib_audio_core::activate_music_track();
        deadlib_audio_core::seed_music_asset_clock(seconds, rate as f32);
        deadlib_audio_core::mark_music_track_started(deadlib_audio_core::music_track_start_frame());
        deadlib_audio_core::resume_music_track();
        begin_editor_unity_replaygain();
        self.editor.music_owned.set(true);
        self.editor.pending_asset.set(Some(asset_epoch));
        self.editor.paused_position.set(None);
        self.editor.music_path.replace(path.clone());
        self.editor.logical_music.set(true);
        let _ = (engine, generation, asset_epoch);
        self.start_editor_prepared_main(owner, path, seconds, rate, None)
    }

    pub fn editor_play_music(
        &self,
        owner: u64,
        path: PathBuf,
        cut: Cut,
        looping: bool,
        rate: f64,
    ) -> bool {
        if !self.invalidate_editor_ticks(owner) {
            return false;
        }
        self.music_request
            .set(Some(normalized_music_rate(rate as f32)));
        self.editor.pending_asset.set(None);
        *self.editor.music_path.borrow_mut() = Some(path.clone());
        self.editor.paused_position.set(None);
        self.editor.logical_music.set(false);
        // Legacy generic Cut/loop fallback retains its existing f32 DSP contract.
        play_music_with_policy(
            self.engine.as_ref(),
            path,
            cut,
            looping,
            rate as f32,
            false,
            ReplayGainPolicy::Editor,
        );
        deadlib_audio_core::activate_music_track();
        self.editor.music_owned.set(true);
        true
    }
    /// Replace only the decoder asset. Clock generation, rate, track origin,
    /// mix ownership and device lease remain the same.
    pub fn editor_swap_music_asset(
        &self,
        owner: u64,
        path: PathBuf,
        clock: &mut crate::MusicClock,
    ) -> bool {
        self.editor_swap_music_asset_optional(owner, Some(path), clock)
    }
    pub fn editor_clear_music_asset(&self, owner: u64, clock: &mut crate::MusicClock) -> bool {
        self.editor_swap_music_asset_optional(owner, None, clock)
    }
    fn editor_swap_music_asset_optional(
        &self,
        owner: u64,
        path: Option<PathBuf>,
        clock: &mut crate::MusicClock,
    ) -> bool {
        if !deadlib_audio_core::editor_main_can_configure() {
            return false;
        }
        if owner == 0 || self.editor.state.get().owner != owner || !self.editor.music_owned.get() {
            return false;
        }
        let Some(engine) = self.engine.as_ref() else {
            return false;
        };
        let Some(rate) = self.music_request.get() else {
            return false;
        };
        let rate = self.exact_editor_main_rate().unwrap_or(f64::from(rate));
        if !self.retarget_editor_ticks(owner) {
            return false;
        }
        let main_frame = self.freeze_editor_main_phase();
        let generation = deadlib_audio_core::music_map_generation();
        let (snapshot, _, absolute_stream) = clock.snapshot_for_editor_time();
        let now = main_frame
            .map(|frame| frame as f64 / f64::from(engine.output.sample_rate_hz))
            .unwrap_or_else(|| {
                self.editor
                    .paused_position
                    .get()
                    .unwrap_or(snapshot.music_nanos as f64 / 1e9)
            });
        let stream = absolute_stream
            - deadlib_audio_core::music_track_start_frame() as f64
                / f64::from(engine.output.sample_rate_hz.max(1));
        deadlib_audio_core::seed_music_asset_clock(now - stream * rate, rate as f32);
        // Logical playback has a callback origin even if the previous asset
        // never supplied a PCM block. Preserve that origin, do not reset it.
        deadlib_audio_core::mark_music_track_started(deadlib_audio_core::music_track_start_frame());
        let asset_epoch = deadlib_audio_core::advance_music_asset_epoch();
        self.editor.pending_asset.set(Some(asset_epoch));
        crate::music_map::discard_music_asset_map();
        // ReplayGain is asset-scoped too: an old asynchronous analysis must
        // not change the replacement track's gain. This id does not reset
        // clock generation, stream origin or the editor mix override.
        begin_editor_unity_replaygain();
        *self.editor.music_path.borrow_mut() = path.clone();
        self.editor.logical_music.set(true);
        let _ = (engine, generation, asset_epoch);
        self.start_editor_prepared_main(owner, path, now, rate, main_frame)
    }
    pub fn editor_seek_music_at(
        &self,
        owner: u64,
        seconds: f64,
        clock: &mut crate::MusicClock,
    ) -> bool {
        if owner == 0 || self.editor.state.get().owner != owner || !seconds.is_finite() {
            return false;
        }
        if self.editor.state.get().generation == u64::MAX
            || self.editor.controls.borrow().is_none()
            || self.engine.is_none()
            || self.music_request.get().is_none()
        {
            return false;
        }
        if self.editor.pending_asset.get().is_some() && clock.snapshot_for_editor_time().1 {
            self.editor.pending_asset.set(None);
        }
        self.editor_seek_music(owner, seconds)
    }
    pub fn editor_music_asset_epoch(&self) -> Option<u64> {
        self.editor_music_owner()
            .map(|_| deadlib_audio_core::music_asset_epoch())
    }
    pub fn editor_logical_phase(&self) -> Option<EditorLogicalPhase> {
        let owner = self.editor_music_owner()?;
        (self.editor.logical_music.get() && !deadlib_audio_core::music_track_paused()).then_some(
            EditorLogicalPhase {
                owner,
                generation: deadlib_audio_core::music_map_generation(),
                asset_epoch: deadlib_audio_core::music_asset_epoch(),
            },
        )
    }
    /// Drain actual callbacks before changing a source Main phase. The control
    /// thread may wait; the callback never does. Thaw follows coherent publish.
    fn exact_editor_main_rate(&self) -> Option<f64> {
        let epoch = deadlib_audio_core::music_asset_epoch();
        let generation = deadlib_audio_core::music_map_generation();
        deadlib_audio_core::editor_main_configuration()
            .filter(|c| c.asset_epoch == epoch && c.generation == generation)
            .map(|c| c.rate)
    }
    fn freeze_editor_main_phase(&self) -> Option<i64> {
        let epoch = deadlib_audio_core::music_asset_epoch();
        let generation = deadlib_audio_core::music_map_generation();
        let control = deadlib_audio_core::editor_main_configuration()
            .filter(|c| c.asset_epoch == epoch && c.generation == generation)?;
        let _ = deadlib_audio_core::freeze_rendered_music_cursor(generation, epoch);
        Some(
            deadlib_audio_core::editor_main_cursor()
                .filter(|c| c.asset_epoch == epoch && c.generation == generation)
                .map_or(control.frame, |c| c.source_frame),
        )
    }
    fn publish_editor_main_phase(&self, frame: i64, rate: f64) -> bool {
        let epoch = deadlib_audio_core::music_asset_epoch();
        if !deadlib_audio_core::configure_editor_main(
            deadlib_audio_core::music_map_generation(),
            epoch,
            frame,
            editor_source_rate(rate),
        ) {
            return false;
        }
        deadlib_audio_core::thaw_editor_main(epoch);
        true
    }
    pub fn editor_pause_music(&self, owner: u64, seconds: f64) -> bool {
        if !deadlib_audio_core::editor_main_can_configure() {
            return false;
        }
        if !seconds.is_finite()
            || owner == 0
            || self.editor.state.get().owner != owner
            || !self.editor.music_owned.get()
        {
            return false;
        }
        let Some(engine) = self.engine.as_ref() else {
            return false;
        };
        if !self.invalidate_editor_ticks(owner) {
            return false;
        }
        let main_rate = self
            .exact_editor_main_rate()
            .unwrap_or(f64::from(self.music_request.get().unwrap_or(1.)));
        let main_frame = self.freeze_editor_main_phase();
        deadlib_audio_core::pause_music_track();
        let generation = clear_music_pos_map();
        self.editor.paused_position.set(Some(seconds));
        self.editor.logical_music.set(false);
        if let Some(frame) = main_frame {
            if !self.publish_editor_main_phase(frame, main_rate) {
                return false;
            }
        }
        let asset_epoch = deadlib_audio_core::music_asset_epoch();
        let _ = engine.command_sender.send(StreamCommand::PauseMusic {
            generation,
            asset_epoch,
        });
        true
    }
    pub fn editor_resume_music(&self, owner: u64, path: PathBuf, seconds: f64, rate: f64) -> bool {
        if !deadlib_audio_core::editor_main_can_configure() {
            return false;
        }
        if owner == 0
            || self.editor.state.get().owner != owner
            || !seconds.is_finite()
            || !rate.is_finite()
        {
            return false;
        }
        let retained = self.editor.music_owned.get()
            && self.music_request.get().is_some()
            && self.editor.paused_position.get().is_some()
            && self
                .editor
                .music_path
                .borrow()
                .as_ref()
                .is_some_and(|held| retained_music_paths_equal(held, &path));
        if !retained {
            return self.editor_start_music(owner, Some(path), seconds, rate);
        }
        let Some(engine) = self.engine.as_ref() else {
            return false;
        };
        if !self.invalidate_editor_ticks(owner) {
            return false;
        }
        let main = self.freeze_editor_main_phase().is_some();
        let rate = editor_source_rate(rate);
        self.music_request.set(Some(rate as f32));
        let generation = reset_music_stream_clock();
        deadlib_audio_core::activate_music_track();
        deadlib_audio_core::seed_music_asset_clock(seconds, rate as f32);
        deadlib_audio_core::mark_music_track_started(deadlib_audio_core::music_track_start_frame());
        self.editor.logical_music.set(true);
        deadlib_audio_core::resume_music_track();
        self.editor.paused_position.set(None);
        if main {
            if !self.publish_editor_main_phase(
                (seconds * f64::from(engine.output.sample_rate_hz)) as i64,
                rate,
            ) {
                return false;
            }
        }
        let asset_epoch = deadlib_audio_core::music_asset_epoch();
        let _ = engine.command_sender.send(StreamCommand::ResumeMusic {
            seconds,
            rate: rate as f32,
            preserve_pitch: false,
            generation,
            asset_epoch,
        });
        true
    }

    pub fn editor_seek_music(&self, owner: u64, seconds: f64) -> bool {
        if !deadlib_audio_core::editor_main_can_configure() {
            return false;
        }
        if !seconds.is_finite() || owner == 0 || self.editor.state.get().owner != owner {
            return false;
        }
        let (Some(engine), Some(rate)) = (self.engine.as_ref(), self.music_request.get()) else {
            return false;
        };
        if !self.invalidate_editor_ticks(owner) {
            return false;
        }
        let main_rate = self.exact_editor_main_rate().unwrap_or(f64::from(rate));
        let main = self.freeze_editor_main_phase().is_some();
        if self.editor.pending_asset.get() == Some(deadlib_audio_core::music_asset_epoch()) {
            let generation = reset_music_stream_clock();
            deadlib_audio_core::activate_music_track();
            deadlib_audio_core::seed_music_asset_clock(seconds, rate);
            deadlib_audio_core::mark_music_track_started(
                deadlib_audio_core::music_track_start_frame(),
            );
            let asset_epoch = deadlib_audio_core::music_asset_epoch();
            let _ = engine.command_sender.send(StreamCommand::SetMusicRateAt {
                rate,
                seconds,
                generation,
                asset_epoch,
            });
            if main {
                if !self.publish_editor_main_phase(
                    (seconds * f64::from(engine.output.sample_rate_hz)) as i64,
                    main_rate,
                ) {
                    return false;
                }
            }
            return true;
        }
        seek_music(&engine.command_sender, seconds, rate);
        if main {
            if !self.publish_editor_main_phase(
                (seconds * f64::from(engine.output.sample_rate_hz)) as i64,
                main_rate,
            ) {
                return false;
            }
        }
        true
    }
    pub fn editor_set_music_rate_at(
        &self,
        owner: u64,
        rate: f64,
        clock: &mut crate::MusicClock,
    ) -> bool {
        if !deadlib_audio_core::editor_main_can_configure() {
            return false;
        }
        if owner == 0 || self.editor.state.get().owner != owner || !rate.is_finite() {
            return false;
        }
        let owned = self.music_request.get().is_some() && self.editor.music_owned.get();
        if owned && self.engine.is_none() {
            return false;
        }
        if !self.retarget_editor_ticks(owner) {
            return false;
        }
        let main_frame = if owned {
            self.freeze_editor_main_phase()
        } else {
            None
        };
        if let (true, Some(engine)) = (owned, self.engine.as_ref()) {
            let (snapshot, decoded, absolute) = clock.snapshot_for_editor_time();
            let now = self
                .editor
                .paused_position
                .get()
                .unwrap_or(snapshot.music_nanos as f64 / 1e9);
            let stream = absolute
                - deadlib_audio_core::music_track_start_frame() as f64
                    / f64::from(engine.output.sample_rate_hz.max(1));
            let rate = editor_source_rate(rate);
            deadlib_audio_core::seed_music_asset_clock(now - stream * rate, rate as f32);
            if decoded {
                self.editor.pending_asset.set(None);
            }
            if self.editor.pending_asset.get() == Some(deadlib_audio_core::music_asset_epoch()) {
                self.music_request.set(Some(rate as f32));
                let generation = clear_music_pos_map();
                let asset_epoch = deadlib_audio_core::music_asset_epoch();
                let _ = engine.command_sender.send(StreamCommand::SetMusicRateAt {
                    rate: rate as f32,
                    seconds: now,
                    generation,
                    asset_epoch,
                });
                if let Some(frame) = main_frame {
                    if !self.publish_editor_main_phase(frame, rate) {
                        return false;
                    }
                }
                return true;
            }
        }
        self.apply_accepted_editor_music_rate(rate);
        if let Some(frame) = main_frame {
            if !self.publish_editor_main_phase(frame, rate) {
                return false;
            }
        }
        true
    }
    pub fn editor_set_music_rate(&self, owner: u64, rate: f64) -> bool {
        if !deadlib_audio_core::editor_main_can_configure() {
            return false;
        }
        if !rate.is_finite() || !self.retarget_editor_ticks(owner) {
            return false;
        }
        let main_frame = self.freeze_editor_main_phase();
        self.apply_accepted_editor_music_rate(rate);
        if let Some(frame) = main_frame {
            if !self.publish_editor_main_phase(frame, rate) {
                return false;
            }
        }
        true
    }
    fn apply_accepted_editor_music_rate(&self, rate: f64) {
        if self.music_request.get().is_some() {
            self.music_request
                .set(Some(editor_source_rate(rate) as f32));
        }
        set_music_rate(self.engine.as_ref(), editor_source_rate(rate) as f32);
    }
    pub fn editor_stop_music(&self, owner: u64) -> bool {
        if owner == 0 || self.editor.state.get().owner != owner {
            return false;
        }
        self.stop_editor_asset_transport();
        self.silence_editor_ticks_terminal();
        self.editor.music_owned.set(false);
        self.editor.logical_music.set(false);
        self.editor.paused_position.set(None);
        self.editor.music_path.borrow_mut().take();
        self.music_request.set(None);
        stop_music(self.engine.as_ref());
        true
    }
    pub fn editor_tick_duration(&self, sound: &SfxId) -> Option<f64> {
        let output = self.engine.as_ref()?.output;
        Some(sound.duration(output))
    }
    /// Authoritative window includes the most recent past onset and future
    /// onsets. No onset is marked submitted when the map or bounded ring is not
    /// available: the producer may submit the next frame's window again.
    pub fn editor_tick_window(
        &mut self,
        owner: u64,
        generation: u64,
        music_generation: u64,
        kind: EditorTickKind,
        samples: &[i64],
        sound: &SfxId,
        clock: &mut MusicClock,
    ) -> bool {
        let state = self.editor.state.get();
        if owner == 0
            || state.owner != owner
            || state.generation != generation
            || self.music_command_generation() != Some(music_generation)
            || samples.len() > 2_000_001
            || samples.windows(2).any(|w| w[0] > w[1])
        {
            return false;
        }
        let Some(engine) = self.engine.as_mut() else {
            return false;
        };
        if !sound.valid_editor_output(engine.output) {
            return false;
        }
        let source_phase =
            self.editor.logical_music.get() && !deadlib_audio_core::music_track_paused();
        let index = match kind {
            EditorTickKind::Assist => 0,
            EditorTickKind::Beat => 1,
        };
        let Some(voice) = self
            .editor
            .tick_voices
            .borrow()
            .as_ref()
            .map(|voices| Arc::clone(&voices[index]))
        else {
            return false;
        };
        let schedule = if source_phase
            && deadlib_audio_core::editor_main_configuration().is_some_and(|c| {
                c.generation == music_generation
                    && c.asset_epoch == deadlib_audio_core::music_asset_epoch()
            }) {
            if let Some(pcm) = sound.editor_pcm(engine.output) {
                SfxSchedule::with_source_main_float(
                    samples.to_vec(),
                    voice,
                    music_generation,
                    deadlib_audio_core::music_asset_epoch(),
                    pcm,
                    engine.output.channels,
                )
            } else {
                SfxSchedule::with_source_main(
                    samples.to_vec(),
                    voice,
                    music_generation,
                    deadlib_audio_core::music_asset_epoch(),
                )
            }
        } else {
            let mut triggers = Vec::with_capacity(samples.len());
            for sample in samples {
                let Some(trigger) = (if source_phase {
                    clock.editor_source_tick_trigger(*sample, engine.output.sample_rate_hz)
                } else {
                    clock.editor_tick_trigger(*sample, engine.output.sample_rate_hz)
                }) else {
                    return false;
                };
                triggers.push(trigger);
            }
            SfxSchedule::with_voice(triggers, voice)
        };
        let Some(schedule) = schedule else {
            return false;
        };
        let bus = match kind {
            EditorTickKind::Assist => ASSIST,
            EditorTickKind::Beat => BEAT,
        };
        engine.sfx_cache.replace_schedule(sound, bus, schedule)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn editor_rate_pitch_is_independent_of_gameplay_option_and_live_changes() {
        let _serial = crate::CLOCK_TEST_LOCK.lock().unwrap();
        let previous = preserve_pitch_enabled();
        let (audio, commands, _) = control();
        let owner = 1129;
        assert!(audio.begin_editor_audio(
            owner,
            EditorAudioGains {
                main: 1.,
                music: 1.,
                assist: 1.,
                beat: 1.
            }
        ));
        set_preserve_pitch_enabled(true);
        assert!(audio.editor_play_music(
            owner,
            PathBuf::from("editor.wav"),
            Cut::default(),
            false,
            2.
        ));
        assert!(matches!(
            commands.try_recv().unwrap(),
            StreamCommand::PlayMusic {
                preserve_pitch: false,
                rate: 2.,
                ..
            }
        ));
        let generation = deadlib_audio_core::music_map_generation();
        audio.set_preserve_pitch_enabled(false);
        audio.set_preserve_pitch_enabled(true);
        assert!(preserve_pitch_enabled());
        assert_eq!(deadlib_audio_core::music_map_generation(), generation);
        assert!(commands.try_recv().is_err());
        assert!(audio.editor_pause_music(owner, 0.5));
        while commands.try_recv().is_ok() {}
        assert!(audio.editor_resume_music(owner, PathBuf::from("editor.wav"), 0.5, 2.));
        assert!(matches!(
            commands.try_recv().unwrap(),
            StreamCommand::ResumeMusic {
                preserve_pitch: false,
                ..
            }
        ));
        assert!(audio.end_editor_audio(owner));
        while commands.try_recv().is_ok() {}
        audio.play_music(PathBuf::from("gameplay.wav"), Cut::default(), false, 2.);
        assert!(matches!(
            commands.try_recv().unwrap(),
            StreamCommand::PlayMusic {
                preserve_pitch: true,
                rate: 2.,
                ..
            }
        ));
        audio.stop_music();
        set_preserve_pitch_enabled(previous);
    }
    #[test]
    fn exhausted_tick_authority_refuses_rate_and_transport_before_state_or_map_mutation() {
        let _serial = crate::CLOCK_TEST_LOCK.lock().unwrap();
        for mapped in [false, true] {
            let (audio, commands, controls) = control();
            let (mut stream, handle) = deadlib_audio_core::music_transport(48000, 2);
            let mut clock = MusicClock::new(stream.played_map, 48000);
            let owner = 986 + u64::from(mapped);
            let path = PathBuf::from("retained-missing.wav");
            assert!(audio.begin_editor_audio(
                owner,
                EditorAudioGains {
                    main: 1.,
                    music: 1.,
                    assist: 1.,
                    beat: 1.
                }
            ));
            assert!(audio.editor_start_music(owner, Some(path.clone()), 0.75, 1.));
            while commands.try_recv().is_ok() {}
            if mapped {
                let generation = deadlib_audio_core::music_map_generation();
                let epoch = deadlib_audio_core::music_asset_epoch();
                stream
                    .writer
                    .try_publish_preview_source(deadlib_audio_core::PreviewSource {
                        editor_main: true,
                        samples: vec![1000f32 / 32768.; 80000].into(),
                        channels: 2,
                        sample_rate_hz: 48000,
                        generation,
                        asset_epoch: epoch,
                        initial_frame: 36000,
                        looping: false,
                    })
                    .unwrap();
                stream
                    .writer
                    .publish_preview_generation_ready(generation, epoch);
                let mut renderer =
                    deadlib_audio_core::RenderState::new(handle, controls.clone(), 2);
                let mut pcm = [0i16; 128];
                renderer.render(
                    deadlib_audio_core::OutputBufferMut::I16(&mut pcm),
                    deadlib_audio_core::CallbackInfo {
                        anchor_nanos: deadlib_platform::host_time::instant_nanos(
                            std::time::Instant::now(),
                        ),
                        clock: deadlib_audio_core::CallbackClockSource::Instant,
                    },
                    std::iter::empty(),
                );
                assert!(pcm.iter().any(|v| *v != 0));
                assert!(clock.editor_tick_trigger(36016, 48000).is_some());
                audio.editor.pending_asset.set(None);
            }
            let before_map = clock.editor_tick_trigger(36016, 48000);
            let voice = audio.editor.tick_voices.borrow().as_ref().unwrap()[0].clone();
            let schedule = deadlib_audio_core::SfxSchedule::with_voice(
                vec![
                    deadlib_audio_core::ScheduledTrigger {
                        frame: 0,
                        skipped_frames: 0,
                    },
                    deadlib_audio_core::ScheduledTrigger {
                        frame: 8,
                        skipped_frames: 0,
                    },
                ],
                voice,
            )
            .unwrap();
            let data = (1..=12).collect::<Vec<i16>>();
            let mix = |before| {
                let mut pcm = vec![0.; 4];
                schedule.mix(&data, &mut pcm, before, 1, 1.);
                pcm.into_iter()
                    .map(|v| (v * 32768.).round() as i16)
                    .collect::<Vec<_>>()
            };
            assert_eq!(mix(0), vec![1, 2, 3, 4]);
            audio.editor.state.set(State {
                owner,
                generation: u64::MAX,
            });
            let capture = || {
                (
                    (
                        deadlib_audio_core::music_asset_epoch(),
                        deadlib_audio_core::music_map_generation(),
                        deadlib_audio_core::music_track_start_frame(),
                        deadlib_audio_core::active_music_track_id(),
                        deadlib_audio_core::music_target_gain().to_bits(),
                        deadlib_audio_core::music_gain_snap_generation(),
                        deadlib_audio_core::seeded_music_position(0.),
                        deadlib_audio_core::music_track_paused(),
                        deadlib_audio_core::music_track_active(),
                    ),
                    (
                        audio.music_request.get(),
                        audio.editor.pending_asset.get(),
                        audio.editor.paused_position.get(),
                        audio.editor.logical_music.get(),
                        audio.editor.music_path.borrow().clone(),
                        audio.editor_audio_status().tick_generation,
                    ),
                    (
                        controls.bus_generation(ASSIST),
                        controls.bus_generation(BEAT),
                        controls.effective_stream_gain().to_bits(),
                    ),
                )
            };
            let before = capture();
            assert!(!audio.editor_set_music_rate_at(owner, 2., &mut clock));
            assert_eq!(capture(), before);
            assert_eq!(clock.editor_tick_trigger(36016, 48000), before_map);
            assert!(!audio.editor_set_music_rate(owner, 2.));
            assert_eq!(capture(), before);
            assert!(!audio.editor_start_music(owner, None, 9., 2.));
            assert_eq!(capture(), before);
            assert!(!audio.editor_play_music(owner, path.clone(), Cut::default(), false, 2.));
            assert_eq!(capture(), before);
            assert!(!audio.editor_pause_music(owner, 9.));
            assert_eq!(capture(), before);
            assert!(!audio.editor_seek_music(owner, 9.));
            assert_eq!(capture(), before);
            assert!(!audio.editor_seek_music_at(owner, 9., &mut clock));
            assert_eq!(capture(), before);
            assert_eq!(clock.editor_tick_trigger(36016, 48000), before_map);
            assert!(!audio.editor_resume_music(owner, path.clone(), 9., 2.));
            assert_eq!(capture(), before);
            assert!(commands.try_recv().is_err());
            assert_eq!(mix(4), vec![5, 6, 7, 8]);
            assert_eq!(mix(8), vec![1, 2, 3, 4]);
            // Stop remains terminal even when no new window can be assigned.
            assert!(audio.editor_stop_music(owner));
            assert_eq!(audio.editor_audio_status().tick_generation, u64::MAX);
            assert!(audio.music_request.get().is_none());
            assert!(audio.editor_music_owner().is_none());
            assert!(!deadlib_audio_core::music_track_active());
            assert_eq!(mix(12), vec![0; 4]);
            assert!(audio.end_editor_audio(owner));
            assert_eq!(audio.editor_audio_status().owner, 0);
            assert!(!audio.begin_editor_audio(
                owner + 100,
                EditorAudioGains {
                    main: 1.,
                    music: 1.,
                    assist: 1.,
                    beat: 1.
                }
            ));
        }
    }
    #[test]
    fn retained_resume_refuses_at_max_and_release_still_revokes_owner_and_gain() {
        let _serial = crate::CLOCK_TEST_LOCK.lock().unwrap();
        let (audio, commands, controls) = control();
        let owner = 988;
        let path = PathBuf::from("retained-missing.wav");
        assert!(audio.begin_editor_audio(
            owner,
            EditorAudioGains {
                main: 1.,
                music: 0.4,
                assist: 0.3,
                beat: 0.2
            }
        ));
        assert!(audio.editor_start_music(owner, Some(path.clone()), 1., 1.));
        assert!(audio.editor_pause_music(owner, 1.25));
        while commands.try_recv().is_ok() {}
        audio.editor.state.set(State {
            owner,
            generation: u64::MAX,
        });
        let epoch = deadlib_audio_core::music_asset_epoch();
        let generation = deadlib_audio_core::music_map_generation();
        let seed = deadlib_audio_core::seeded_music_position(0.);
        let gain = controls.effective_stream_gain();
        assert!(!audio.editor_resume_music(owner, path, 3., 2.));
        assert_eq!(audio.editor.paused_position.get(), Some(1.25));
        assert_eq!(audio.music_request.get(), Some(1.));
        assert!(deadlib_audio_core::music_track_paused());
        assert_eq!(deadlib_audio_core::music_asset_epoch(), epoch);
        assert_eq!(deadlib_audio_core::music_map_generation(), generation);
        assert_eq!(deadlib_audio_core::seeded_music_position(0.), seed);
        assert_eq!(controls.effective_stream_gain(), gain);
        assert!(commands.try_recv().is_err());
        // A terminal release must also finish if its cached controls vanished;
        // the engine's same controls provide the fallback cleanup handle.
        audio.editor.controls.replace(None);
        assert!(audio.end_editor_audio(owner));
        assert_eq!(audio.editor_audio_status().owner, 0);
        assert!(audio.music_request.get().is_none());
        assert!(!deadlib_audio_core::music_track_active());
        assert!(!deadlib_audio_core::music_track_paused());
        assert_eq!(controls.effective_stream_gain(), 1.);
        assert!(!audio.end_editor_audio(owner + 1));
    }

    #[test]
    fn music_asset_focus_swap_retargets_future_ticks_without_cutting_current_voice() {
        let _serial = crate::CLOCK_TEST_LOCK.lock().unwrap();
        let (audio, _commands, controls) = control();
        let owner = 984;
        assert!(audio.begin_editor_audio(
            owner,
            EditorAudioGains {
                main: 1.,
                music: 1.,
                assist: 1.,
                beat: 1.
            }
        ));
        assert!(audio.editor_start_music(owner, None, 0., 1.));
        let (stream, _handle) = deadlib_audio_core::music_transport(48000, 2);
        let mut clock = MusicClock::new(stream.played_map, 48000);
        let voice = audio.editor.tick_voices.borrow().as_ref().unwrap()[0].clone();
        let schedule = deadlib_audio_core::SfxSchedule::with_voice(
            vec![
                deadlib_audio_core::ScheduledTrigger {
                    frame: 0,
                    skipped_frames: 0,
                },
                deadlib_audio_core::ScheduledTrigger {
                    frame: 8,
                    skipped_frames: 0,
                },
            ],
            voice,
        )
        .unwrap();
        let data = (1..=12).collect::<Vec<i16>>();
        let mix = |before| {
            let mut pcm = vec![0.; 4];
            schedule.mix(&data, &mut pcm, before, 1, 1.);
            pcm.into_iter()
                .map(|v| (v * 32768.).round() as i16)
                .collect::<Vec<_>>()
        };
        assert_eq!(mix(0), vec![1, 2, 3, 4]);
        let bus = controls.bus_generation(ASSIST);
        let tick = audio.editor_audio_status().tick_generation;
        let music = audio.music_command_generation();
        assert!(audio.editor_swap_music_asset(
            owner,
            std::env::temp_dir().join(format!(
                "never-created-deadsync-tick-focus-{}.wav",
                std::process::id()
            )),
            &mut clock
        ));
        assert_eq!(controls.bus_generation(ASSIST), bus);
        assert_ne!(audio.editor_audio_status().tick_generation, tick);
        assert_eq!(audio.music_command_generation(), music);
        assert_eq!(mix(4), vec![5, 6, 7, 8]);
        assert_eq!(mix(8), vec![9, 10, 11, 12]);
        assert!(audio.end_editor_audio(owner));
    }
    #[test]
    fn exhausted_swap_or_clear_refuses_before_clock_asset_request_gain_or_future_mutation() {
        let _serial = crate::CLOCK_TEST_LOCK.lock().unwrap();
        let (audio, commands, controls) = control();
        let owner = 985;
        assert!(audio.begin_editor_audio(
            owner,
            EditorAudioGains {
                main: 1.,
                music: 0.4,
                assist: 0.3,
                beat: 0.2
            }
        ));
        assert!(audio.editor_start_music(owner, None, 1.25, 1.));
        while commands.try_recv().is_ok() {}
        let (stream, _handle) = deadlib_audio_core::music_transport(48000, 2);
        let mut clock = MusicClock::new(stream.played_map, 48000);
        let voice = audio.editor.tick_voices.borrow().as_ref().unwrap()[0].clone();
        let schedule = deadlib_audio_core::SfxSchedule::with_voice(
            vec![
                deadlib_audio_core::ScheduledTrigger {
                    frame: 0,
                    skipped_frames: 0,
                },
                deadlib_audio_core::ScheduledTrigger {
                    frame: 8,
                    skipped_frames: 0,
                },
            ],
            voice,
        )
        .unwrap();
        let data = (1..=12).collect::<Vec<i16>>();
        let mix = |before| {
            let mut pcm = vec![0.; 4];
            schedule.mix(&data, &mut pcm, before, 1, 1.);
            pcm.into_iter()
                .map(|v| (v * 32768.).round() as i16)
                .collect::<Vec<_>>()
        };
        assert_eq!(mix(0), vec![1, 2, 3, 4]);
        audio.editor.state.set(State {
            owner,
            generation: u64::MAX,
        });
        let capture = || {
            (
                (
                    deadlib_audio_core::music_asset_epoch(),
                    deadlib_audio_core::music_map_generation(),
                    deadlib_audio_core::music_track_start_frame(),
                    deadlib_audio_core::active_music_track_id(),
                    deadlib_audio_core::music_target_gain().to_bits(),
                    deadlib_audio_core::music_gain_snap_generation(),
                    deadlib_audio_core::seeded_music_position(0.),
                    deadlib_audio_core::music_track_paused(),
                ),
                (
                    audio.music_request.get(),
                    audio.editor.pending_asset.get(),
                    audio.editor.paused_position.get(),
                    audio.editor.logical_music.get(),
                    audio.editor.music_path.borrow().clone(),
                    audio.editor_audio_status().tick_generation,
                ),
                (
                    controls.bus_generation(ASSIST),
                    controls.bus_generation(BEAT),
                    controls.effective_stream_gain().to_bits(),
                ),
            )
        };
        let before = capture();
        assert!(!audio.editor_swap_music_asset(
            owner,
            PathBuf::from("not-applied.wav"),
            &mut clock
        ));
        assert_eq!(capture(), before);
        assert!(!audio.editor_clear_music_asset(owner, &mut clock));
        assert_eq!(capture(), before);
        assert!(commands.try_recv().is_err());
        assert_eq!(mix(4), vec![5, 6, 7, 8]);
        assert_eq!(mix(8), vec![1, 2, 3, 4]);
        assert!(audio.end_editor_audio(owner));
    }

    #[test]
    fn preference_retarget_preserves_actual_tick_pcm_tail_and_revokes_future_windows() {
        let _serial = crate::CLOCK_TEST_LOCK.lock().unwrap();
        let (mut audio, _commands, controls) = control();
        let (sender, mut receiver) = deadlib_audio_core::sfx_transport(16);
        audio.engine.as_mut().unwrap().sfx_cache = SfxCache::new(controls.clone(), sender);
        let path = std::env::temp_dir().join(format!(
            "deadsync-tick-tail-{}-{}.wav",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let frames = 10u32;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + frames * 4).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&48000u32.to_le_bytes());
        bytes.extend_from_slice(&192000u32.to_le_bytes());
        bytes.extend_from_slice(&4u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&(frames * 4).to_le_bytes());
        for value in 1..=10i16 {
            bytes.extend_from_slice(&value.to_le_bytes());
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        std::fs::write(&path, bytes).unwrap();
        let sound = audio.prepare_sfx(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        let (stream, handle) = deadlib_audio_core::music_transport(48000, 2);
        let mut clock = MusicClock::new(stream.played_map, 48000);
        let mut renderer = deadlib_audio_core::RenderState::new(handle, controls.clone(), 2);
        renderer.attach_sfx_retirement(receiver.take_retirement());
        let owner = 982;
        assert!(audio.begin_editor_audio(
            owner,
            EditorAudioGains {
                main: 1.,
                music: 1.,
                assist: 1.,
                beat: 1.
            }
        ));
        assert!(audio.editor_start_music(owner, None, 0., 1.));
        let music_generation = audio.music_command_generation().unwrap();
        let first = audio.editor_audio_status().tick_generation;
        assert!(audio.editor_tick_window(
            owner,
            first,
            music_generation,
            EditorTickKind::Assist,
            &[0, 8],
            &sound,
            &mut clock
        ));
        let callback = |renderer: &mut deadlib_audio_core::RenderState,
                        receiver: &mut deadlib_audio_core::SfxReceiver| {
            let mut pcm = vec![0i16; 8];
            renderer.render(
                deadlib_audio_core::OutputBufferMut::I16(&mut pcm),
                deadlib_audio_core::CallbackInfo {
                    anchor_nanos: deadlib_platform::host_time::instant_nanos(
                        std::time::Instant::now(),
                    ),
                    clock: deadlib_audio_core::CallbackClockSource::Instant,
                },
                receiver.try_iter(),
            );
            pcm.chunks_exact(2)
                .map(|frame| {
                    assert_eq!(frame[0], frame[1]);
                    frame[0]
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(callback(&mut renderer, &mut receiver), vec![1, 2, 3, 4]);
        let bus = controls.bus_generation(ASSIST);
        assert!(!audio.retarget_editor_ticks(owner + 1));
        assert!(audio.retarget_editor_ticks(owner));
        assert_eq!(controls.bus_generation(ASSIST), bus);
        assert!(!audio.editor_tick_window(
            owner,
            first,
            music_generation,
            EditorTickKind::Assist,
            &[8],
            &sound,
            &mut clock
        ));
        // Replacing with the disabled empty window preserves the current tail.
        let current = audio.editor_audio_status().tick_generation;
        assert!(audio.editor_tick_window(
            owner,
            current,
            music_generation,
            EditorTickKind::Assist,
            &[],
            &sound,
            &mut clock
        ));
        assert_eq!(callback(&mut renderer, &mut receiver), vec![5, 6, 7, 8]);
        assert_eq!(callback(&mut renderer, &mut receiver), vec![9, 10, 0, 0]);
        assert_eq!(callback(&mut renderer, &mut receiver), vec![0; 4]);
        assert!(audio.invalidate_editor_ticks(owner));
        assert_ne!(controls.bus_generation(ASSIST), bus);
        assert!(audio.editor_start_music(owner, None, 0., 1.));
        let generation = audio.editor_audio_status().tick_generation;
        let music_generation = audio.music_command_generation().unwrap();
        assert!(audio.editor_tick_window(
            owner,
            generation,
            music_generation,
            EditorTickKind::Assist,
            &[0, 8],
            &sound,
            &mut clock
        ));
        assert_eq!(callback(&mut renderer, &mut receiver), vec![1, 2, 3, 4]);
        assert!(audio.update_editor_audio(
            owner,
            EditorAudioGains {
                main: 1.,
                music: 1.,
                assist: 0.,
                beat: 0.
            }
        ));
        assert!(audio.retarget_editor_ticks(owner));
        let generation = audio.editor_audio_status().tick_generation;
        assert!(audio.editor_tick_window(
            owner,
            generation,
            music_generation,
            EditorTickKind::Assist,
            &[],
            &sound,
            &mut clock
        ));
        assert_eq!(callback(&mut renderer, &mut receiver), vec![0; 4]);
        assert!(audio.update_editor_audio(
            owner,
            EditorAudioGains {
                main: 1.,
                music: 1.,
                assist: 1.,
                beat: 1.
            }
        ));
        assert!(audio.retarget_editor_ticks(owner));
        let generation = audio.editor_audio_status().tick_generation;
        assert!(audio.editor_tick_window(
            owner,
            generation,
            music_generation,
            EditorTickKind::Assist,
            &[],
            &sound,
            &mut clock
        ));
        assert_eq!(callback(&mut renderer, &mut receiver), vec![9, 10, 0, 0]);
        assert!(audio.editor_start_music(owner, None, 0., 1.));
        let generation = audio.editor_audio_status().tick_generation;
        let music_generation = audio.music_command_generation().unwrap();
        assert!(audio.editor_tick_window(
            owner,
            generation,
            music_generation,
            EditorTickKind::Assist,
            &[0, 8],
            &sound,
            &mut clock
        ));
        assert_eq!(callback(&mut renderer, &mut receiver), vec![1, 2, 3, 4]);
        let bus = controls.bus_generation(ASSIST);
        assert!(audio.editor_set_music_rate_at(owner, 2., &mut clock));
        assert_eq!(controls.bus_generation(ASSIST), bus);
        let generation = audio.editor_audio_status().tick_generation;
        let music_generation = audio.music_command_generation().unwrap();
        assert!(audio.editor_tick_window(
            owner,
            generation,
            music_generation,
            EditorTickKind::Assist,
            &[],
            &sound,
            &mut clock
        ));
        assert_eq!(callback(&mut renderer, &mut receiver), vec![5, 6, 7, 8]);
        assert_eq!(callback(&mut renderer, &mut receiver), vec![9, 10, 0, 0]);
        assert!(audio.end_editor_audio(owner));
    }
    #[test]
    fn exhausted_tick_generation_refuses_before_any_future_voice_or_bus_mutation() {
        let _serial = crate::CLOCK_TEST_LOCK.lock().unwrap();
        let (audio, _commands, controls) = control();
        let owner = 983;
        assert!(audio.begin_editor_audio(
            owner,
            EditorAudioGains {
                main: 1.,
                music: 1.,
                assist: 1.,
                beat: 1.
            }
        ));
        let voice = audio.editor.tick_voices.borrow().as_ref().unwrap()[0].clone();
        let schedule = deadlib_audio_core::SfxSchedule::with_voice(
            vec![
                deadlib_audio_core::ScheduledTrigger {
                    frame: 0,
                    skipped_frames: 0,
                },
                deadlib_audio_core::ScheduledTrigger {
                    frame: 8,
                    skipped_frames: 0,
                },
            ],
            voice,
        )
        .unwrap();
        let data = (1..=12).collect::<Vec<i16>>();
        let mix = |before| {
            let mut pcm = vec![0.; 4];
            schedule.mix(&data, &mut pcm, before, 1, 1.);
            pcm.into_iter()
                .map(|v| (v * 32768.).round() as i16)
                .collect::<Vec<_>>()
        };
        assert_eq!(mix(0), vec![1, 2, 3, 4]);
        audio.editor.state.set(State {
            owner,
            generation: u64::MAX,
        });
        let bus = controls.bus_generation(ASSIST);
        assert!(!audio.retarget_editor_ticks(owner));
        assert!(!audio.invalidate_editor_ticks(owner));
        assert_eq!(audio.editor_audio_status().tick_generation, u64::MAX);
        assert_eq!(controls.bus_generation(ASSIST), bus);
        assert_eq!(mix(4), vec![5, 6, 7, 8]);
        assert_eq!(mix(8), vec![1, 2, 3, 4]);
    }

    use super::*;
    use deadlib_audio_core::MixControls;
    #[test]
    fn pause_resume_keeps_asset_authority_and_rejects_foreign_owner() {
        let _serial = crate::CLOCK_TEST_LOCK.lock().unwrap();
        let (audio, receiver, _) = control();
        let owner = 981;
        assert!(audio.begin_editor_audio(
            owner,
            EditorAudioGains {
                main: 1.,
                music: 0.7,
                assist: 0.4,
                beat: 0.2
            }
        ));
        assert!(audio.editor_play_music(
            owner,
            PathBuf::from("retained.wav"),
            Cut::default(),
            false,
            1.
        ));
        let _ = receiver.try_recv().unwrap();
        let epoch = deadlib_audio_core::music_asset_epoch();
        let gains = deadlib_audio_core::music_target_gain();
        assert!(!audio.editor_pause_music(owner + 1, 1.25));
        assert!(!deadlib_audio_core::music_track_paused());
        assert!(audio.editor_pause_music(owner, 1.25));
        assert!(deadlib_audio_core::music_track_paused());
        assert!(audio.editor_logical_phase().is_none());
        assert!(
            matches!(receiver.try_recv().unwrap(),StreamCommand::PauseMusic{asset_epoch,..}if asset_epoch==epoch)
        );
        assert!(!audio.editor_resume_music(owner + 1, PathBuf::from("retained.wav"), 1.25, 1.));
        assert!(deadlib_audio_core::music_track_paused());
        assert!(audio.editor_resume_music(owner, PathBuf::from("retained.wav"), 1.25, 1.));
        assert!(!deadlib_audio_core::music_track_paused());
        assert!(
            matches!(receiver.try_recv().unwrap(),StreamCommand::ResumeMusic{asset_epoch,seconds,..}if asset_epoch==epoch&&seconds==1.25)
        );
        let phase = audio.editor_logical_phase().unwrap();
        assert_eq!(phase.owner, owner);
        assert_eq!(phase.asset_epoch, epoch);
        assert_eq!(deadlib_audio_core::music_target_gain(), gains);
        assert_eq!(audio.editor_music_owner(), Some(owner));
        assert_eq!(
            deadlib_audio_core::seeded_music_position(0.).unwrap().0,
            1.25
        );
        assert!(audio.end_editor_audio(owner));
        assert!(audio.editor_logical_phase().is_none());
        assert!(audio.editor_music_asset_epoch().is_none());
        assert!(deadlib_audio_core::music_asset_epoch() > epoch);
        assert!(!audio.editor_resume_music(owner, PathBuf::from("retained.wav"), 1.25, 1.));
    }
    #[test]
    fn source_initial_start_zero_and_failed_decode_keep_logical_phase_without_open_ack() {
        let _serial = crate::CLOCK_TEST_LOCK.lock().unwrap();
        let (mut audio, receiver, _) = control();
        let (stream, handle) = deadlib_audio_core::music_transport(48_000, 2);
        let mut clock = crate::MusicClock::new(stream.played_map, 48_000);
        let mut renderer = deadlib_audio_core::RenderState::new(
            handle,
            std::sync::Arc::new(MixControls::new()),
            2,
        );
        let mut runtime = stream::MusicStreamRuntime::new(
            stream.writer,
            OutputFormat {
                sample_rate_hz: 48_000,
                channels: 2,
            },
        );
        audio.engine.as_mut().unwrap().decoder_status = runtime.decoder_status();
        let owner = 997;
        assert!(audio.begin_editor_audio(
            owner,
            EditorAudioGains {
                main: 1.,
                music: 0.5,
                assist: 0.2,
                beat: 0.1
            }
        ));
        assert!(audio.editor_start_music(owner, None, 0., 1.));
        assert_eq!(
            deadlib_audio_core::seeded_music_position(0.),
            Some((0., 1.))
        );
        runtime.handle(receiver.try_recv().unwrap());
        assert!(
            audio.decoder_status().is_none(),
            "no selected asset must not invent an opened or failed decoder"
        );
        assert!(audio.editor_logical_phase().is_some());
        assert!(clock.editor_tick_trigger(24_000, 48_000).is_none());
        let tick = clock.editor_source_tick_trigger(24_000, 48_000).unwrap();
        assert_eq!(
            tick.frame,
            deadlib_audio_core::music_track_start_frame() + 24_000
        );
        assert_eq!(tick.skipped_frames, 0);
        let missing = std::env::temp_dir().join(format!(
            "deadsync-initial-missing-{}.wav",
            std::process::id()
        ));
        assert!(!missing.exists());
        assert!(audio.editor_start_music(owner, Some(missing), 0., 1.));
        runtime.handle(receiver.try_recv().unwrap());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let failure = loop {
            audio.poll_editor_assets(&mut clock);
            while let Ok(command) = receiver.try_recv() {
                runtime.handle(command);
            }
            if let Some(s) = audio.decoder_status() {
                if s.phase == stream::lifecycle::DecoderPhase::Failed {
                    break s;
                }
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        };
        assert!(!failure.opened_successfully);
        assert!(failure.codec.is_none());
        assert!(failure.failure.is_some());
        assert_eq!(failure.render_ready_generation, None);
        let mut output = [0i16; 480];
        for _ in 0..100 {
            renderer.render(
                deadlib_audio_core::OutputBufferMut::I16(&mut output),
                deadlib_audio_core::CallbackInfo {
                    anchor_nanos: deadlib_platform::host_time::instant_nanos(
                        std::time::Instant::now(),
                    ),
                    clock: deadlib_audio_core::CallbackClockSource::Instant,
                },
                std::iter::empty(),
            );
            assert!(output.iter().all(|value| *value == 0));
        }
        let position = clock.snapshot_for_editor_time();
        assert!(!position.1, "logical clock is not a decoded PCM map");
        let now = position.0.music_nanos as f64 / 1e9;
        assert!(
            (now - 0.5).abs() < 0.02,
            "initial Failed music must still advance DSP phase: {now}"
        );
        let phase = audio.editor_logical_phase().unwrap();
        assert_eq!(phase.owner, owner);
        assert_eq!(phase.generation, failure.command_generation);
        assert_eq!(phase.asset_epoch, failure.asset_epoch);
        assert!(!audio.decoder_status().unwrap().opened_successfully);
        let generation = phase.generation;
        assert!(audio.editor_clear_music_asset(owner, &mut clock));
        runtime.handle(receiver.try_recv().unwrap());
        assert!(audio.decoder_status().is_none());
        assert_eq!(audio.editor_logical_phase().unwrap().generation, generation);
        assert!(audio.editor_logical_phase().unwrap().asset_epoch > phase.asset_epoch);
        assert!(audio.end_editor_audio(owner));
        assert!(audio.editor_logical_phase().is_none());
    }
    #[test]
    fn source_initial_start_without_audio_engine_has_no_logical_proof_or_mutation() {
        let _serial = crate::CLOCK_TEST_LOCK.lock().unwrap();
        let (mut audio, _receiver, _) = control();
        let owner = 999;
        assert!(audio.begin_editor_audio(
            owner,
            EditorAudioGains {
                main: 1.,
                music: 1.,
                assist: 1.,
                beat: 1.
            }
        ));
        audio.engine = None;
        let epoch = deadlib_audio_core::music_asset_epoch();
        let generation = deadlib_audio_core::music_map_generation();
        assert!(!audio.editor_start_music(owner, None, 0., 1.));
        assert!(audio.editor_logical_phase().is_none());
        assert!(audio.music_request.get().is_none());
        assert_eq!(deadlib_audio_core::music_asset_epoch(), epoch);
        assert_eq!(deadlib_audio_core::music_map_generation(), generation);
    }
    pub(super) fn control() -> (
        AudioControl,
        std::sync::mpsc::Receiver<StreamCommand>,
        std::sync::Arc<MixControls>,
    ) {
        let controls = std::sync::Arc::new(MixControls::new());
        let (sender, receiver) = std::sync::mpsc::channel();
        let (sfx, _receiver) = deadlib_audio_core::sfx_transport(8);
        let audio = AudioControl {
            engine: Some(AudioEngine {
                command_sender: sender,
                decoder_status: Default::default(),
                sfx_cache: SfxCache::new(controls.clone(), sfx),
                output: OutputFormat {
                    sample_rate_hz: 48000,
                    channels: 2,
                },
                startup_output_devices: Vec::new(),
                live: None,
            }),
            music_request: Cell::new(None),
            editor: Scope::new(),
            output_reconfigure: super::super::output_reconfigure::OutputReconfigure::new(),
        };
        (audio, receiver, controls)
    }
    #[test]
    fn asset_swaps_keep_editor_owner_clock_and_reject_old_failed_aba_completions() {
        let _serial = crate::CLOCK_TEST_LOCK.lock().unwrap();
        let (mut audio, receiver, _controls) = control();
        let (stream, _render) = deadlib_audio_core::music_transport(48_000, 2);
        let mut clock = crate::MusicClock::new(stream.played_map, 48_000);
        let mut runtime = stream::MusicStreamRuntime::new(
            stream.writer,
            OutputFormat {
                sample_rate_hz: 48_000,
                channels: 2,
            },
        );
        audio.engine.as_mut().unwrap().decoder_status = runtime.decoder_status();
        let owner = 817;
        assert!(audio.begin_editor_audio(
            owner,
            EditorAudioGains {
                main: 1.,
                music: 0.4,
                assist: 0.2,
                beat: 0.1
            }
        ));
        let missing = std::env::temp_dir().join(format!(
            "deadlib-never-exists-{}-asset.wav",
            std::process::id()
        ));
        assert!(!missing.exists());
        assert!(audio.editor_play_music(owner, missing.clone(), Cut::default(), false, 1.));
        runtime.handle(receiver.try_recv().unwrap());
        let generation = audio.music_command_generation().unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !audio
            .decoder_status()
            .is_some_and(|s| s.phase == stream::lifecycle::DecoderPhase::Failed)
        {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        let old_track_id = deadlib_audio_core::active_music_track_id();
        let before = deadlib_audio_core::music_asset_epoch();
        assert!(!audio.editor_swap_music_asset(owner + 1, missing.clone(), &mut clock));
        assert_eq!(deadlib_audio_core::music_asset_epoch(), before);
        assert!(audio.editor_swap_music_asset(
            owner,
            missing.with_file_name("B-missing.wav"),
            &mut clock
        ));
        let b = deadlib_audio_core::music_asset_epoch();
        let gain = deadlib_audio_core::music_target_gain();
        set_music_replaygain_if_matches(old_track_id, 0.125);
        assert_eq!(deadlib_audio_core::music_target_gain(), gain);
        assert!(
            audio.decoder_status().is_none(),
            "Failed A is stale despite identical logical generation"
        );
        assert!(audio.editor_swap_music_asset(owner, missing, &mut clock));
        let a2 = deadlib_audio_core::music_asset_epoch();
        assert!(a2 > b && b > before);
        runtime.handle(receiver.try_recv().unwrap());
        assert!(
            audio.decoder_status().is_none(),
            "stale B must not replace A2 status owner"
        );
        runtime.handle(receiver.try_recv().unwrap());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            audio.poll_editor_assets(&mut clock);
            while let Ok(command) = receiver.try_recv() {
                runtime.handle(command);
            }
            if let Some(snapshot) = audio.decoder_status() {
                assert_eq!(snapshot.asset_epoch, a2);
                if snapshot.phase == stream::lifecycle::DecoderPhase::Failed {
                    break;
                }
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(audio.music_command_generation(), Some(generation));
        assert_eq!(audio.editor_audio_status().owner, owner);
        assert_eq!(audio.editor_music_owner(), Some(owner));
        audio.end_editor_audio(owner);
        assert!(!audio.editor_swap_music_asset(owner, PathBuf::from("rejected.wav"), &mut clock));
    }
    #[test]
    fn queued_play_swap_stop_play_capture_distinct_asset_authorities_before_manager() {
        let _serial = crate::CLOCK_TEST_LOCK.lock().unwrap();
        let (audio, receiver, _) = control();
        let (stream, _) = deadlib_audio_core::music_transport(48_000, 2);
        let mut clock = crate::MusicClock::new(stream.played_map, 48_000);
        let mut runtime = stream::MusicStreamRuntime::new(
            stream.writer,
            OutputFormat {
                sample_rate_hz: 48_000,
                channels: 2,
            },
        );
        let owner = 823;
        assert!(audio.begin_editor_audio(
            owner,
            EditorAudioGains {
                main: 1.,
                music: 1.,
                assist: 1.,
                beat: 1.
            }
        ));
        assert!(audio.editor_play_music(
            owner,
            PathBuf::from("A-never-open.wav"),
            Cut::default(),
            false,
            1.
        ));
        let a = receiver.try_recv().unwrap();
        assert!(audio.editor_swap_music_asset(
            owner,
            PathBuf::from("B-never-open.wav"),
            &mut clock
        ));
        let b = receiver.try_recv().unwrap();
        assert!(audio.editor_stop_music(owner));
        let stop = receiver.try_recv().unwrap();
        assert!(audio.editor_play_music(
            owner,
            PathBuf::from("C-current-missing.wav"),
            Cut::default(),
            false,
            1.
        ));
        let c = receiver.try_recv().unwrap();
        let a_epoch = match &a {
            StreamCommand::PlayMusic { asset_epoch, .. } => *asset_epoch,
            _ => panic!(),
        };
        let b_epoch = match &b {
            StreamCommand::ClearMusicAsset { asset_epoch } => *asset_epoch,
            _ => panic!(),
        };
        let stop_epoch = match &stop {
            StreamCommand::StopMusic { asset_epoch } => *asset_epoch,
            _ => panic!(),
        };
        let c_epoch = match &c {
            StreamCommand::PlayMusic { asset_epoch, .. } => *asset_epoch,
            _ => panic!(),
        };
        assert!(a_epoch < b_epoch && b_epoch < stop_epoch && stop_epoch < c_epoch);
        runtime.handle(a);
        assert!(runtime.decoder_status().snapshot().is_none());
        runtime.handle(b);
        assert!(runtime.decoder_status().snapshot().is_none());
        runtime.handle(stop);
        assert!(
            deadlib_audio_core::music_track_active(),
            "stale Stop must not revoke the newly accepted C context"
        );
        runtime.handle(c);
        let state = runtime.decoder_status().snapshot().unwrap();
        assert_eq!(state.asset_epoch, c_epoch);
        assert_eq!(
            state.command_generation,
            deadlib_audio_core::music_map_generation()
        );
    }
    #[test]
    fn rate_during_asset_load_reanchors_music_seed_instead_of_multiplying_elapsed_origin() {
        let _serial = crate::CLOCK_TEST_LOCK.lock().unwrap();
        let (audio, _receiver, _) = control();
        let (stream, _) = deadlib_audio_core::music_transport(48_000, 2);
        let mut clock = crate::MusicClock::new(stream.played_map, 48_000);
        let owner = 827;
        assert!(audio.begin_editor_audio(
            owner,
            EditorAudioGains {
                main: 1.,
                music: 1.,
                assist: 1.,
                beat: 1.
            }
        ));
        assert!(audio.editor_play_music(
            owner,
            PathBuf::from("A-never-open.wav"),
            Cut {
                start_sec: 1.,
                ..Default::default()
            },
            false,
            1.
        ));
        assert!(audio.editor_swap_music_asset(
            owner,
            PathBuf::from("B-never-open.wav"),
            &mut clock
        ));
        let start = deadlib_audio_core::music_track_start_frame();
        deadlib_audio_core::publish_callback_window_end(start, 24_000);
        let elapsed = 0.5;
        let before = clock.snapshot_for_editor_time().0.music_nanos as f64 / 1e9;
        assert!(audio.editor_set_music_rate_at(owner, 2., &mut clock));
        let after = clock.snapshot_for_editor_time().0.music_nanos as f64 / 1e9;
        assert!(
            (after - before).abs() < 0.02,
            "rate change jumped seed from {before} to {after}"
        );
        let future = deadlib_audio_core::seeded_music_position(elapsed + 0.5)
            .unwrap()
            .0;
        let seeded_now = deadlib_audio_core::seeded_music_position(elapsed)
            .unwrap()
            .0;
        assert!((future - seeded_now - 1.).abs() < 1e-9);
    }

    #[test]
    fn beginning_gain_lease_does_not_attribute_existing_gameplay_music() {
        let _serial = crate::CLOCK_TEST_LOCK.lock().unwrap();
        let (audio, _receiver, _controls) = control();
        let gains = EditorAudioGains {
            main: 1.,
            music: 0.5,
            assist: 0.5,
            beat: 0.5,
        };
        audio.play_music(PathBuf::from("inherited.opus"), Cut::default(), false, 1.);
        assert_eq!(audio.editor_music_owner(), None);
        assert!(audio.begin_editor_audio(10, gains));
        assert_eq!(audio.editor_audio_status().owner, 10);
        assert_eq!(audio.editor_music_owner(), None);
        assert!(audio.editor_seek_music(10, 1.));
        assert_eq!(audio.editor_music_owner(), None);
        assert!(audio.editor_set_music_rate(10, 1.2));
        assert_eq!(audio.editor_music_owner(), None);
        assert!(!audio.editor_play_music(
            11,
            PathBuf::from("stale.opus"),
            Cut::default(),
            false,
            1.
        ));
        assert_eq!(audio.editor_music_owner(), None);
        assert!(audio.editor_play_music(
            10,
            PathBuf::from("sample.opus"),
            Cut::default(),
            false,
            1.
        ));
        assert_eq!(audio.editor_music_owner(), Some(10));
        assert!(audio.editor_seek_music(10, 2.));
        assert!(audio.editor_set_music_rate(10, 1.));
        assert_eq!(audio.editor_music_owner(), Some(10));
        assert!(!audio.editor_stop_music(11));
        assert_eq!(audio.editor_music_owner(), Some(10));
        assert!(audio.editor_stop_music(10));
        assert_eq!(audio.editor_music_owner(), None);
        assert!(audio.editor_play_music(
            10,
            PathBuf::from("chart.opus"),
            Cut::default(),
            false,
            1.
        ));
        audio.play_music(PathBuf::from("external.wav"), Cut::default(), false, 1.);
        assert_eq!(audio.editor_music_owner(), None);
        assert_eq!(audio.editor_audio_status().owner, 0);
    }
    #[test]
    fn release_reveals_new_external_levels_and_stale_editor_does_not_stop_next_owner() {
        let _serial = crate::CLOCK_TEST_LOCK.lock().unwrap();
        let (audio, receiver, controls) = control();
        controls.set_stream_gain(0.8);
        let gains = EditorAudioGains {
            main: 1.,
            music: 0.5,
            assist: 0.7,
            beat: 0.75,
        };
        assert!(audio.begin_editor_audio(10, gains));
        assert_eq!(controls.effective_stream_gain(), 0.5);
        controls.set_stream_gain(0.3);
        assert!(!audio.begin_editor_audio(11, gains));
        assert!(audio.editor_play_music(10, PathBuf::from("first.wav"), Cut::default(), false, 1.));
        assert!(matches!(
            receiver.try_recv().unwrap(),
            StreamCommand::PlayMusic { .. }
        ));
        assert!(audio.end_editor_audio(10));
        assert!(matches!(
            receiver.try_recv().unwrap(),
            StreamCommand::StopMusic { .. }
        ));
        assert_eq!(controls.effective_stream_gain(), 0.3);
        assert!(audio.begin_editor_audio(11, gains));
        assert!(!audio.end_editor_audio(10));
        assert!(receiver.try_recv().is_err());
        assert_eq!(audio.editor_audio_status().owner, 11);
        assert!(audio.end_editor_audio(11));
    }
    #[test]
    fn transport_boundaries_cancel_tick_windows_and_unscoped_transport_revokes_gain_owner() {
        let _serial = crate::CLOCK_TEST_LOCK.lock().unwrap();
        let (audio, receiver, controls) = control();
        let gains = EditorAudioGains {
            main: 1.,
            music: 0.5,
            assist: 0.7,
            beat: 0.75,
        };
        assert!(audio.begin_editor_audio(20, gains));
        let first = audio.editor_audio_status().tick_generation;
        assert!(audio.editor_play_music(20, PathBuf::from("first.wav"), Cut::default(), false, 1.));
        receiver.try_recv().unwrap();
        let play = audio.editor_audio_status().tick_generation;
        assert_ne!(first, play);
        let bus_generation = controls.bus_generation(ASSIST);
        assert!(audio.editor_seek_music(20, 1.5));
        receiver.try_recv().unwrap();
        assert_ne!(controls.bus_generation(ASSIST), bus_generation);
        assert!(audio.editor_set_music_rate(20, 1.5));
        receiver.try_recv().unwrap();
        assert_ne!(audio.editor_audio_status().tick_generation, play);
        audio.play_music(PathBuf::from("external.wav"), Cut::default(), false, 1.);
        assert!(matches!(
            receiver.try_recv().unwrap(),
            StreamCommand::StopMusic { .. }
        ));
        assert!(matches!(
            receiver.try_recv().unwrap(),
            StreamCommand::PlayMusic { .. }
        ));
        assert_eq!(audio.editor_audio_status().owner, 0);
        assert!(!audio.editor_stop_music(20));
        assert!(receiver.try_recv().is_err());
    }
}

impl AudioControl {
    /// Gain ownership alone never attributes the inherited gameplay decoder.
    /// Only an accepted EditorPlayMusic request owns this transport, across its
    /// subsequent seek/rate commands. Global playback releases the scope first.
    pub fn editor_music_owner(&self) -> Option<u64> {
        self.engine.as_ref()?;
        let owner = self.editor.state.get().owner;
        (owner != 0 && self.editor.music_owned.get() && self.music_request.get().is_some())
            .then_some(owner)
    }
}

// Pinned Fumen.Path.ArePathsEqual: strip the device prefix before converting
// backslashes. Do not resolve dot components, case or filesystem identities.
fn retained_music_paths_equal(a: &std::path::Path, b: &std::path::Path) -> bool {
    if a.as_os_str() == b.as_os_str() {
        return true;
    }
    let (Some(a), Some(b)) = (a.to_str(), b.to_str()) else {
        return false;
    };
    let strip = |path: &str| {
        path.strip_prefix(r"\\?\")
            .unwrap_or(path)
            .replace('\\', "/")
    };
    strip(a) == strip(b)
}
#[cfg(test)]
mod retained_path_tests {
    use super::retained_music_paths_equal as equal;
    use std::path::Path;
    #[test]
    fn source_equivalent_spelling_reuses_retained_reader() {
        assert!(equal(
            Path::new(r"\\?\C:\Music\a.wav"),
            Path::new("C:/Music/a.wav")
        ));
        assert!(!equal(
            Path::new("C:/Music/a.wav"),
            Path::new("c:/Music/a.wav")
        ));
        assert!(!equal(Path::new("a/./b.wav"), Path::new("a/b.wav")));
    }
}
#[cfg(test)]
mod source_tick_scope_tests {
    use super::*;
    use deadlib_audio_core::*;
    #[test]
    fn source_integer_tick_onsets_reach_real_callback_with_actual_prepared_editor_assets() {
        let _serial = crate::CLOCK_TEST_LOCK.lock().unwrap();
        for (kind, name, gain) in [
            (EditorTickKind::Assist, "editor_assist_tick.wav", 0.7f32),
            (EditorTickKind::Beat, "editor_beat_tick.wav", 0.75f32),
        ] {
            for rate in [1.1f64, 1.35] {
                let (mut audio, _commands, controls) = super::tests::control();
                let (sender, mut receiver) = sfx_transport(16);
                audio.engine.as_mut().unwrap().sfx_cache = SfxCache::new(controls.clone(), sender);
                let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../assets/sounds")
                    .join(name);
                let sound = audio.prepare_sfx(&path).unwrap();
                // This reference intentionally uses the current generic prepared bytes.
                // Original Fumen float preparation is a separately tracked fidelity gap.
                let dry = deadlib_audio::stream::load_and_resample_sfx(
                    &path,
                    OutputFormat {
                        sample_rate_hz: 48000,
                        channels: 2,
                    },
                    crate::DECODE_OPTIONS,
                )
                .unwrap();
                let (stream, handle) = music_transport(48000, 2);
                let mut clock = MusicClock::new(stream.played_map, 48000);
                let mut renderer = RenderState::new(handle, controls.clone(), 2);
                renderer.attach_sfx_retirement(receiver.take_retirement());
                let owner = 1701;
                assert!(audio.begin_editor_audio(
                    owner,
                    EditorAudioGains {
                        main: 1.,
                        music: 0.,
                        assist: 0.7,
                        beat: 0.75
                    }
                ));
                assert!(audio.editor_start_music(owner, None, 0., rate));
                let generation = audio.music_command_generation().unwrap();
                let tick = audio.editor_audio_status().tick_generation;
                let onsets = [17i64, 2253, 2254];
                assert!(!audio.editor_tick_window(
                    owner + 1,
                    tick,
                    generation,
                    kind,
                    &onsets,
                    &sound,
                    &mut clock
                ));
                assert!(!audio.editor_tick_window(
                    owner,
                    tick,
                    generation.wrapping_add(1),
                    kind,
                    &onsets,
                    &sound,
                    &mut clock
                ));
                assert!(audio.editor_tick_window(
                    owner, tick, generation, kind, &onsets, &sound, &mut clock
                ));
                let mut cursor = 0i64;
                let mut next = 0usize;
                let mut tail = None;
                for frames in [4097usize, 257] {
                    let mut actual = vec![0f32; frames * 2];
                    renderer.render(
                        OutputBufferMut::F32(&mut actual),
                        CallbackInfo {
                            anchor_nanos: deadlib_platform::host_time::instant_nanos(
                                std::time::Instant::now(),
                            ),
                            clock: CallbackClockSource::Instant,
                        },
                        receiver.try_iter(),
                    );
                    let end = cursor + (frames as f64 * rate) as i64;
                    for frame in 0..frames {
                        let scaled = cursor + (frame as f64 * rate) as i64;
                        if next < onsets.len() && onsets[next] < end && onsets[next] <= scaled {
                            tail = Some(0usize);
                            next += 1;
                        }
                        for ch in 0..2 {
                            let expected = tail
                                .and_then(|index| dry.get(index * 2 + ch))
                                .map_or(0., |v| i16_to_f32(*v) * gain);
                            assert_eq!(
                                actual[frame * 2 + ch].to_bits(),
                                expected.to_bits(),
                                "{name} rate{rate} frame{frame} ch{ch}"
                            );
                        }
                        if let Some(index) = tail.as_mut() {
                            *index += 1;
                        }
                    }
                    cursor = end;
                    assert_eq!(editor_main_cursor().unwrap().source_frame, cursor);
                }
                assert!(audio.end_editor_audio(owner));
            }
        }
    }
}
#[cfg(test)]
mod float_tick_scope_tests {
    use super::*;
    use deadlib_audio_core::*;
    #[test]
    fn actual_editor_float_assets_mix_with_separate_master_in_scoped_main_callback() {
        let _serial = crate::CLOCK_TEST_LOCK.lock().unwrap();
        for rate in [1.1f64, 1.35] {
            let (mut audio, _commands, controls) = super::tests::control();
            let (sender, mut receiver) = sfx_transport(16);
            audio.engine.as_mut().unwrap().sfx_cache = SfxCache::new(controls.clone(), sender);
            let format = OutputFormat {
                sample_rate_hz: 48000,
                channels: 2,
            };
            assert_eq!(
                audio.editor_tick_output_format().unwrap().sample_rate_hz,
                48000
            );
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/sounds");
            let a = root.join("editor_assist_tick.wav");
            let b = root.join("editor_beat_tick.wav");
            let assist = audio.prepare_editor_tick(&a).unwrap();
            let beat = audio.prepare_editor_tick(&b).unwrap();
            let cancelled = std::sync::atomic::AtomicBool::new(false);
            let dry_a = deadlib_audio_decode::editor_ticks::prepare(
                &a,
                crate::DECODE_OPTIONS,
                48000,
                2,
                &cancelled,
            )
            .unwrap();
            let dry_b = deadlib_audio_decode::editor_ticks::prepare(
                &b,
                crate::DECODE_OPTIONS,
                48000,
                2,
                &cancelled,
            )
            .unwrap();
            assert!(
                dry_a
                    .iter()
                    .any(|v| ((v * 32768.) as i16) as f32 / 32768. != *v),
                "float resampling must survive without i16 requantization"
            );
            assert_eq!(assist.duration(format), dry_a.len() as f64 / 96000.);
            let (mut stream, handle) = music_transport(48000, 2);
            let mut clock = MusicClock::new(stream.played_map, 48000);
            let mut renderer = RenderState::new(handle, controls.clone(), 2);
            renderer.attach_sfx_retirement(receiver.take_retirement());
            let owner = 1703;
            let gains = EditorAudioGains {
                main: 0.61,
                music: 0.43,
                assist: 0.71,
                beat: 0.79,
            };
            assert!(audio.begin_editor_audio(owner, gains));
            assert!(audio.editor_start_music(owner, None, 0., rate));
            let generation = audio.music_command_generation().unwrap();
            let epoch = music_asset_epoch();
            let tick = audio.editor_audio_status().tick_generation;
            stream
                .writer
                .try_publish_preview_source(PreviewSource {
                    editor_main: true,
                    samples: vec![-0.125; 40000].into(),
                    channels: 2,
                    sample_rate_hz: 48000,
                    generation,
                    asset_epoch: epoch,
                    initial_frame: 0,
                    looping: false,
                })
                .unwrap();
            stream
                .writer
                .publish_preview_generation_ready(generation, epoch);
            let onsets = [17i64, 2253, 2254];
            audio.engine.as_mut().unwrap().output.sample_rate_hz = 24000;
            assert!(!audio.editor_tick_window(
                owner,
                tick,
                generation,
                EditorTickKind::Assist,
                &onsets,
                &assist,
                &mut clock
            ));
            audio.engine.as_mut().unwrap().output = format;
            assert!(audio.editor_tick_window(
                owner,
                tick,
                generation,
                EditorTickKind::Assist,
                &onsets,
                &assist,
                &mut clock
            ));
            assert!(audio.editor_tick_window(
                owner,
                tick,
                generation,
                EditorTickKind::Beat,
                &onsets,
                &beat,
                &mut clock
            ));
            let mut cursor = 0i64;
            let mut next = 0;
            let mut tail = None;
            for frames in [4097usize, 257] {
                let mut actual = vec![0f32; frames * 2];
                renderer.render(
                    OutputBufferMut::F32(&mut actual),
                    CallbackInfo {
                        anchor_nanos: deadlib_platform::host_time::instant_nanos(
                            std::time::Instant::now(),
                        ),
                        clock: CallbackClockSource::Instant,
                    },
                    receiver.try_iter(),
                );
                let end = cursor + (frames as f64 * rate) as i64;
                for frame in 0..frames {
                    let scaled = cursor + (frame as f64 * rate) as i64;
                    if next < onsets.len() && onsets[next] < end && onsets[next] <= scaled {
                        tail = Some(0usize);
                        next += 1;
                    }
                    for ch in 0..2 {
                        let a = tail
                            .and_then(|i| dry_a.get(i * 2 + ch))
                            .copied()
                            .unwrap_or(0.)
                            * gains.assist;
                        let b = tail
                            .and_then(|i| dry_b.get(i * 2 + ch))
                            .copied()
                            .unwrap_or(0.)
                            * gains.beat;
                        let expected = (((-0.125 * gains.music) + a) + b) * gains.main;
                        assert_eq!(
                            actual[frame * 2 + ch].to_bits(),
                            expected.to_bits(),
                            "rate{rate} frame{frame} ch{ch}"
                        );
                    }
                    if let Some(i) = tail.as_mut() {
                        *i += 1;
                    }
                }
                cursor = end;
                assert_eq!(editor_main_cursor().unwrap().source_frame, cursor);
            }
            assert!(audio.end_editor_audio(owner));
        }
    }
}
