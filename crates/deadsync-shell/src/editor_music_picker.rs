use deadsync_editor::{
    Error,
    music_file::{Browse, Kind, Picker},
};
use deadsync_theme_simply_love::i18n;
use std::path::PathBuf;
pub(crate) struct NativeMusicPicker;
impl Picker for NativeMusicPicker {
    fn choose(&mut self, request: &Browse) -> Result<Option<PathBuf>, Error> {
        let directory = initial_directory(request);
        let (title, filter) = match request.kind {
            Kind::Audio => ("MusicPickerTitle", "MusicPickerAudio"),
            Kind::Image => ("MediaPickerTitle", "MediaPickerImage"),
            Kind::ImageOrVideo => ("MediaPickerTitle", "MediaPickerImageOrVideo"),
            Kind::Video => ("MediaPickerTitle", "MediaPickerVideo"),
            Kind::Lyrics => ("MediaPickerTitle", "MediaPickerLyrics"),
        };
        let title = i18n::tr("Edition", title);
        let filter = i18n::tr("Edition", filter);
        let all = i18n::tr("Edition", "MusicPickerAll");
        Ok(rfd::FileDialog::new()
            .set_title(&*title)
            .set_directory(directory)
            .add_filter(&*filter, &request.kind.extensions())
            .add_filter(&*all, &["*"])
            .pick_file())
    }
}

fn initial_directory(request: &Browse) -> PathBuf {
    let current = request.directory.join(request.current.replace('\\', "/"));
    if request.current.is_empty() || request.current.chars().any(char::is_control) {
        request.directory.clone()
    } else {
        current
            .parent()
            .filter(|p| p.is_dir())
            .unwrap_or(&request.directory)
            .to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::initial_directory;
    use deadsync_editor::music_file::Browse;
    #[test]
    fn picker_starts_in_song_directory_or_current_audio_folder_without_opening_a_dialog() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("audio");
        std::fs::create_dir(&sub).unwrap();
        let request = |current: &str| Browse {
            kind: deadsync_editor::music_file::Kind::Audio,
            directory: dir.path().to_path_buf(),
            current: current.into(),
        };
        assert_eq!(initial_directory(&request("")), dir.path());
        assert_eq!(initial_directory(&request("audio/song.ogg")), sub);
        assert_eq!(initial_directory(&request("audio\\song.ogg")), sub);
        assert_eq!(initial_directory(&request("missing/song.ogg")), dir.path());
        assert_eq!(initial_directory(&request("invalid\0")), dir.path());
    }
}
