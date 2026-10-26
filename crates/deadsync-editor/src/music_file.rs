//! Simfile media selection. Paths are lexical: symlinks are not resolved.
use crate::Error;
use std::path::{Component, Path, PathBuf};
pub const AUDIO_EXTENSIONS: [&str; 5] = ["mp3", "oga", "ogg", "opus", "wav"];
pub const IMAGE_EXTENSIONS: [&str; 8] = ["bmp", "gif", "jpeg", "jpg", "png", "tif", "tiff", "webp"];
pub const VIDEO_EXTENSIONS: [&str; 11] = [
    "avi", "f4v", "flv", "mkv", "mp4", "mpeg", "mpg", "mov", "ogv", "webm", "wmv",
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Audio,
    Image,
    ImageOrVideo,
    Video,
    Lyrics,
}
impl Kind {
    pub fn extensions(self) -> Vec<&'static str> {
        match self {
            Self::Audio => AUDIO_EXTENSIONS.to_vec(),
            Self::Image => IMAGE_EXTENSIONS.to_vec(),
            Self::ImageOrVideo => IMAGE_EXTENSIONS
                .into_iter()
                .chain(VIDEO_EXTENSIONS)
                .collect(),
            Self::Video => VIDEO_EXTENSIONS.to_vec(),
            Self::Lyrics => vec!["lrc"],
        }
    }
}
pub struct Browse {
    pub directory: PathBuf,
    pub current: String,
    pub kind: Kind,
}
pub trait Picker {
    fn choose(&mut self, request: &Browse) -> Result<Option<PathBuf>, Error>;
}
/// Store a portable relative path, including ../ for files outside the song.
/// Different filesystem roots retain an absolute path. No file is copied.
pub fn selected_path(simfile: &Path, selected: &Path) -> Result<String, Error> {
    if !selected.is_file() {
        return Err(Error::InvalidEdit(
            "Selected music file does not exist".into(),
        ));
    }
    relative_path(simfile.parent().unwrap_or(Path::new(".")), selected)
}
/// Lexical relative-path conversion, independent of file existence.
pub fn relative_path(directory: &Path, selected: &Path) -> Result<String, Error> {
    let base = absolute(directory)?;
    let selected = absolute(selected)?;
    let left: Vec<_> = base.components().collect();
    let right: Vec<_> = selected.components().collect();
    let common = left.iter().zip(&right).take_while(|(a, b)| a == b).count();
    let path = if common == 0
        || left
            .get(common)
            .is_some_and(|c| matches!(c, Component::Prefix(_) | Component::RootDir))
        || right
            .get(common)
            .is_some_and(|c| matches!(c, Component::Prefix(_) | Component::RootDir))
    {
        selected
    } else {
        let mut path = PathBuf::new();
        for _ in &left[common..] {
            path.push("..");
        }
        for c in &right[common..] {
            path.push(c.as_os_str());
        }
        path
    };
    let text = path
        .to_str()
        .ok_or_else(|| Error::Unsupported("Music path is not valid Unicode".into()))?;
    #[cfg(unix)]
    if text.contains('\\') {
        return Err(Error::Unsupported(
            "Literal backslashes cannot be represented in a portable music path".into(),
        ));
    }
    if text.chars().any(char::is_control) {
        return Err(Error::InvalidEdit(
            "Music path contains a control character".into(),
        ));
    }
    Ok(text.replace('\\', "/"))
}
fn absolute(path: &Path) -> Result<PathBuf, Error> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            other => result.push(other.as_os_str()),
        }
    }
    Ok(result)
}
