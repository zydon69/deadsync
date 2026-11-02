//! One bounded passive Pack.ini observer. All filesystem work stays off UI.
use super::*;
use deadsync_editor::pack::storage::{Error, ExternalObservation, ExternalSnapshot, FileSession};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
    time::{Duration, Instant},
};
static BUSY: AtomicBool = AtomicBool::new(false);
struct Permit;
impl Drop for Permit {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Release);
    }
}
enum ResultValue {
    Loaded(Option<FileSession>),
    Observed(Option<ExternalObservation>, Option<FileSession>),
}
struct Flight {
    cancel: Arc<AtomicBool>,
    receive: Receiver<Result<ResultValue, Error>>,
    snapshot: Option<ExternalSnapshot>,
}
impl Drop for Flight {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}
#[derive(Default)]
pub(super) struct Runtime {
    root: Option<PathBuf>,
    discovered: bool,
    file: Option<FileSession>,
    baseline: Option<ExternalSnapshot>,
    flight: Option<Flight>,
    last: Option<Instant>,
    notice: Option<ExternalObservation>,
    ignored: Option<ExternalObservation>,
    reported: bool,
}
impl Runtime {
    fn update(
        &mut self,
        root: Option<PathBuf>,
        published: Option<ExternalSnapshot>,
        now: Instant,
        suppressed: bool,
    ) -> Option<Result<bool, Error>> {
        if self.root != root {
            *self = Self {
                root,
                ..Default::default()
            };
        }
        if let Some(snapshot) = published {
            if self
                .baseline
                .as_ref()
                .is_none_or(|current| !current.same_capture(&snapshot))
            {
                self.flight = None;
                self.file = None;
                self.baseline = Some(snapshot);
                self.discovered = true;
                self.notice = None;
                self.ignored = None;
                self.reported = false;
                self.last = None;
            }
        }
        if suppressed {
            self.flight = None;
            self.notice = None;
            return None;
        }
        let result = self
            .flight
            .as_ref()
            .and_then(|flight| match flight.receive.try_recv() {
                Ok(v) => Some(v),
                Err(TryRecvError::Disconnected) => Some(Err(Error::StaleSnapshot)),
                Err(TryRecvError::Empty) => None,
            });
        if let Some(result) = result {
            let flight = self.flight.take().unwrap();
            if !flight.cancel.load(Ordering::Acquire)
                && match &flight.snapshot {
                    Some(s) => self.baseline.as_ref().is_some_and(|b| b.same_capture(s)),
                    None => self.baseline.is_none(),
                }
            {
                match result {
                    Ok(ResultValue::Loaded(file)) => {
                        self.discovered = true;
                        self.baseline = file.as_ref().map(FileSession::external_snapshot);
                        self.file = file;
                    }
                    Ok(ResultValue::Observed(changed, baseline)) => {
                        if let Some(file) = baseline {
                            if self
                                .baseline
                                .as_ref()
                                .is_some_and(|snapshot| snapshot.is_current(&file))
                            {
                                self.file = Some(file);
                            }
                        }
                        if changed.is_none() {
                            self.ignored = None;
                            self.reported = false;
                        } else if self.ignored != changed {
                            self.notice = changed;
                        }
                    }
                    Err(error) => {
                        if !self.reported {
                            self.reported = true;
                            return Some(Err(error));
                        }
                    }
                }
            }
        }
        if self.notice.is_some() {
            return Some(Ok(true));
        }
        if self.discovered && self.baseline.is_none() {
            return None;
        }
        let root = self.root.clone()?;
        if self
            .last
            .is_some_and(|last| now.saturating_duration_since(last) < Duration::from_secs(1))
        {
            return None;
        }
        if self.flight.is_some()
            || BUSY
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return None;
        }
        self.last = Some(now);
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let snapshot = self.baseline.clone();
        let worker_snapshot = snapshot.clone();
        let rebuild = self.file.is_none();
        let (tx, receive) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("editor-pack-observer".into())
            .spawn(move || {
                let _permit = Permit;
                let result = if worker_cancel.load(Ordering::Acquire) {
                    Err(Error::StaleSnapshot)
                } else if let Some(snapshot) = worker_snapshot {
                    snapshot.probe(&worker_cancel).and_then(|observation| {
                        Ok(ResultValue::Observed(
                            observation,
                            if rebuild {
                                snapshot.rebuild_observer_baseline()?
                            } else {
                                None
                            },
                        ))
                    })
                } else {
                    FileSession::open(&root).map(ResultValue::Loaded)
                };
                if !worker_cancel.load(Ordering::Acquire) {
                    let _ = tx.send(result);
                }
            });
        if spawned.is_err() {
            BUSY.store(false, Ordering::Release);
            return Some(Err(Error::StaleSnapshot));
        }
        self.flight = Some(Flight {
            cancel,
            receive,
            snapshot,
        });
        None
    }
    fn acknowledge(&mut self) {
        self.ignored = self.notice.take();
    }
}
impl State {
    pub(super) fn update_pack_watch(&mut self, now: Instant) {
        let root = self
            .workspace
            .file()
            .existing_path()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .map(Path::to_path_buf);
        let published = self.pack_watch_snapshot(root.as_deref());
        let result =
            self.pack_watch
                .update(root, published, now, self.general_options.suppress_pack);
        match result {
            Some(Ok(true))
                if self.panel.is_none()
                    && self.text_buffer.is_none()
                    && !self.is_generating()
                    && !self.file_modal()
                    && !self.audio_output_busy() =>
            {
                let mut baseline = self.pack_watch.file.take().filter(|file| {
                    self.pack_watch
                        .baseline
                        .as_ref()
                        .is_some_and(|snapshot| snapshot.is_current(file))
                });
                let opened = self.open_passive_pack_notice(&mut baseline);
                self.pack_watch.file = baseline;
                if opened {
                    self.pack_watch.acknowledge();
                }
            }
            Some(Err(error)) => {
                log::warn!("Pack observer: {error}");
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, State, PathBuf) {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let song = root.path().join("Song/song.ssc");
        std::fs::create_dir(song.parent().unwrap()).unwrap();
        std::fs::write(&song,b"#VERSION:0.83;#TITLE:Native;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        let mut pack = FileSession::create(
            root.path(),
            deadsync_editor::pack::Data::new("Pack", "", 2026, None, 0.009),
        )
        .unwrap();
        pack.save().unwrap();
        let mut state = State::open(&song, 0).unwrap();
        state.open_pack_properties();
        state
            .pack_properties
            .as_mut()
            .unwrap()
            .set_text(super::super::pack_properties::Field::Title, "Unsaved title")
            .unwrap();
        state.text_buffer = None;
        state.panel = None;
        let path = root.path().join("Pack.ini");
        (root, state, path)
    }
    fn ready(state: &mut State, root: &Path, path: &Path, now: Instant) {
        let root = root.canonicalize().unwrap();
        let snapshot = state.pack_watch_snapshot(Some(&root)).unwrap();
        std::fs::write(path, b"[Group]\nTitle=External\n").unwrap();
        let observation = snapshot.probe(&AtomicBool::new(false)).unwrap();
        let (tx, receive) = mpsc::channel();
        tx.send(Ok(ResultValue::Observed(observation, None)))
            .unwrap();
        state.pack_watch = Runtime {
            root: Some(root),
            baseline: Some(snapshot.clone()),
            flight: Some(Flight {
                cancel: Arc::new(AtomicBool::new(false)),
                receive,
                snapshot: Some(snapshot),
            }),
            last: Some(now),
            ..Default::default()
        };
    }
    #[test]
    fn real_external_result_opens_existing_conflict_review_without_losing_local_edits() {
        let (root, mut state, path) = fixture();
        let now = Instant::now();
        ready(&mut state, root.path(), &path, now);
        state.update_pack_watch(now);
        assert!(matches!(state.panel, Some(Panel::PackProperties)));
        assert_eq!(state.status, i18n::tr("Edition", "PackPassiveChanged"));
        assert!(state.pack_has_unsaved_changes());
        assert_eq!(
            state
                .pack_properties
                .as_ref()
                .unwrap()
                .text(super::super::pack_properties::Field::Title),
            "Unsaved title"
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"[Group]\nTitle=External\n");
        assert!(state.pack_watch.ignored.is_some());
    }
    #[test]
    fn suppression_cancels_passive_notification_but_manual_publication_still_refuses_external_bytes()
     {
        let (root, mut state, path) = fixture();
        let now = Instant::now();
        ready(&mut state, root.path(), &path, now);
        let cancel = state.pack_watch.flight.as_ref().unwrap().cancel.clone();
        let mut p = state.general_options.clone();
        p.suppress_pack = true;
        state.configure_general_options(p).unwrap();
        state.update_pack_watch(now);
        assert!(state.panel.is_none());
        assert!(cancel.load(Ordering::Acquire));
        assert!(!state.save_coupled_pack());
        assert!(state.pack_has_unsaved_changes());
        assert_eq!(std::fs::read(&path).unwrap(), b"[Group]\nTitle=External\n");
    }
    #[test]
    fn rebinding_another_pack_discards_old_flight_and_notice() {
        let (root, mut state, path) = fixture();
        let now = Instant::now();
        ready(&mut state, root.path(), &path, now);
        let cancel = state.pack_watch.flight.as_ref().unwrap().cancel.clone();
        let other = root.path().join("Other");
        state
            .pack_watch
            .update(Some(other.clone()), None, now, true);
        assert!(cancel.load(Ordering::Acquire));
        assert_eq!(state.pack_watch.root, Some(other));
        assert!(state.pack_watch.notice.is_none());
        assert!(state.pack_watch.baseline.is_none());
    }
    #[test]
    fn unopened_properties_adopts_real_worker_old_baseline_without_reading_external_metadata() {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let canonical = root.path().canonicalize().unwrap();
        let song = canonical.join("Song/song.ssc");
        std::fs::create_dir(song.parent().unwrap()).unwrap();
        std::fs::write(&song,b"#VERSION:0.83;#TITLE:Native;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        let mut pack = FileSession::create(
            &canonical,
            deadsync_editor::pack::Data::new("Original baseline", "", 2026, None, 0.009),
        )
        .unwrap();
        pack.save().unwrap();
        let path = pack.path().to_owned();
        let mut state = State::open(&song, 0).unwrap();
        assert!(state.pack_properties.is_none());
        let deadline = Instant::now() + Duration::from_secs(5);
        while state.pack_watch.file.is_none() {
            assert!(
                Instant::now() < deadline,
                "bounded worker baseline admission"
            );
            state
                .pack_watch
                .update(Some(canonical.clone()), None, Instant::now(), false);
            std::thread::yield_now();
        }
        let old = state.pack_watch.baseline.clone().unwrap();
        std::fs::write(&path, b"[Group]\nTitle=External latest\n").unwrap();
        let changed = old.probe(&AtomicBool::new(false)).unwrap();
        let (tx, receive) = mpsc::channel();
        tx.send(Ok(ResultValue::Observed(changed, None))).unwrap();
        state.pack_watch.flight = Some(Flight {
            cancel: Arc::new(AtomicBool::new(false)),
            receive,
            snapshot: Some(old.clone()),
        });
        state.pack_watch.last = Some(Instant::now());
        state.update_pack_watch(Instant::now());
        assert!(matches!(state.panel, Some(Panel::PackProperties)));
        assert_eq!(
            state
                .pack_properties
                .as_ref()
                .unwrap()
                .text(super::super::pack_properties::Field::Title),
            "Original baseline"
        );
        assert!(state.pack_watch.file.is_none());
        assert!(old.same_capture(&state.pack_watch_snapshot(Some(&canonical)).unwrap()));
        assert!(
            state
                .pack_properties
                .as_mut()
                .unwrap()
                .set_text(
                    super::super::pack_properties::Field::Title,
                    "Local edit after notice"
                )
                .is_err()
        );
        // Conflict cannot be edited until explicitly acknowledged.
        state.pack_properties_key(KeyCode::Enter);
        state
            .pack_properties
            .as_mut()
            .unwrap()
            .set_text(
                super::super::pack_properties::Field::Title,
                "Explicit local edit",
            )
            .unwrap();
        state.text_buffer = None;
        assert!(state.pack_has_unsaved_changes());
        assert!(!state.save_coupled_pack());
        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"[Group]\nTitle=External latest\n"
        );
    }
    #[test]
    fn initially_absent_pack_is_not_silently_adopted_after_external_creation() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let mut runtime = Runtime {
            root: Some(root.clone()),
            discovered: true,
            last: Some(Instant::now()),
            ..Default::default()
        };
        let mut file = FileSession::create(
            &root,
            deadsync_editor::pack::Data::new("Created elsewhere", "", 2026, None, 0.009),
        )
        .unwrap();
        file.save().unwrap();
        assert!(
            runtime
                .update(
                    Some(root.clone()),
                    None,
                    Instant::now() + Duration::from_secs(2),
                    false
                )
                .is_none()
        );
        assert!(runtime.flight.is_none());
        assert!(runtime.file.is_none());
        assert!(runtime.baseline.is_none());
        // An explicit native load/create supplies a new typed baseline and arms observation.
        let snapshot = file.external_snapshot();
        runtime.update(Some(root), Some(snapshot.clone()), Instant::now(), true);
        assert!(runtime.baseline.as_ref().unwrap().same_capture(&snapshot));
    }

