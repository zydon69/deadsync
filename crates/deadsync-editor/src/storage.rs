//! Native storage adapter. Saving never marks the document clean until the
//! atomic replacement succeeds; failures leave the current document editable.
mod auto_recovery;
mod authoring;
mod save_output;
mod external;
mod published_open;
pub use published_open::PublishedOpenWitness;
pub use external::{ExternalSnapshot,ExternalObservation};
mod recovery_codec;
mod recovery_publication;
pub use recovery_publication::RecoveryOutcome;
pub use save_output::{SaveAttempt, SaveReview};
mod origin;
pub use origin::PersistedFile;
mod save_as;
pub use save_as::RelocationReceipt;
pub use save_as::{SaveAsReview,SaveAsOutcome,PendingDurability,PreviousRecovery};
use crate::{
    Error,
    document::{Document, Format},
    history::Session,
};
pub use auto_recovery::AutoRecovery;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

pub struct FileSession {
    generation_profile_load:Option<crate::generation::profile_load::Guard>,
    pub session: Session,
    output_policy: crate::output_policy::OutputPolicy,
    policy_revision: u64,
    publication_revision:u64,
    publication_witness:Option<PublishedOpenWitness>,
    publication_epoch:Arc<std::sync::atomic::AtomicU64>,
    origin: crate::file_origin::Origin,
    resource_base: PathBuf,
    expected: Option<Arc<[u8]>>,
    recovery_receipt: Option<Arc<Mutex<Option<Arc<[u8]>>>>>,
    recovery_durability: recovery_publication::SharedPending,
}

impl FileSession {
    pub fn open(path: &Path) -> Result<Self, Error> {
        Self::open_with_context(path,Default::default())
    }

    /// Strict editor opening with an explicit byte cap and cooperative read
    /// cancellation. The existing tokenizer is unchanged; parsing a bounded
    /// buffer remains synchronous and cannot be interrupted internally.
    pub fn open_bounded(
        path: &Path,
        maximum: usize,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<Self, Error> {
        Self::open_bounded_with_context(path,maximum,cancelled,Default::default())
    }
    pub fn open_bounded_with_context(path:&Path,maximum:usize,cancelled:&std::sync::atomic::AtomicBool,context:crate::document::LoadContext)->Result<Self,Error>{
        Self::open_read_with_context(path, context,Some(cancelled),|path| {
            let file = fs::File::open(path)?;
            if file.metadata()?.len() > maximum as u64 {
                return Err(Error::Unsupported("Song byte limit exceeded".into()));
            }
            read_bounded(file, maximum, cancelled)
        })
    }

    fn open_read_with_context(path: &Path, context:crate::document::LoadContext,cancelled:Option<&std::sync::atomic::AtomicBool>,read: impl FnOnce(&Path) -> Result<Vec<u8>, Error>) -> Result<Self, Error> {
        let path = path.canonicalize()?;
        let format = match path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("sm") => Format::Sm,
            Some("ssc") => Format::Ssc,
            _ => {
                return Err(Error::Unsupported(
                    "Only SM and SSC files can be opened".into(),
                ));
            }
        };
        let expected: Arc<[u8]> = read(&path)?.into();
        let document = Document::parse_with_context(expected.clone(), format,context)?;
        let mut session=Session::new(document);session.initialize_song_timing_choice_checked(cancelled)?;
        Ok(Self {
            generation_profile_load:None,
            session,
            output_policy: crate::output_policy::OutputPolicy::NativeLossless,
            policy_revision: 0,
            publication_revision:0,
            publication_witness:None,
            publication_epoch:Arc::new(std::sync::atomic::AtomicU64::new(0)),
            resource_base: path.parent().ok_or_else(||Error::InvalidEdit("Song has no resource directory".into()))?.to_owned(),
            origin: crate::file_origin::Origin::Existing(path),
            expected: Some(expected),
            recovery_receipt: Some(Default::default()),
            recovery_durability: Default::default(),
        })
    }

    pub fn has_external_change(&self) -> Result<bool, Error> {
        let persisted=self.require_persisted()?;
        Ok(fs::read(persisted.path)?.as_slice() != persisted.expected.as_ref())
    }

