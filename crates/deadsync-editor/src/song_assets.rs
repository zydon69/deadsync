//! GrooveAuthor song asset discovery, preserving the caller's file order.
//!
//! Rules ported from EditorSongImageUtils.cs at GrooveAuthor 48e18f71655d.
//! Dimension decoding is injected so this domain has no graphics dependency.
use crate::{
    Error, music_file,
    song_text::{Field, Text},
};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageType {
    Background,
    Banner,
    CdTitle,
    Jacket,
    CdImage,
    DiscImage,
}
impl ImageType {
    pub const ALL: [Self; 6] = [
        Self::Background,
        Self::Banner,
        Self::CdTitle,
        Self::Jacket,
        Self::CdImage,
        Self::DiscImage,
    ];
    pub fn field(self) -> Field {
        match self {
            Self::Background => Field::Background,
            Self::Banner => Field::Banner,
            Self::CdTitle => Field::CdTitle,
            Self::Jacket => Field::Jacket,
            Self::CdImage => Field::CdImage,
            Self::DiscImage => Field::DiscImage,
        }
    }
    fn matches_name(self, name: &str) -> bool {
        match self {
            Self::Background => name.contains("background") || name.ends_with("bg"),
            Self::Banner => name.contains("banner") || name.ends_with(" bn"),
            Self::CdTitle => name.contains("cdtitle"),
            Self::Jacket => {
                name.starts_with("jk_") || name.contains("jacket") || name.contains("albumart")
            }
            Self::CdImage => name.ends_with("-cd"),
            Self::DiscImage => name.ends_with(" disc") || name.ends_with(" title"),
        }
    }
    fn expected(self) -> Option<(i32, i32)> {
        match self {
            Self::Background => Some((640, 480)),
            Self::Banner => Some((418, 164)),
            _ => None,
        }
    }
}

/// Enumerate direct children only; neither recurse nor sort. GrooveAuthor uses
/// Directory.GetFiles order, which is unspecified and must not become a score.
pub fn files(directory: &Path) -> std::io::Result<Vec<PathBuf>> {
    std::fs::read_dir(directory)?
        .filter_map(|entry| match entry {
            Ok(entry) => match entry.file_type() {
                Ok(kind) if kind.is_file() => Some(Ok(entry.path())),
                Ok(kind) if kind.is_symlink() && !entry.path().is_dir() => Some(Ok(entry.path())),
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            },
            Err(error) => Some(Err(error)),
        })
        .collect()
}
fn name(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_lowercase()
}
fn extension(path: &Path) -> &str {
    path.extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
}
fn aspect((w, h): (i32, i32), (ew, eh): (i32, i32)) -> bool {
    w != 0
        && h != 0
        && ew != 0
        && eh != 0
        && ((w as f32 / h as f32) - (ew as f32 / eh as f32)).abs() < 0.05_f32
}

/// Find one image using names, exact dimensions, first matching aspect ratio,
/// background's largest qualifying image, then background's first video.
/// Probe failures match the original decoder fallback dimensions (1, 1).
/// Extension matching intentionally remains case-sensitive, as in the reference.
pub fn find_image<F>(
    kind: ImageType,
    directory: &Path,
    files: &[PathBuf],
    mut dimensions: F,
) -> Option<String>
where
    F: FnMut(&Path) -> Option<(i32, i32)>,
{
    let assets: Vec<_> = files
        .iter()
        .filter(|path| {
            music_file::IMAGE_EXTENSIONS.contains(&extension(path))
                || (kind == ImageType::Background
                    && music_file::VIDEO_EXTENSIONS.contains(&extension(path)))
        })
        .collect();
    if let Some(path) = assets.iter().find(|path| kind.matches_name(&name(path))) {
        return music_file::relative_path(directory, path).ok();
    }
    if let Some(expected) = kind.expected() {
        let mut ratio = None;
        let mut biggest = None;
        let mut biggest_size = 0;
        for path in &assets {
            let size = dimensions(path).unwrap_or((1, 1));
            if ratio.is_none() && aspect(size, expected) {
                ratio = Some(*path);
            }
            // C# uses unchecked Int32 multiplication here.
            let area = size.0.wrapping_mul(size.1);
            if area > biggest_size && size.0 >= expected.0 && size.1 >= expected.1 {
                biggest_size = area;
                biggest = Some(*path);
            }
            if size == expected {
                return music_file::relative_path(directory, path).ok();
            }
        }
        if let Some(path) = ratio.or_else(|| {
            if kind == ImageType::Background {
                biggest
            } else {
                None
            }
        }) {
            return music_file::relative_path(directory, path).ok();
        }
    }
    if kind == ImageType::Background {
        if let Some(path) = assets
            .iter()
            .find(|path| music_file::VIDEO_EXTENSIONS.contains(&extension(path)))
        {
            return music_file::relative_path(directory, path).ok();
        }
    }
    None
}

