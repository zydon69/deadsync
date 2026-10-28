//! Direct File/Open and Recent candidate preparation reuse the strict editor
//! loader, preferred-open comparison inputs and application snapshot boundary.
use super::{State, application_preferences};
use crate::screens::edition::pack_song_preference::Prepared;
use deadsync_editor::{
    Error, chart_order::Orderer, chart_views::ActiveChartViews, open_preferences::Preferences,
    recent_songs::Song, storage::FileSession,
};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
};

pub(in crate::screens::edition) struct Job {
    cancelled: Arc<AtomicBool>,
    receiver: Receiver<Result<Prepared, Error>>,
}
impl Job {
    pub(in crate::screens::edition) fn spawn(
        path: PathBuf,
        preferences: Preferences,
    ) -> Result<Self, Error> {
        let permit = crate::screens::edition::file_open_permit::Permit::acquire()?;
        let cancelled = Arc::new(AtomicBool::new(false));
        let signal = Arc::clone(&cancelled);
        let (sender, receiver) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("editor-file-open".into())
            .spawn(move || {
                let _permit = permit;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    prepare(&path, preferences, &signal)
                }))
                .unwrap_or_else(|_| Err(Error::InvalidEdit("Song loading worker failed".into())));
                let _ = sender.send(result);
            })?;
        Ok(Self {
            cancelled,
            receiver,
        })
    }
    /// Keep this Job in the native I/O slot until poll yields a result, even
    /// when its modal closes. The synchronous parser cannot be interrupted.
    pub(in crate::screens::edition) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
    pub(in crate::screens::edition) fn poll(&mut self) -> Option<Result<Prepared, Error>> {
        match self.receiver.try_recv() {
            Ok(value) if !self.cancelled.load(Ordering::Relaxed) => Some(value),
            Ok(_) | Err(TryRecvError::Disconnected) => {
                Some(Err(Error::InvalidEdit("Song loading cancelled".into())))
            }
            Err(TryRecvError::Empty) => None,
        }
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}
fn prepare(
    path: &Path,
    preferences: Preferences,
    cancelled: &AtomicBool,
) -> Result<Prepared, Error> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(Error::InvalidEdit("Song loading cancelled".into()));
    }
    let file = FileSession::open_bounded(path, 16 * 1024 * 1024, cancelled)?;
    let original = file.require_existing_path()?.to_owned();
    let prepared = Prepared::new(file, preferences, cancelled)?;
    if cancelled.load(Ordering::Relaxed) {
        return Err(Error::InvalidEdit("Song loading cancelled".into()));
    }
    if path.canonicalize()? != original || prepared.file.has_external_change()? {
        return Err(Error::ExternalChange);
    }
    Ok(prepared)
}

