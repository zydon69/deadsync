//! Native storage adapter. Saving never marks the document clean until the
//! atomic replacement succeeds; failures leave the current document editable.
mod auto_recovery;
mod save_as;
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
    path: PathBuf,
    expected: Arc<[u8]>,
    recovery_receipt: Arc<Mutex<Option<Arc<[u8]>>>>,
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
            path,
            expected,
            recovery_receipt: Default::default(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn has_external_change(&self) -> Result<bool, Error> {
        Ok(fs::read(&self.path)?.as_slice() != self.expected.as_ref())
    }

    pub fn save(&mut self) -> Result<(), Error> {
        let bytes = self.session.serialize()?;
        // The sidecar prevents two DeadSync editors from saving the same file
        // concurrently. Other editors are detected by comparing exact bytes.
        let lock_path = self.sidecar_path(".deadsync-save-lock");
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)?;
        lock.try_lock().map_err(std::io::Error::other)?;
        if self.has_external_change()? {
            return Err(Error::ExternalChange);
        }
        let parent = self
            .path
            .parent()
            .ok_or_else(|| Error::InvalidEdit("Simfile has no parent directory".into()))?;
        let mut pending = tempfile::NamedTempFile::new_in(parent)?;
        pending
            .as_file()
            .set_permissions(fs::metadata(&self.path)?.permissions())?;
        pending.write_all(&bytes)?;
        pending.as_file().sync_all()?;
        if self.has_external_change()? {
            return Err(Error::ExternalChange);
        }
        pending
            .persist(&self.path)
            .map_err(|error| Error::Io(error.error))?;
        self.expected = bytes.into();
        sync_directory(parent)?;
        self.session.mark_saved();
        Ok(())
    }

    pub fn recovery_path(&self) -> PathBuf {
        self.sidecar_path(".deadsync-recovery")
    }

    fn sidecar_path(&self, suffix: &str) -> PathBuf {
        let mut name = std::ffi::OsString::from(".");
        name.push(self.path.file_name().unwrap());
        name.push(suffix);
        self.path.with_file_name(name)
    }

    /// Durable draft; the original song is untouched. Recovery is explicit and
    /// refuses a draft whose original file has since changed.
    pub fn save_recovery(&self) -> Result<(), Error> {
        self.recovery_snapshot()?.write(false)
    }

    fn recovery_snapshot(&self) -> Result<RecoverySnapshot, Error> {
        Ok(RecoverySnapshot {
            source: self.path.clone(),
            path: self.recovery_path(),
            lock_path: self.sidecar_path(".deadsync-save-lock"),
            expected: self.expected.clone(),
            content: self.session.serialize()?,
            expected_draft: self
                .recovery_receipt
                .lock()
                .map_err(|_| std::io::Error::other("Recovery receipt unavailable"))?
                .clone(),
            receipt: self.recovery_receipt.clone(),
        })
    }

    pub fn recover(&mut self) -> Result<(), Error> {
        if self.session.is_dirty() {
            return Err(Error::InvalidEdit(
                "Save or discard current edits before recovering a draft".into(),
            ));
        }
        let bytes = fs::read(self.recovery_path())?;
        let magic = b"DEADSYNC-EDITOR-RECOVERY-1\n";
        let length = bytes
            .get(magic.len()..magic.len() + 8)
            .filter(|_| bytes.starts_with(magic))
            .ok_or_else(|| Error::InvalidFormat("Invalid recovery header".into()))?;
        let length = usize::try_from(u64::from_le_bytes(length.try_into().unwrap()))
            .map_err(|_| Error::InvalidFormat("Recovery length overflow".into()))?;
        let start = magic.len() + 8;
        let end = start
            .checked_add(length)
            .filter(|&end| end <= bytes.len())
            .ok_or_else(|| Error::InvalidFormat("Incomplete recovery file".into()))?;
        if &bytes[start..end] != self.expected.as_ref() || self.has_external_change()? {
            return Err(Error::ExternalChange);
        }
        let recovered = Document::parse(
            Arc::<[u8]>::from(&bytes[end..]),
            self.session.document().format,
        )?;
        // Recovery keeps the on-disk baseline and a fresh session identity.
        self.session = Session::recovered(recovered);
        *self
            .recovery_receipt
            .lock()
            .map_err(|_| std::io::Error::other("Recovery receipt unavailable"))? =
            Some(bytes.into());
        Ok(())
    }
}

struct RecoverySnapshot {
    source: PathBuf,
    path: PathBuf,
    lock_path: PathBuf,
    expected: Arc<[u8]>,
    content: Vec<u8>,
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
                && fs::read(&self.source)?.as_slice() != self.expected.as_ref()
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
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"DEADSYNC-EDITOR-RECOVERY-1\n");
        bytes.extend_from_slice(&(self.expected.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&self.expected);
        bytes.extend_from_slice(&self.content);
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
