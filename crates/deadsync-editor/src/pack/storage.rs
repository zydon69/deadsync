//! Explicit Pack.ini publication. No watcher or simfile storage is involved.
//!
//! Locks coordinate DeadSync writers; exact byte checks detect other editors.
//! Like ordinary rename-based editors, comparison and rename are not a kernel
//! compare-and-swap against unrelated processes. Creation is always no-clobber.
use super::{Data, Document, Draft, MAX_BYTES, Prepared};
mod deletion;
pub mod history;
pub use deletion::{DeleteSnapshot,DeleteOutcome,DeletionDurability};
use deletion::PendingDeletion;
use std::{
    fs::{self, File, Metadata, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

/// Successful publication guarantees available on the current platform.
/// No result from another platform implies that Windows was validated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Durability {
    /// File contents and the parent directory were explicitly synchronized.
    FileAndDirectory,
    /// File contents were synchronized and atomically published. Directory
    /// synchronization is unavailable in the current std-based non-Unix adapter.
    FileOnly,
}
pub fn durability_guarantee() -> Durability {
    #[cfg(unix)]
    {
        Durability::FileAndDirectory
    }
    #[cfg(not(unix))]
    {
        Durability::FileOnly
    }
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Metadata(super::Error),
    ExternalChange,
    StaleSnapshot,
    SaveLocked,
    /// Bytes were published, but the directory sync or acknowledgement failed.
    /// Retry durability before preparing another save; the draft remains dirty.
    DurabilityPending(io::Error),
    PendingSave,
    DeletionDurabilityPending(io::Error),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "Pack I/O: {e}"),
            Self::Metadata(e) => e.fmt(f),
            Self::ExternalChange => f.write_str("Pack.ini changed outside this editor"),
            Self::StaleSnapshot => {
                f.write_str("Pack save snapshot belongs to another or older file session")
            }
            Self::SaveLocked => f.write_str("Another editor is saving this pack"),
            Self::DurabilityPending(e) => {
                write!(f, "Pack bytes published; durability still pending: {e}")
            }
            Self::DeletionDurabilityPending(error)=>write!(f,"Pack.ini deleted; directory durability pending: {error}"),
            Self::PendingSave => f.write_str("Finish the pending durable pack save first"),
        }
    }
}
impl std::error::Error for Error {}
impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<super::Error> for Error {
    fn from(e: super::Error) -> Self {
        Self::Metadata(e)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Identity {
    #[cfg(unix)]
    dev: u64,
    #[cfg(unix)]
    ino: u64,
    // Portable fallback detects changes in the available stable metadata. It
    // cannot prove inode identity on platforms without std's Unix extensions.
    #[cfg(not(unix))]
    created: Option<std::time::SystemTime>,
}
impl Identity {
    fn of(m: &Metadata) -> Self {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Self {
                dev: m.dev(),
                ino: m.ino(),
            }
        }
        #[cfg(not(unix))]
        {
            Self {
                created: m.created().ok(),
            }
        }
    }
}
#[derive(Clone, Debug)]
struct Directory {
    path: PathBuf,
    identity: Identity,
}
impl Directory {
    fn capture(path: &Path) -> Result<Self, Error> {
        let path = path.canonicalize()?;
        let metadata = fs::metadata(&path)?;
        if !metadata.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                "Pack parent is not a directory",
            )
            .into());
        }
        Ok(Self {
            path,
            identity: Identity::of(&metadata),
        })
    }
    fn check(&self) -> Result<(), Error> {
        let metadata = fs::metadata(&self.path).map_err(conflict_if_missing)?;
        if !metadata.is_dir() || Identity::of(&metadata) != self.identity {
            return Err(Error::ExternalChange);
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
struct Binding {
    logical: PathBuf,
    pack_parent: Directory,
    target: PathBuf,
    target_parent: Directory,
    identity: Option<Identity>,
}
impl Binding {
    fn capture(pack_root: &Path) -> Result<Self, Error> {
        let pack_parent = Directory::capture(pack_root)?;
        let logical = pack_parent.path.join("Pack.ini");
        let (target, identity) = match fs::symlink_metadata(&logical) {
            Ok(_) => {
                // Resolve existing symlinks, and replace the target rather than
                // destroying a user-managed Pack.ini link. Dangling links fail.
                let target = logical.canonicalize()?;
                let metadata = fs::metadata(&target)?;
                if !metadata.is_file() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Pack.ini is not a regular file",
                    )
                    .into());
                }
                (target, Some(Identity::of(&metadata)))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => (logical.clone(), None),
            Err(e) => return Err(e.into()),
        };
        let target_parent = Directory::capture(target.parent().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "Pack target has no parent")
        })?)?;
        Ok(Self {
            logical,
            pack_parent,
            target,
            target_parent,
            identity,
        })
    }
    fn check(&self) -> Result<(), Error> {
        self.pack_parent.check()?;
        self.target_parent.check()?;
        match &self.identity {
            Some(expected) => {
                let target = self.logical.canonicalize().map_err(conflict_if_missing)?;
                if target != self.target {
                    return Err(Error::ExternalChange);
                }
                let metadata = fs::metadata(&target).map_err(conflict_if_missing)?;
                if !metadata.is_file() || Identity::of(&metadata) != *expected {
                    return Err(Error::ExternalChange);
                }
            }
            None => match fs::symlink_metadata(&self.logical) {
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Ok(_) => return Err(Error::ExternalChange),
                Err(e) => return Err(e.into()),
            },
        }
        Ok(())
    }
}
fn conflict_if_missing(error: io::Error) -> Error {
    if matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
    ) {
        Error::ExternalChange
    } else {
        Error::Io(error)
    }
}
fn read_bounded(path: &Path) -> Result<Vec<u8>, Error> {
    let file = File::open(path)?;
    if file.metadata()?.len() > MAX_BYTES as u64 {
        return Err(super::Error("Pack.ini exceeds the metadata size limit".into()).into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_BYTES {
        return Err(super::Error("Pack.ini exceeds the metadata size limit".into()).into());
    }
    Ok(bytes)
}
fn sync_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    File::open(path)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// Snapshot is bound to one adapter generation and one immutable Draft payload.
/// It cannot be edited or manufactured by callers.
pub struct Snapshot {
    owner: Arc<()>,
    generation: u64,
    prepared: Prepared,
}
impl Snapshot {
    pub fn bytes(&self) -> &[u8] {
        self.prepared.bytes()
    }
}

pub struct FileSession {
    draft: Draft,
    binding: Binding,
    expected: Option<Arc<[u8]>>,
    owner: Arc<()>,
    generation: u64,
    pending: Option<Prepared>,
    pending_deletion:Option<PendingDeletion>,
    deleted:bool,
    mutation_revision:u64,
}
impl FileSession {
    /// Read-only discovery: absence returns None and never creates Pack.ini.
    pub fn open(pack_root: &Path) -> Result<Option<Self>, Error> {
        let binding = Binding::capture(pack_root)?;
        if binding.identity.is_none() {
            return Ok(None);
        }
        let bytes = read_bounded(&binding.target)?;
        binding.check()?;
        let draft = Draft::existing(Document::parse(bytes.clone())?);
        Ok(Some(Self {
            draft,
            binding,
            expected: Some(bytes.into()),
            owner: Arc::new(()),
            generation: 0,
            pending: None,pending_deletion:None,deleted:false,mutation_revision:0,
        }))
    }
    /// Prepare explicit creation without publishing anything. A file appearing
    /// now or before publication is a conflict, including dangling symlinks.
    pub fn create(pack_root: &Path, data: Data) -> Result<Self, Error> {
        let binding = Binding::capture(pack_root)?;
        if binding.identity.is_some() {
            return Err(Error::ExternalChange);
        }
        Ok(Self {
            draft: Draft::create(data)?,
            binding,
            expected: None,
            owner: Arc::new(()),
            generation: 0,
            pending: None,pending_deletion:None,deleted:false,mutation_revision:0,
        })
    }
    pub fn path(&self) -> &Path {
        &self.binding.logical
    }
    pub fn draft(&self) -> &Draft {
        &self.draft
    }
    pub fn replace(&mut self, data: Data) -> Result<bool, Error> {
        if self.deleted{return Err(if self.deletion_pending(){Error::PendingSave}else{Error::StaleSnapshot});}
        let next=self.mutation_revision.checked_add(1).ok_or_else(||io::Error::other("Pack edit revision overflow"))?;
        let changed=self.draft.replace(data)?;if changed{self.mutation_revision=next;}Ok(changed)
    }
    pub fn set_year(&mut self, year: i32) -> Result<bool, Error> {
        if self.deleted{return Err(if self.deletion_pending(){Error::PendingSave}else{Error::StaleSnapshot});}
        let next=self.mutation_revision.checked_add(1).ok_or_else(||io::Error::other("Pack edit revision overflow"))?;
        let changed=self.draft.set_year(year)?;if changed{self.mutation_revision=next;}Ok(changed)
    }
    pub fn set_sync_offset(&mut self, sync_offset: i32) -> Result<bool, Error> {
        if self.deleted{return Err(if self.deletion_pending(){Error::PendingSave}else{Error::StaleSnapshot});}
        let next=self.mutation_revision.checked_add(1).ok_or_else(||io::Error::other("Pack edit revision overflow"))?;
        let changed=self.draft.set_sync_offset(sync_offset)?;if changed{self.mutation_revision=next;}Ok(changed)
    }
    pub fn durability_pending(&self) -> bool {
        self.pending.is_some()||self.pending_deletion.is_some()
    }
    pub fn has_external_change(&self) -> Result<bool, Error> {
        match if self.deleted{self.check_deleted_entry()}else{self.check_expected()} {
            Ok(()) => Ok(false),
            Err(Error::ExternalChange) => Ok(true),
            Err(e) => Err(e),
        }
    }
    fn check_expected(&self) -> Result<(), Error> {
        self.binding.check()?;
        if let Some(expected) = &self.expected {
            let actual = read_bounded(&self.binding.target).map_err(|e| match e {
                Error::Io(e) => conflict_if_missing(e),
                e => e,
            })?;
            if actual.as_slice() != expected.as_ref() {
                return Err(Error::ExternalChange);
            }
        }
        Ok(())
    }
    pub fn prepare_save(&self) -> Result<Snapshot, Error> {
        if self.durability_pending() {
            return Err(Error::PendingSave);
        }
        if self.deleted{return Err(Error::StaleSnapshot);}
        Ok(Snapshot {
            owner: self.owner.clone(),
            generation: self.generation,
            prepared: self.draft.prepare()?,
        })
    }
    pub fn save(&mut self) -> Result<Durability, Error> {
        let snapshot = self.prepare_save()?;
        self.publish(snapshot)
    }
    pub fn publish(&mut self, snapshot: Snapshot) -> Result<Durability, Error> {
        self.publish_with(snapshot, |_| Ok(()), sync_directory)
    }
    fn lock(&self) -> Result<File, Error> {
        self.binding.pack_parent.check()?;
        self.binding.target_parent.check()?;
        let mut name = std::ffi::OsString::from(".");
        name.push(self.binding.target.file_name().unwrap());
        name.push(".deadsync-save-lock");
        let path = self.binding.target.with_file_name(name);
        // Create exclusively; existing locks are opened without create/truncate.
        // Verify the opened inode against the non-following path metadata before
        // using it, so a raced symlink cannot redirect lock ownership.
        let lock = match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                let metadata = fs::symlink_metadata(&path)?;
                if !metadata.is_file() || metadata.file_type().is_symlink() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Pack save lock is not a regular file",
                    )
                    .into());
                }
                OpenOptions::new().read(true).write(true).open(&path)?
            }
            Err(e) => return Err(e.into()),
        };
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || Identity::of(&metadata) != Identity::of(&lock.metadata()?)
        {
            return Err(Error::ExternalChange);
        }
        lock.try_lock().map_err(|_| Error::SaveLocked)?;
        Ok(lock)
    }
    fn publish_with(
        &mut self,
        snapshot: Snapshot,
        before_publish: impl FnOnce(&Path) -> io::Result<()>,
        sync: impl FnOnce(&Path) -> io::Result<()>,
    ) -> Result<Durability, Error> {
        if self.durability_pending() {
            return Err(Error::PendingSave);
        }
        if self.deleted{return Err(Error::StaleSnapshot);}
        if !Arc::ptr_eq(&self.owner, &snapshot.owner) || snapshot.generation != self.generation {
            return Err(Error::StaleSnapshot);
        }
        if snapshot.prepared.baseline() != self.expected.as_deref() {
            return Err(Error::StaleSnapshot);
        }
        let next_generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| io::Error::other("Pack save generation overflow"))?;
        let _lock = self.lock()?;
        self.check_expected()?;
        let parent = &self.binding.target_parent.path;
        let mut pending = tempfile::NamedTempFile::new_in(parent)?;
        let permissions = if self.expected.is_some() {
            Some(fs::metadata(&self.binding.target)?.permissions())
        } else {
            None
        };
        pending.write_all(snapshot.prepared.bytes())?;
        if let Some(permissions) = permissions {
            pending.as_file().set_permissions(permissions)?;
        }
        pending.as_file().sync_all()?;
        // The hook is private and exists for deterministic fault/race tests.
        // Existing files must be checked after it; creation additionally proves
        // kernel no-clobber when a file appears after the final comparison.
        if self.expected.is_some() {
            before_publish(&self.binding.target)?;
            self.check_expected()?;
        } else {
            self.check_expected()?;
            before_publish(&self.binding.target)?;
        }
        let published_identity = Identity::of(&pending.as_file().metadata()?);
        let published = if self.expected.is_some() {
            pending
                .persist(&self.binding.target)
                .map_err(|e| Error::Io(e.error))?
        } else {
            pending
                .persist_noclobber(&self.binding.target)
                .map_err(|e| {
                    if e.error.kind() == io::ErrorKind::AlreadyExists {
                        Error::ExternalChange
                    } else {
                        Error::Io(e.error)
                    }
                })?
        };
        // Bind the receipt to the file handle we actually published. A new path
        // lookup here can fail or observe an unrelated external replacement.
        self.expected = Some(Arc::from(snapshot.prepared.bytes()));
        self.pending = Some(snapshot.prepared);
        self.generation = next_generation;
        self.binding.identity = Some(published_identity);
        sync(parent).map_err(Error::DurabilityPending)?;
        drop(published);
        self.finish_acknowledgement()
    }
    fn finish_acknowledgement(&mut self) -> Result<Durability, Error> {
        let prepared = self.pending.as_ref().ok_or(Error::PendingSave)?;
        self.draft
            .acknowledge_saved(prepared)
            .map_err(|e| Error::DurabilityPending(io::Error::other(e)))?;
        self.pending = None;
        Ok(durability_guarantee())
    }
    /// Complete a published snapshot without publishing it a second time.
    pub fn retry_durability(&mut self) -> Result<Durability, Error> {
        if self.pending.is_none() {
            return Err(Error::PendingSave);
        }
        let _lock = self.lock()?;
        self.check_expected()?;
        sync_directory(&self.binding.target_parent.path).map_err(Error::DurabilityPending)?;
        self.finish_acknowledgement()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn create() -> (tempfile::TempDir, FileSession) {
        let root = tempfile::tempdir().unwrap();
        let file =
            FileSession::create(root.path(), Data::new("Initial", "", 2026, None, 0.009)).unwrap();
        (root, file)
    }
    fn title(file: &mut FileSession, value: &str) {
        let mut data = file.draft().data().clone();
        data.title = Some(value.into());
        file.replace(data).unwrap();
    }
    #[test]
    fn creation_race_uses_atomic_no_clobber_after_the_final_comparison() {
        let (_root, mut file) = create();
        let snapshot = file.prepare_save().unwrap();
        assert!(matches!(
            file.publish_with(
                snapshot,
                |path| fs::write(path, b"race winner"),
                sync_directory
            ),
            Err(Error::ExternalChange)
        ));
        assert_eq!(fs::read(file.path()).unwrap(), b"race winner");
        assert!(file.draft().has_unsaved_changes());
        assert!(!file.durability_pending());
    }
    #[test]
    fn prepublication_error_keeps_baseline_payload_and_dirty_state_for_retry() {
        let (_root, mut file) = create();
        file.save().unwrap();
        let before = fs::read(file.path()).unwrap();
        title(&mut file, "Changed");
        let snapshot = file.prepare_save().unwrap();
        assert!(matches!(
            file.publish_with(
                snapshot,
                |_| Err(io::Error::other("injected before publication")),
                sync_directory
            ),
            Err(Error::Io(_))
        ));
        assert_eq!(fs::read(file.path()).unwrap(), before);
        assert!(file.draft().has_unsaved_changes());
        assert!(!file.has_external_change().unwrap());
        assert!(!file.durability_pending());
        file.save().unwrap();
        assert!(!file.draft().has_unsaved_changes());
    }
    #[test]
    fn published_creation_keeps_receipt_after_directory_sync_failure_and_retries_without_rewrite() {
        let (_root, mut file) = create();
        let snapshot = file.prepare_save().unwrap();
        let saved = snapshot.bytes().to_vec();
        assert!(matches!(
            file.publish_with(
                snapshot,
                |_| Ok(()),
                |_| Err(io::Error::other("injected directory sync failure"))
            ),
            Err(Error::DurabilityPending(_))
        ));
        assert_eq!(fs::read(file.path()).unwrap(), saved);
        assert!(file.draft().has_unsaved_changes());
        assert!(file.durability_pending());
        assert!(!file.has_external_change().unwrap());
        assert!(matches!(file.prepare_save(), Err(Error::PendingSave)));
        let published_identity = Identity::of(&fs::metadata(file.path()).unwrap());
        title(&mut file, "Later live edit");
        file.retry_durability().unwrap();
        assert!(!file.durability_pending());
        assert!(file.draft().has_unsaved_changes());
        assert_eq!(
            Identity::of(&fs::metadata(file.path()).unwrap()),
            published_identity
        );
        assert_eq!(fs::read(file.path()).unwrap(), saved);
        file.save().unwrap();
        assert!(!file.draft().has_unsaved_changes());
    }
    #[test]
    fn published_replacement_external_edit_blocks_durability_retry_without_losing_live_edits() {
        let (_root, mut file) = create();
        file.save().unwrap();
        title(&mut file, "Snapshot");
        let snapshot = file.prepare_save().unwrap();
        assert!(matches!(
            file.publish_with(
                snapshot,
                |_| Ok(()),
                |_| Err(io::Error::other("sync failure"))
            ),
            Err(Error::DurabilityPending(_))
        ));
        fs::write(file.path(), b"external after publication").unwrap();
        title(&mut file, "Live edit");
        assert!(matches!(
            file.retry_durability(),
            Err(Error::ExternalChange)
        ));
        assert!(file.durability_pending());
        assert!(file.draft().has_unsaved_changes());
        assert_eq!(file.draft().data().title.as_deref(), Some("Live edit"));
        assert_eq!(
            fs::read(file.path()).unwrap(),
            b"external after publication"
        );
    }
    #[test]
    fn published_receipt_does_not_require_a_postrename_path_metadata_lookup() {
        for replace in [false, true] {
            let (root, mut file) = create();
            if replace {
                file.save().unwrap();
                title(&mut file, "Replacement");
            }
            let snapshot = file.prepare_save().unwrap();
            let bytes = snapshot.bytes().to_vec();
            let target = file.binding.target.clone();
            let hidden = root.path().join("temporarily-unlinked.ini");
            assert!(matches!(
                file.publish_with(
                    snapshot,
                    |_| Ok(()),
                    |_| {
                        fs::rename(&target, &hidden)?;
                        assert!(fs::metadata(&target).is_err());
                        Err(io::Error::other("injected missing path after publication"))
                    }
                ),
                Err(Error::DurabilityPending(_))
            ));
            assert!(file.draft().has_unsaved_changes());
            assert!(file.durability_pending());
            let identity = Identity::of(&fs::metadata(&hidden).unwrap());
            assert_eq!(file.binding.identity.as_ref(), Some(&identity));
            fs::rename(&hidden, &target).unwrap();
            file.retry_durability().unwrap();
            assert!(!file.draft().has_unsaved_changes());
            assert_eq!(fs::read(&target).unwrap(), bytes);
            assert_eq!(Identity::of(&fs::metadata(&target).unwrap()), identity);
        }
    }
    #[test]
    fn external_write_during_temporary_file_preparation_prevents_existing_replacement() {
        let (_root, mut file) = create();
        file.save().unwrap();
        title(&mut file, "Changed");
        let snapshot = file.prepare_save().unwrap();
        assert!(matches!(
            file.publish_with(
                snapshot,
                |path| fs::write(path, b"external during preparation"),
                sync_directory
            ),
            Err(Error::ExternalChange)
        ));
        assert_eq!(
            fs::read(file.path()).unwrap(),
            b"external during preparation"
        );
        assert!(file.draft().has_unsaved_changes());
        assert!(!file.durability_pending());
    }
}
/// Immutable exact baseline for passive observation. Capturing never performs I/O.
#[derive(Clone)]
pub struct ExternalSnapshot { binding:Binding,expected:Option<Arc<[u8]>>,owner:Arc<()>,generation:u64,adoptable:bool }
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct ExternalObservation{bytes:Option<Arc<[u8]>>,identity:Option<Identity>,target:Option<PathBuf>}
impl FileSession {pub fn external_snapshot(&self)->ExternalSnapshot{ExternalSnapshot{binding:self.binding.clone(),expected:self.expected.clone(),owner:self.owner.clone(),generation:self.generation,adoptable:!self.durability_pending()&&!self.is_deleted()}}}
impl ExternalSnapshot{
 /// Rebuild only the settled captured baseline; never open the current path.
 /// Parsing belongs to the observer worker, and a pending durability receipt is never cloned.
 pub fn rebuild_observer_baseline(&self)->Result<Option<FileSession>,Error>{
  if !self.adoptable{return Ok(None);}let Some(bytes)=&self.expected else{return Ok(None);};
  Ok(Some(FileSession{draft:Draft::existing(Document::parse(bytes.to_vec())?),binding:self.binding.clone(),expected:Some(bytes.clone()),owner:self.owner.clone(),generation:self.generation,pending:None,pending_deletion:None,deleted:false,mutation_revision:0}))
 }
 pub fn same_capture(&self,other:&Self)->bool{Arc::ptr_eq(&self.owner,&other.owner)&&self.generation==other.generation&&self.adoptable==other.adoptable}
 pub fn is_current(&self,file:&FileSession)->bool{Arc::ptr_eq(&self.owner,&file.owner)&&self.generation==file.generation&&self.adoptable==(!file.durability_pending()&&!file.is_deleted())}
 pub fn probe(&self,cancelled:&std::sync::atomic::AtomicBool)->Result<Option<ExternalObservation>,Error>{
  let cancelled=||cancelled.load(std::sync::atomic::Ordering::Acquire);
  if cancelled(){return Err(Error::StaleSnapshot);}
  let binding_changed=match self.binding.check(){Ok(())=>false,Err(Error::ExternalChange)=>true,Err(error)=>return Err(error)};
  let actual=match read_bounded(&self.binding.target){Ok(bytes)=>Some(bytes),Err(Error::Io(error))if error.kind()==io::ErrorKind::NotFound=>None,Err(error)=>return Err(error)};
  if cancelled(){return Err(Error::StaleSnapshot);}
  // Identity replacement and exact bytes are both changes, even if bytes match.
  let identity=match fs::metadata(&self.binding.target){Ok(metadata)=>Some(Identity::of(&metadata)),Err(error)if error.kind()==io::ErrorKind::NotFound=>None,Err(error)=>return Err(error.into())};
  let target=match self.binding.logical.canonicalize(){Ok(path)=>Some(path),Err(error)if error.kind()==io::ErrorKind::NotFound=>None,Err(error)=>return Err(error.into())};
  if !binding_changed&&actual.as_deref()==self.expected.as_deref()&&identity==self.binding.identity{return Ok(None);}
  Ok(Some(ExternalObservation{bytes:actual.map(Arc::from),identity,target}))
 }
}
