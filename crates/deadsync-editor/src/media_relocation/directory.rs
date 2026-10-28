//! Explicit resource-directory relocation for native Untitled documents.
//! Unlike source rebase(None), this API intentionally preserves the resource
//! base chosen by the native editor. It never invents an existing simfile.
use super::*;

pub fn rebase_from_directory(
    old_directory: &Path,
    new_file: &Path,
    media: &str,
) -> Result<String, Error> {
    if media.is_empty() {
        return Ok(String::new());
    }
    if media.contains('\0') {
        return Err(invalid("Media reference contains a null character"));
    }
    if !old_directory.is_absolute() {
        return Err(invalid(
            "Media relocation requires an absolute resource directory",
        ));
    }
    let new_parent = absolute_parent(new_file)?;
    let path = Path::new(media);
    let target = if path.is_absolute() {
        normalized(path)
    } else {
        normalized(&old_directory.join(path))
    };
    relative_target(&new_parent, &target)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(unix)]
    fn untitled_relative_resources_keep_the_chosen_directory_without_a_fake_old_file() {
        assert_eq!(
            rebase_from_directory(
                Path::new("/pack/old"),
                Path::new("/pack/new/song.ssc"),
                "audio/music.ogg"
            )
            .unwrap(),
            "../old/audio/music.ogg"
        );
        assert_eq!(
            rebase_from_directory(
                Path::new("/pack/old"),
                Path::new("/pack/new/song.ssc"),
                "../shared/music.ogg"
            )
            .unwrap(),
            "../shared/music.ogg"
        );
        assert_eq!(
            rebase_from_directory(
                Path::new("/pack/old"),
                Path::new("/pack/new/song.ssc"),
                "/pack/shared/music.ogg"
            )
            .unwrap(),
            "../shared/music.ogg"
        );
        assert_eq!(
            super::super::rebase(None, Path::new("/pack/new/song.ssc"), "audio/music.ogg").unwrap(),
            "audio/music.ogg",
            "original source None behavior must stay unchanged"
        );
    }
    #[test]
    fn invalid_directory_and_destination_never_fall_back_to_a_placeholder_path() {
        let dir = tempfile::tempdir().unwrap();
        let new_file = dir.path().join("song.ssc");
        assert!(rebase_from_directory(Path::new("relative"), &new_file, "music.ogg").is_err());
        assert!(rebase_from_directory(dir.path(), Path::new("relative.ssc"), "music.ogg").is_err());
        assert!(rebase_from_directory(dir.path(), &new_file, "invalid\0music.ogg").is_err());
    }
}
