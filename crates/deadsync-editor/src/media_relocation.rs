//! Lexical media-reference relocation; no filesystem access or save publication.
mod directory;
mod source;
pub(crate) use source::SourceBase;
pub use directory::rebase_from_directory;
use crate::Error;
use std::path::{Component, Path, PathBuf};

/// Preserve an asset's location when its containing simfile moves. A new song
/// without an old location retains relative references, matching GrooveAuthor.
pub fn rebase(old_file: Option<&Path>, new_file: &Path, media: &str) -> Result<String, Error> {
    if media.is_empty() {
        return Ok(String::new());
    }
    if media.contains('\0') {
        return Err(invalid("Media reference contains a null character"));
    }
    let new_parent = absolute_parent(new_file)?;
    let media_path = Path::new(media);
    let target = if media_path.is_absolute() {
        normalized(media_path)
    } else if let Some(old_file) = old_file.filter(|path| !path.as_os_str().is_empty()) {
        normalized(&absolute_parent(old_file)?.join(media_path))
    } else {
        return Ok(media.to_owned());
    };
    relative_target(&new_parent, &target)
}

fn relative_target(new_parent: &Path, target: &Path) -> Result<String, Error> {
    let base: Vec<_> = new_parent.components().collect();
    let target: Vec<_> = target.components().collect();
    if base.first() != target.first() {
        return Err(invalid(
            "Media relocation crosses unsupported filesystem roots",
        ));
    }
    let common = base.iter().zip(&target).take_while(|(a, b)| a == b).count();
    let mut relative = PathBuf::new();
    for component in &base[common..] {
        if !matches!(component, Component::Normal(_)) {
            return Err(invalid("Invalid media relocation base"));
        }
        relative.push("..");
    }
    for component in &target[common..] {
        relative.push(component.as_os_str());
    }
    if relative.as_os_str().is_empty() {
        relative.push(".");
    }
    relative
        .to_str()
        .map(|path| path.replace(std::path::MAIN_SEPARATOR, "/"))
        .ok_or_else(|| invalid("Media relocation is not representable as UTF-8"))
}

fn absolute_parent(file: &Path) -> Result<PathBuf, Error> {
    if !file.is_absolute() || file.file_name().is_none() {
        return Err(invalid(
            "Media relocation requires an absolute simfile path",
        ));
    }
    Ok(normalized(
        file.parent()
            .ok_or_else(|| invalid("Simfile has no directory"))?,
    ))
}
fn normalized(path: &Path) -> PathBuf {
    let mut output = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                output.pop();
            }
            _ => output.push(component.as_os_str()),
        }
    }
    output
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
