//! Read-only strict song preparation. Dropping/cancelling never publishes a session.
use super::pack_song_preference::Prepared;
use deadsync_editor::{
    Error, open_preferences::Preferences, pack::song_list::Row, storage::FileSession,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, TryRecvError},
};

pub(super) struct Job {
    cancelled: Arc<AtomicBool>,
    receiver: Receiver<Result<Prepared, Error>>,
}
impl Job {
    pub(super) fn spawn(row: Row, preferences: Preferences) -> Result<Self, Error> {
        Self::spawn_with_profiles(
            row,
            preferences,
            deadsync_editor::runtime::configs_dir().map(std::path::Path::to_path_buf),
        )
    }
    pub(super) fn spawn_with_profiles(
        row: Row,
        preferences: Preferences,
        directory: Option<std::path::PathBuf>,
    ) -> Result<Self, Error> {
        let permit = super::file_open_permit::Permit::acquire()?;
        let cancelled = Arc::new(AtomicBool::new(false));
        let signal = cancelled.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("editor-pack-song-open".into())
            .spawn(move || {
                let _permit = permit;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    prepare_with_profiles(&row, preferences, &signal, directory.as_deref())
                }))
                .unwrap_or_else(|_| Err(Error::InvalidEdit("Song loading worker failed".into())));
                let _ = sender.send(result);
            })?;
        Ok(Self {
            cancelled,
            receiver,
        })
    }
    pub(super) fn poll(&mut self) -> Option<Result<Prepared, Error>> {
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
fn prepare(row: &Row, preferences: Preferences, cancelled: &AtomicBool) -> Result<Prepared, Error> {
    prepare_with_profiles(
        row,
        preferences,
        cancelled,
        deadsync_editor::runtime::configs_dir(),
    )
}
fn prepare_with_profiles(
    row: &Row,
    preferences: Preferences,
    cancelled: &AtomicBool,
    directory: Option<&std::path::Path>,
) -> Result<Prepared, Error> {
    let check = || -> Result<(), Error> {
        if cancelled.load(Ordering::Relaxed) {
            return Err(Error::InvalidEdit("Song loading cancelled".into()));
        }
        if !row.is_current()? {
            return Err(Error::ExternalChange);
        }
        Ok(())
    };
    check()?;
    // This is the strict existing editor loader, never Metadata -> Document.
    // Cancellation brackets its synchronous parser; it cannot interrupt parsing.
    let file = FileSession::open_bounded_with_context(
        &row.logical_path,
        16 * 1024 * 1024,
        cancelled,
        preferences.load_context(),
    )?;
    check()?;
    if file.require_existing_path()? != row.path || file.has_external_change()? {
        return Err(Error::ExternalChange);
    }
    let prepared = Prepared::new_with_profiles(file, preferences, cancelled, directory)?;
    check()?;
    Ok(prepared)
}

#[cfg(test)]
mod profile_resolution_tests {
    use super::*;
    #[test]
    fn actual_pack_song_worker_resolves_the_configured_catalog_and_retains_its_guard() {
        use deadsync_editor::{
            generation::{
                metadata::{ChartData, DYNAMIC, Guid, Registry},
                profiles::Config,
            },
            pack::song_list::{HeaderReader, Limits, Task},
        };
        let root = tempfile::tempdir().unwrap();
        let song = root.path().join("pack/song");
        std::fs::create_dir_all(&song).unwrap();
        let path = song.join("shared.ssc");
        let unknown = "12345678-9abc-4def-8123-456789abcdef";
        std::fs::write(&path,format!("#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;#GAChartDataVersion:1;#GAChartData:{{\"ExpressedChartConfig\":\"{unknown}\"}};")).unwrap();
        let raw = std::fs::read(&path).unwrap();
        let profiles = root.path().join("profiles");
        std::fs::create_dir(&profiles).unwrap();
        let config = Config::Expressed(
            Registry::builtin().expressed[&DYNAMIC.parse::<Guid>().unwrap()].clone(),
        );
        let profile = profiles.join("ecc-custom.json");
        std::fs::write(&profile,serde_json::to_vec(&serde_json::json!({"Guid":unknown,"Name":"Custom","Config":config.json().unwrap()})).unwrap()).unwrap();
        let cancel = AtomicBool::new(false);
        let catalog = Task::from_song_file(&path, Limits::default())
            .unwrap()
            .run(&HeaderReader, &cancel, &mut |_| {})
            .unwrap();
        let row = catalog
            .rows
            .into_iter()
            .find(|row| row.path == path.canonicalize().unwrap())
            .unwrap();
        let mut job =
            Job::spawn_with_profiles(row, Preferences::default(), Some(profiles.clone())).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let prepared = loop {
            if let Some(result) = job.poll() {
                break result.unwrap();
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        };
        assert_eq!(
            ChartData::read(prepared.file.session.document(), 0)
                .unwrap()
                .unwrap()
                .expressed_chart_config
                .to_string(),
            unknown
        );
        assert!(!prepared.file.session.is_dirty());
        assert!(prepared.file.session.undo_label().is_none());
        assert_eq!(std::fs::read(path).unwrap(), raw);
        prepared
            .file
            .validate_generation_profile_context(Some(&profiles), &cancel)
            .unwrap();
        std::fs::write(profile, b"external config").unwrap();
        assert!(prepared.workspace(Some(0)).is_err());
    }
}
