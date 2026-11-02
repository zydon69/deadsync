//! One global history action owns an external profile's exact byte snapshots.
use super::*;
#[derive(Debug)]
pub enum WriteOutcome {
    Committed { durability_warning: Option<Error> },
}
#[derive(Debug)]
pub struct Update {
    pub path: PathBuf,
    pub durability_warning: Option<String>,
}
#[derive(Debug)]
pub(crate) struct DurabilityReceipt {
    path: PathBuf,
    expected: Arc<[u8]>,
    warning: String,
}
impl DurabilityReceipt {
    pub(super) fn capture(file: &ProfileFile, error: &Error) -> Self {
        Self {
            path: file.path.clone(),
            expected: file.expected.clone(),
            warning: error.to_string(),
        }
    }
    pub(super) fn warning(&self) -> &str {
        &self.warning
    }

    pub(crate) fn update(&self) -> Update {
        Update {
            path: self.path.clone(),
            durability_warning: Some(self.warning.clone()),
        }
    }
    pub(crate) fn retry(&self) -> Result<(), Error> {
        let _lock = profile_lock(&self.path)?;
        if fs::read(&self.path)?.as_slice() != self.expected.as_ref() {
            return Err(Error::ExternalChange);
        }
        sync_directory(
            self.path
                .parent()
                .ok_or_else(|| invalid_error("Profile has no parent directory"))?,
        )
    }
}
#[derive(Debug)]
pub(crate) struct Edit {
    file: ProfileFile,
    before: (Arc<[u8]>, Profile),
    after: (Arc<[u8]>, Profile),
}
impl Edit {
    pub(crate) fn capture(file: &ProfileFile, profile: &Profile) -> Result<Option<Self>, Error> {
        if profile.guid() != file.profile().guid()
            || profile.config().kind() != file.profile().config().kind()
        {
            return invalid("Profile identity cannot change during save");
        }
        if builtin(profile.config().kind(), profile.guid()) {
            return invalid("Built-in editor profiles cannot be overwritten");
        }
        let (path, before) = file.snapshot();
        let captured =
            ProfileFile::capture_snapshot(&path, before.clone(), file.profile().clone())?;
        let after: Arc<[u8]> = profile.serialize()?.into();
        if after == before {
            return Ok(None);
        }
        Ok(Some(Self {
            file: captured,
            before: (before, file.profile().clone()),
            after: (after, profile.clone()),
        }))
    }
    pub(crate) fn apply(
        &mut self,
        forward: bool,
    ) -> Result<(Update, Option<DurabilityReceipt>), Error> {
        let (bytes, profile) = if forward { &self.after } else { &self.before };
        let WriteOutcome::Committed { durability_warning } = self
            .file
            .replace_bytes_committed(bytes.clone(), profile.clone())?;
        let pending = durability_warning.as_ref().map(|error| DurabilityReceipt {
            path: self.file.path.clone(),
            expected: self.file.expected.clone(),
            warning: error.to_string(),
        });
        Ok((
            Update {
                path: self.file.path.clone(),
                durability_warning: durability_warning.map(|error| error.to_string()),
            },
            pending,
        ))
    }
}

// A real rename is performed in tests; only the subsequent durability barrier
// can fail deterministically. Thread-local injection cannot affect other tests.
#[cfg(test)]
thread_local! {static FAIL_NEXT_SYNC:std::cell::Cell<bool>=const{std::cell::Cell::new(false)};}
pub(super) fn sync_after_publish(parent: &Path) -> Result<(), Error> {
    #[cfg(test)]
    if FAIL_NEXT_SYNC.with(|flag| flag.replace(false)) {
        return Err(Error::Io(std::io::Error::other(
            "injected directory sync failure",
        )));
    }
    sync_directory(parent)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::{Document, Format},
        history::Session,
        notes::{Note, Position},
    };
    fn fixture() -> (tempfile::TempDir, ProfileFile, Vec<u8>, Session) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ecc-global.json");
        let bytes=br#"{ "Guid":"12345678-9abc-4def-8123-456789abcdef", "Name":"Original", "Future":1e999, "Config":{} }
