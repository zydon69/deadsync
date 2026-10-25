//! Native storage adapter. Saving never marks the document clean until the
//! atomic replacement succeeds; failures leave the current document editable.
use crate::{
    Error,
    document::{Document, Format},
    history::Session,
};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

pub struct FileSession {
    pub session: Session,
    path: PathBuf,
    expected: Arc<[u8]>,
}

impl FileSession {
    pub fn open(path: &Path) -> Result<Self, Error> {
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
        let expected: Arc<[u8]> = fs::read(&path)?.into();
        let document = Document::parse(expected.clone(), format)?;
        Ok(Self {
            session: Session::new(document),
            path,
            expected,
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
        let path = self.recovery_path();
        let parent = path.parent().unwrap();
        let mut pending = tempfile::NamedTempFile::new_in(parent)?;
        pending.write_all(b"DEADSYNC-EDITOR-RECOVERY-1\n")?;
        pending.write_all(&(self.expected.len() as u64).to_le_bytes())?;
        pending.write_all(&self.expected)?;
        pending.write_all(&self.session.serialize()?)?;
        pending.as_file().sync_all()?;
        pending
            .persist(&path)
            .map_err(|error| Error::Io(error.error))?;
        sync_directory(parent)?;
        Ok(())
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
        // Recovery is a new unsaved document, whose clean baseline remains the
        // on-disk file. It must not be silently dismissed as an unchanged file.
        self.session = Session::recovered(recovered);
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
