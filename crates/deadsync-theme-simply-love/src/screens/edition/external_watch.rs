//! Keyboard notification of externally changed bytes. Disk IO never runs in a
//! frame callback. A cancelled worker retains its global slot until it exits.
use super::*;
use deadsync_editor::{
    Error,
    storage::{ExternalObservation, ExternalSnapshot},
};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
    time::{Duration, Instant},
};
static WORKER_BUSY: AtomicBool = AtomicBool::new(false);
struct Permit<'a>(&'a AtomicBool);
impl Drop for Permit<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
fn try_acquire(gate: &AtomicBool) -> Option<Permit<'_>> {
    gate.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .ok()
        .map(|_| Permit(gate))
}
enum Probe {
    Observed(ExternalObservation),
    Ignored,
}
struct Flight {
    snapshot: ExternalSnapshot,
    cancel: Arc<AtomicBool>,
    receiver: Receiver<Result<Probe, Error>>,
}
impl Drop for Flight {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}
#[derive(Clone)]
struct Notice {
    snapshot: ExternalSnapshot,
    observation: ExternalObservation,
}
#[derive(Default)]
pub(super) struct Runtime {
    context: Option<ExternalSnapshot>,
    flight: Option<Flight>,
    last: Option<Instant>,
    notice: Option<Notice>,
    ignored: Option<Notice>,
    reported_error: bool,
}
impl Runtime {
    fn update(
        &mut self,
        file: &deadsync_editor::storage::FileSession,
        now: Instant,
        allowed: bool,
        suppressed: bool,
    ) -> Option<Result<(), Error>> {
        if self
            .context
            .as_ref()
            .is_some_and(|snapshot| !snapshot.is_current(file))
        {
            self.context = None;
            self.notice = None;
            self.ignored = None;
            self.reported_error = false;
        }
        if self.context.is_none() {
            self.context = file.external_snapshot();
        }
        if self
            .notice
            .as_ref()
            .is_some_and(|n| !n.snapshot.is_current(file))
        {
            self.notice = None;
        }
        if self
            .ignored
            .as_ref()
            .is_some_and(|n| !n.snapshot.is_current(file))
        {
            self.ignored = None;
            self.reported_error = false;
        }
        let result = self
            .flight
            .as_ref()
            .and_then(|flight| match flight.receiver.try_recv() {
                Ok(result) => Some(result),
                Err(TryRecvError::Disconnected) => {
                    Some(Err(Error::InvalidEdit("External watcher stopped".into())))
                }
                Err(TryRecvError::Empty) => None,
            });
        if let Some(result) = result {
            let flight = self.flight.take().unwrap();
            if flight.snapshot.is_current(file)
                && !flight.cancel.load(Ordering::Acquire)
                && !suppressed
            {
                match result {
                    Ok(Probe::Observed(ExternalObservation::Unchanged)) => {
                        self.ignored = None;
                        self.reported_error = false;
                    }
                    Ok(Probe::Ignored) => {
                        self.reported_error = false;
                    }
                    Ok(Probe::Observed(observation)) => {
                        self.reported_error = false;
                        self.notice = Some(Notice {
                            snapshot: flight.snapshot.clone(),
                            observation,
                        });
                    }
                    Err(error) => {
                        if !self.reported_error {
                            self.reported_error = true;
                            return Some(Err(error));
                        }
                    }
                }
            }
        }
        if let Some(flight) = &self.flight {
            if suppressed || !flight.snapshot.is_current(file) {
                flight.cancel.store(true, Ordering::Release);
            }
        }
        if suppressed {
            self.notice = None;
            self.ignored = None;
            return None;
        }
        if allowed && self.notice.is_some() {
            return Some(Ok(()));
        }
        if !allowed || self.flight.is_some() || self.notice.is_some() {
            return None;
        }
        if self
            .last
            .is_some_and(|last| now.saturating_duration_since(last) < Duration::from_secs(1))
        {
            return None;
        }
        self.last = Some(now);
        let Some(snapshot) = file.external_snapshot() else {
            return None;
        };
        let Some(permit) = try_acquire(&WORKER_BUSY) else {
            return None;
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let worker_snapshot = snapshot.clone();
        let ignored = self
            .ignored
            .as_ref()
            .map(|notice| notice.observation.clone());
        let (sender, receiver) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("editor-external-watch".into())
            .spawn(move || {
                let _permit = permit;
                let result = worker_snapshot.probe(&worker_cancel).map(|observation| {
                    if ignored.as_ref() == Some(&observation) {
                        Probe::Ignored
                    } else {
                        Probe::Observed(observation)
                    }
                });
                let _ = sender.send(result);
            });
        match spawned {
            Ok(_) => {
                self.flight = Some(Flight {
                    snapshot,
                    cancel,
                    receiver,
                })
            }
            Err(error) => return Some(Err(error.into())),
        };
        None
    }
    fn ignore(&mut self) {
        self.ignored = self.notice.take();
    }
}
impl State {
    pub(super) fn update_external_watch(&mut self, now: Instant) {
        let suppressed = self
            .application_preferences_owner()
            .is_some_and(|file| file.suppress_external_song_notifications());
        let allowed = self.panel.is_none()
            && !self.save_as_pending()
            && !self.file_io_active()
            && !self.pack_file_io_active()
            && !self.auto_recovery.is_pending()
            && self.analysis_job.is_none()
            && self.job.is_none()
            && self.catalog_job.is_none()
            && self.preview.is_none();
        match self
            .external_watch
            .update(self.workspace.file(), now, allowed, suppressed)
        {
            Some(Ok(())) => {
                // Source ShowFileChangedModal only presents a notification.
                // Ignore/suppression must leave the loaded transport running.
                self.panel = Some(Panel::ExternalFile(0));
                self.status = i18n::tr("Edition", "FileExternalHelp");
            }
            Some(Err(error)) => {
                log::warn!("Editor external watcher: {error}");
                self.status = i18n::tr(
                    "Edition",
                    if matches!(error, Error::Unsupported(_)) {
                        "FileExternalUnsupported"
                    } else {
                        "FileExternalProbeFailed"
                    },
                );
            }
            None => {}
        }
        if matches!(self.panel, Some(Panel::ExternalFile(_)))
            && self.external_watch.notice.is_none()
        {
            self.panel = None;
            self.status = i18n::tr("Edition", "FileExternalStale");
        }
    }
    pub(super) fn external_file_key(&mut self, index: usize, code: KeyCode) {
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let backward = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            self.panel = Some(Panel::ExternalFile(
                (index + if backward { 2 } else { 1 }) % 3,
            ));
            return;
        }
        if code == KeyCode::Escape
            || (matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) && index == 0)
        {
            self.external_watch.ignore();
            self.panel = None;
            self.status = i18n::tr("Edition", "FileExternalIgnored");
            return;
        }
        if !matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) {
            return;
        }
        let Some(notice) = self.external_watch.notice.clone() else {
            self.panel = None;
            return;
        };
        if !notice.snapshot.is_current(self.workspace.file()) {
            self.external_watch.notice = None;
            self.panel = None;
            self.status = i18n::tr("Edition", "FileExternalStale");
            return;
        }
        if index == 2 {
            if let Some(file) = self.application_preferences_owner_mut() {
                file.set_suppress_external_song_notifications(true);
            } else {
                self.status = i18n::tr("Edition", "FileExternalPrefsUnavailable");
                return;
            }
            self.external_watch.ignore();
            self.panel = None;
            self.status = i18n::tr("Edition", "FileExternalSuppressed");
        } else {
            self.external_watch.ignore();
            self.panel = None;
            self.command_file_reload_external(&notice.snapshot);
        }
    }
    pub(super) fn external_file_actors(&self, index: usize, actors: &mut Vec<Actor>) {
        let x = screen_width() * 0.5;
        let y = (screen_height() * 0.5 - 100.0).max(110.0);
        actors.push(
            act!(quad: xy(x,y+80.0): setsize(720.0,240.0): diffuse(0.04,0.06,0.1,0.99): z(8050)),
        );
        actors.push(act!(text: font("miso"): settext(i18n::tr("Edition","FileExternalTitle")): xy(x,y-12.0): zoom(0.7): maxwidth(660.0): z(8051)));
        let path = self
            .external_watch
            .notice
            .as_ref()
            .map(|n| n.snapshot.path().to_string_lossy().into_owned())
            .unwrap_or_default();
        actors.push(act!(text: font("miso"): settext(path): xy(x,y+20.0): zoom(0.5): maxwidth(660.0): z(8051)));
        if self.workspace.file().has_unsaved_document() || self.pack_has_unsaved_changes() {
            actors.push(act!(text: font("miso"): settext(i18n::tr("Edition","FileExternalUnsaved")): xy(x,y+49.0): zoom(0.5): maxwidth(660.0): diffuse(1.0,0.8,0.3,1.0): z(8051)));
        }
        for (row, key) in [
            "FileExternalIgnore",
            "FileExternalReload",
            "FileExternalSuppress",
        ]
        .into_iter()
        .enumerate()
        {
            let ui_row_start = actors.len();
            actors.push(act!(text: font("miso"): settext(i18n::tr("Edition",key)): xy(x,y+82.0+row as f32*31.0): zoom(0.6): maxwidth(660.0): diffuse(1.0,if row==index{0.8}else{1.0},if row==index{0.3}else{1.0},1.0): z(8051)));

            self.ui_row(actors, ui_row_start, row == index, 31.);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct NoSort;
    impl deadsync_editor::chart_order::Orderer for NoSort {
        fn order(
            &mut self,
            _: &deadsync_editor::chart_order::Snapshot,
        ) -> Result<Vec<usize>, Error> {
            panic!("one chart needs no cultural sort")
        }
    }
    fn state(language: &str) -> (tempfile::TempDir, State) {
        i18n::init(deadsync_assets::language::load_for_tests(language));
        deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
            854.0, 480.0,
        ));
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("source.ssc");
        std::fs::write(&path,b"#VERSION:0.83;#TITLE:Native;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        let state = State::open(&path, 0).unwrap();
        (root, state)
    }
    fn changed(state: &mut State) {
        let snapshot = state.workspace.file().external_snapshot().unwrap();
        std::fs::write(snapshot.path(),b"#VERSION:0.83;#TITLE:External;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0100;").unwrap();
        let observation = snapshot.probe(&AtomicBool::new(false)).unwrap();
        state.external_watch.notice = Some(Notice {
            snapshot,
            observation,
        });
        state.panel = Some(Panel::ExternalFile(0));
    }
    #[test]
    fn ignore_keeps_native_dirty_and_save_still_refuses_external_en_fr() {
        for language in ["en", "fr"] {
            let (_root, mut state) = state(language);
            state.workspace.execute(Command::Place(1)).unwrap();
            let before = state.workspace.file().session.serialize().unwrap();
            changed(&mut state);
            state.external_file_key(0, KeyCode::Enter);
            assert!(state.panel.is_none());
            assert!(state.external_watch.ignored.is_some());
            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
            assert!(state.workspace.file().session.is_dirty());
            assert!(matches!(
                state.workspace.execute(Command::Save),
                Err(Error::ExternalChange)
            ));
        }
    }
    #[test]
    fn dirty_reload_requires_regular_file_review_and_cancel_preserves_source_en_fr() {
        for language in ["en", "fr"] {
            let (_root, mut state) = state(language);
            state.workspace.execute(Command::Place(1)).unwrap();
            changed(&mut state);
            let native = state.workspace.file().session.serialize().unwrap();
            state.external_file_key(1, KeyCode::Enter);
            assert!(matches!(state.panel, Some(Panel::FileUnsaved(0))));
            assert!(!state.file_io_active());
            state.file_menu_key(0, KeyCode::Enter);
            assert!(state.panel.is_none());
            assert_eq!(state.workspace.file().session.serialize().unwrap(), native);
            assert!(state.workspace.file().session.is_dirty());
        }
    }
    #[test]
    fn stale_probe_does_not_reappear_after_publication_or_source_replacement() {
        let (_root, mut state) = state("en");
        let snapshot = state.workspace.file().external_snapshot().unwrap();
        let (sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        state.external_watch.flight = Some(Flight {
            snapshot,
            cancel,
            receiver,
        });
        state.workspace.execute(Command::Save).unwrap();
        sender
            .send(Ok(Probe::Observed(ExternalObservation::Missing)))
            .unwrap();
        let now = Instant::now();
        assert!(
            state
                .external_watch
                .update(state.workspace.file(), now, false, false)
                .is_none()
        );
        assert!(state.external_watch.notice.is_none());
        assert!(state.external_watch.flight.is_none());
    }
    #[test]
    fn clean_reload_targets_current_path_instead_of_mru_zero_en_fr() {
        for language in ["en", "fr"] {
            let (root, mut state) = state(language);
            let current = state
                .workspace
                .file()
                .require_existing_path()
                .unwrap()
                .to_owned();
            let other = root.path().join("other.ssc");
            std::fs::write(&other, b"#VERSION:0.83;#TITLE:Other;").unwrap();
            let other_state = State::open(&other, 0).unwrap();
            let song = other_state.capture_recent_song().unwrap().unwrap();
            let prefs = root.path().join("prefs.json");
            std::fs::write(&prefs, b"{\"Version\":1}").unwrap();
            let mut owner = deadsync_editor::audio_preferences::File::open(&prefs).unwrap();
            owner.record_recent_song(&song).unwrap();
            state.audio_preferences_file = Some(owner);
            changed(&mut state);
            state.external_file_key(1, KeyCode::Enter);
            let deadline = Instant::now() + Duration::from_secs(10);
            while state.file_io_active() {
                state.process_file_menu(&mut NoSort);
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(2));
            }
            assert_eq!(
                state.workspace.file().existing_path(),
                Some(current.as_path())
            );
            assert_eq!(
                state
                    .workspace
                    .file()
                    .session
                    .document()
                    .text(None, "TITLE")
                    .as_deref(),
                Some("External")
            );
        }
    }
    #[test]
    fn source_memory_import_can_suppress_external_song_and_flush_without_disk_path() {
        let (_root, mut state) = state("en");
        let file = state.capture_application_preferences_file().unwrap();
        let draft = state
            .workspace
            .prepare_source_preferences(
                &file,
                br#"{"VendorPreserved":{"opaque":1}}"#,
                deadsync_editor::key_binding_preferences::source_profile::Platform::Mac,
            )
            .unwrap();
        state
            .workspace
            .import_source_preferences(draft, &file, true)
            .unwrap();
        state.sync_source_preferences_history().unwrap();
        let label = state
            .workspace
            .file()
            .session
            .undo_label()
            .map(str::to_owned);
        changed(&mut state);
        state.external_file_key(2, KeyCode::Enter);
        assert!(
            state
                .application_preferences_owner()
                .unwrap()
                .suppress_external_song_notifications()
        );
        assert!(state.external_watch.notice.is_none());
        state.flush_wheel_settings().unwrap();
        assert!(!state.application_preferences_owner().unwrap().has_changes());
        assert!(state.audio_preferences_file.is_none());
        assert!(state.audio_preferences_path.is_none());
        assert_eq!(
            state.workspace.file().session.undo_label(),
            label.as_deref()
        );
    }
    #[test]
    fn suppression_stages_only_shared_option_and_keeps_other_owners() {
        let (root, mut state) = state("en");
        let prefs = root.path().join("prefs.json");
        std::fs::write(
            &prefs,
            b"{\"Version\":1,\"Options\":{\"NewSongSyncOffset\":0.017,\"Future\":73},\"Other\":19}",
        )
        .unwrap();
        state.audio_preferences_file =
            Some(deadsync_editor::audio_preferences::File::open(&prefs).unwrap());
        changed(&mut state);
        state.external_file_key(2, KeyCode::Enter);
        let owner = state.audio_preferences_file.as_mut().unwrap();
        assert!(owner.suppress_external_song_notifications());
        owner.save().unwrap();
        let raw: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&prefs).unwrap()).unwrap();
        assert_eq!(
            raw["Options"]["SuppressExternalSongModificationNotification"],
            true
        );
        assert_eq!(raw["Options"]["Future"], 73);
        assert_eq!(raw["Other"], 19);
        assert_eq!(raw["Options"]["NewSongSyncOffset"], 0.017);
    }
    #[test]
    fn cancellation_retains_worker_permit_until_completion_and_unwind_releases_it() {
        let gate = AtomicBool::new(false);
        let permit = try_acquire(&gate).unwrap();
        let cancelled = AtomicBool::new(false);
        cancelled.store(true, Ordering::Release);
        for _ in 0..20 {
            assert!(try_acquire(&gate).is_none());
        }
        drop(permit);
        assert!(try_acquire(&gate).is_some());
        let _ = std::panic::catch_unwind(|| {
            let _permit = try_acquire(&gate).unwrap();
            panic!("injected worker unwind");
        });
        assert!(try_acquire(&gate).is_some());
    }
    #[test]
    fn pinned_source_oracle_and_intentional_keyboard_adaptations() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../deadsync-editor/tests/fixtures/external-watch/expected.json");
        let cases: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let case = |name: &str| {
            cases
                .as_array()
                .unwrap()
                .iter()
                .find(|case| case["name"] == name)
                .unwrap()
        };
        assert_eq!(case("ignore")["reload_count"], 0);
        assert_eq!(case("ignore")["shown"], false);
        assert_eq!(case("reload_dirty_song")["reload_forced"], true);
        assert_eq!(
            case("reload_dirty_song")["warnings"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(case("suppressed")["modal_count"], 0);
        assert_eq!(case("grace")["modal_count"], 0);
        let (_root, mut state) = state("en");
        // Unlike the source's three-second heuristic, actual changed bytes
        // are reported even immediately after a successful own publication.
        state.workspace.execute(Command::Save).unwrap();
        changed(&mut state);
        state.panel = None;
        state.update_external_watch(Instant::now());
        assert!(matches!(state.panel, Some(Panel::ExternalFile(0))));
        state.external_file_key(0, KeyCode::Enter);
        assert!(state.panel.is_none());
        assert!(!state.file_io_active());
        // Native Reload retains the explicit dirty-song controller review;
        // it never interprets a warning render as permission to discard.
        state.workspace.execute(Command::Place(1)).unwrap();
        changed(&mut state);
        state.external_file_key(1, KeyCode::Enter);
        assert!(matches!(state.panel, Some(Panel::FileUnsaved(0))));
        assert!(!state.file_io_active());
    }
}

