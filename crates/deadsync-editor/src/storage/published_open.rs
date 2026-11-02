//! A namespace publication, never a later metadata observation, authorizes an
//! exact reopen. Preparation allocates before persist; activation is memory-only.
use super::save_as::{ParentIdentity, file_identity, parent_identity};
use super::*;
use crate::{history::file_intent::IntentGuard, output_policy::OutputPolicy};
use std::sync::{
    OnceLock,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
#[derive(Clone)]
pub struct PublishedOpenWitness {
    inner: Arc<Inner>,
}
pub(super) struct Publication {
    pub(super) destination: PathBuf,
    pub(super) parent: PathBuf,
    pub(super) parent_id: ParentIdentity,
    pub(super) identity: ParentIdentity,
    pub(super) expected: Arc<[u8]>,
    pub(super) policy: OutputPolicy,
    pub(super) policy_revision: u64,
    pub(super) publication_revision: u64,
}
struct Inner {
    publication: Publication,
    epoch: Arc<AtomicU64>,
    format: Format,
    guard: OnceLock<IntentGuard>,
}
impl PublishedOpenWitness {
    pub(super) fn prepare(publication: Publication, epoch: Arc<AtomicU64>, format: Format) -> Self {
        Self {
            inner: Arc::new(Inner {
                publication,
                epoch,
                format,
                guard: OnceLock::new(),
            }),
        }
    }
    pub(super) fn activate(&self, session: &Session) {
        // This object has not escaped preparation. IntentGuard capture only
        // clones its existing Arc token; no filesystem or heap allocation.
        self.inner
            .guard
            .get_or_init(|| IntentGuard::capture(session));
    }
    pub fn path(&self) -> &Path {
        &self.inner.publication.destination
    }
    pub fn format(&self) -> Format {
        self.inner.format
    }
    fn check_epoch(&self, cancelled: &AtomicBool) -> Result<(), Error> {
        if cancelled.load(Ordering::Acquire) {
            return Err(Error::InvalidEdit(
                "Published song opening cancelled".into(),
            ));
        }
        if self.inner.guard.get().is_none()
            || self.inner.epoch.load(Ordering::Acquire)
                != self.inner.publication.publication_revision
        {
            return Err(Error::ExternalChange);
        }
        Ok(())
    }
    fn memory_current(&self, file: &FileSession) -> bool {
        file.publication_witness
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(&current.inner, &self.inner))
            && Arc::ptr_eq(&self.inner.epoch, &file.publication_epoch)
            && self.inner.epoch.load(Ordering::Acquire)
                == self.inner.publication.publication_revision
            && file.publication_revision == self.inner.publication.publication_revision
            && file.policy_revision == self.inner.publication.policy_revision
            && file.output_policy == self.inner.publication.policy
            && file.existing_path() == Some(self.path())
            && file
                .expected
                .as_ref()
                .is_some_and(|expected| Arc::ptr_eq(expected, &self.inner.publication.expected))
            && self
                .inner
                .guard
                .get()
                .is_some_and(|guard| guard.is_current(&file.session))
            && !file.has_unsaved_document()
    }
    fn checked_file(&self, cancelled: &AtomicBool) -> Result<fs::File, Error> {
        self.check_epoch(cancelled)?;
        if self.path().canonicalize()?.as_path() != self.path()
            || parent_identity(&self.inner.publication.parent)? != self.inner.publication.parent_id
        {
            return Err(Error::ExternalChange);
        }
        let metadata = fs::symlink_metadata(self.path())?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(Error::ExternalChange);
        }
        let file = fs::File::open(self.path())?;
        if file_identity(&file)? != self.inner.publication.identity
            || file.metadata()?.len() != self.inner.publication.expected.len() as u64
        {
            return Err(Error::ExternalChange);
        }
        Ok(file)
    }
    fn validate_disk(&self, cancelled: &AtomicBool) -> Result<(), Error> {
        let mut file = self.checked_file(cancelled)?;
        let mut offset = 0usize;
        let mut chunk = [0u8; 64 * 1024];
        loop {
            self.check_epoch(cancelled)?;
            let count = file.read(&mut chunk)?;
            if count == 0 {
                break;
            }
            let end = offset.checked_add(count).ok_or(Error::ExternalChange)?;
            if self.inner.publication.expected.get(offset..end) != Some(&chunk[..count]) {
                return Err(Error::ExternalChange);
            }
            offset = end;
        }
        if offset != self.inner.publication.expected.len()
            || file_identity(&file)? != self.inner.publication.identity
        {
            return Err(Error::ExternalChange);
        }
        // Validate namespace again rather than trusting the still-open old fd.
        let _current = self.checked_file(cancelled)?;
        Ok(())
    }
    pub fn validate_source(&self, file: &FileSession) -> Result<(), Error> {
        if !self.memory_current(file) {
            return Err(Error::ExternalChange);
        }
        self.validate_disk(&AtomicBool::new(false))
    }
    pub fn validate_opened(&self, file: &FileSession, cancelled: &AtomicBool) -> Result<(), Error> {
        if file.existing_path() != Some(self.path())
            || file.session.document().format != self.inner.format
            || file.expected.as_deref() != Some(self.inner.publication.expected.as_ref())
        {
            return Err(Error::ExternalChange);
        }
        self.validate_disk(cancelled)
    }
    /// Read-only detached preparation. Call validate_source against the source
    /// FileSession again immediately before replacing any editor state.
    pub fn open_bounded(
        &self,
        maximum: usize,
        cancelled: &AtomicBool,
    ) -> Result<FileSession, Error> {
        self.open_bounded_with_context(maximum,cancelled,Default::default())
    }
    pub fn open_bounded_with_context(&self,maximum:usize,cancelled:&AtomicBool,context:crate::document::LoadContext)->Result<FileSession,Error>{self.open_with_context_hook(maximum,cancelled,context,||{})}
    #[cfg(test)]
    fn open_with_hook(
        &self,
        maximum: usize,
        cancelled: &AtomicBool,
        after_read: impl FnOnce(),
    ) -> Result<FileSession, Error> {
        self.open_with_context_hook(maximum,cancelled,Default::default(),after_read)
    }
    fn open_with_context_hook(&self,maximum:usize,cancelled:&AtomicBool,context:crate::document::LoadContext,after_read:impl FnOnce())->Result<FileSession,Error>{
        if self.inner.publication.expected.len() > maximum {
            return Err(Error::Unsupported(
                "Published song exceeds opening byte limit".into(),
            ));
        }
        let opened = FileSession::open_read_with_context(self.path(),context, Some(cancelled), |canonical| {
            if canonical != self.path() {
                return Err(Error::ExternalChange);
            }
            let file = self.checked_file(cancelled)?;
            let bytes = read_bounded(file, maximum, cancelled)?;
            if bytes.as_slice() != self.inner.publication.expected.as_ref() {
                return Err(Error::ExternalChange);
            }
            after_read();
            self.validate_disk(cancelled)?;
            Ok(bytes)
        })?;
        self.validate_opened(&opened, cancelled)?;
        Ok(opened)
    }
}
impl FileSession {
    /// A pending namespace output becomes capturable only after its exact
    /// durability retry marks the unchanged native session saved.
    pub fn published_open_witness(&self) -> Result<PublishedOpenWitness, Error> {
        let witness = self.publication_witness.as_ref().ok_or_else(|| {
            Error::InvalidEdit("No published output is available for exact reopening".into())
        })?;
        if !witness.memory_current(self) {
            return Err(Error::InvalidEdit(
                "Published output is not durably current".into(),
            ));
        }
        Ok(witness.clone())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    fn file() -> (tempfile::TempDir, FileSession) {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("source.ssc");
        fs::write(
            &path,
            b"#VERSION:0.83;#TITLE:Original;#CREDIT:Private author;#OFFSET:0;",
        )
        .unwrap();
        let mut file = FileSession::open(&path).unwrap();
        file.session.set_text(None, "TITLE", "Published").unwrap();
        file.save().unwrap();
        (root, file)
    }
    fn replace(path: &Path, bytes: &[u8]) {
        let mut pending = tempfile::NamedTempFile::new_in(path.parent().unwrap()).unwrap();
        pending.write_all(bytes).unwrap();
        pending.as_file().sync_all().unwrap();
        super::super::save_as::persist_output(pending, path).unwrap();
    }
    #[test]
    fn witness_requires_real_publication_and_reopens_exact_output() {
        let (_root, file) = file();
        let witness = file.published_open_witness().unwrap();
        witness.validate_source(&file).unwrap();
        let reopened = witness
            .open_bounded(1024 * 1024, &AtomicBool::new(false))
            .unwrap();
        witness
            .validate_opened(&reopened, &AtomicBool::new(false))
            .unwrap();
        assert_eq!(reopened.expected, file.expected);
        assert_eq!(witness.format(), Format::Ssc);
        let merely_opened = FileSession::open(file.require_existing_path().unwrap()).unwrap();
        assert!(merely_opened.published_open_witness().is_err());
    }
    #[test]
    fn identical_bytes_new_inode_and_changed_payload_are_not_adopted_before_open() {
        for identical in [false, true] {
            let (_root, file) = file();
            let witness = file.published_open_witness().unwrap();
            let path = file.require_existing_path().unwrap();
            let bytes = fs::read(path).unwrap();
            replace(
                path,
                if identical {
                    &bytes
                } else {
                    b"#VERSION:0.83;#TITLE:External;"
                },
            );
            assert!(
                witness
                    .open_bounded(1024 * 1024, &AtomicBool::new(false))
                    .is_err()
            );
            assert!(witness.validate_source(&file).is_err());
            if identical {
                assert!(
                    !file.has_external_change().unwrap(),
                    "byte comparison alone is insufficient for reopening authority"
                );
            }
        }
    }
    #[test]
    fn replacement_after_read_is_rejected_even_when_payload_is_identical() {
        for identical in [false, true] {
            let (_root, file) = file();
            let witness = file.published_open_witness().unwrap();
            let path = file.require_existing_path().unwrap().to_owned();
            let bytes = fs::read(&path).unwrap();
            let result = witness.open_with_hook(1024 * 1024, &AtomicBool::new(false), || {
                replace(
                    &path,
                    if identical {
                        &bytes
                    } else {
                        b"#VERSION:0.83;#TITLE:After read;"
                    },
                )
            });
            assert!(result.is_err());
        }
    }
    #[test]
    fn last_publication_barrier_rejects_a_replacement_after_candidate_open() {
        for identical in [false, true] {
            let (_root, file) = file();
            let witness = file.published_open_witness().unwrap();
            let opened = witness
                .open_bounded(1024 * 1024, &AtomicBool::new(false))
                .unwrap();
            let bytes = fs::read(witness.path()).unwrap();
            replace(
                witness.path(),
                if identical {
                    &bytes
                } else {
                    b"#VERSION:0.83;#TITLE:After candidate;"
                },
            );
            assert!(
                witness
                    .validate_opened(&opened, &AtomicBool::new(false))
                    .is_err()
            );
            assert!(witness.validate_source(&file).is_err());
        }
    }
    #[test]
    fn edit_undo_new_publication_and_recovery_revoke_old_authority() {
        let (_root, mut file) = file();
        let old = file.published_open_witness().unwrap();
        file.session
            .set_text(None, "TITLE", "Intervening edit")
            .unwrap();
        file.session.undo().unwrap();
        assert!(!file.session.is_dirty());
        assert!(file.published_open_witness().is_err());
        assert!(old.validate_source(&file).is_err());
        file.save().unwrap();
        let newer = file.published_open_witness().unwrap();
        assert!(
            old.open_bounded(1024 * 1024, &AtomicBool::new(false))
                .is_err()
        );
        newer.validate_source(&file).unwrap();
        file.session
            .set_text(None, "TITLE", "Recoverable draft")
            .unwrap();
        file.save_recovery().unwrap();
        file.session.undo().unwrap();
        file.recover().unwrap();
        assert!(file.publication_witness.is_none());
        assert!(
            newer
                .open_bounded(1024 * 1024, &AtomicBool::new(false))
                .is_err()
        );
    }
    #[test]
    fn save_as_uses_real_target_format_policy_and_origin_without_rewriting_native() {
        let (root, mut file) = file();
        let old = file.published_open_witness().unwrap();
        let target = root.path().join("export.sm");
        let policy = OutputPolicy::AdvancedChartless {
            format: Format::Sm,
            options: crate::advanced_export::Options {
                anonymize: true,
                ..Default::default()
            },
        };
        let review = file
            .prepare_save_as_with_policy(&target, policy.clone())
            .unwrap();
        let ack = review.issues().iter().enumerate().map(|(i, _)| i).collect();
        let outcome = file.save_as_reviewed(review, &ack).unwrap();
        assert_eq!(file.session.document().format, Format::Ssc);
        assert_eq!(file.output_policy(), &policy);
        assert!(old.validate_source(&file).is_err());
        assert_eq!(
            file.publication_witness.as_ref().unwrap().format(),
            Format::Sm
        );
        match outcome {
            SaveAsOutcome::Durable { .. } => {
                let witness = file.published_open_witness().unwrap();
                assert_eq!(witness.path(), target.canonicalize().unwrap().as_path());
                let opened = witness
                    .open_bounded(1024 * 1024, &AtomicBool::new(false))
                    .unwrap();
                assert_eq!(opened.session.document().format, Format::Sm);
                assert_eq!(
                    file.session.document().text(None, "CREDIT").as_deref(),
                    Some("Private author")
                );
            }
            SaveAsOutcome::WrittenPendingDurability { .. } => {
                assert!(file.published_open_witness().is_err())
            }
        }
    }
    #[test]
    fn bounds_cancellation_and_policy_republication_do_not_reuse_old_witness() {
        let (_root, mut file) = file();
        let old = file.published_open_witness().unwrap();
        let bytes = fs::read(old.path()).unwrap();
        assert!(old.open_bounded(4, &AtomicBool::new(false)).is_err());
        assert!(
            old.open_bounded(1024 * 1024, &AtomicBool::new(true))
                .is_err()
        );
        assert_eq!(fs::read(old.path()).unwrap(), bytes);
        let policy = OutputPolicy::AdvancedChartless {
            format: Format::Ssc,
            options: Default::default(),
        };
        let review = file.prepare_save_with_policy(policy).unwrap();
        let ack = review.issues().iter().enumerate().map(|(i, _)| i).collect();
        file.save_reviewed(review, &ack).unwrap();
        assert!(old.validate_source(&file).is_err());
        file.published_open_witness()
            .unwrap()
            .validate_source(&file)
            .unwrap();
    }
    #[test]
    fn parent_directory_replacement_never_refreshes_publication_identity() {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("Published");
        fs::create_dir(&parent).unwrap();
        let path = parent.join("source.ssc");
        fs::write(&path, b"#VERSION:0.83;#TITLE:Owned;").unwrap();
        let mut file = FileSession::open(&path).unwrap();
        file.save().unwrap();
        let witness = file.published_open_witness().unwrap();
        let bytes = fs::read(&path).unwrap();
        fs::rename(&parent, root.path().join("Moved")).unwrap();
        fs::create_dir(&parent).unwrap();
        fs::write(&path, bytes).unwrap();
        assert!(
            witness
                .open_bounded(1024 * 1024, &AtomicBool::new(false))
                .is_err()
        );
        assert!(witness.validate_source(&file).is_err());
    }
}
