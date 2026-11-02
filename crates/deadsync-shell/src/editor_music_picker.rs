use deadsync_editor::{
    Error,
    music_file::{Browse, Kind, Picker},
};
use deadsync_theme_simply_love::i18n;
use std::path::PathBuf;
pub(crate) struct NativeMusicPicker;
impl Picker for NativeMusicPicker {
    fn begin(&mut self, request: &Browse) -> deadsync_editor::music_file::Selection {
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
        // Construct the native asynchronous sheet on the application thread.
        // Only waiting for its result moves to the worker; runModal never runs
        // on the render/event loop, and Cocoa objects are created on main.
        let future = rfd::AsyncFileDialog::new()
            .set_title(&*title)
            .set_directory(directory)
            .add_filter(&*filter, &request.kind.extensions())
            .add_filter(&*all, &["*"])
            .pick_file();
        let (sender, receiver) = std::sync::mpsc::channel();
        match std::thread::Builder::new()
            .name("deadsync-media-picker".into())
            .spawn(move || {
                let result = pollster::block_on(future).map(|file| file.path().to_owned());
                let _ = sender.send(Ok(result));
            }) {
            Ok(_) => deadsync_editor::music_file::Selection::Pending(receiver),
            Err(error) => deadsync_editor::music_file::Selection::Ready(Err(Error::Io(error))),
        }
    }
    fn choose(&mut self, _: &Browse) -> Result<Option<PathBuf>, Error> {
        Err(Error::Unsupported(
            "Native selection requires the asynchronous picker API".into(),
        ))
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