/// Returns a detached replacement; caller still owns all dirty/pending review
/// and the old State. No recent entry or filesystem output is published here.
pub(in crate::screens::edition) fn finish(
    source: &State,
    prepared: Prepared,
    saved: Option<&Song>,
    orderer: &mut impl Orderer,
) -> Result<State, Error> {
    if prepared.file.has_external_change()? {
        return Err(Error::ExternalChange);
    }
    let culture = crate::i18n::revision();
    let restoration = prepared.restoration(saved, orderer)?;
    if culture != crate::i18n::revision() {
        return Err(Error::InvalidEdit(
            "Editor language changed while ordering the opened song".into(),
        ));
    }
    if prepared.file.has_external_change()? {
        return Err(Error::ExternalChange);
    }
    let (workspace, failures) = prepared.workspace(restoration.focused)?;
    let mut candidate = State::from_workspace(workspace)?;
    application_preferences::Snapshot::capture(source).install(&mut candidate)?;
    candidate.song_open_warnings = failures;
    let row = saved.map_or(0.0, |saved| saved.chart_position);
    let views =
        ActiveChartViews::restore_recent(&candidate.workspace.file().session, &restoration, row)?;
    if let Some(focused) = views.focused() {
        let position = focused.position().clone();
        candidate
            .workspace
            .seek_preserving_selection(position.edit_row())?;
        candidate.focus_position = Some(position);
    }
    candidate.active_views = Some(views);
    candidate
        .zoom
        .set_zoom(saved.map_or(1.0, |saved| saved.spacing_zoom))?;
    candidate.refresh();
    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadsync_editor::{chart_order::Snapshot, recent_songs::SavedChart, workspace::Command};
    struct NoSort;
    impl Orderer for NoSort {
        fn order(&mut self, _: &Snapshot) -> Result<Vec<usize>, Error> {
            panic!("one supported chart must not request cultural sorting")
        }
    }
    fn simfile() -> &'static str {
        "#VERSION:0.83;#BPMS:0=120;#WARPS:2=4;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Challenge;#METER:9;#NOTES:1000\n0000\n0000\n0000;"
    }
    #[test]
    fn detached_candidate_restores_warp_row_and_shared_settings_without_losing_old_undo() {
        crate::i18n::init_for_tests();
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("original.ssc");
        let destination = dir.path().join("opened.ssc");
        std::fs::write(&original, simfile()).unwrap();
        std::fs::write(&destination, simfile()).unwrap();
        let mut source = State::open(&original, 0).unwrap();
        source.workspace.execute(Command::Seek(96)).unwrap();
        source.workspace.execute(Command::Place(1)).unwrap();
        let before = source.workspace.file().session.serialize().unwrap();
        let ids = source.workspace.file().session.chart_ids().to_vec();
        let colors = Arc::clone(&source.note_colors);
        let old_undo = source
            .workspace
            .file()
            .session
            .undo_label()
            .map(str::to_owned);
        let saved = Song {
            file: destination.clone(),
            title: None,
            pack: None,
            spacing_zoom: 1.75,
            chart_position: 160.5,
            active: vec![SavedChart {
                steps_type: "dance-single".into(),
                difficulty: 4,
                rating: 9,
                name: Some(String::new()),
                description: Some(String::new()),
            }],
            focused: Some(0),
        };
        let prepared = prepare(
            &destination,
            Preferences::default(),
            &AtomicBool::new(false),
        )
        .unwrap();
        let candidate = finish(&source, prepared, Some(&saved), &mut NoSort).unwrap();
        assert_eq!(candidate.workspace.row(), 160);
        assert_eq!(candidate.focus_position.as_ref().unwrap().row(), 160.5);
        assert_eq!(
            candidate
                .active_views
                .as_ref()
                .unwrap()
                .focused()
                .unwrap()
                .position()
                .row(),
            160.5
        );
        assert_eq!(candidate.zoom.spacing_zoom(), 1.75);
        assert!(Arc::ptr_eq(&candidate.note_colors, &colors));
        assert!(
            candidate
                .active_views
                .as_ref()
                .unwrap()
                .focused()
                .unwrap()
                .dedicated()
        );
        assert_eq!(source.workspace.file().session.serialize().unwrap(), before);
        assert_eq!(source.workspace.file().session.chart_ids(), ids);
        assert_eq!(
            source.workspace.file().session.undo_label(),
            old_undo.as_deref()
        );
        assert!(source.workspace.file().session.is_dirty());
        assert!(
            !candidate
                .active_views
                .as_ref()
                .unwrap()
                .belongs_to(&source.workspace.file().session)
        );
    }
    #[test]
    fn cancelled_and_changed_sources_fail_before_any_candidate_is_published() {
        crate::i18n::init_for_tests();
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("song.ssc");
        std::fs::write(&file, simfile()).unwrap();
        assert!(prepare(&file, Preferences::default(), &AtomicBool::new(true)).is_err());
        let source = State::open(&file, 0).unwrap();
        let prepared = prepare(&file, Preferences::default(), &AtomicBool::new(false)).unwrap();
        std::fs::write(&file, b"external change").unwrap();
        assert!(finish(&source, prepared, None, &mut NoSort).is_err());
        assert_eq!(source.workspace.file().session.document().charts().len(), 1);
    }

    #[test]
    fn cancelled_job_keeps_busy_slot_until_worker_acknowledges_completion() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut job = Job {
            cancelled: Arc::new(AtomicBool::new(false)),
            receiver,
        };
        job.cancel();
        assert!(job.poll().is_none());
        assert!(
            sender
                .send(Err(Error::InvalidEdit("worker acknowledged cancel".into())))
                .is_ok()
        );
        assert!(matches!(job.poll(), Some(Err(_))));
    }
}
