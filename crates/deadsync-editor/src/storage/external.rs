//! Immutable external probes; capture/currentness never access the filesystem.
//! Probe IO belongs to a cancellable, byte-bounded background worker.
use super::*;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
#[derive(Clone)]
pub struct ExternalSnapshot {
    path: PathBuf,
    baseline: Arc<[u8]>,
    epoch: Arc<AtomicU64>,
    revision: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExternalObservation {
    Unchanged,
    Changed(Arc<[u8]>),
    Missing,
}
impl FileSession {
    pub fn external_snapshot(&self) -> Option<ExternalSnapshot> {
        Some(ExternalSnapshot {
            path: self.existing_path()?.to_owned(),
            baseline: Arc::clone(self.expected.as_ref()?),
            epoch: Arc::clone(&self.publication_epoch),
            revision: self.publication_revision,
        })
    }
}
impl ExternalSnapshot {
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn is_current(&self, file: &FileSession) -> bool {
        Arc::ptr_eq(&self.epoch, &file.publication_epoch)
            && self.revision == file.publication_revision
            && file
                .expected
                .as_ref()
                .is_some_and(|baseline| Arc::ptr_eq(baseline, &self.baseline))
            && file.existing_path() == Some(self.path.as_path())
    }
    pub fn probe(&self, cancelled: &AtomicBool) -> Result<ExternalObservation, Error> {
        let check = || {
            if cancelled.load(Ordering::Acquire) {
                Err(Error::InvalidEdit("External probe cancelled".into()))
            } else {
                Ok(())
            }
        };
        check()?;
        // Reuse the existing advanced-file byte budget. Never allocate an
        // unbounded external file merely to display a change notification.
        let limit = crate::advanced_export::Limits::default().bytes;
        if self.baseline.len() > limit {
            return Err(Error::Unsupported(
                "External change monitoring byte limit exceeded".into(),
            ));
        }
        let metadata = match fs::metadata(&self.path) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ExternalObservation::Missing);
            }
            Err(error) => return Err(error.into()),
        };
        if !metadata.is_file() {
            return Err(Error::Unsupported(
                "External song path is not a regular file".into(),
            ));
        }
        if metadata.len() > limit as u64 {
            return Err(Error::Unsupported(
                "External change monitoring byte limit exceeded".into(),
            ));
        }
        check()?;
        let file = match fs::File::open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ExternalObservation::Missing);
            }
            Err(error) => return Err(error.into()),
        };
        let bytes = read_bounded(file, limit, cancelled)?;
        check()?;
        if bytes.as_slice() == self.baseline.as_ref() {
            Ok(ExternalObservation::Unchanged)
        } else {
            Ok(ExternalObservation::Changed(bytes.into()))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn file() -> (tempfile::TempDir, FileSession) {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.ssc");
        fs::write(&path, b"#VERSION:0.83;#TITLE:Baseline;").unwrap();
        let file = FileSession::open(&path).unwrap();
        (root, file)
    }
    #[test]
    fn ignore_observation_never_adopts_external_baseline_or_recovery() {
        let (_root, mut file) = file();
        file.session
            .set_text(None, "TITLE", "Native draft")
            .unwrap();
        file.save_recovery().unwrap();
        let recovery = fs::read(file.recovery_path().unwrap()).unwrap();
        let watch = file.external_snapshot().unwrap();
        fs::write(watch.path(), b"#VERSION:0.83;#TITLE:External;").unwrap();
        let ignored = watch.probe(&AtomicBool::new(false)).unwrap();
        assert!(matches!(ignored, ExternalObservation::Changed(_)));
        assert!(watch.is_current(&file));
        assert_eq!(watch.probe(&AtomicBool::new(false)).unwrap(), ignored);
        assert!(matches!(file.save(), Err(Error::ExternalChange)));
        assert_eq!(fs::read(file.recovery_path().unwrap()).unwrap(), recovery);
        assert!(file.session.is_dirty());
    }
    #[test]
    fn publication_or_another_session_revokes_probe_without_rebinding_baseline() {
        let (_root, mut file) = file();
        let old = file.external_snapshot().unwrap();
        let other = FileSession::open(file.require_existing_path().unwrap()).unwrap();
        assert!(!old.is_current(&other));
        file.save().unwrap();
        assert!(!old.is_current(&file));
        assert!(matches!(
            file.external_snapshot()
                .unwrap()
                .probe(&AtomicBool::new(false))
                .unwrap(),
            ExternalObservation::Unchanged
        ));
    }
    #[test]
    fn missing_access_failure_cancellation_and_oversize_are_distinct() {
        let (root, file) = file();
        let snapshot = file.external_snapshot().unwrap();
        assert!(snapshot.probe(&AtomicBool::new(true)).is_err());
        fs::remove_file(snapshot.path()).unwrap();
        assert_eq!(
            snapshot.probe(&AtomicBool::new(false)).unwrap(),
            ExternalObservation::Missing
        );
        fs::create_dir(snapshot.path()).unwrap();
        assert!(matches!(
            snapshot.probe(&AtomicBool::new(false)),
            Err(Error::Unsupported(_))
        ));
        fs::remove_dir(snapshot.path()).unwrap();
        let large = fs::File::create(snapshot.path()).unwrap();
        large
            .set_len(crate::advanced_export::Limits::default().bytes as u64 + 1)
            .unwrap();
        assert!(matches!(
            snapshot.probe(&AtomicBool::new(false)),
            Err(Error::Unsupported(_))
        ));
        let mut inaccessible = snapshot.clone();
        inaccessible.path = snapshot.path.join("child.ssc");
        assert!(matches!(
            inaccessible.probe(&AtomicBool::new(false)),
            Err(Error::Io(_))
        ));
        assert!(root.path().exists());
    }
}