    pub fn save(&mut self) -> Result<(), Error> {
        match self.save_or_review()? {
            SaveAttempt::Saved=>Ok(()),
            SaveAttempt::NeedsReview(_)=>Err(Error::OutputReviewRequired),
            SaveAttempt::WrittenPendingDurability{error,..}=>Err(error),
        }
    }

    pub fn recovery_path(&self) -> Option<PathBuf> {
        self.existing_path()?;
        self.sidecar_path(".deadsync-recovery").ok()
    }
    fn sidecar_path(&self,suffix:&str)->Result<PathBuf,Error> {
        let path=self.require_existing_path()?;
        let file_name=path.file_name().ok_or_else(||Error::InvalidEdit("Song has no file name".into()))?;
        let mut name=std::ffi::OsString::from(".");name.push(file_name);name.push(suffix);
        Ok(path.with_file_name(name))
    }

    /// Publish a recovery draft without changing the original song. Inspect the
    /// Published outcome: directory durability may remain uncertain after rename.
    /// Recovery is explicit and refuses a draft whose original file changed.
    pub fn save_recovery(&self) -> Result<RecoveryOutcome, Error> {
        self.recovery_snapshot()?.ok_or_else(||Error::Unsupported("Save As is required before creating a sidecar recovery".into()))?.write(false)
    }

    fn recovery_snapshot(&self) -> Result<Option<RecoverySnapshot>, Error> {
        let Some(persisted)=self.persisted() else {return Ok(None)};
        Ok(Some(RecoverySnapshot {
            source: persisted.path.to_owned(),
            path: self.sidecar_path(".deadsync-recovery")?,
            lock_path: self.sidecar_path(".deadsync-save-lock")?,
            expected: Arc::clone(persisted.expected),
            content: self.session.serialize()?,
            source_format:self.session.document().format,
            load_context:self.session.document().load_context(),
            policy:self.output_policy().clone(),
            output_requires_review:self.output_policy().timing_chart(&self.session).is_err(),
            chart_ids:self.session.chart_ids().to_vec(),
            song_timing:self.session.song_timing_choice()?,
            publication_epoch:Arc::clone(&self.publication_epoch),
            publication_revision:self.publication_revision,
            expected_draft: persisted.recovery
                .lock()
                .map_err(|_| std::io::Error::other("Recovery receipt unavailable"))?
                .clone(),
            receipt: Arc::clone(persisted.recovery),
            pending_durability: Arc::clone(&self.recovery_durability),
        }))
    }

    pub fn recover(&mut self) -> Result<(), Error> {
        if self.session.is_dirty() && !self.generation_profile_load.as_ref().is_some_and(|guard|guard.permits_dirty_recovery(&self.session)) {
            return Err(Error::InvalidEdit(
                "Save or discard current edits before recovering a draft".into(),
            ));
        }
        self.retry_recovery_durability()?;
        let persisted=self.require_persisted()?;
        let expected=Arc::clone(persisted.expected);
        let receipt=Arc::clone(persisted.recovery);
        let bytes = fs::read(self.sidecar_path(".deadsync-recovery")?)?;
        if self.has_external_change()?{return Err(Error::ExternalChange);}
        let mut recovered=recovery_codec::decode(&bytes,&expected,self.require_existing_path()?,self.session.document().format)?;
        let recovered_profiles=if let Some(guard)=&self.generation_profile_load{
            let cancelled=std::sync::atomic::AtomicBool::new(false);
            let plan=crate::generation::profile_load::Plan::capture(&recovered.session,guard.directory(),&cancelled)?;
            let mut next=plan.apply(&mut recovered.session,&cancelled)?;next.after_recovery();Some(next)
        }else{None};
        if self.has_external_change()?{return Err(Error::ExternalChange);}
        let next_policy=self.next_policy_revision(&recovered.policy)?;
        let next_publication=self.next_publication_revision()?;
        let bytes:Arc<[u8]>=bytes.into();
        let mut ownership=receipt.lock().map_err(|_|std::io::Error::other("Recovery receipt unavailable"))?;
        *ownership=Some(bytes);
        self.session=recovered.session;self.generation_profile_load=recovered_profiles;self.publication_witness=None;self.output_policy=recovered.policy;self.policy_revision=next_policy;self.publication_revision=next_publication;self.publication_epoch.store(next_publication,std::sync::atomic::Ordering::Release);
        Ok(())
    }
}

