//! Relocation provenance used by the same current/Undo/Redo media walker.
use super::*;
#[derive(Clone, Copy)]
pub(crate) enum SourceBase<'a> {
    ExistingFile(&'a Path),
    ResourceDirectory(&'a Path),
}
impl SourceBase<'_> {
    pub(crate) fn is_absolute(self) -> bool {
        match self {
            Self::ExistingFile(path) | Self::ResourceDirectory(path) => path.is_absolute(),
        }
    }
    pub(crate) fn rebase(self, new_file: &Path, media: &str) -> Result<String, Error> {
        match self {
            Self::ExistingFile(old) => rebase(Some(old), new_file, media),
            Self::ResourceDirectory(old) => rebase_from_directory(old, new_file, media),
        }
    }
}
