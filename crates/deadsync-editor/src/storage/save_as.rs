//! Same-format Save As adapter. Review and output are staged before live mutation.
use super::{FileSession, sync_directory};
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
struct ParentIdentity {
    device: u64,
    inode: u64,
}
fn parent_identity(path: &Path) -> Result<ParentIdentity, Error> {
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
            inode: metadata.ino(),
        })
    }
    #[cfg(not(unix))]
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
fn file_identity(file: &fs::File) -> Result<ParentIdentity, Error> {
    #[cfg(unix)]
    {
        let metadata = file.metadata()?;
        Ok(ParentIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
    #[cfg(not(unix))]
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
    source: PathBuf,
    source_expected: Arc<[u8]>,
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
    destination: PathBuf,
    parent: PathBuf,
    parent_id: ParentIdentity,
    expected: Arc<[u8]>,
    identity: ParentIdentity,
    content: ContentToken,
}
pub enum SaveAsOutcome {
    Durable {
        previous_recovery: PreviousRecovery,
        relocation: RelocationReceipt,
    },
    WrittenPendingDurability {
        receipt: PendingDurability,
        previous_recovery: PreviousRecovery,
        relocation: RelocationReceipt,
        error: Error,
    },
}
impl SaveAsReview {
    pub fn destination(&self) -> &Path {
        &self.destination
    }
    pub fn relocation(&self) -> &RelocationReceipt { &self.relocation }
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
        self.prepare_save_as_with_revisions(destination,&[])
    }
    pub fn prepare_save_as_with_revisions(&self,destination:&Path,extra:&[u64])->Result<SaveAsReview,Error> {
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
        if destination == self.path {
            return Err(Error::InvalidEdit("Use Save for the current file".into()));
        }
        let parent_id = parent_identity(&parent)?;
        let expected = baseline(&destination)?;
        let mut plan = Plan::prepare(&self.session, &self.path, &destination)?;
        plan.include_external_revisions(extra)?;
        let relocation = plan.rebind_receipt();
        Ok(SaveAsReview {
            source: self.path.clone(),
            source_expected: Arc::clone(&self.expected),
            destination,
            parent,
            parent_id,
            expected,
            plan,
            relocation,
        })
    }
    pub fn save_as(&mut self, review: SaveAsReview) -> Result<SaveAsOutcome, Error> {
        self.save_as_with_sync(review, sync_directory)
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
        review: SaveAsReview,
        sync: impl FnOnce(&Path) -> Result<(), Error>,
        persist: impl FnOnce(tempfile::NamedTempFile, &Path, bool) -> Result<(), Error>,
    ) -> Result<SaveAsOutcome, Error> {
        if self.path != review.source || self.expected != review.source_expected {
            return Err(Error::InvalidEdit(
                "Save As source changed since review".into(),
            ));
        }
        review.plan.validate(&self.session)?;
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
        let expected: Arc<[u8]> = review.plan.bytes().to_vec().into();
        let previous_recovery = PreviousRecovery {
            path: self.recovery_path(),
            receipt: Arc::clone(&self.recovery_receipt),
        };
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
            destination: review.destination.clone(),
            parent: review.parent.clone(),
            parent_id: review.parent_id,
            expected: Arc::clone(&expected),
            identity: file_identity(pending.as_file())?,
            content: review.plan.relocated_content_token(),
        };
        review.validate_target()?;
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
        self.path = destination;
        self.expected = expected;
        self.recovery_receipt = new_recovery;
        match sync(&parent) {
            Ok(()) => {
                self.session.mark_saved();
                Ok(SaveAsOutcome::Durable { previous_recovery, relocation })
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
    pub fn destination(&self) -> &Path {
        &self.destination
    }
    pub fn retry(&self, file: &mut FileSession) -> Result<(), Error> {
        if file.path != self.destination
            || file.expected != self.expected
            || parent_identity(&self.parent)? != self.parent_id
            || baseline(&self.destination)?
                .as_ref()
                .is_none_or(|baseline| {
                    baseline.bytes != self.expected || baseline.identity != self.identity
                })
        {
            return Err(Error::ExternalChange);
        }
        sync_directory(&self.parent)?;
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
    fn durable_save_as_preserves_source_history_chart_music_and_supports_next_save() {
        let (_root, mut file, destination, original) = file();
        let source = file.path.clone();
        file.session
            .set_text(None, "TITLE", "Working edit")
            .unwrap();
        let ids = file.session.chart_ids().to_vec();
        let review = file.prepare_save_as(&destination).unwrap();
        let bytes = review.staged_bytes().to_vec();
        assert!(!destination.exists());
        assert!(file.session.is_dirty());
        assert!(matches!(
            file.save_as(review).unwrap(),
            SaveAsOutcome::Durable { .. }
        ));
        assert_eq!(fs::read(&source).unwrap(), original);
        assert_eq!(fs::read(&destination).unwrap(), bytes);
        assert_eq!(file.path(), destination);
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
        assert_eq!(fs::read(file.path()).unwrap(), original);
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
        let source = file.path.clone();
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
        let source = file.path.clone();
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
        assert_eq!(file.path(), destination);
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
        let old_path = file.recovery_path();
        let old_bytes = fs::read(&old_path).unwrap();
        let old_receipt = Arc::clone(&file.recovery_receipt);
        let review = file.prepare_save_as(&destination).unwrap();
        let previous = match file.save_as(review).unwrap() {
            SaveAsOutcome::Durable { previous_recovery, .. } => previous_recovery,
            _ => panic!(),
        };
        assert_eq!(previous.path, old_path);
        assert!(previous.has_owned_receipt());
        assert!(Arc::ptr_eq(&previous.receipt, &old_receipt));
        assert!(!Arc::ptr_eq(&file.recovery_receipt, &old_receipt));
        assert_eq!(fs::read(&old_path).unwrap(), old_bytes);
        file.session.set_text(None, "TITLE", "New draft").unwrap();
        file.save_recovery().unwrap();
        assert!(file.recovery_path().exists());
        assert_eq!(fs::read(old_path).unwrap(), old_bytes);
    }
    #[test]
    fn symlinks_same_file_and_changed_parent_are_rejected_before_publication() {
        let (root, mut file, destination, original) = file();
        assert!(file.prepare_save_as(file.path()).is_err());
        std::os::unix::fs::symlink(file.path(), &destination).unwrap();
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
        assert_eq!(fs::read(file.path()).unwrap(), original);
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
        file.save_as(review).unwrap();
        assert_eq!(fs::read(destination).unwrap(), staged);
        assert_eq!(
            fs::read(_root.path().join("old/song.ssc")).unwrap(),
            original
        );
    }
    #[test]
    fn failed_atomic_publication_leaves_source_live_history_and_destination_baseline_untouched() {
        for existing in [false, true] {
            let (_root, mut file, destination, original) = file();
            let source = file.path.clone();
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
            assert_eq!(file.path(), source);
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
