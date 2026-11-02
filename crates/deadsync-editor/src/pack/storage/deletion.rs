//! Explicit terminal Pack.ini deletion. A review never adopts an external entry.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeletionDurability {
    Directory,
    NamespaceOnly,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeleteOutcome {
    DiscardedStaged,
    Deleted(DeletionDurability),
}
/// Exact live draft and logical entry reviewed by the user. Not a save receipt.
pub struct DeleteSnapshot {
    owner: Arc<()>,
    generation: u64,
    mutation: u64,
    prepared: Prepared,
    logical: Option<Identity>,
}
pub(super) struct PendingDeletion {
    parent: Directory,
    logical: PathBuf,
}
fn deletion_guarantee() -> DeletionDurability {
    #[cfg(unix)]
    {
        DeletionDurability::Directory
    }
    #[cfg(not(unix))]
    {
        DeletionDurability::NamespaceOnly
    }
}
impl FileSession {
    pub fn is_deleted(&self) -> bool {
        self.deleted
    }
    pub fn deletion_pending(&self) -> bool {
        self.pending_deletion.is_some()
    }
    pub fn prepare_delete(&self) -> Result<DeleteSnapshot, Error> {
        if self.durability_pending() {
            return Err(Error::PendingSave);
        }
        if self.deleted {
            return Err(Error::StaleSnapshot);
        }
        self.check_expected()?;
        let logical = if self.expected.is_some() {
            Some(Identity::of(&fs::symlink_metadata(&self.binding.logical)?))
        } else {
            None
        };
        Ok(DeleteSnapshot {
            owner: self.owner.clone(),
            generation: self.generation,
            mutation: self.mutation_revision,
            prepared: self.draft.prepare()?,
            logical,
        })
    }
    pub fn delete(&mut self, review: DeleteSnapshot) -> Result<DeleteOutcome, Error> {
        self.delete_with(
            review,
            |_| Ok(()),
            |path| fs::remove_file(path),
            sync_directory,
        )
    }
    fn delete_with(
        &mut self,
        review: DeleteSnapshot,
        before: impl FnOnce(&Path) -> io::Result<()>,
        unlink: impl FnOnce(&Path) -> io::Result<()>,
        sync: impl FnOnce(&Path) -> io::Result<()>,
    ) -> Result<DeleteOutcome, Error> {
        if self.durability_pending() {
            return Err(Error::PendingSave);
        }
        if self.deleted
            || !Arc::ptr_eq(&self.owner, &review.owner)
            || review.generation != self.generation
            || review.mutation != self.mutation_revision
            || review.prepared.baseline() != self.expected.as_deref()
        {
            return Err(Error::StaleSnapshot);
        }
        let current = self.draft.prepare()?;
        if current.bytes() != review.prepared.bytes() {
            return Err(Error::StaleSnapshot);
        }
        let next = self
            .generation
            .checked_add(1)
            .ok_or_else(|| io::Error::other("Pack deletion generation overflow"))?;
        if review.logical.is_none() {
            self.check_expected()?;
            self.deleted = true;
            self.generation = next;
            return Ok(DeleteOutcome::DiscardedStaged);
        }
        // Allocate acknowledgement ownership before namespace mutation. Remove the
        // logical Pack.ini entry, including a symlink; never unlink its target.
        let receipt = PendingDeletion {
            parent: self.binding.pack_parent.clone(),
            logical: self.binding.logical.clone(),
        };
        let _lock = self.lock()?;
        before(&self.binding.logical)?;
        self.check_expected()?;
        let metadata = fs::symlink_metadata(&self.binding.logical).map_err(conflict_if_missing)?;
        if Some(Identity::of(&metadata)) != review.logical {
            return Err(Error::ExternalChange);
        }
        unlink(&self.binding.logical)?;
        self.deleted = true;
        self.generation = next;
        self.pending_deletion = Some(receipt);
        sync(&self.binding.pack_parent.path).map_err(Error::DeletionDurabilityPending)?;
        self.finish_deletion()
    }
    pub(super) fn check_deleted_entry(&self) -> Result<(), Error> {
        self.binding.pack_parent.check()?;
        match fs::symlink_metadata(&self.binding.logical) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Ok(_) => Err(Error::ExternalChange),
            Err(error) => Err(error.into()),
        }
    }
    fn finish_deletion(&mut self) -> Result<DeleteOutcome, Error> {
        self.check_deleted_entry()?;
        self.pending_deletion = None;
        Ok(DeleteOutcome::Deleted(deletion_guarantee()))
    }
    pub fn retry_delete(&mut self) -> Result<DeleteOutcome, Error> {
        self.retry_delete_with(sync_directory)
    }
    fn retry_delete_with(
        &mut self,
        sync: impl FnOnce(&Path) -> io::Result<()>,
    ) -> Result<DeleteOutcome, Error> {
        let pending = self.pending_deletion.as_ref().ok_or(Error::PendingSave)?;
        pending.parent.check()?;
        if pending.logical != self.binding.logical {
            return Err(Error::StaleSnapshot);
        }
        self.check_deleted_entry()?;
        sync(&pending.parent.path).map_err(Error::DeletionDurabilityPending)?;
        self.finish_deletion()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, FileSession) {
        let root = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join("Pack.ini"),
            Data::new("Original", "", 2026, None, 0.009)
                .canonical()
                .unwrap(),
        )
        .unwrap();
        let file = FileSession::open(root.path()).unwrap().unwrap();
        (root, file)
    }
    #[test]
    fn reviewed_deletion_is_terminal_and_preserves_unrelated_pack_assets() {
        let (root, mut file) = fixture();
        fs::write(root.path().join("banner.png"), b"keep").unwrap();
        file.set_year(2030).unwrap();
        let review = file.prepare_delete().unwrap();
        assert!(root.path().join("Pack.ini").exists());
        assert!(matches!(
            file.delete(review).unwrap(),
            DeleteOutcome::Deleted(_)
        ));
        assert!(!root.path().join("Pack.ini").exists());
        assert_eq!(fs::read(root.path().join("banner.png")).unwrap(), b"keep");
        assert!(file.is_deleted());
        assert!(!file.deletion_pending());
        assert!(file.save().is_err());
        assert!(file.set_year(2040).is_err());
    }
    #[test]
    fn edit_then_restore_foreign_and_external_replacements_revoke_review() {
        let (root, mut file) = fixture();
        let review = file.prepare_delete().unwrap();
        file.set_year(2030).unwrap();
        file.set_year(2026).unwrap();
        assert!(matches!(file.delete(review), Err(Error::StaleSnapshot)));
        let review = file.prepare_delete().unwrap();
        let (_other, mut other) = fixture();
        assert!(matches!(other.delete(review), Err(Error::StaleSnapshot)));
        let review = file.prepare_delete().unwrap();
        let bytes = fs::read(root.path().join("Pack.ini")).unwrap();
        let replacement = root.path().join("replace.ini");
        fs::write(&replacement, &bytes).unwrap();
        fs::rename(replacement, root.path().join("Pack.ini")).unwrap();
        assert!(matches!(file.delete(review), Err(Error::ExternalChange)));
        assert_eq!(fs::read(root.path().join("Pack.ini")).unwrap(), bytes);
    }
    #[test]
    fn unlink_failure_keeps_the_draft_and_never_claims_deletion() {
        let (root, mut file) = fixture();
        file.set_year(2030).unwrap();
        let review = file.prepare_delete().unwrap();
        let result = file.delete_with(
            review,
            |_| Ok(()),
            |_| {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "injected unlink failure",
                ))
            },
            |_| Ok(()),
        );
        assert!(matches!(result, Err(Error::Io(_))));
        assert!(root.path().join("Pack.ini").exists());
        assert_eq!(file.draft().data().year, 2030);
        assert!(file.draft().has_unsaved_changes());
        assert!(!file.is_deleted());
        assert!(!file.deletion_pending());
    }
    #[test]
    fn namespace_deletion_pending_sync_retries_only_ack_and_refuses_recreated_entry() {
        let (root, mut file) = fixture();
        let review = file.prepare_delete().unwrap();
        assert!(matches!(
            file.delete_with(
                review,
                |_| Ok(()),
                |path| fs::remove_file(path),
                |_| Err(io::Error::other("sync fault"))
            ),
            Err(Error::DeletionDurabilityPending(_))
        ));
        assert!(!root.path().join("Pack.ini").exists());
        assert!(file.deletion_pending());
        assert!(matches!(file.prepare_delete(), Err(Error::PendingSave)));
        assert!(matches!(file.prepare_save(), Err(Error::PendingSave)));
        fs::write(root.path().join("Pack.ini"), b"external").unwrap();
        assert!(matches!(
            file.retry_delete_with(|_| Ok(())),
            Err(Error::ExternalChange)
        ));
        assert_eq!(fs::read(root.path().join("Pack.ini")).unwrap(), b"external");
        assert!(file.deletion_pending());
        fs::remove_file(root.path().join("Pack.ini")).unwrap();
        assert!(matches!(
            file.retry_delete_with(|_| Ok(())).unwrap(),
            DeleteOutcome::Deleted(_)
        ));
        assert!(!file.deletion_pending());
    }
    #[test]
    fn staged_creation_discard_has_no_filesystem_publication() {
        let root = tempfile::tempdir().unwrap();
        let mut file =
            FileSession::create(root.path(), Data::new("Staged", "", 2026, None, 0.009)).unwrap();
        let review = file.prepare_delete().unwrap();
        assert_eq!(file.delete(review).unwrap(), DeleteOutcome::DiscardedStaged);
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }
    #[cfg(unix)]
    #[test]
    fn deleting_symlink_removes_only_logical_entry_and_replacement_link_revokes_review() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let shared = tempfile::tempdir().unwrap();
        let target = shared.path().join("shared.ini");
        fs::write(
            &target,
            Data::new("Shared", "", 2026, None, 0.009)
                .canonical()
                .unwrap(),
        )
        .unwrap();
        let original = fs::read(&target).unwrap();
        symlink(&target, root.path().join("Pack.ini")).unwrap();
        let mut file = FileSession::open(root.path()).unwrap().unwrap();
        let review = file.prepare_delete().unwrap();
        symlink(&target, root.path().join("replacement-link")).unwrap();
        fs::rename(
            root.path().join("replacement-link"),
            root.path().join("Pack.ini"),
        )
        .unwrap();
        assert!(matches!(file.delete(review), Err(Error::ExternalChange)));
        let mut file = FileSession::open(root.path()).unwrap().unwrap();
        let review = file.prepare_delete().unwrap();
        file.delete(review).unwrap();
        assert!(fs::symlink_metadata(root.path().join("Pack.ini")).is_err());
        assert_eq!(fs::read(target).unwrap(), original);
    }
    #[test]
    fn deleted_and_pending_observer_captures_are_current_but_never_adoptable() {
        let (_root, mut file) = fixture();
        let before = file.external_snapshot();
        let review = file.prepare_delete().unwrap();
        assert!(matches!(
            file.delete_with(
                review,
                |_| Ok(()),
                |path| fs::remove_file(path),
                |_| Err(io::Error::other("pending sync"))
            ),
            Err(Error::DeletionDurabilityPending(_))
        ));
        assert!(!before.is_current(&file));
        let pending = file.external_snapshot();
        assert!(pending.is_current(&file));
        assert!(pending.rebuild_observer_baseline().unwrap().is_none());
        file.retry_delete_with(|_| Ok(())).unwrap();
        let deleted = file.external_snapshot();
        assert!(deleted.is_current(&file));
        assert!(deleted.rebuild_observer_baseline().unwrap().is_none());
    }
}
