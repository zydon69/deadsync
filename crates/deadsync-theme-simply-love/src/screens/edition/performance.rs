//! On-demand keyboard adaptation of the source performance window.
//! Collection belongs to the published host and survives document replacement.
use super::*;
use deadsync_editor::performance::{
    monitor::{Monitor, Timing},
    preferences::{Preferences, Scale},
    summary::Summary,
};
use std::{
    cell::RefCell,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
mod render;

struct Recorder {
    monitor: Monitor,
    revision: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadlib_platform::input::RawKeyboardEvent;
    fn press(state: &mut State, code: KeyCode) {
        for pressed in [true, false] {
            state.raw_key(&RawKeyboardEvent {
                code,
                pressed,
                repeat: false,
                timestamp: Instant::now(),
                host_nanos: 0,
            });
        }
    }
    fn menu_item(state: &mut State, index: usize) {
        press(state, KeyCode::F1);
        assert!(matches!(state.panel, Some(Panel::FileMenu(0))));
        for _ in 0..index {
            press(state, KeyCode::ArrowDown);
        }
        press(state, KeyCode::Enter);
    }
    fn flattened<'a>(actors: &'a [Actor], result: &mut Vec<&'a Actor>) {
        for actor in actors {
            result.push(actor);
            match actor {
                Actor::Frame { children, .. } => flattened(children, result),
                Actor::SharedFrame { children, .. } | Actor::SharedTransform { children, .. } => {
                    flattened(children, result)
                }
                _ => {}
            }
        }
    }
    #[test]
    fn no_song_keyboard_reaches_performance_log_and_help_and_restore_is_undoable_en_fr() {
        for language in ["en", "fr"] {
            i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            let root = tempfile::tempdir().unwrap();
            let prefs = root.path().join("prefs.json");
            std::fs::write(&prefs,br#"{"Version":1,"Performance":{"MaxFramesToDraw":128,"FrameMaxTime":"99","ShowPerformanceWindow":true,"Future":17},"KeyBinds":{"Undo":[["F15"]],"Redo":[["F16"]]}}"#).unwrap();
            let mut state = State::without_song(root.path()).unwrap();
            state.configure_audio_preferences(&prefs).unwrap();
            state.panel = None;
            let original = state.performance_preferences();
            let song = state.workspace.file().session.serialize().unwrap();
            menu_item(&mut state, 16);
            assert!(matches!(state.panel, Some(Panel::Performance(0))));
            let mut actors = Vec::new();
            state.actors(&mut actors);
            let mut all = Vec::new();
            flattened(&actors, &mut all);
            assert!(all.iter().any(|actor|matches!(actor,Actor::Text{content,..}if content.as_str()==&*i18n::tr("Edition","PerformanceTitle"))));
            for _ in 0..4 {
                press(&mut state, KeyCode::ArrowDown);
            }
            press(&mut state, KeyCode::Enter);
            assert_eq!(state.performance_preferences(), original.restored());
            press(&mut state, KeyCode::Escape);
            press(&mut state, KeyCode::F15);
            assert_eq!(state.performance_preferences(), original);
            press(&mut state, KeyCode::F16);
            assert_eq!(state.performance_preferences(), original.restored());
            assert_eq!(state.workspace.file().session.serialize().unwrap(), song);
            assert!(!state.workspace.file().session.is_dirty());
            let disk: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&prefs).unwrap()).unwrap();
            assert_eq!(disk["Performance"]["Future"], 17);
            assert_eq!(disk["Performance"]["ShowPerformanceWindow"], true);
            menu_item(&mut state, 14);
            assert!(matches!(state.panel, Some(Panel::Log)));
            press(&mut state, KeyCode::Escape);
            menu_item(&mut state, 15);
            assert!(matches!(state.panel, Some(Panel::EditorHelp(0))));
            assert!(state.take_editor_external_open().is_none());
        }
    }
    #[test]
    fn actual_host_stages_pause_and_resume_without_recording_inactive_gap() {
        let root = tempfile::tempdir().unwrap();
        let state = State::without_song(root.path()).unwrap();
        let now = state.performance.epoch + Duration::from_secs(1);
        assert!(state.performance_summary().is_none());
        state.begin_performance_frame(now);
        state.record_performance_stage(Timing::Update, Duration::from_millis(2));
        state.record_performance_stage(Timing::ChartEvents, Duration::from_millis(1));
        state.begin_performance_frame(now + Duration::from_millis(16));
        let summary = state.performance_summary().unwrap();
        assert_eq!(summary.current[0], 0.016);
        assert_eq!(summary.current[Timing::Update as usize], 0.002);
        assert_eq!(summary.current[Timing::ChartEvents as usize], 0.001);
        state.suspend_performance_frame(now + Duration::from_millis(20));
        state.begin_performance_frame(now + Duration::from_secs(60));
        state.begin_performance_frame(now + Duration::from_secs(60) + Duration::from_millis(16));
        let summary = state.performance_summary().unwrap();
        assert!(summary.frames.iter().all(|frame| frame[0] < 1.0));
        assert_eq!(summary.current[0], 0.016);
    }
    #[test]
    fn document_snapshot_retains_monitor_and_pause_cache_is_stable() {
        let root = tempfile::tempdir().unwrap();
        let mut source = State::without_song(root.path()).unwrap();
        let now = source.performance.epoch + Duration::from_secs(1);
        source.begin_performance_frame(now);
        source.record_performance_stage(Timing::Update, Duration::from_millis(3));
        source.begin_performance_frame(now + Duration::from_millis(16));
        let snapshot =
            super::super::pack_song_list::application_preferences::Snapshot::capture(&source);
        let mut target = State::without_song(root.path()).unwrap();
        snapshot.install(&mut target).unwrap();
        assert!(Arc::ptr_eq(
            &source.performance.shared,
            &target.performance.shared
        ));
        assert_eq!(
            target.performance_summary().unwrap().current[Timing::Update as usize],
            0.003
        );
        let mut preferences = source.performance_preferences();
        preferences.paused = true;
        source
            .configure_performance_preferences(preferences)
            .unwrap();
        source.begin_performance_frame(now + Duration::from_millis(32));
        let first = source.performance_summary().unwrap();
        source.begin_performance_frame(now + Duration::from_millis(48));
        assert!(Arc::ptr_eq(&first, &source.performance_summary().unwrap()));
    }
    #[test]
    fn chart_pages_expose_every_stage_and_cancel_draft_without_disk_or_song_changes() {
        i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let mut state = State::without_song(root.path()).unwrap();
        state.panel = None;
        state.open_performance();
        let before = state.performance_preferences();
        press(&mut state, KeyCode::ArrowRight);
        assert_eq!(state.performance_preferences(), before);
        let now = state.performance.epoch + Duration::from_secs(1);
        state.begin_performance_frame(now);
        for timing in Timing::ALL.into_iter().skip(1) {
            state.record_performance_stage(timing, Duration::from_millis(1));
        }
        state.begin_performance_frame(now + Duration::from_millis(16));
        for expected in [8, 11, 14] {
            press(&mut state, KeyCode::PageDown);
            assert!(matches!(state.panel,Some(Panel::Performance(index))if index==expected));
            let mut actors = Vec::new();
            state.actors(&mut actors);
            let mut all = Vec::new();
            flattened(&actors, &mut all);
            assert_eq!(
                all.iter()
                    .filter(|actor| matches!(actor, Actor::Mesh { .. }))
                    .count(),
                3
            );
        }
        press(&mut state, KeyCode::Escape);
        assert_eq!(state.performance_preferences(), before);
        assert!(state.workspace.file().session.undo_label().is_none());
    }
    #[test]
    fn profiling_panel_preserves_scoped_transport_and_keyboard_controls_playback() {
        use deadsync_gameplay::{
            EditorBufferControl, GameplayAudioCommand as Command, GameplayAudioSnapshot,
        };
        i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("music.ogg"), b"command-only fixture").unwrap();
        let path = root.path().join("song.ssc");
        std::fs::write(&path, r#"#VERSION:0.83;#MUSIC:music.ogg;#BPMS:0=120;#GASongDataVersion:1;#GASongData:{"SyncOffset":0};#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;"#).unwrap();
        let mut state = State::open(&path, 0).unwrap();
        let owner = state
            .take_audio_commands()
            .iter()
            .find_map(|command| match command {
                Command::EditorBeginAudio { owner, .. } => Some(*owner),
                _ => None,
            })
            .unwrap();
        let mut snapshot = GameplayAudioSnapshot::default();
        snapshot.editor.owner = owner;
        snapshot.editor.output.available = true;
        snapshot.editor.output.leased = true;
        snapshot.editor.output.period_control = EditorBufferControl::Adjustable;
        snapshot.editor.output.requested_period = Some(512);
        snapshot.editor.output.negotiated_period = Some(512);
        state.update_audio(snapshot);
        assert!(state.take_audio_commands().is_empty());
        let bytes = state.workspace.file().session.serialize().unwrap();
        state.toggle_audio();
        assert!(
            matches!(&state.take_audio_commands()[..], [Command::EditorStartMusic {owner: actual, ..}] if *actual == owner)
        );
        assert!(state.is_playing());
        state.open_performance();
        assert!(matches!(state.panel, Some(Panel::Performance(0))));
        assert!(state.is_playing());
        assert!(
            state.take_audio_commands().is_empty(),
            "opening profiling must preserve the audio lease"
        );
        for _ in 0..6 {
            press(&mut state, KeyCode::ArrowDown);
        }
        press(&mut state, KeyCode::Enter);
        assert!(!state.is_playing());
        assert!(!state.take_audio_commands().is_empty());
        press(&mut state, KeyCode::Enter);
        assert!(state.is_playing());
        assert!(
            matches!(&state.take_audio_commands()[..], [Command::EditorResumeMusic {owner: actual, ..}] if *actual == owner)
        );
        assert_eq!(state.workspace.file().session.serialize().unwrap(), bytes);
        assert!(state.workspace.file().session.undo_label().is_none());
        assert!(!state.workspace.file().session.is_dirty());
    }
}
struct Cached {
    revision: u64,
    preferences: Preferences,
    summary: Option<Arc<Summary>>,
}
pub(super) struct Runtime {
    shared: Arc<Mutex<Recorder>>,
    epoch: Instant,
    preferences: Preferences,
    draft: Option<Preferences>,
    cache: RefCell<Option<Cached>>,
}
impl Default for Runtime {
    fn default() -> Self {
        Self {
            shared: Arc::new(Mutex::new(Recorder {
                monitor: Monitor::default(),
                revision: 0,
            })),
            epoch: Instant::now(),
            preferences: Preferences::default(),
            draft: None,
            cache: RefCell::new(None),
        }
    }
}
impl Clone for Runtime {
    fn clone(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
            epoch: self.epoch,
            preferences: self.preferences,
            draft: None,
            cache: RefCell::new(None),
        }
    }
}
impl State {
    pub fn performance_preferences(&self) -> Preferences {
        self.performance.preferences
    }
    pub fn configure_performance_preferences(
        &mut self,
        preferences: Preferences,
    ) -> Result<(), deadsync_editor::Error> {
        preferences.validate()?;
        self.performance.preferences = preferences;
        self.performance.cache.replace(None);
        Ok(())
    }
    /// Only the published host invokes this; candidates never collect timings.
    pub fn begin_performance_frame(&self, now: Instant) {
        let Ok(mut recorder) = self.performance.shared.lock() else {
            return;
        };
        let was_enabled = recorder.monitor.enabled();
        recorder
            .monitor
            .set_enabled(!self.performance.preferences.paused);
        let ticks = now
            .saturating_duration_since(self.performance.epoch)
            .as_nanos()
            .min(i64::MAX as u128) as i64;
        recorder.monitor.begin_frame(ticks);
        if was_enabled || !self.performance.preferences.paused {
            recorder.revision = recorder.revision.wrapping_add(1);
        }
    }
    pub(super) fn suspend_performance_frame(&self, now: Instant) {
        let Ok(mut recorder) = self.performance.shared.lock() else {
            return;
        };
        recorder.monitor.set_enabled(false);
        let ticks = now
            .saturating_duration_since(self.performance.epoch)
            .as_nanos()
            .min(i64::MAX as u128) as i64;
        recorder.monitor.begin_frame(ticks);
        recorder.revision = recorder.revision.wrapping_add(1);
    }
    pub fn record_performance_stage(&self, timing: Timing, duration: Duration) {
        if let Ok(mut recorder) = self.performance.shared.lock() {
            recorder
                .monitor
                .add(timing, duration.as_nanos().min(i64::MAX as u128) as i64);
        }
    }
    pub fn performance_summary(&self) -> Option<Arc<Summary>> {
        let recorder = self.performance.shared.lock().ok()?;
        let preferences = self.performance.preferences;
        let mut cache = self.performance.cache.borrow_mut();
        if let Some(cached) = cache.as_ref()
            && cached.revision == recorder.revision
            && cached.preferences == preferences
        {
            return cached.summary.clone();
        }
        let summary = Summary::capture(&recorder.monitor, preferences, 1_000_000_000).map(Arc::new);
        *cache = Some(Cached {
            revision: recorder.revision,
            preferences,
            summary: summary.clone(),
        });
        summary
    }
    pub(super) fn sync_performance_history(&mut self) -> Result<(), deadsync_editor::Error> {
        if let Some(update) = self.workspace.take_performance_preferences_update() {
            self.configure_performance_preferences(update.preferences)?;
            self.performance.draft = None;
            if let Some(file) = update.file {
                self.publish_application_preferences_owner(file);
            }
        }
        Ok(())
    }
    pub(super) fn open_performance(&mut self) {
        if !matches!(
            self.panel,
            None | Some(Panel::Menu(_) | Panel::EmptyMenu(_))
        ) {
            return;
        }
        self.minimap_release_input();
        self.receptor_views.release_input();
        self.performance.draft = Some(self.performance.preferences);
        self.panel = Some(Panel::Performance(0));
        self.status = i18n::tr("Edition", "PerformanceHelp");
    }
    fn commit_performance(&mut self, restore: bool) -> Result<(), deadsync_editor::Error> {
        let file = self.capture_application_preferences_file()?;
        let before = self.performance.preferences;
        if restore {
            self.workspace
                .restore_performance_preferences(before, Some(&file))?;
        } else if let Some(after) = self.performance.draft {
            self.workspace
                .set_performance_preferences(before, after, Some(&file))?;
        }
        self.sync_performance_history()?;
        self.performance.draft = Some(self.performance.preferences);
        Ok(())
    }
    pub(super) fn performance_numeric_draft(&self) -> Option<Preferences> {
        self.performance.draft
    }
    pub(super) fn set_performance_numeric_draft(
        &mut self,
        p: Preferences,
    ) -> Result<(), deadsync_editor::Error> {
        p.validate()?;
        self.performance.draft = Some(p);
        Ok(())
    }
    pub(super) fn performance_key(&mut self, index: usize, code: KeyCode) {
        let enter = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter);
        if code == KeyCode::Escape || (index == 7 && enter) {
            self.performance.draft = None;
            self.panel = None;
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let back = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            self.panel = Some(Panel::Performance((index + if back { 16 } else { 1 }) % 17));
            return;
        }
        if matches!(code, KeyCode::PageUp | KeyCode::PageDown) {
            let next = if code == KeyCode::PageUp {
                if index < 11 { 0 } else { index - 3 }
            } else if index < 8 {
                8
            } else if index < 14 {
                index + 3
            } else {
                0
            };
            self.panel = Some(Panel::Performance(next));
            return;
        }
        if index == 6 && enter {
            self.toggle_audio();
            return;
        }
        if matches!(index, 4 | 5) && enter {
            self.status = match self.commit_performance(index == 4) {
                Ok(()) => i18n::tr("Edition", "PerformanceSaved"),
                Err(error) => i18n::tr_fmt(
                    "Edition",
                    "PerformanceSaveFailed",
                    &[("error", &i18n::editor_error(&error))],
                ),
            };
            return;
        }
        if index >= 4
            || !matches!(
                code,
                KeyCode::ArrowLeft
                    | KeyCode::ArrowRight
                    | KeyCode::Enter
                    | KeyCode::NumpadEnter
                    | KeyCode::Home
                    | KeyCode::Digit0
                    | KeyCode::Numpad0
            )
        {
            return;
        }
        let draft = self
            .performance
            .draft
            .get_or_insert(self.performance.preferences);
        let reset = matches!(code, KeyCode::Home | KeyCode::Digit0 | KeyCode::Numpad0);
        let backwards = code == KeyCode::ArrowLeft;
        match index {
            0 => draft.paused = if reset { false } else { !draft.paused },
            1 => {
                draft.scale = if reset {
                    Scale::Shared
                } else {
                    let index = match draft.scale {
                        Scale::Independent => 0,
                        Scale::Shared => 1,
                        Scale::Explicit => 2,
                        Scale::Unknown(_) => {
                            if backwards {
                                1
                            } else {
                                2
                            }
                        }
                    };
                    [Scale::Independent, Scale::Shared, Scale::Explicit]
                        [(index + if backwards { 2 } else { 1 }) % 3]
                }
            }
            2 if !enter && draft.scale == Scale::Explicit => {
                draft.explicit_seconds = if reset {
                    1.0 / 60.0
                } else {
                    (draft.explicit_seconds + if backwards { -0.0001 } else { 0.0001 })
                        .clamp(0.0, 1.0)
                }
            }
            3 if !enter => {
                draft.max_frames = if reset {
                    512
                } else {
                    draft
                        .max_frames
                        .saturating_add(if backwards { -1 } else { 1 })
                        .clamp(2, 1024)
                }
            }
            _ => {}
        }
    }
}
