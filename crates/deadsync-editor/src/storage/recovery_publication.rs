//! Namespace publication and directory durability are separate recovery outcomes.
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
pub(super) type SharedPending = Arc<Mutex<Option<Pending>>>;
#[derive(Clone)]
pub(super) struct Pending {
    source: PathBuf,
    path: PathBuf,
    lock_path: PathBuf,
    expected: Arc<[u8]>,
    bytes: Arc<[u8]>,
    epoch: Arc<AtomicU64>,
    revision: u64,
    warning: Option<String>,
}
/// Err means no new recovery publication happened in this call. A successful
/// rename always returns Published, even when the later durability barrier fails.
#[derive(Debug)]
pub enum RecoveryOutcome {
    Published {
        newly_written: bool,
        durability_warning: Option<Error>,
    },
}
fn poisoned() -> Error {
    std::io::Error::other("Recovery publication receipt unavailable").into()
}
fn lock_file(path: &Path) -> Result<fs::File, Error> {
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    lock.try_lock().map_err(std::io::Error::other)?;
    Ok(lock)
}
impl Pending {
    fn current(&self) -> Result<(), Error> {
        if self.epoch.load(Ordering::Acquire) != self.revision
            || fs::read(&self.source)?.as_slice() != self.expected.as_ref()
        {
            return Err(Error::ExternalChange);
        }
        if fs::read(&self.path)?.as_slice() != self.bytes.as_ref() {
            return Err(Error::RecoveryConflict);
        }
        Ok(())
    }
}
fn retry(
    shared: &SharedPending,
    sync: &mut impl FnMut(&Path) -> Result<(), Error>,
) -> Result<(), Error> {
    let Some(pending) = shared.lock().map_err(|_| poisoned())?.clone() else {
        return Ok(());
    };
    let _lock = lock_file(&pending.lock_path)?;
    let mut current = shared.lock().map_err(|_| poisoned())?;
    let Some(current_pending) = current.as_ref() else {
        return Ok(());
    };
    if current_pending.path != pending.path
        || current_pending.bytes != pending.bytes
        || current_pending.revision != pending.revision
    {
        return Err(Error::RecoveryConflict);
    }
    pending.current()?;
    if let Err(error) = sync(
        pending
            .path
            .parent()
            .ok_or_else(|| std::io::Error::other("Recovery path has no parent"))?,
    ) {
        if let Some(pending) = current.as_mut() {
            pending.warning = Some(error.to_string());
        }
        return Err(error);
    }
    pending.current()?;
    *current = None;
    Ok(())
}
impl FileSession {
    pub fn has_pending_recovery_durability(&self) -> bool {
        self.recovery_durability
            .try_lock()
            .map_or(true, |value| value.is_some())
    }
    pub fn recovery_durability_warning(&self) -> Option<String> {
        match self.recovery_durability.try_lock() {
            Ok(value) => value.as_ref().map(|pending| {
                pending
                    .warning
                    .clone()
                    .unwrap_or_else(|| "Directory durability not confirmed".into())
            }),
            Err(std::sync::TryLockError::WouldBlock) => None,
            Err(std::sync::TryLockError::Poisoned(_)) => {
                Some("Recovery publication receipt unavailable".into())
            }
        }
    }
    /// Recheck source identity/epoch and exact owned sidecar bytes under the
    /// recovery file lock, then retry only fsync. Never rename or rewrite a file.
    pub fn retry_recovery_durability(&self) -> Result<(), Error> {
        retry(&self.recovery_durability, &mut sync_directory)
    }
}
impl RecoverySnapshot {
    pub(super) fn write(self, require_current_source: bool) -> Result<RecoveryOutcome, Error> {
        self.write_with_sync(require_current_source, sync_directory)
    }
    pub(super) fn write_with_sync(
        self,
        require_current_source: bool,
        mut sync: impl FnMut(&Path) -> Result<(), Error>,
    ) -> Result<RecoveryOutcome, Error> {
        retry(&self.pending_durability, &mut sync)?;
        let _lock = lock_file(&self.lock_path)?;
        let check = || -> Result<(), Error> {
            if require_current_source
                && (self.publication_epoch.load(Ordering::Acquire) != self.publication_revision
                    || fs::read(&self.source)?.as_slice() != self.expected.as_ref())
            {
                return Err(Error::ExternalChange);
            }
            Ok(())
        };
        check()?;
        if require_current_source {
            match fs::read(&self.path) {
                Ok(bytes) if self.expected_draft.as_deref() == Some(bytes.as_slice()) => {}
                Err(error)
                    if error.kind() == std::io::ErrorKind::NotFound
                        && self.expected_draft.is_none() => {}
                Ok(_) => return Err(Error::RecoveryConflict),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Err(Error::RecoveryConflict);
                }
                Err(error) => return Err(error.into()),
            }
        }
        let parent = self
            .path
            .parent()
            .ok_or_else(|| std::io::Error::other("Recovery path has no parent"))?;
        let bytes: Arc<[u8]> = recovery_codec::encode(&self)?.into();
        // Acquire every fallible ownership guard before rename. No post-rename
        // poisoned mutex can turn an actual publication into a generic refusal.
        let mut pending_receipt = self.pending_durability.lock().map_err(|_| poisoned())?;
        let mut receipt = self.receipt.lock().map_err(|_| poisoned())?;
        if receipt.as_deref() == Some(bytes.as_ref())
            && fs::read(&self.path).ok().as_deref() == Some(bytes.as_ref())
        {
            return Ok(RecoveryOutcome::Published {
                newly_written: false,
                durability_warning: None,
            });
        }
        let prepared = Pending {
            source: self.source.clone(),
            path: self.path.clone(),
            lock_path: self.lock_path.clone(),
            expected: Arc::clone(&self.expected),
            bytes: Arc::clone(&bytes),
            epoch: Arc::clone(&self.publication_epoch),
            revision: self.publication_revision,
            warning: None,
        };
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary.write_all(&bytes)?;
        temporary.as_file().sync_all()?;
        check()?;
        temporary
            .persist(&self.path)
            .map_err(|error| Error::Io(error.error))?;
        *receipt = Some(bytes);
        *pending_receipt = Some(prepared);
        let warning = match sync(parent) {
            Ok(()) => {
                *pending_receipt = None;
                None
            }
            Err(error) => {
                if let Some(pending) = pending_receipt.as_mut() {
                    pending.warning = Some(error.to_string());
                }
                Some(error)
            }
        };
        Ok(RecoveryOutcome::Published {
            newly_written: true,
            durability_warning: warning,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn file() -> (tempfile::TempDir, FileSession) {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.ssc");
        fs::write(&path,b"#VERSION:0.83;#TITLE:Original;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        let mut file = FileSession::open(&path).unwrap();
        file.session.set_text(None, "TITLE", "Recovered").unwrap();
        (root, file)
    }
    fn late_failure(parent: &Path) -> Result<(), Error> {
        sync_directory(parent)?;
        Err(std::io::Error::other("injected post-publication directory failure").into())
    }
    #[test]
    fn late_sync_failure_keeps_exact_owned_publication_and_public_retry_does_not_rewrite() {
        let (_root, file) = file();
        let source = fs::read(file.existing_path().unwrap()).unwrap();
        let snapshot = file.recovery_snapshot().unwrap().unwrap();
        let path = snapshot.path.clone();
        let outcome = snapshot.write_with_sync(true, late_failure).unwrap();
        assert!(matches!(
            outcome,
            RecoveryOutcome::Published {
                newly_written: true,
                durability_warning: Some(_)
            }
        ));
        let published = fs::read(&path).unwrap();
        assert!(!published.is_empty());
        assert_eq!(
            file.recovery_receipt
                .as_ref()
                .unwrap()
                .lock()
                .unwrap()
                .as_deref(),
            Some(published.as_slice())
        );
        assert!(file.has_pending_recovery_durability());
        assert!(
            file.recovery_durability_warning()
                .unwrap()
                .contains("injected")
        );
        let metadata = fs::metadata(&path).unwrap();
        let outcome = file.save_recovery().unwrap();
        assert!(matches!(
            outcome,
            RecoveryOutcome::Published {
                newly_written: false,
                durability_warning: None
            }
        ));
        assert_eq!(fs::read(&path).unwrap(), published);
        assert!(!file.has_pending_recovery_durability());
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(metadata.ino(), fs::metadata(&path).unwrap().ino());
        }
        assert_eq!(fs::read(file.existing_path().unwrap()).unwrap(), source);
        assert!(file.session.is_dirty());
        let mut reopened = FileSession::open(file.existing_path().unwrap()).unwrap();
        reopened.recover().unwrap();
        assert_eq!(
            reopened.session.document().text(None, "TITLE").as_deref(),
            Some("Recovered")
        );
    }
    #[test]
    fn refusal_before_rename_has_no_receipt_and_can_retry_real_publication() {
        let (_root, file) = file();
        let path = file.recovery_path().unwrap();
        fs::create_dir(&path).unwrap();
        assert!(file.save_recovery().is_err());
        assert!(
            file.recovery_receipt
                .as_ref()
                .unwrap()
                .lock()
                .unwrap()
                .is_none()
        );
        assert!(!file.has_pending_recovery_durability());
        fs::remove_dir(&path).unwrap();
        assert!(matches!(
            file.save_recovery().unwrap(),
            RecoveryOutcome::Published {
                newly_written: true,
                durability_warning: None
            }
        ));
        assert!(path.is_file());
    }
    #[test]
    fn late_publication_retry_never_overwrites_external_sidecar_or_changed_source() {
        let (_root, file) = file();
        let outcome = file
            .recovery_snapshot()
            .unwrap()
            .unwrap()
            .write_with_sync(true, late_failure)
            .unwrap();
        assert!(matches!(
            outcome,
            RecoveryOutcome::Published {
                durability_warning: Some(_),
                ..
            }
        ));
        let path = file.recovery_path().unwrap();
        let owned = fs::read(&path).unwrap();
        fs::write(&path, b"external recovery").unwrap();
        assert!(matches!(
            file.retry_recovery_durability(),
            Err(Error::RecoveryConflict)
        ));
        assert!(matches!(file.save_recovery(), Err(Error::RecoveryConflict)));
        assert_eq!(fs::read(&path).unwrap(), b"external recovery");
        assert!(file.has_pending_recovery_durability());
        fs::write(&path, &owned).unwrap();
        fs::write(file.existing_path().unwrap(), b"external song").unwrap();
        assert!(matches!(
            file.retry_recovery_durability(),
            Err(Error::ExternalChange)
        ));
        assert_eq!(fs::read(&path).unwrap(), owned);
        assert_eq!(
            fs::read(file.existing_path().unwrap()).unwrap(),
            b"external song"
        );
    }
    #[test]
    fn actual_auto_recovery_worker_retains_published_warning_and_retries_only_sync() {
        let (_root, file) = file();
        let mut auto = AutoRecovery::new(std::time::Duration::ZERO);
        auto.request_with_sync(&file, std::time::Instant::now(), late_failure)
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !auto.poll_pending().unwrap() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(file.has_pending_recovery_durability());
        let path = file.recovery_path().unwrap();
        let bytes = fs::read(&path).unwrap();
        let metadata = fs::metadata(&path).unwrap();
        auto.request(&file, std::time::Instant::now()).unwrap();
        assert!(auto.is_pending());
        while !auto.poll_pending().unwrap() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(!file.has_pending_recovery_durability());
        assert_eq!(fs::read(&path).unwrap(), bytes);
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(metadata.ino(), fs::metadata(&path).unwrap().ino());
        }
        auto.request(&file, std::time::Instant::now()).unwrap();
        assert!(!auto.is_pending());
    }
    #[test]
    fn normal_save_retries_owned_recovery_barrier_and_retains_previous_draft() {
        let (_root, mut file) = file();
        let outcome = file
            .recovery_snapshot()
            .unwrap()
            .unwrap()
            .write_with_sync(true, late_failure)
            .unwrap();
        assert!(matches!(
            outcome,
            RecoveryOutcome::Published {
                durability_warning: Some(_),
                ..
            }
        ));
        let path = file.recovery_path().unwrap();
        assert!(path.exists());
        let draft_before = fs::read(&path).unwrap();
        file.save().unwrap();
        assert!(!file.has_pending_recovery_durability());
        assert_eq!(fs::read(&path).unwrap(), draft_before);
        assert!(!file.session.is_dirty());
        let reopened = FileSession::open(file.existing_path().unwrap()).unwrap();
        assert_eq!(
            reopened.session.document().text(None, "TITLE").as_deref(),
            Some("Recovered")
        );
    }
    #[test]
    fn newer_edits_after_warning_are_published_after_retry_without_losing_current_content() {
        let (_root, mut file) = file();
        let outcome = file
            .recovery_snapshot()
            .unwrap()
            .unwrap()
            .write_with_sync(true, late_failure)
            .unwrap();
        assert!(matches!(
            outcome,
            RecoveryOutcome::Published {
                durability_warning: Some(_),
                ..
            }
        ));
        file.session.set_text(None, "TITLE", "Newest edit").unwrap();
        let outcome = file.save_recovery().unwrap();
        assert!(matches!(
            outcome,
            RecoveryOutcome::Published {
                newly_written: true,
                durability_warning: None
            }
        ));
        let mut reopened = FileSession::open(file.existing_path().unwrap()).unwrap();
        reopened.recover().unwrap();
        assert_eq!(
            reopened.session.document().text(None, "TITLE").as_deref(),
            Some("Newest edit")
        );
        assert!(file.session.is_dirty());
    }
    #[test]
    fn frame_warning_queries_never_wait_for_a_worker_receipt_lock() {
        let (_root, file) = file();
        let _worker_guard = file.recovery_durability.lock().unwrap();
        assert!(file.has_pending_recovery_durability());
        assert!(file.recovery_durability_warning().is_none());
    }
}