struct RecoverySnapshot {
    source: PathBuf,
    path: PathBuf,
    lock_path: PathBuf,
    expected: Arc<[u8]>,
    content: Vec<u8>,
    source_format:Format,
    load_context:crate::document::LoadContext,
    policy:crate::output_policy::OutputPolicy,
    output_requires_review:bool,
    chart_ids:Vec<crate::generation::metadata::Guid>,
    song_timing:crate::song_timing_chart::Choice,
    publication_epoch:Arc<std::sync::atomic::AtomicU64>,
    publication_revision:u64,
    expected_draft: Option<Arc<[u8]>>,
    receipt: Arc<Mutex<Option<Arc<[u8]>>>>,
    pending_durability: recovery_publication::SharedPending,
}

pub(crate) fn sync_directory(path: &Path) -> Result<(), Error> {
    #[cfg(unix)]
    fs::File::open(path)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}


fn read_bounded(
    mut reader: impl Read,
    maximum: usize,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<Vec<u8>, Error> {
    let check = || {
        if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            Err(Error::InvalidEdit("Song opening cancelled".into()))
        } else { Ok(()) }
    };
    let mut bytes = Vec::new();
    let mut chunk = [0; 32 * 1024];
    loop {
        check()?;
        let length = match reader.read(&mut chunk) {
            Ok(length) => length,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        };
        check()?;
        if length == 0 { return Ok(bytes); }
        if length > maximum.saturating_sub(bytes.len()) {
            return Err(Error::Unsupported("Song byte limit exceeded".into()));
        }
        bytes.extend_from_slice(&chunk[..length]);
    }
}

#[cfg(test)]
mod bounded_open_tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    #[test]
    fn growing_and_cancelled_reads_never_pass_a_partial_buffer_to_the_parser() {
        let cancelled = AtomicBool::new(false);
        assert!(read_bounded(std::io::Cursor::new(vec![0; 32*1024+1]),32*1024,&cancelled).is_err());
        assert_eq!(read_bounded(std::io::Cursor::new(b"abc"),3,&cancelled).unwrap(),b"abc");
        struct Cancel<'a>(&'a AtomicBool);
        impl Read for Cancel<'_> {
            fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
                self.0.store(true,Ordering::Relaxed);output[0]=b'#';Ok(1)
            }
        }
        assert!(read_bounded(Cancel(&cancelled),1024,&cancelled).is_err());
    }
}

impl FileSession {pub fn open_with_context(path:&Path,context:crate::document::LoadContext)->Result<Self,Error>{Self::open_read_with_context(path,context,None,|path|Ok(fs::read(path)?))}}
impl FileSession {
    pub(crate) fn install_generation_profile_guard(&mut self,guard:crate::generation::profile_load::Guard){self.generation_profile_load=Some(guard);}
    pub fn validate_generation_profiles(&self,cancelled:&std::sync::atomic::AtomicBool)->Result<(),Error>{
        if cancelled.load(std::sync::atomic::Ordering::Relaxed){return Err(Error::InvalidEdit("Profile resolution cancelled".into()));}
        if let Some(guard)=&self.generation_profile_load{guard.validate(cancelled)?;}
        Ok(())
    }
}

impl FileSession {
    pub(crate) fn has_generation_profile_resolution(&self)->bool{self.generation_profile_load.is_some()}
    pub fn validate_generation_profile_context(&self,directory:Option<&Path>,cancelled:&std::sync::atomic::AtomicBool)->Result<(),Error>{
        if cancelled.load(std::sync::atomic::Ordering::Relaxed){return Err(Error::InvalidEdit("Profile resolution cancelled".into()));}
        if let Some(guard)=&self.generation_profile_load{guard.validate_for(directory,cancelled)?;}Ok(())
    }
}

impl FileSession {pub fn generation_profile_repairs(&self)->&[crate::generation::profile_load::Repair]{self.generation_profile_load.as_ref().map(|guard|guard.repairs()).unwrap_or(&[])}}
