//! Reviewed Save As adapter. Review and output are staged before live mutation.
#[cfg(test)]
mod untitled_tests;
use super::FileSession;
#[cfg(windows)]
mod windows_identity;
#[cfg(windows)]
mod windows_publish;

fn sync_save_as_directory(path: &std::path::Path) -> Result<(), crate::Error> {
    #[cfg(unix)]
    {
        super::sync_directory(path)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err(crate::Error::Unsupported(
            "Save As directory durability is not implemented on this platform".into(),
        ))
    }
}
pub use crate::history::media_relocation::rebind::RelocationReceipt;
use crate::{
    Error,
    history::{ContentToken, media_relocation::Plan},
};
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ParentIdentity {
    device: u64,
    inode: u128,
}
pub(super) fn parent_identity(path: &Path) -> Result<ParentIdentity, Error> {
    #[cfg(unix)]
    {
        let metadata = fs::metadata(path)?;
        if !metadata.is_dir() {
            return Err(Error::InvalidEdit(
                "Save As parent is not a directory".into(),
            ));
        }
        Ok(ParentIdentity {
            device: metadata.dev(),
            inode: u128::from(metadata.ino()),
        })
    }
    #[cfg(windows)]
    {
        let id = windows_identity::directory(path)?;
        Ok(ParentIdentity {
            device: id.volume,
            inode: id.file,
        })
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        Err(Error::Unsupported(
            "Save As directory identity is not implemented on this platform".into(),
        ))
    }
}
#[derive(PartialEq, Eq)]
struct Baseline {
    bytes: Arc<[u8]>,
    identity: ParentIdentity,
}
pub(super) fn file_identity(file: &fs::File) -> Result<ParentIdentity, Error> {
    #[cfg(unix)]
    {
        let metadata = file.metadata()?;
        Ok(ParentIdentity {
            device: metadata.dev(),
            inode: u128::from(metadata.ino()),
        })
    }
    #[cfg(windows)]
    {
        let id = windows_identity::file(file)?;
        Ok(ParentIdentity {
            device: id.volume,
            inode: id.file,
        })
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = file;
        Err(Error::Unsupported(
            "Save As file identity is not implemented on this platform".into(),
        ))
    }
}
fn baseline(path: &Path) -> Result<Option<Baseline>, Error> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => Err(
            Error::Unsupported("Save As destination must be a regular file or absent".into()),
        ),
        Ok(_) => {
            let mut file = fs::File::open(path)?;
            let identity = file_identity(&file)?;
            const MAX_REVIEW_BYTES: u64 = 128 * 1024 * 1024;
            if file.metadata()?.len() > MAX_REVIEW_BYTES {
                return Err(Error::Unsupported(
                    "Save As destination exceeds review byte limit".into(),
                ));
            }
            let mut bytes = Vec::new();
            (&mut file)
                .take(MAX_REVIEW_BYTES + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() as u64 > MAX_REVIEW_BYTES {
                return Err(Error::Unsupported(
                    "Save As destination exceeds review byte limit".into(),
                ));
            }
            Ok(Some(Baseline {
                bytes: bytes.into(),
                identity,
            }))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}
/// Exact destination baseline to display for overwrite review before commit.
/// This object alone does not write or authorize another destination later.
pub struct SaveAsReview {
    source: crate::file_origin::Origin,
    source_expected: Option<Arc<[u8]>>,
    previous_policy: crate::output_policy::OutputPolicy,
    policy_revision: u64,
    publication_revision: u64,
    candidate: crate::history::Session,
    output: crate::output_policy::PreparedOutput,
    generated: Option<crate::output_policy::Output>,
    maximum_bytes: usize,
    destination: PathBuf,
    parent: PathBuf,
    parent_id: ParentIdentity,
    expected: Option<Baseline>,
    plan: Plan,
    relocation: RelocationReceipt,
}
/// Old recovery ownership is retained explicitly; no source draft is deleted.
pub struct PreviousRecovery {
    pub path: PathBuf,
    receipt: Arc<Mutex<Option<Arc<[u8]>>>>,
}
impl PreviousRecovery {
    pub fn has_owned_receipt(&self) -> bool {
        self.receipt
            .lock()
            .ok()
            .is_some_and(|value| value.is_some())
    }
}
pub struct PendingDurability {
    policy: crate::output_policy::OutputPolicy,
    policy_revision: u64,
    publication_revision: u64,
    destination: PathBuf,
    parent: PathBuf,
    parent_id: ParentIdentity,
    expected: Arc<[u8]>,
    identity: ParentIdentity,
    content: ContentToken,
}
pub enum SaveAsOutcome {
    Durable {
        previous_recovery: Option<PreviousRecovery>,
        relocation: RelocationReceipt,
    },
    WrittenPendingDurability {
        receipt: PendingDurability,
        previous_recovery: Option<PreviousRecovery>,
        relocation: RelocationReceipt,
        error: Error,
    },
}
impl SaveAsReview {
    pub fn is_current(&self, file: &FileSession) -> bool {
        self.source == file.origin
            && self.source_expected == file.expected
            && &self.previous_policy == file.output_policy()
            && self.policy_revision == file.policy_revision
            && self.publication_revision == file.publication_revision
            && self.plan.validate(&file.session).is_ok()
            && self.output.is_current(&self.candidate)
            && self
                .generated
                .as_ref()
                .is_none_or(|output| output.is_current(&self.candidate))
    }
    pub fn issues(&self) -> &[crate::advanced_export::Issue] {
        self.output.issues()
    }
    pub fn output_policy(&self) -> &crate::output_policy::OutputPolicy {
        self.output.policy()
    }
    fn generate_output(
        &mut self,
        acknowledged: &std::collections::BTreeSet<usize>,
    ) -> Result<(), Error> {
        self.generated = Some(self.output.generate(
            &self.candidate,
            acknowledged,
            self.maximum_bytes,
            &std::sync::atomic::AtomicBool::new(false),
        )?);
        Ok(())
    }

    pub fn destination(&self) -> &Path {
        &self.destination
    }
    pub fn relocation(&self) -> &RelocationReceipt {
        &self.relocation
    }
    pub fn replaces_existing(&self) -> bool {
        self.expected.is_some()
    }
    pub fn staged_bytes(&self) -> &[u8] {
        self.plan.bytes()
    }
    fn validate_target(&self) -> Result<(), Error> {
        if parent_identity(&self.parent)? != self.parent_id
            || baseline(&self.destination)? != self.expected
        {
            return Err(Error::ExternalChange);
        }
        Ok(())
    }
}
impl FileSession {
    pub fn prepare_save_as(&self, destination: &Path) -> Result<SaveAsReview, Error> {
        self.prepare_save_as_with_revisions(destination, &[])
    }
    pub fn prepare_save_as_with_revisions(
        &self,
        destination: &Path,
        extra: &[u64],
    ) -> Result<SaveAsReview, Error> {
        self.prepare_save_as_with_policy_and_revisions(
            destination,
            self.output_policy().clone(),
            extra,
        )
    }
    pub fn prepare_save_as_with_policy(
        &self,
        destination: &Path,
        policy: crate::output_policy::OutputPolicy,
    ) -> Result<SaveAsReview, Error> {
        self.prepare_save_as_with_policy_and_revisions(destination, policy, &[])
    }
    pub fn prepare_save_as_with_policy_and_revisions(
        &self,
        destination: &Path,
        policy: crate::output_policy::OutputPolicy,
        extra: &[u64],
    ) -> Result<SaveAsReview, Error> {
        self.prepare_save_as_order(destination, policy, extra, None)
    }
    pub fn prepare_save_as_with_policy_and_revisions_ordered(
        &self,
        destination: &Path,
        policy: crate::output_policy::OutputPolicy,
        extra: &[u64],
        order: &crate::source_output_order::SourceOutputOrder,
    ) -> Result<SaveAsReview, Error> {
        self.prepare_save_as_order(destination, policy, extra, Some(order))
    }
    fn prepare_save_as_order(
        &self,
        destination: &Path,
        policy: crate::output_policy::OutputPolicy,
        extra: &[u64],
        order: Option<&crate::source_output_order::SourceOutputOrder>,
    ) -> Result<SaveAsReview, Error> {
        self.retry_recovery_durability()?;
        if !destination.is_absolute() {
            return Err(Error::InvalidEdit(
                "Save As requires an absolute destination".into(),
            ));
        }
        let name = destination
            .file_name()
            .ok_or_else(|| Error::InvalidEdit("Save As destination has no file name".into()))?;
        let parent = destination
            .parent()
            .ok_or_else(|| Error::InvalidEdit("Save As destination has no parent".into()))?
            .canonicalize()?;
        let destination = parent.join(name);
        if self.existing_path() == Some(destination.as_path()) {
            return Err(Error::InvalidEdit("Use Save for the current file".into()));
        }
        let parent_id = parent_identity(&parent)?;
        let expected = baseline(&destination)?;
        let target = super::save_output::policy_format(&policy, self.session.document().format);
        let source = if let Some(old) = self.existing_path() {
            crate::media_relocation::SourceBase::ExistingFile(old)
        } else {
            crate::media_relocation::SourceBase::ResourceDirectory(self.resource_base())
        };
        let mut plan = Plan::prepare_for_output(&self.session, source, &destination, target)?;
        let mut candidate = plan.detached_output_snapshot(&self.session)?;
        let limits = super::save_output::limits_for(&policy);
        let output = if matches!(policy, crate::output_policy::OutputPolicy::NativeLossless) {
            policy.prepare_output(
                &mut candidate,
                limits,
                &std::sync::atomic::AtomicBool::new(false),
            )?
        } else if let Some(order) = order {
            let candidate_order = order.for_rebased_candidate(&self.session, &mut candidate)?;
            policy.prepare_ordered_output(
                &mut candidate,
                limits,
                &std::sync::atomic::AtomicBool::new(false),
                &candidate_order,
            )?
        } else {
            policy.prepare_output(
                &mut candidate,
                limits,
                &std::sync::atomic::AtomicBool::new(false),
            )?
        };
        plan.include_external_revisions(extra)?;
        let mut relocation = plan.rebind_receipt();
        relocation.bind_output_policy(self.policy_revision, self.next_policy_revision(&policy)?);
        Ok(SaveAsReview {
            source: self.origin.clone(),
            source_expected: self.expected.clone(),
            previous_policy: self.output_policy().clone(),
            policy_revision: self.policy_revision,
            publication_revision: self.publication_revision,
            candidate,
            output,
            generated: None,
            maximum_bytes: limits.bytes,
            destination,
            parent,
            parent_id,
            expected,
            plan,
            relocation,
        })
    }
    pub fn save_as_reviewed(
        &mut self,
        mut review: SaveAsReview,
        acknowledged: &std::collections::BTreeSet<usize>,
    ) -> Result<SaveAsOutcome, Error> {
        review.plan.validate(&self.session)?;
        if self.origin != review.source
            || self.expected != review.source_expected
            || self.output_policy() != &review.previous_policy
            || self.policy_revision != review.policy_revision
            || self.publication_revision != review.publication_revision
        {
            return Err(Error::InvalidEdit(
                "Save As source policy changed since review".into(),
            ));
        }
        review.generate_output(acknowledged)?;
        self.save_as(review)
    }
    pub fn save_as(&mut self, review: SaveAsReview) -> Result<SaveAsOutcome, Error> {
        self.retry_recovery_durability()?;
        #[cfg(windows)]
        {
            // The successful persist callback includes the Windows namespace
            // WRITE_THROUGH barrier. No Unix-style directory fsync is assumed.
            self.save_as_with_io(
                review,
                |_| Ok(()),
                |pending, destination, absent| {
                    windows_publish::publish(pending, destination, absent)?;
                    Ok(())
                },
            )
        }
        #[cfg(not(windows))]
        {
            self.save_as_with_sync(review, sync_save_as_directory)
        }
    }
    fn save_as_with_sync(
        &mut self,
        review: SaveAsReview,
        sync: impl FnOnce(&Path) -> Result<(), Error>,
    ) -> Result<SaveAsOutcome, Error> {
        self.save_as_with_io(review, sync, |pending, destination, absent| {
            if absent {
                pending
                    .persist_noclobber(destination)
                    .map_err(|error| Error::Io(error.error))?;
            } else {
                pending
                    .persist(destination)
                    .map_err(|error| Error::Io(error.error))?;
            }
            Ok(())
        })
    }
    fn save_as_with_io(
        &mut self,
        mut review: SaveAsReview,
        sync: impl FnOnce(&Path) -> Result<(), Error>,
        persist: impl FnOnce(tempfile::NamedTempFile, &Path, bool) -> Result<(), Error>,
    ) -> Result<SaveAsOutcome, Error> {
        if self.origin != review.source
            || self.expected != review.source_expected
            || self.output_policy() != &review.previous_policy
            || self.policy_revision != review.policy_revision
            || self.publication_revision != review.publication_revision
        {
            return Err(Error::InvalidEdit(
                "Save As source changed since review".into(),
            ));
        }
        review.plan.validate(&self.session)?;
        if review.generated.is_none() {
            if !review.issues().is_empty() {
                return Err(Error::OutputReviewRequired);
            }
            review.generate_output(&std::collections::BTreeSet::new())?;
        }
        let output = review
            .generated
            .as_ref()
            .ok_or_else(|| Error::InvalidEdit("Missing generated output".into()))?;
        if !output.is_current(&review.candidate) {
            return Err(Error::InvalidEdit("Generated output became stale".into()));
        }
        let published_format = output.format();
        let policy = output.policy().clone();
        let next_policy = self.next_policy_revision(&policy)?;
        let next_publication = self.next_publication_revision()?;
        let expected: Arc<[u8]> = output.bytes().to_vec().into();
        let mut lock_name = std::ffi::OsString::from(".");
        lock_name.push(review.destination.file_name().unwrap());
        lock_name.push(".deadsync-save-lock");
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(review.parent.join(lock_name))?;
        lock.try_lock().map_err(std::io::Error::other)?;
        review.validate_target()?;
        let previous_recovery = match self.persisted() {
            Some(persisted) => Some(PreviousRecovery {
                path: self.sidecar_path(".deadsync-recovery")?,
                receipt: Arc::clone(persisted.recovery),
            }),
            None => None,
        };
        let new_origin = crate::file_origin::Origin::Existing(review.destination.clone());
        let new_resource_base = review.parent.clone();
        let new_recovery = Arc::new(Mutex::new(None));
        let mut pending = tempfile::NamedTempFile::new_in(&review.parent)?;
        if review.expected.is_some() {
            pending
                .as_file()
                .set_permissions(fs::metadata(&review.destination)?.permissions())?;
        }
        pending.write_all(&expected)?;
        pending.as_file().sync_all()?;
        let receipt = PendingDurability {
            policy: policy.clone(),
            policy_revision: next_policy,
            publication_revision: next_publication,
            destination: review.destination.clone(),
            parent: review.parent.clone(),
            parent_id: review.parent_id,
            expected: Arc::clone(&expected),
            identity: file_identity(pending.as_file())?,
            content: review.plan.relocated_content_token(),
        };
        let witness = receipt.prepare_open_witness(self, output.format());
        review.validate_target()?;
        if !output.is_current(&review.candidate) || !review.output.is_current(&review.candidate) {
            return Err(Error::InvalidEdit(
                "Source-compatible output order changed before publication".into(),
            ));
        }
        let SaveAsReview {
            destination,
            parent,
            plan,
            expected: baseline,
            relocation,
            ..
        } = review;
        // Holding this borrow prevents an edit between stale checking and commit.
        let publication = plan.publication(&mut self.session)?;
        persist(pending, &destination, baseline.is_none())?;
        publication.commit();
        self.session
            .rebase_authoring_policy_history(&policy, published_format);
        self.output_policy = policy;
        self.policy_revision = next_policy;
        self.publication_revision = next_publication;
        self.publication_epoch
            .store(next_publication, std::sync::atomic::Ordering::Release);
        self.origin = new_origin;
        self.resource_base = new_resource_base;
        self.expected = Some(expected);
        self.recovery_receipt = Some(new_recovery);
        self.recovery_durability = Default::default();
        witness.activate(&self.session);
        self.publication_witness = Some(witness);
        match sync(&parent) {
            Ok(()) => {
                self.session.mark_saved();
                Ok(SaveAsOutcome::Durable {
                    previous_recovery,
                    relocation,
                })
            }
            Err(error) => {
                // The rename happened. Publish that truth, but never claim durability.
                self.session.invalidate_saved_position();
                Ok(SaveAsOutcome::WrittenPendingDurability {
                    receipt,
                    previous_recovery,
                    relocation,
                    error,
                })
            }
        }
    }
}
impl PendingDurability {
    pub(super) fn prepare_open_witness(
        &self,
        file: &FileSession,
        format: crate::document::Format,
    ) -> super::PublishedOpenWitness {
        super::PublishedOpenWitness::prepare(
            super::published_open::Publication {
                destination: self.destination.clone(),
                parent: self.parent.clone(),
                parent_id: self.parent_id,
                identity: self.identity,
                expected: Arc::clone(&self.expected),
                policy: self.policy.clone(),
                policy_revision: self.policy_revision,
                publication_revision: self.publication_revision,
            },
            Arc::clone(&file.publication_epoch),
            format,
        )
    }
    pub(super) fn for_output(
        destination: &Path,
        parent: &Path,
        expected: Arc<[u8]>,
        pending: &fs::File,
        content: ContentToken,
        policy: crate::output_policy::OutputPolicy,
        policy_revision: u64,
        publication_revision: u64,
    ) -> Result<Self, Error> {
        Ok(Self {
            destination: destination.to_owned(),
            parent: parent.to_owned(),
            parent_id: parent_identity(parent)?,
            expected,
            identity: file_identity(pending)?,
            content,
            policy,
            policy_revision,
            publication_revision,
        })
    }
    pub fn destination(&self) -> &Path {
        &self.destination
    }
    pub fn retry(&self, file: &mut FileSession) -> Result<(), Error> {
        if file.output_policy() != &self.policy
            || file.policy_revision != self.policy_revision
            || file.publication_revision != self.publication_revision
            || file.existing_path() != Some(self.destination.as_path())
            || file.expected.as_ref() != Some(&self.expected)
            || parent_identity(&self.parent)? != self.parent_id
            || baseline(&self.destination)?
                .as_ref()
                .is_none_or(|baseline| {
                    baseline.bytes != self.expected || baseline.identity != self.identity
                })
        {
            return Err(Error::ExternalChange);
        }
        sync_save_as_directory(&self.parent)?;
        if !file.session.accepts_content_token(&self.content) {
            return Err(Error::InvalidEdit(
                "Song changed after Save As output; save current edits".into(),
            ));
        }
        file.session.mark_saved();
        Ok(())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    fn file() -> (tempfile::TempDir, FileSession, PathBuf, Vec<u8>) {
        let root = tempfile::tempdir().unwrap();
        let root_path = root.path().canonicalize().unwrap();
        let old = root_path.join("old");
        let new = root_path.join("new");
        fs::create_dir(&old).unwrap();
        fs::create_dir(&new).unwrap();
        let source = old.join("song.ssc");
        let bytes=b"#VERSION:0.83;#MUSIC:music.ogg;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#MUSIC:chart.ogg;#NOTES:0000;".to_vec();
        fs::write(&source, &bytes).unwrap();
        let file = FileSession::open(&source).unwrap();
        (root, file, new.join("copy.ssc"), bytes)
    }
    #[test]
    fn advanced_policy_is_installed_after_namespace_publication_even_when_directory_sync_fails() {
        let (_root, mut file, destination, original) = file();
        let old = file.require_existing_path().unwrap().to_owned();
        let policy = crate::output_policy::OutputPolicy::Advanced {
            format: crate::document::Format::Ssc,
            options: Default::default(),
            timing_chart: file.session.chart_ids()[0].clone(),
        };
        let review = file
            .prepare_save_as_with_policy(&destination, policy.clone())
            .unwrap();
        assert!(review.issues().is_empty(), "{:?}", review.issues());
        let receipt = match file
            .save_as_with_sync(review, |_| {
                Err(Error::Io(std::io::Error::other("injected directory sync")))
            })
            .unwrap()
        {
            SaveAsOutcome::WrittenPendingDurability {
                receipt,
                relocation,
                ..
            } => {
                assert!(relocation.validate_after(&file.session).is_ok());
                receipt
            }
            _ => panic!("expected pending namespace output"),
        };
        assert_eq!(file.output_policy(), &policy);
        assert_eq!(file.existing_path(), Some(destination.as_path()));
        assert_eq!(fs::read(old).unwrap(), original);
        assert!(file.session.is_dirty());
        let bytes = fs::read(&destination).unwrap();
        receipt.retry(&mut file).unwrap();
        assert!(!file.session.is_dirty());
        assert_eq!(fs::read(destination).unwrap(), bytes);
    }
    #[test]
    fn failed_advanced_namespace_publication_preserves_source_policy_history_and_inactive_receipt()
    {
        let (_root, mut file, destination, _) = file();
        let before = file.session.serialize().unwrap();
        let ids = file.session.chart_ids().to_vec();
        let path = file.require_existing_path().unwrap().to_owned();
        let policy = crate::output_policy::OutputPolicy::Advanced {
            format: crate::document::Format::Ssc,
            options: Default::default(),
            timing_chart: file.session.chart_ids()[0].clone(),
        };
        let review = file
            .prepare_save_as_with_policy(&destination, policy)
            .unwrap();
        assert!(review.issues().is_empty(), "{:?}", review.issues());
        let receipt = review.relocation().clone();
        assert!(
            file.save_as_with_io(
                review,
                |_| Ok(()),
                |_pending, _destination, _absent| Err(Error::Io(std::io::Error::other(
                    "injected namespace failure"
                )))
            )
            .is_err()
        );
        assert!(!destination.exists());
        assert!(matches!(
            file.output_policy(),
            crate::output_policy::OutputPolicy::NativeLossless
        ));
        assert_eq!(file.require_existing_path().unwrap(), path);
        assert_eq!(file.session.serialize().unwrap(), before);
        assert_eq!(file.session.chart_ids(), ids);
        assert!(receipt.validate_after(&file.session).is_err());
    }
    #[test]
    fn durable_save_as_preserves_source_history_chart_music_and_supports_next_save() {
        let (_root, mut file, destination, original) = file();
        let source = file.require_existing_path().unwrap().to_owned();
        file.session
            .set_text(None, "TITLE", "Working edit")
            .unwrap();
        let ids = file.session.chart_ids().to_vec();
        let review = file.prepare_save_as(&destination).unwrap();
        let staged = review.staged_bytes().to_vec();
        assert_eq!(review.candidate.serialize().unwrap(), staged);
        let bytes = review.candidate.serialize_for_output().unwrap();
        assert_ne!(bytes, staged, "explicit output materializes captured sync");
        assert!(!destination.exists());
        assert!(file.session.is_dirty());
        assert!(matches!(
            file.save_as(review).unwrap(),
            SaveAsOutcome::Durable { .. }
        ));
        assert_eq!(fs::read(&source).unwrap(), original);
        assert_eq!(fs::read(&destination).unwrap(), bytes);
        assert_eq!(file.require_existing_path().unwrap(), destination);
        assert!(!file.session.is_dirty());
        assert_eq!(file.session.chart_ids(), ids);
        assert_eq!(
            file.session.document().text(Some(0), "MUSIC").as_deref(),
            Some("../old/chart.ogg")
        );
        file.session.undo().unwrap();
        assert!(file.session.is_dirty());
        file.session.redo().unwrap();
        assert!(!file.session.is_dirty());
        file.session.set_text(None, "TITLE", "Next save").unwrap();
        file.save().unwrap();
        assert_eq!(fs::read(source).unwrap(), original);
        assert!(!file.session.is_dirty());
    }
    #[test]
    fn stale_review_and_changed_destination_never_overwrite_any_song() {
        let (_root, mut file, destination, original) = file();
        let review = file.prepare_save_as(&destination).unwrap();
        file.session.set_text(None, "TITLE", "Transient").unwrap();
        file.session.undo().unwrap();
        assert!(file.save_as(review).is_err());
        assert!(!destination.exists());
        assert_eq!(
            fs::read(file.require_existing_path().unwrap()).unwrap(),
            original
        );
        let review = file.prepare_save_as(&destination).unwrap();
        fs::write(&destination, b"appeared after review").unwrap();
        assert!(file.save_as(review).is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"appeared after review");
        let review = file.prepare_save_as(&destination).unwrap();
        assert!(review.replaces_existing());
        fs::write(&destination, b"changed since overwrite review").unwrap();
        assert!(file.save_as(review).is_err());
        assert_eq!(
            fs::read(destination).unwrap(),
            b"changed since overwrite review"
        );
    }
    #[test]
    fn external_source_change_can_be_rescued_without_writing_or_readopting_old_source() {
        let (_root, mut file, destination, _original) = file();
        let source = file.require_existing_path().unwrap().to_owned();
        file.session
            .set_text(None, "TITLE", "Rescue working copy")
            .unwrap();
        let review = file.prepare_save_as(&destination).unwrap();
        fs::write(&source, b"external source").unwrap();
        assert!(file.has_external_change().unwrap());
        file.save_as(review).unwrap();
        assert_eq!(fs::read(source).unwrap(), b"external source");
        assert!(!file.session.is_dirty());
        assert!(!file.has_external_change().unwrap());
    }
    #[test]
    fn clean_source_late_sync_failure_publishes_written_state_but_keeps_dirty_and_retries_without_rewrite()
     {
        let (_root, mut file, destination, original) = file();
        let source = file.require_existing_path().unwrap().to_owned();
        assert!(!file.session.is_dirty());
        let review = file.prepare_save_as(&destination).unwrap();
        let receipt = match file
            .save_as_with_sync(review, |_| {
                Err(std::io::Error::other("injected directory sync failure").into())
            })
            .unwrap()
        {
            SaveAsOutcome::WrittenPendingDurability { receipt, .. } => receipt,
            _ => panic!("Expected pending durability"),
        };
        assert_eq!(file.require_existing_path().unwrap(), destination);
        assert!(file.session.is_dirty());
        assert_eq!(fs::read(source).unwrap(), original);
        let before = fs::metadata(&destination).unwrap();
        let written = fs::read(&destination).unwrap();
        receipt.retry(&mut file).unwrap();
        assert!(!file.session.is_dirty());
        assert_eq!(fs::read(&destination).unwrap(), written);
        assert_eq!(fs::metadata(&destination).unwrap().ino(), before.ino());
    }
    #[test]
    fn pending_receipt_cannot_clean_new_edits_but_accepts_exact_undo_content_and_global_preferences()
     {
        let (_root, mut file, destination, _) = file();
        let review = file.prepare_save_as(&destination).unwrap();
        let receipt = match file
            .save_as_with_sync(review, |_| Err(std::io::Error::other("late sync").into()))
            .unwrap()
        {
            SaveAsOutcome::WrittenPendingDurability { receipt, .. } => receipt,
            _ => panic!(),
        };
        file.session
            .set_text(None, "TITLE", "Unsaved after output")
            .unwrap();
        assert!(receipt.retry(&mut file).is_err());
        assert!(file.session.is_dirty());
        file.session.undo().unwrap();
        let before = crate::audio_preferences::Audio::default();
        let mut after = before;
        after.set_music_rate(1.1).unwrap();
        file.session
            .set_audio_preferences(before, after, None)
            .unwrap();
        receipt.retry(&mut file).unwrap();
        assert!(!file.session.is_dirty());
        file.session.undo().unwrap();
        assert!(!file.session.is_dirty());
        // The preference action intentionally creates a new redo branch after
        // Undo. Current content still equals output; no song edit was cleaned.
        file.session.set_text(None, "TITLE", "Fresh edit").unwrap();
        assert!(file.session.is_dirty());
        assert!(receipt.retry(&mut file).is_err());
        assert!(file.session.is_dirty());
    }
    #[test]
    fn pending_receipt_rejects_destination_inode_or_parent_replacement_even_with_identical_bytes() {
        for parent_swap in [false, true] {
            let (root, mut file, destination, _) = file();
            let review = file.prepare_save_as(&destination).unwrap();
            let receipt = match file
                .save_as_with_sync(review, |_| Err(std::io::Error::other("late sync").into()))
                .unwrap()
            {
                SaveAsOutcome::WrittenPendingDurability { receipt, .. } => receipt,
                _ => panic!(),
            };
            let bytes = fs::read(&destination).unwrap();
            if parent_swap {
                fs::rename(
                    destination.parent().unwrap(),
                    root.path().join("moved-parent"),
                )
                .unwrap();
                fs::create_dir(destination.parent().unwrap()).unwrap();
            } else {
                fs::rename(&destination, root.path().join("replaced-destination.ssc")).unwrap();
            }
            fs::write(&destination, &bytes).unwrap();
            assert!(receipt.retry(&mut file).is_err());
            assert!(file.session.is_dirty());
        }
    }
    #[test]
    fn recovery_receipt_stays_owned_by_old_path_and_new_recovery_is_separate() {
        let (_root, mut file, destination, _) = file();
        file.session.set_text(None, "TITLE", "Draft").unwrap();
        file.save_recovery().unwrap();
        let old_path = file.recovery_path().unwrap();
        let old_bytes = fs::read(&old_path).unwrap();
        let old_receipt = Arc::clone(file.recovery_receipt.as_ref().unwrap());
        let review = file.prepare_save_as(&destination).unwrap();
        let previous = match file.save_as(review).unwrap() {
            SaveAsOutcome::Durable {
                previous_recovery, ..
            } => previous_recovery.unwrap(),
            _ => panic!(),
        };
        assert_eq!(previous.path, old_path);
        assert!(previous.has_owned_receipt());
        assert!(Arc::ptr_eq(&previous.receipt, &old_receipt));
        assert!(!Arc::ptr_eq(
            file.recovery_receipt.as_ref().unwrap(),
            &old_receipt
        ));
        assert_eq!(fs::read(&old_path).unwrap(), old_bytes);
        file.session.set_text(None, "TITLE", "New draft").unwrap();
        file.save_recovery().unwrap();
        assert!(file.recovery_path().unwrap().exists());
        assert_eq!(fs::read(old_path).unwrap(), old_bytes);
    }
    #[test]
    fn symlinks_same_file_and_changed_parent_are_rejected_before_publication() {
        let (root, mut file, destination, original) = file();
        assert!(
            file.prepare_save_as(file.require_existing_path().unwrap())
                .is_err()
        );
        std::os::unix::fs::symlink(file.require_existing_path().unwrap(), &destination).unwrap();
        assert!(file.prepare_save_as(&destination).is_err());
        fs::remove_file(&destination).unwrap();
        let review = file.prepare_save_as(&destination).unwrap();
        fs::rename(
            destination.parent().unwrap(),
            root.path().join("old-parent"),
        )
        .unwrap();
        fs::create_dir(destination.parent().unwrap()).unwrap();
        assert!(file.save_as(review).is_err());
        assert!(!destination.exists());
        assert_eq!(
            fs::read(file.require_existing_path().unwrap()).unwrap(),
            original
        );
    }
    #[test]
    fn same_format_sm_save_as_preserves_notes_unknowns_and_undo() {
        let root = tempfile::tempdir().unwrap();
        let old = root.path().join("old");
        let new = root.path().join("new");
        fs::create_dir(&old).unwrap();
        fs::create_dir(&new).unwrap();
        let source = old.join("song.sm");
        let original=b"#TITLE:Original;#MUSIC:music.ogg;#VENDOR:keep;#BPMS:0=120;#NOTES:dance-single::Hard:9:0,0,0,0,0:0000;";
        fs::write(&source, original).unwrap();
        let mut file = FileSession::open(&source).unwrap();
        file.session.set_text(None, "TITLE", "Edited").unwrap();
        let grid = file.session.grid(0).unwrap().clone();
        let review = file.prepare_save_as(&new.join("copy.SM")).unwrap();
        file.save_as(review).unwrap();
        assert_eq!(file.session.grid(0).unwrap(), &grid);
        assert_eq!(
            file.session.document().text(None, "MUSIC").as_deref(),
            Some("../old/music.ogg")
        );
        assert_eq!(
            file.session.document().text(None, "VENDOR").as_deref(),
            Some("keep")
        );
        file.session.undo().unwrap();
        assert_eq!(
            file.session.document().text(None, "TITLE").as_deref(),
            Some("Original")
        );
        assert_eq!(fs::read(source).unwrap(), original);
    }
    #[test]
    fn approved_existing_destination_is_replaced_only_against_its_exact_reviewed_baseline() {
        let (_root, mut file, destination, original) = file();
        fs::write(&destination, b"reviewed old destination").unwrap();
        let review = file.prepare_save_as(&destination).unwrap();
        assert!(review.replaces_existing());
        let staged = review.staged_bytes().to_vec();
        assert_eq!(review.candidate.serialize().unwrap(), staged);
        let output = review.candidate.serialize_for_output().unwrap();
        assert_ne!(output, staged, "explicit output materializes captured sync");
        file.save_as(review).unwrap();
        assert_eq!(fs::read(destination).unwrap(), output);
        assert_eq!(
            fs::read(_root.path().join("old/song.ssc")).unwrap(),
            original
        );
    }
    #[test]
    fn failed_atomic_publication_leaves_source_live_history_and_destination_baseline_untouched() {
        for existing in [false, true] {
            let (_root, mut file, destination, original) = file();
            let source = file.require_existing_path().unwrap().to_owned();
            if existing {
                fs::write(&destination, b"reviewed target").unwrap();
            }
            file.session
                .set_text(None, "TITLE", "Working edit")
                .unwrap();
            let live = file.session.serialize().unwrap();
            let revision = file.session.revision();
            let review = file.prepare_save_as(&destination).unwrap();
            let result = file.save_as_with_io(
                review,
                |_| panic!("Directory sync must not run before publication succeeds"),
                |_, _, _| {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        "injected rename failure",
                    )
                    .into())
                },
            );
            assert!(result.is_err());
            assert_eq!(file.require_existing_path().unwrap(), source);
            assert_eq!(file.session.revision(), revision);
            assert_eq!(file.session.serialize().unwrap(), live);
            assert!(file.session.is_dirty());
            assert_eq!(fs::read(&source).unwrap(), original);
            if existing {
                assert_eq!(fs::read(&destination).unwrap(), b"reviewed target");
            } else {
                assert!(!destination.exists());
            }
        }
    }
}

