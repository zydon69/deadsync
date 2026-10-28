//! Typed provenance for true in-memory New songs. No placeholder file path.
use crate::{
    Error,
    document::{Document, Format},
    history::Session,
};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    Existing(PathBuf),
    Untitled { resource_base: PathBuf },
}
impl Origin {
    pub fn existing_path(&self) -> Option<&Path> {
        match self {
            Self::Existing(path) => Some(path),
            Self::Untitled { .. } => None,
        }
    }
    pub fn resource_base(&self) -> Result<&Path, Error> {
        match self {
            Self::Existing(path) => path
                .parent()
                .ok_or_else(|| Error::InvalidEdit("Song has no resource directory".into())),
            Self::Untitled { resource_base } => Ok(resource_base),
        }
    }
    pub fn requires_save_as(&self) -> bool {
        matches!(self, Self::Untitled { .. })
    }
}
/// Detached candidate. Only the guarded native publication replaces old state;
/// construction never touches an existing Workspace or its history.
pub struct NewSong {
    pub origin: Origin,
    pub session: Session,
}
impl NewSong {
    pub fn prepare(resource_base: &Path, new_song_sync_offset: f64) -> Result<Self, Error> {
        if !resource_base.is_absolute() || !new_song_sync_offset.is_finite() {
            return Err(Error::InvalidEdit(
                "Invalid New song resource directory or sync offset".into(),
            ));
        }
        let resource_base = resource_base.canonicalize()?;
        if !resource_base.is_dir() {
            return Err(Error::InvalidEdit(
                "New song resource base is not a directory".into(),
            ));
        }
        // GA OnNewNoSave creates a song without charts. No arbitrary starter
        // chart, generated music or fake disk document is introduced.
        let data = serde_json::json!({"SyncOffset":new_song_sync_offset}).to_string();
        let mut bytes = b"#VERSION:0.83;\n#GASongDataVersion:1;\n#GASongData:".to_vec();
        bytes.extend(crate::document::escape_text(&data));
        bytes.extend(b";\n");
        let document = Document::parse(bytes, Format::Ssc)?;
        Ok(Self {
            origin: Origin::Untitled { resource_base },
            session: Session::new(document),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_song_has_no_file_or_chart_and_history_remains_undoable() {
        let base = tempfile::tempdir().unwrap();
        let mut candidate = NewSong::prepare(base.path(), 0.023).unwrap();
        assert!(candidate.origin.requires_save_as());
        assert!(candidate.origin.existing_path().is_none());
        assert_eq!(
            candidate.origin.resource_base().unwrap(),
            base.path().canonicalize().unwrap()
        );
        assert!(candidate.session.document().charts().is_empty());
        assert_eq!(
            crate::song_properties::sync_offset(candidate.session.document()).unwrap(),
            0.023
        );
        let original = candidate.session.serialize().unwrap();
        candidate
            .session
            .set_text(None, "TITLE", "A new title")
            .unwrap();
        assert!(candidate.session.undo().unwrap());
        assert_eq!(candidate.session.serialize().unwrap(), original);
        assert!(candidate.session.redo().unwrap());
        assert_eq!(
            candidate.session.document().text(None, "TITLE").as_deref(),
            Some("A new title")
        );
        assert!(candidate.origin.requires_save_as());
        assert_eq!(
            std::fs::read_dir(base.path()).unwrap().count(),
            0,
            "New must not create a placeholder file"
        );
    }
    #[test]
    fn invalid_resource_base_and_nonfinite_offset_never_produce_a_candidate() {
        let base = tempfile::tempdir().unwrap();
        assert!(NewSong::prepare(Path::new("relative"), 0.009).is_err());
        assert!(NewSong::prepare(base.path(), f64::NAN).is_err());
        assert!(NewSong::prepare(&base.path().join("missing"), 0.009).is_err());
    }
}