pub fn find_lyrics(directory: &Path, files: &[PathBuf]) -> Option<String> {
    files
        .iter()
        .find(|path| extension(path) == "lrc")
        .and_then(|path| music_file::relative_path(directory, path).ok())
}

/// Classify a dropped image. Earlier categories win even over later name
/// matches: a 4:3 file named jacket.png is classified as a background.
pub fn best_image_type<F>(path: &Path, mut dimensions: F) -> ImageType
where
    F: FnMut(&Path) -> Option<(i32, i32)>,
{
    let name = name(path);
    let size = dimensions(path).unwrap_or((1, 1));
    ImageType::ALL
        .into_iter()
        .find(|kind| {
            kind.matches_name(&name)
                || kind
                    .expected()
                    .is_some_and(|expected| aspect(size, expected))
        })
        .unwrap_or(ImageType::Background)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Assets {
    pub images: Vec<(ImageType, String)>,
    pub lyrics: Option<String>,
}
impl Assets {
    /// Source global auto-assets action: one fresh song transaction, no inspector
    /// draft, and no history entry when every discovered path is already set.
    pub fn apply(&self, session: &mut crate::history::Session, only_unset: bool) -> Result<usize, Error> {
        let mut draft = crate::song_properties::Draft::capture(session)?;
        let count = draft.stage_assets(self, only_unset)?;
        if draft.apply_with_presentation_receipt(session)?.changed() { Ok(count) } else { Ok(0) }
    }
    pub fn discover<F>(directory: &Path, files: &[PathBuf], mut dimensions: F) -> Self
    where
        F: FnMut(&Path) -> Option<(i32, i32)>,
    {
        Self {
            images: ImageType::ALL
                .into_iter()
                .filter_map(|kind| {
                    find_image(kind, directory, files, &mut dimensions).map(|path| (kind, path))
                })
                .collect(),
            lyrics: find_lyrics(directory, files),
        }
    }
    /// Changes for the existing staged text transaction. Unsupported SM tags
    /// are preserved, and absent candidates never clear existing paths.
    pub fn updates(&self, text: &Text, only_unset: bool) -> Vec<(Field, String)> {
        self.images
            .iter()
            .map(|(kind, path)| (kind.field(), path))
            .chain(self.lyrics.iter().map(|path| (Field::Lyrics, path)))
            .filter(|(field, path)| {
                text.supports(*field)
                    && !path.is_empty()
                    && text.get(*field) != path.as_str()
                    && (!only_unset || text.get(*field).is_empty())
            })
            .map(|(field, path)| (field, path.clone()))
            .collect()
    }
    /// Validate the full candidate set before changing the draft.
    pub fn stage(&self, text: &mut Text, only_unset: bool) -> Result<usize, Error> {
        let updates = self.updates(text, only_unset);
        if updates
            .iter()
            .any(|(_, path)| path.chars().any(char::is_control))
        {
            return Err(Error::InvalidEdit(
                "Discovered asset path contains a control character".into(),
            ));
        }
        for (field, path) in &updates {
            text.set(*field, path)?;
        }
        Ok(updates.len())
    }
}