"#.to_vec();
        fs::write(&path, &bytes).unwrap();
        let file = ProfileFile::open(&path).unwrap();
        let session = Session::new(
            Document::parse(
                b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;"
                    .as_slice(),
                Format::Ssc,
            )
            .unwrap(),
        );
        (dir, file, bytes, session)
    }
    #[test]
    fn global_order_note_then_profile_undo_redo_preserves_exact_song_and_profile_bytes() {
        let (_dir, file, original, mut s) = fixture();
        let song = s.serialize().unwrap();
        s.edit_notes(
            0,
            "note",
            [(
                Position {
                    row: 0,
                    lane: 0,
                    player: 0,
                },
                Some(Note::tap()),
            )],
        )
        .unwrap();
        let notes = s.serialize().unwrap();
        let content = s.content_revision();
        let mut edited = file.profile().clone();
        edited.rename("Changed").unwrap();
        assert!(s.edit_profile(&file, &edited).unwrap());
        let after = fs::read(file.path()).unwrap();
        assert_ne!(after, original);
        assert_eq!(s.serialize().unwrap(), notes);
        assert_eq!(s.content_revision(), content);
        assert!(s.is_dirty());
        assert_eq!(s.undo_label(), Some("Edit generation profile"));
        assert!(s.undo().unwrap());
        assert_eq!(fs::read(file.path()).unwrap(), original);
        assert_eq!(s.serialize().unwrap(), notes);
        assert!(s.undo().unwrap());
        assert_eq!(s.serialize().unwrap(), song);
        assert!(!s.is_dirty());
        assert!(s.redo().unwrap());
        assert_eq!(s.serialize().unwrap(), notes);
        assert_eq!(fs::read(file.path()).unwrap(), original);
        assert!(s.redo().unwrap());
        assert_eq!(fs::read(file.path()).unwrap(), after);
        assert_eq!(s.serialize().unwrap(), notes);
        let refreshed = ProfileFile::open(file.path()).unwrap();
        let same = refreshed.profile().clone();
        let revision = s.revision();
        assert!(!s.edit_profile(&refreshed, &same).unwrap());
        assert_eq!(s.revision(), revision);
    }
    #[test]
    fn precommit_and_undo_redo_conflicts_leave_global_stacks_and_external_bytes_unchanged() {
        let (_dir, file, original, mut s) = fixture();
        s.set_text(None, "TITLE", "temporary").unwrap();
        s.undo().unwrap();
        let redo = s.redo_label().map(str::to_owned);
        let mut edited = file.profile().clone();
        edited.rename("Changed").unwrap();
        let revision = s.revision();
        fs::write(file.path(), b"external").unwrap();
        assert!(matches!(
            s.edit_profile(&file, &edited),
            Err(Error::ExternalChange)
        ));
        assert_eq!(s.revision(), revision);
        assert!(s.undo_label().is_none());
        assert_eq!(s.redo_label().map(str::to_owned), redo);
        assert_eq!(fs::read(file.path()).unwrap(), b"external");
        fs::remove_file(file.path()).unwrap();
        assert!(matches!(s.edit_profile(&file, &edited), Err(Error::Io(_))));
        assert_eq!(s.revision(), revision);
        assert_eq!(s.redo_label().map(str::to_owned), redo);
        fs::write(file.path(), &original).unwrap();
        assert!(s.edit_profile(&file, &edited).unwrap());
        assert!(!s.is_dirty());
        let saved = fs::read(file.path()).unwrap();
        let revision = s.revision();
        fs::write(file.path(), b"external").unwrap();
        assert!(s.undo().is_err());
        assert_eq!(s.revision(), revision);
        assert_eq!(s.undo_label(), Some("Edit generation profile"));
        assert!(s.redo_label().is_none());
        assert_eq!(fs::read(file.path()).unwrap(), b"external");
        fs::write(file.path(), &saved).unwrap();
        assert!(s.undo().unwrap());
        fs::write(file.path(), b"external redo").unwrap();
        let revision = s.revision();
        assert!(s.redo().is_err());
        assert_eq!(s.revision(), revision);
        assert!(s.undo_label().is_none());
        assert_eq!(s.redo_label(), Some("Edit generation profile"));
        assert_eq!(fs::read(file.path()).unwrap(), b"external redo");
        fs::write(file.path(), &original).unwrap();
        assert!(s.redo().unwrap());
        assert_eq!(fs::read(file.path()).unwrap(), saved);
        assert!(!s.is_dirty());
    }
    #[test]
    fn postrename_sync_failure_is_committed_with_owned_retry_receipt_and_undo() {
        let (_dir, file, original, mut s) = fixture();
        let mut edited = file.profile().clone();
        edited.rename("Committed").unwrap();
        FAIL_NEXT_SYNC.with(|flag| flag.set(true));
        assert!(s.edit_profile(&file, &edited).unwrap());
        let saved = fs::read(file.path()).unwrap();
        assert_ne!(saved, original);
        let update = s.take_profile_edit_update().unwrap();
        assert!(update.durability_warning.is_some());
        assert_eq!(s.undo_label(), Some("Edit generation profile"));
        assert!(!s.is_dirty());
        assert!(s.undo().unwrap());
        assert_eq!(fs::read(file.path()).unwrap(), original);
        FAIL_NEXT_SYNC.with(|flag| flag.set(true));
        assert!(s.redo().unwrap());
        assert!(
            s.take_profile_edit_update()
                .unwrap()
                .durability_warning
                .is_some()
        );
        assert_eq!(fs::read(file.path()).unwrap(), saved);
        // Receipt survives history eviction and refuses a later external writer.
        s.set_history_capacity(1).unwrap();
        s.set_text(None, "TITLE", "evict profile entry").unwrap();
        fs::write(file.path(), b"external after commit").unwrap();
        assert!(s.retry_profile_durability().is_err());
        assert_eq!(fs::read(file.path()).unwrap(), b"external after commit");
        fs::write(file.path(), &saved).unwrap();
        s.retry_profile_durability().unwrap();
        s.retry_profile_durability().unwrap();
        let current = ProfileFile::open(file.path()).unwrap();
        let mut next = current.profile().clone();
        next.rename("Next").unwrap();
        s.set_history_capacity(1024).unwrap();
        assert!(s.edit_profile(&current, &next).unwrap());
        assert!(s.undo().unwrap());
        assert_eq!(fs::read(file.path()).unwrap(), saved);
    }
}

