//! Native storage adapter. Saving never marks the document clean until the
//! atomic replacement succeeds; failures leave the current document editable.
mod auto_recovery;
mod save_output;
mod recovery_codec;
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
    pub session: Session,
    output_policy: crate::output_policy::OutputPolicy,
    policy_revision: u64,
    publication_revision:u64,
    publication_epoch:Arc<std::sync::atomic::AtomicU64>,
    origin: crate::file_origin::Origin,
    resource_base: PathBuf,
    expected: Option<Arc<[u8]>>,
    recovery_receipt: Option<Arc<Mutex<Option<Arc<[u8]>>>>>,
}

impl FileSession {
    pub fn open(path: &Path) -> Result<Self, Error> {
        Self::open_read(path, |path| Ok(fs::read(path)?))
    }

    /// Strict editor opening with an explicit byte cap and cooperative read
    /// cancellation. The existing tokenizer is unchanged; parsing a bounded
    /// buffer remains synchronous and cannot be interrupted internally.
    pub fn open_bounded(
        path: &Path,
        maximum: usize,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<Self, Error> {
        Self::open_read(path, |path| {
            let file = fs::File::open(path)?;
            if file.metadata()?.len() > maximum as u64 {
                return Err(Error::Unsupported("Song byte limit exceeded".into()));
            }
            read_bounded(file, maximum, cancelled)
        })
    }

    fn open_read(path: &Path, read: impl FnOnce(&Path) -> Result<Vec<u8>, Error>) -> Result<Self, Error> {
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
        let document = Document::parse(expected.clone(), format)?;
        Ok(Self {
            session: Session::new(document),
            output_policy: crate::output_policy::OutputPolicy::NativeLossless,
            policy_revision: 0,
            publication_revision:0,
            publication_epoch:Arc::new(std::sync::atomic::AtomicU64::new(0)),
            resource_base: path.parent().ok_or_else(||Error::InvalidEdit("Song has no resource directory".into()))?.to_owned(),
            origin: crate::file_origin::Origin::Existing(path),
            expected: Some(expected),
            recovery_receipt: Some(Default::default()),
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

    /// Durable draft; the original song is untouched. Recovery is explicit and
    /// refuses a draft whose original file has since changed.
    pub fn save_recovery(&self) -> Result<(), Error> {
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
            policy:self.output_policy.clone(),
            chart_ids:self.session.chart_ids().to_vec(),
            publication_epoch:Arc::clone(&self.publication_epoch),
            publication_revision:self.publication_revision,
            expected_draft: persisted.recovery
                .lock()
                .map_err(|_| std::io::Error::other("Recovery receipt unavailable"))?
                .clone(),
            receipt: Arc::clone(persisted.recovery),
        }))
    }

    pub fn recover(&mut self) -> Result<(), Error> {
        if self.session.is_dirty() {
            return Err(Error::InvalidEdit(
                "Save or discard current edits before recovering a draft".into(),
            ));
        }
        let persisted=self.require_persisted()?;
        let expected=Arc::clone(persisted.expected);
        let receipt=Arc::clone(persisted.recovery);
        let bytes = fs::read(self.sidecar_path(".deadsync-recovery")?)?;
        if self.has_external_change()?{return Err(Error::ExternalChange);}
        let recovered=recovery_codec::decode(&bytes,&expected,self.require_existing_path()?,self.session.document().format)?;
        let next_policy=self.next_policy_revision(&recovered.policy)?;
        let next_publication=self.next_publication_revision()?;
        let bytes:Arc<[u8]>=bytes.into();
        let mut ownership=receipt.lock().map_err(|_|std::io::Error::other("Recovery receipt unavailable"))?;
        *ownership=Some(bytes);
        self.session=recovered.session;self.output_policy=recovered.policy;self.policy_revision=next_policy;self.publication_revision=next_publication;self.publication_epoch.store(next_publication,std::sync::atomic::Ordering::Release);
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
    policy:crate::output_policy::OutputPolicy,
    chart_ids:Vec<crate::generation::metadata::Guid>,
    publication_epoch:Arc<std::sync::atomic::AtomicU64>,
    publication_revision:u64,
    expected_draft: Option<Arc<[u8]>>,
    receipt: Arc<Mutex<Option<Arc<[u8]>>>>,
}
impl RecoverySnapshot {
    fn write(self, require_current_source: bool) -> Result<(), Error> {
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.lock_path)?;
        lock.try_lock().map_err(std::io::Error::other)?;
        let check = || -> Result<(), Error> {
            if require_current_source
                && (self.publication_epoch.load(std::sync::atomic::Ordering::Acquire)!=self.publication_revision || fs::read(&self.source)?.as_slice() != self.expected.as_ref())
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
        let parent = self.path.parent().unwrap();
        let bytes=recovery_codec::encode(&self)?;
        let mut pending = tempfile::NamedTempFile::new_in(parent)?;
        pending.write_all(&bytes)?;
        pending.as_file().sync_all()?;
        check()?;
        pending
            .persist(&self.path)
            .map_err(|error| Error::Io(error.error))?;
        sync_directory(parent)?;
        *self
            .receipt
            .lock()
            .map_err(|_| std::io::Error::other("Recovery receipt unavailable"))? =
            Some(bytes.into());
        Ok(())
    }
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