// Ordinary Save uses the same Windows namespace barrier as Save As. Unix keeps
// publication and parent-directory sync separate to retain a pending receipt.
pub(super) fn persist_output(
    pending: tempfile::NamedTempFile,
    destination: &Path,
) -> Result<(), Error> {
    #[cfg(windows)]
    {
        windows_publish::publish(pending, destination, false)?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        pending
            .persist(destination)
            .map_err(|error| Error::Io(error.error))?;
        Ok(())
    }
}
pub(super) fn sync_output_directory(parent: &Path) -> Result<(), Error> {
    #[cfg(windows)]
    {
        let _ = parent;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        sync_save_as_directory(parent)
    }
}

#[cfg(all(test, unix))]
mod authoring_boundary_tests {
    use super::*;
    use crate::document::Format;
    #[test]
    fn promotion_policy_history_rebases_once_at_publication_and_not_again_at_durability_retry() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().canonicalize().unwrap().join("source.sm");
        let destination = root.path().canonicalize().unwrap().join("copy.ssc");
        fs::write(
            &source,
            b"#TITLE:Test;#BPMS:0=120;#NOTES:dance-single::Hard:9:0,0,0,0,0:1000;",
        )
        .unwrap();
        let mut file = FileSession::open(&source).unwrap();
        let plan = file.prepare_sm_authoring().unwrap();
        file.apply_sm_authoring(plan, Default::default()).unwrap();
        let policy = crate::output_policy::OutputPolicy::AdvancedSongTiming {
            format: Format::Ssc,
            options: Default::default(),
        };
        let review = file
            .prepare_save_as_with_policy(&destination, policy.clone())
            .unwrap();
        assert!(review.issues().is_empty());
        let receipt = match file
            .save_as_with_sync(review, |_| {
                Err(Error::Io(std::io::Error::other("injected sync failure")))
            })
            .unwrap()
        {
            SaveAsOutcome::WrittenPendingDurability { receipt, .. } => receipt,
            _ => panic!("pending output required"),
        };
        let revision = file.output_policy_revision();
        let bytes = fs::read(&destination).unwrap();
        assert_eq!(file.output_policy(), &policy);
        receipt.retry(&mut file).unwrap();
        assert_eq!(file.output_policy_revision(), revision);
        assert_eq!(fs::read(&destination).unwrap(), bytes);
        file.authoring_history_with_effect(false).unwrap();
        assert_eq!(file.session.document().format, Format::Sm);
        assert_eq!(file.output_policy(), &policy);
        assert_eq!(file.require_existing_path().unwrap(), destination);
        file.authoring_history_with_effect(true).unwrap();
        assert_eq!(file.session.document().format, Format::Ssc);
        assert_eq!(file.output_policy(), &policy);
    }
}