#[cfg(test)]
mod public_profile_receipt_tests {
    use super::*;
    #[test]
    fn public_save_undo_redo_commit_history_and_retain_sync_failure_receipt() {
        for operation in ["save", "undo", "redo"] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("ecc-public.json");
            let original=br#"{ "Guid":"12345678-9abc-4def-8123-456789abcdef", "Name":"Original", "Future":1e999, "Config":{} }
"#;
            fs::write(&path, original).unwrap();
            let mut file = ProfileFile::open(&path).unwrap();
            let mut edited = file.profile().clone();
            edited.rename("Changed").unwrap();
            if operation != "save" {
                file.save(&edited).unwrap();
            }
            if operation == "redo" {
                assert!(file.undo().unwrap());
            }
            FAIL_NEXT_SYNC.with(|flag| flag.set(true));
            match operation {
                "save" => file.save(&edited).unwrap(),
                "undo" => assert!(file.undo().unwrap()),
                "redo" => assert!(file.redo().unwrap()),
                _ => unreachable!(),
            };
            let committed = fs::read(file.path()).unwrap();
            if operation == "undo" {
                assert_eq!(committed, original);
                assert_eq!(file.profile().name(), Some("Original"));
                assert!(!file.can_undo());
                assert!(file.can_redo());
            } else {
                assert_ne!(committed, original);
                assert_eq!(file.profile().name(), Some("Changed"));
                assert!(file.can_undo());
                assert!(!file.can_redo());
            }
            assert!(
                file.has_pending_durability(),
                "{operation}: actual publication must own retry receipt"
            );
            assert!(file.durability_warning().is_some());
            let history = (file.can_undo(), file.can_redo());
            // Retrying must not overwrite a later external writer or consume the
            // history cursor. Restore the committed bytes to permit a safe retry.
            fs::write(file.path(), b"external after publication").unwrap();
            assert!(matches!(
                file.retry_durability(),
                Err(Error::ExternalChange)
            ));
            assert_eq!(
                fs::read(file.path()).unwrap(),
                b"external after publication"
            );
            assert!(file.has_pending_durability());
            assert_eq!((file.can_undo(), file.can_redo()), history);
            fs::write(file.path(), &committed).unwrap();
            file.retry_durability().unwrap();
            assert_eq!(fs::read(file.path()).unwrap(), committed);
            assert!(!file.has_pending_durability());
            assert!(file.durability_warning().is_none());
            // The inverse still exists after the warning and durability retry.
            if operation == "undo" {
                assert!(file.redo().unwrap());
                assert_eq!(file.profile().name(), Some("Changed"));
                assert!(file.undo().unwrap());
                assert_eq!(fs::read(file.path()).unwrap(), original);
            } else {
                assert!(file.undo().unwrap());
                assert_eq!(fs::read(file.path()).unwrap(), original);
                assert!(file.redo().unwrap());
                assert_eq!(fs::read(file.path()).unwrap(), committed);
            }
        }
    }
}