    #[test]
    fn own_save_then_another_song_cannot_reinstall_the_observers_old_file() {
        let (root, mut first, path) = fixture();
        let canonical = root.path().canonicalize().unwrap();
        let old = FileSession::open(&canonical).unwrap().unwrap();
        let old_snapshot = old.external_snapshot();
        first.pack_watch = Runtime {
            root: Some(canonical.clone()),
            discovered: true,
            file: Some(old),
            baseline: Some(old_snapshot),
            last: Some(Instant::now()),
            ..Default::default()
        };
        first
            .pack_properties
            .as_mut()
            .unwrap()
            .set_text(
                super::super::pack_properties::Field::Title,
                "Latest own publication",
            )
            .unwrap();
        assert!(first.save_coupled_pack());
        let latest = first.pack_watch_snapshot(Some(&canonical)).unwrap();
        first.pack_watch.update(
            Some(canonical.clone()),
            Some(latest.clone()),
            Instant::now(),
            true,
        );
        assert!(first.pack_watch.file.is_none());
        let next_song = canonical.join("Other/song.ssc");
        std::fs::create_dir(next_song.parent().unwrap()).unwrap();
        std::fs::copy(first.workspace.file().existing_path().unwrap(), &next_song).unwrap();
        let mut next = State::open(&next_song, 0).unwrap();
        next.pack_watch = std::mem::take(&mut first.pack_watch);
        assert!(next.pack_properties.is_none());
        std::fs::write(&path, b"[Group]\nTitle=Concurrent external\n").unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while next.pack_properties.is_none() {
            assert!(
                Instant::now() < deadline,
                "observer latest publication baseline"
            );
            next.update_pack_watch(Instant::now());
            std::thread::yield_now();
        }
        assert_eq!(
            next.pack_properties
                .as_ref()
                .unwrap()
                .text(super::super::pack_properties::Field::Title),
            "Latest own publication"
        );
        assert!(latest.same_capture(&next.pack_watch_snapshot(Some(&canonical)).unwrap()));
        next.pack_properties_key(KeyCode::Enter);
        next.pack_properties
            .as_mut()
            .unwrap()
            .set_text(
                super::super::pack_properties::Field::Title,
                "Local following own publication",
            )
            .unwrap();
        next.text_buffer = None;
        assert!(!next.save_coupled_pack());
        assert!(next.pack_has_unsaved_changes());
        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"[Group]\nTitle=Concurrent external\n"
        );
    }
}

impl State {
    /// Call only after confirmed own removal (including a live durability receipt).
    /// This records absence; it does not acknowledge or release that receipt.
    pub(super) fn pack_observer_own_delete(&mut self) {
        let root = self
            .workspace
            .file()
            .existing_path()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .map(Path::to_path_buf);
        self.pack_watch = Runtime {
            root,
            discovered: true,
            ..Default::default()
        };
    }
}