#[cfg(test)]
mod preference_notification_consumer_tests {
    use super::*;
    use std::time::{Duration, Instant};
    #[test]
    fn general_setting_reenables_real_external_song_probe_without_resetting_other_options() {
        i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.ssc");
        std::fs::write(&path,b"#VERSION:0.83;#TITLE:Original;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        let mut state = State::open(&path, 0).unwrap();
        let file = state.capture_application_preferences_file().unwrap();
        let draft = state
            .workspace
            .prepare_source_preferences(
                &file,
                br#"{"PreferencesOptions":{"SuppressExternalSongModificationNotification":true}}"#,
                deadsync_editor::key_binding_preferences::source_profile::Platform::Mac,
            )
            .unwrap();
        state
            .workspace
            .import_source_preferences(draft, &file, true)
            .unwrap();
        state.sync_source_preferences_history().unwrap();
        let other = state.general_options.clone();
        std::fs::write(&path,b"#VERSION:0.83;#TITLE:Actually replaced externally;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0100;").unwrap();
        let now = Instant::now();
        state.update_external_watch(now);
        assert!(state.external_watch.notice.is_none());
        assert!(state.external_watch.flight.is_none());
        assert!(state.panel.is_none());
        state.open_general_options();
        state.general_field = 13;
        state.panel = Some(Panel::GeneralOptions(1));
        state.general_options_key(1, KeyCode::Enter);
        assert!(!state.options_snapshot().suppress_external_song);
        state.general_options_key(1, KeyCode::Escape);
        assert_eq!(state.general_options, other);
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut attempt = 1u64;
        while state.external_watch.notice.is_none() && Instant::now() < deadline {
            state.update_external_watch(now + Duration::from_secs(attempt * 2));
            attempt += 1;
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(
            state.external_watch.notice.is_some(),
            "actual changed file must reach normal notification consumer: {}",
            state.status
        );
        assert!(matches!(state.panel, Some(Panel::ExternalFile(0))));
        assert_eq!(
            state
                .workspace
                .file()
                .session
                .document()
                .text(None, "TITLE")
                .as_deref(),
            Some("Original")
        );
    }
}

#[cfg(test)]
mod notification_transport_tests {
    use super::*;

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

    #[test]
    fn real_external_notification_ignore_and_suppression_preserve_main_transport_en_fr() {
        for language in ["en", "fr"] {
            for suppress in [false, true] {
                i18n::init(deadsync_assets::language::load_for_tests(language));
                let root = tempfile::tempdir().unwrap();
                let path = root.path().join("song.ssc");
                std::fs::write(&path, "#VERSION:0.83;#TITLE:Original;#MUSIC:missing.wav;#BPMS:0=120;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
                let mut state = State::open(&path, 0).unwrap();
                let preferences = root.path().join("prefs.json");
                std::fs::write(&preferences, br#"{"Version":1}"#).unwrap();
                state.configure_audio_preferences(&preferences).unwrap();
                let owner = state.editor_asset_owner();
                state.take_audio_commands();
                let mut audio = deadsync_gameplay::GameplayAudioSnapshot::default();
                audio.editor.owner = owner;
                audio.editor.output.available = true;
                audio.editor.output.leased = true;
                audio.editor.output.period_control =
                    deadsync_gameplay::EditorBufferControl::Adjustable;
                audio.editor.output.requested_period = Some(512);
                audio.editor.output.negotiated_period = Some(512);
                state.update_audio(audio);
                state.take_audio_commands();
                state.navigate_exact_time(1.25, false).unwrap();
                state.toggle_audio();
                state.take_audio_commands();
                state.bind_decoder_generation(Some(41));
                state.bind_decoder_asset_epoch(Some(owner), Some(53));
                audio.editor.logical_music = Some(deadsync_gameplay::EditorLogicalMusicClock {
                    generation: 41,
                    asset_epoch: 53,
                    music_nanos: 1_250_000_000,
                });
                state.update_audio(audio);
                state.take_audio_commands();
                assert!(state.chart_transport_is_playing());
                assert_eq!(state.chart_audio_generation(), Some(41));
                let position = state.transport_music_time();
                let output = state.audio_output_snapshot();
                let bytes = state.workspace.file().session.serialize().unwrap();
                let revision = state.workspace.file().session.revision();
                std::fs::write(&path, "#VERSION:0.83;#TITLE:External;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0100;").unwrap();
                // Exercise the real asynchronous file probe, not an injected Notice.
                let now = Instant::now();
                let deadline = now + Duration::from_secs(5);
                let mut attempt = 0;
                while state.external_watch.notice.is_none() && Instant::now() < deadline {
                    state.update_external_watch(now + Duration::from_secs(attempt * 2));
                    attempt += 1;
                    std::thread::sleep(Duration::from_millis(1));
                }
                assert!(
                    matches!(state.panel, Some(Panel::ExternalFile(0))),
                    "{}",
                    state.status
                );
                assert!(state.chart_transport_is_playing());
                assert_eq!(state.transport_music_time(), position);
                assert!(state.take_audio_commands().is_empty());
                if suppress {
                    press(&mut state, KeyCode::ArrowDown);
                    press(&mut state, KeyCode::ArrowDown);
                }
                press(&mut state, KeyCode::Enter);
                assert!(state.panel.is_none());
                assert_eq!(state.options_snapshot().suppress_external_song, suppress);
                assert!(state.chart_transport_is_playing());
                assert_eq!(state.chart_audio_generation(), Some(41));
                assert_eq!(state.editor_asset_owner(), owner);
                let current = state.audio_output_snapshot();
                assert_eq!(
                    (
                        current.available,
                        current.leased,
                        current.requested_period,
                        current.negotiated_period,
                        current.ack
                    ),
                    (
                        output.available,
                        output.leased,
                        output.requested_period,
                        output.negotiated_period,
                        output.ack
                    )
                );
                assert_eq!(state.workspace.file().session.serialize().unwrap(), bytes);
                assert_eq!(state.workspace.file().session.revision(), revision);
                assert!(state.take_audio_commands().is_empty());
                audio.stream_clock.valid_at = Instant::now();
                audio.editor.logical_music.as_mut().unwrap().music_nanos = 1_500_000_000;
                state.update_audio(audio);
                assert_eq!(
                    state.transport_music_time(),
                    Some(1.5 - state.audio_preferences.effective().offset)
                );
                assert!(state.chart_transport_is_playing());
                assert_eq!(state.chart_audio_generation(), Some(41));
                if suppress {
                    std::fs::write(&path, b"#VERSION:0.83;#TITLE:Second external change;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0010;").unwrap();
                    state.update_external_watch(now + Duration::from_secs(100));
                    assert!(state.external_watch.flight.is_none());
                    assert!(state.external_watch.notice.is_none());
                    assert!(state.panel.is_none());
                    assert!(state.chart_transport_is_playing());
                    assert!(state.take_audio_commands().is_empty());
                }
            }
        }
    }
}
