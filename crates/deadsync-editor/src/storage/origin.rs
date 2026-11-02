//! Proposed typed persistence boundary. Undeclared until the complete Untitled
//! migration patch and all callers are integrated together.
use super::*;
use crate::file_origin::{NewSong, Origin};

pub struct PersistedFile<'a> {
    pub(super) path: &'a Path,
    pub(super) expected: &'a Arc<[u8]>,
    pub(super) recovery: &'a Arc<Mutex<Option<Arc<[u8]>>>>,
}
impl PersistedFile<'_> {
    pub fn path(&self) -> &Path {
        self.path
    }
}
impl FileSession {
    pub fn from_new(candidate: NewSong) -> Result<Self, Error> {
        let Origin::Untitled { resource_base } = candidate.origin else {
            return Err(Error::InvalidEdit(
                "New song must have an Untitled origin".into(),
            ));
        };
        if !resource_base.is_absolute() {
            return Err(Error::InvalidEdit(
                "Untitled resources need an absolute directory".into(),
            ));
        }
        let resource_base = resource_base.canonicalize()?;
        if !resource_base.is_dir() {
            return Err(Error::InvalidEdit(
                "Untitled resource base is not a directory".into(),
            ));
        }
        Ok(Self {
            generation_profile_load: None,
            session: candidate.session,
            output_policy: crate::output_policy::OutputPolicy::NativeLossless,
            policy_revision: 0,
            publication_revision: 0,
            publication_witness: None,
            publication_epoch: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            origin: Origin::Untitled {
                resource_base: resource_base.clone(),
            },
            resource_base,
            expected: None,
            recovery_receipt: None,
            recovery_durability: Default::default(),
        })
    }
    pub fn origin(&self) -> &Origin {
        &self.origin
    }
    pub fn existing_path(&self) -> Option<&Path> {
        self.origin.existing_path()
    }
    pub fn require_existing_path(&self) -> Result<&Path, Error> {
        self.existing_path()
            .ok_or_else(|| Error::Unsupported("Untitled song requires Save As".into()))
    }
    pub fn resource_base(&self) -> &Path {
        &self.resource_base
    }
    pub fn requires_save_as(&self) -> bool {
        self.origin.requires_save_as()
    }
    pub fn has_unsaved_document(&self) -> bool {
        self.requires_save_as() || self.session.is_dirty()
    }
    /// A path getter exists only on this checked persisted view; no Untitled
    /// document can masquerade as an existing file with a suggested filename.
    pub fn persisted(&self) -> Option<PersistedFile<'_>> {
        Some(PersistedFile {
            path: self.existing_path()?,
            expected: self.expected.as_ref()?,
            recovery: self.recovery_receipt.as_ref()?,
        })
    }
    pub(super) fn require_persisted(&self) -> Result<PersistedFile<'_>, Error> {
        self.persisted().ok_or_else(|| {
            Error::Unsupported("Untitled song has no saved baseline or recovery sidecar".into())
        })
    }
}