#[cfg(test)]
mod export_receipt_tests {
    use super::*;
    use crate::{
        document::{Document, Format},
        history::Session,
    };
    fn profile() -> Profile {
        Profile::parse(Kind::Expressed,br#"{"Guid":"12345678-9abc-4def-8123-456789abcdef","Name":"Export","Future":1e999,"Config":{}}"#).unwrap()
    }
    fn session() -> Session {
        Session::new(
            Document::parse(
                b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;"
                    .as_slice(),
                Format::Ssc,
            )
            .unwrap(),
        )
    }
    #[test]
    fn export_no_clobber_refuses_before_publication_and_sync_failure_returns_owned_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ecc-export.json");
        let source = profile();
        fs::write(&path, b"existing owner").unwrap();
        assert!(source.export_to_committed(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"existing owner");
        assert!(
            source
                .export_to_committed(&dir.path().join("wrong-prefix.json"))
                .is_err()
        );
        fs::remove_file(&path).unwrap();
        FAIL_NEXT_SYNC.with(|flag| flag.set(true));
        let mut file = source.export_to_committed(&path).unwrap().into_file();
        let published = fs::read(&path).unwrap();
        assert_eq!(published, source.serialize().unwrap());
        assert!(file.has_pending_durability());
        assert!(file.durability_warning().is_some());
        let history = (file.can_undo(), file.can_redo());
        fs::write(&path, b"external after export").unwrap();
        assert!(matches!(
            file.retry_durability(),
            Err(Error::ExternalChange)
        ));
        assert_eq!(fs::read(&path).unwrap(), b"external after export");
        assert_eq!((file.can_undo(), file.can_redo()), history);
        fs::write(&path, &published).unwrap();
        #[cfg(unix)]
        let inode = {
            use std::os::unix::fs::MetadataExt;
            fs::metadata(&path).unwrap().ino()
        };
        file.retry_durability().unwrap();
        assert_eq!(fs::read(&path).unwrap(), published);
        assert!(!file.has_pending_durability());
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(
                fs::metadata(&path).unwrap().ino(),
                inode,
                "retry must not rewrite/rename the published file"
            );
        }
    }
    #[test]
    fn global_creation_commits_warning_receipt_and_preserves_no_clobber_and_exact_undo() {
        let dir = tempfile::tempdir().unwrap();
        let source = profile();
        let mut s = session();
        let song = s.serialize().unwrap();
        FAIL_NEXT_SYNC.with(|flag| flag.set(true));
        let path = s.create_profile(dir.path(), &source).unwrap();
        let published = fs::read(&path).unwrap();
        assert!(
            s.take_profile_edit_update()
                .unwrap()
                .durability_warning
                .is_some()
        );
        assert!(s.has_profile_durability());
        assert_eq!(s.undo_label(), Some("Create generation profile"));
        assert!(!s.is_dirty());
        assert_eq!(s.serialize().unwrap(), song);
        fs::write(&path, b"external after create").unwrap();
        let revision = s.revision();
        assert!(s.undo().is_err());
        assert_eq!(s.revision(), revision);
        assert_eq!(s.undo_label(), Some("Create generation profile"));
        assert_eq!(fs::read(&path).unwrap(), b"external after create");
        fs::write(&path, &published).unwrap();
        s.retry_profile_durability().unwrap();
        assert!(s.undo().unwrap());
        assert!(!path.exists());
        assert!(!s.is_dirty());
        fs::write(&path, b"external blocks redo").unwrap();
        assert!(s.redo().is_err());
        assert_eq!(fs::read(&path).unwrap(), b"external blocks redo");
        assert_eq!(s.redo_label(), Some("Create generation profile"));
        fs::remove_file(&path).unwrap();
        assert!(s.redo().unwrap());
        assert_eq!(fs::read(&path).unwrap(), published);
        assert_eq!(s.serialize().unwrap(), song);
        assert!(!s.is_dirty());
        let revision = s.revision();
        assert!(s.create_profile(dir.path(), &source).is_err());
        assert_eq!(s.revision(), revision);
        assert_eq!(fs::read(&path).unwrap(), published);
    }
    #[test]
    fn explicit_export_receipt_joins_departure_barrier_without_a_fake_history_action() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ecc-external.json");
        let mut s = session();
        let song = s.serialize().unwrap();
        FAIL_NEXT_SYNC.with(|flag| flag.set(true));
        let committed = profile().export_to_committed(&path).unwrap();
        let published = fs::read(&path).unwrap();
        s.retain_profile_publication(committed.into_file());
        assert!(s.has_profile_durability());
        assert!(
            s.pending_profile_durability_update()
                .unwrap()
                .durability_warning
                .is_some()
        );
        assert!(s.undo_label().is_none());
        assert_eq!(s.serialize().unwrap(), song);
        s.retry_profile_durability().unwrap();
        assert!(!s.has_profile_durability());
        assert_eq!(fs::read(&path).unwrap(), published);
    }
}
