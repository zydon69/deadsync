use super::State;
use crate::i18n;
use deadsync_editor::music_file::{self, Browse, Picker};
pub(super) struct Request {
    serial: u64,
    target: super::text_fields::Target,
}
impl State {
    pub(super) fn queue_music_picker(&mut self) {
        if let Some(target) = self.text_target().filter(|t| t.is_file()) {
            self.music_picker_request = Some(Request {
                serial: self.text_edit_serial,
                target,
            });
        }
    }
    pub(super) fn clear_music(&mut self) {
        if let Some(buffer) = self.text_buffer.as_mut() {
            buffer.select_all();
            buffer.delete(false);
        }
        if self.sync_text() {
            self.status = i18n::tr(
                "Edition",
                if self.text_target().is_some_and(|t| !t.is_music()) {
                    "SongAssetCleared"
                } else if self.text_target().is_some_and(|t| t.is_preview()) {
                    "SongPreviewMusicCleared"
                } else if self.text_target().is_some_and(|t| t.is_song()) {
                    "SongMusicCleared"
                } else {
                    "ChartMusicCleared"
                },
            );
        }
    }
    pub(super) fn find_all_song_media(&mut self, only_unset: bool) {
        use deadsync_editor::song_assets;
        let directory = self
            .workspace
            .file()
            .path()
            .parent()
            .unwrap_or(std::path::Path::new("."));
        let assets = match song_assets::files(directory) {
            Ok(files) => song_assets::Assets::discover(directory, &files, |path| {
                let (width, height) = image::image_dimensions(path).ok()?;
                Some((i32::try_from(width).ok()?, i32::try_from(height).ok()?))
            }),
            Err(_) => {
                self.status = i18n::tr("Edition", "SongAssetNotFound");
                return;
            }
        };
        if let Some(draft) = self.song_properties.as_mut() {
            self.status = match draft.stage_assets(&assets, only_unset) {
                Ok(0) => i18n::tr("Edition", "SongAssetsUnchanged"),
                Ok(count) => i18n::tr_fmt(
                    "Edition",
                    "SongAssetsStaged",
                    &[("count", &count.to_string())],
                ),
                Err(error) => i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())]),
            };
        }
    }
    pub(super) fn find_song_media(&mut self) -> bool {
        use deadsync_editor::{song_assets, song_text::Field};
        let Some(super::text_fields::Target::Song(field)) = self.text_target() else {
            return false;
        };
        let kind = song_assets::ImageType::ALL
            .into_iter()
            .find(|kind| kind.field() == field);
        if kind.is_none() && field != Field::Lyrics {
            return false;
        }
        if !self.sync_text() {
            return true;
        }
        let directory = self
            .workspace
            .file()
            .path()
            .parent()
            .unwrap_or(std::path::Path::new("."));
        let found = song_assets::files(directory).ok().and_then(|files| {
            if let Some(kind) = kind {
                song_assets::find_image(kind, directory, &files, |path| {
                    let (width, height) = image::image_dimensions(path).ok()?;
                    Some((i32::try_from(width).ok()?, i32::try_from(height).ok()?))
                })
            } else {
                song_assets::find_lyrics(directory, &files)
            }
        });
        if let Some(path) = found {
            if let Some(buffer) = self.text_buffer.as_mut() {
                buffer.select_all();
            }
            self.edit_text_input(&path);
            self.status = i18n::tr("Edition", "SongAssetStaged");
        } else {
            self.status = i18n::tr("Edition", "SongAssetNotFound");
        }
        true
    }
    pub fn process_music_picker(&mut self, picker: &mut impl Picker) -> bool {
        let Some(request) = self.music_picker_request.take() else {
            return false;
        };
        if request.serial != self.text_edit_serial
            || self.text_target() != Some(request.target)
            || !self.label_preedit.is_empty()
        {
            return false;
        }
        let current = self
            .text_buffer
            .as_ref()
            .map(|b| b.value().to_owned())
            .unwrap_or_default();
        let browse = Browse {
            kind: request.target.file_kind().expect("file selection target"),
            directory: self
                .workspace
                .file()
                .path()
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .to_path_buf(),
            current,
        };
        let result = picker.choose(&browse).and_then(|path| {
            path.map(|path| music_file::selected_path(self.workspace.file().path(), &path))
                .transpose()
        });
        match result {
            Ok(Some(path)) => {
                if let Some(buffer) = self.text_buffer.as_mut() {
                    buffer.select_all();
                }
                self.edit_text_input(&path);
                self.status = i18n::tr(
                    "Edition",
                    if !request.target.is_music() {
                        "SongAssetStaged"
                    } else if request.target.is_preview() {
                        "SongPreviewMusicStaged"
                    } else if request.target.is_song() {
                        "SongMusicStaged"
                    } else {
                        "ChartMusicStaged"
                    },
                );
            }
            Ok(None) => {
                self.status = i18n::tr(
                    "Edition",
                    if request.target.is_music() {
                        "ChartMusicCancelled"
                    } else {
                        "MediaPickerCancelled"
                    },
                )
            }
            Err(_) => {
                self.status = i18n::tr(
                    "Edition",
                    if request.target.is_music() {
                        "ChartMusicInvalid"
                    } else {
                        "MediaPickerInvalid"
                    },
                )
            }
        }
        true
    }
}
