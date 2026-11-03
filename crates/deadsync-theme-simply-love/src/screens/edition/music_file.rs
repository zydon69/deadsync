use super::State;
use crate::i18n;
use deadsync_editor::music_file::{self, Browse, Picker};
pub(super) struct Request {
    serial: u64,
    target: super::text_fields::Target,
    directory: std::path::PathBuf,
    pack: Option<super::pack_properties::CouplingGuard>,
}
pub(super) struct Pending {
    request: Request,
    receiver: std::sync::mpsc::Receiver<Result<Option<std::path::PathBuf>, deadsync_editor::Error>>,
}
impl State {
    pub(super) fn queue_music_picker(&mut self) {
        if let Some(target) = self.text_target().filter(|t| t.is_file()) {
            let is_pack = matches!(target, super::text_fields::Target::Pack(_));
            let directory = if is_pack {
                let Some(directory) = self.pack_media_directory() else {
                    return;
                };
                directory
            } else {
                self.workspace.file().resource_base().to_path_buf()
            };
            self.music_picker_request = Some(Request {
                serial: self.text_edit_serial,
                target,
                directory,
                pack: if is_pack {
                    self.capture_pack_coupling()
                } else {
                    None
                },
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
    fn discover_song_assets(&self) -> std::io::Result<deadsync_editor::song_assets::Assets> {
        use deadsync_editor::song_assets;
        let directory = self.workspace.file().resource_base();
        let files = song_assets::files(directory)?;
        Ok(song_assets::Assets::discover(directory, &files, |path| {
            let (width, height) = image::image_dimensions(path).ok()?;
            Some((i32::try_from(width).ok()?, i32::try_from(height).ok()?))
        }))
    }
    pub(super) fn command_auto_song_assets(&mut self, only_unset: bool) {
        if !self.has_song() {
            self.status = i18n::tr("Edition", "FileNoSongHelp");
            return;
        }
        // The original action treats directory errors as an empty discovery.
        let assets = self.discover_song_assets().unwrap_or_default();
        let result = (|| -> Result<usize, deadsync_editor::Error> {
            let mut draft = self.workspace.song_properties()?;
            let count = draft.stage_assets(&assets, only_unset)?;
            if count == 0 {
                return Ok(0);
            }
            self.commit_song_properties_presented(&draft)?;
            Ok(count)
        })();
        match result {
            Ok(count) => {
                self.refresh();
                self.status = if count == 0 {
                    i18n::tr("Edition", "SongAssetsUnchanged")
                } else {
                    i18n::tr_fmt(
                        "Edition",
                        "SongAssetsApplied",
                        &[("count", &count.to_string())],
                    )
                };
            }
            Err(error) => {
                self.status = i18n::tr_fmt(
                    "Edition",
                    "Error",
                    &[("message", &crate::i18n::editor_error(&error))],
                )
            }
        }
    }

    pub(super) fn find_all_song_media(&mut self, only_unset: bool) {
        let assets = match self.discover_song_assets() {
            Ok(assets) => assets,
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
                Err(error) => i18n::tr_fmt(
                    "Edition",
                    "Error",
                    &[("message", &crate::i18n::editor_error(&error))],
                ),
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
        let directory = self.workspace.file().resource_base();
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
        let (request, result) = if let Some(pending) = self.music_picker_pending.take() {
            match pending.receiver.try_recv() {
                Ok(result) => (pending.request, Some(result)),
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    self.music_picker_pending = Some(pending);
                    return false;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => (
                    pending.request,
                    Some(Err(deadsync_editor::Error::InvalidEdit(
                        "Media picker worker stopped".into(),
                    ))),
                ),
            }
        } else {
            let Some(request) = self.music_picker_request.take() else {
                return false;
            };
            (request, None)
        };
        if request.serial != self.text_edit_serial
            || self.text_target() != Some(request.target)
            || !self.label_preedit.is_empty()
            || (matches!(request.target, super::text_fields::Target::Pack(_))
                && (!self.pack_coupling_current(&request.pack)
                    || self.pack_media_directory().as_ref() != Some(&request.directory)))
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
            directory: request.directory.clone(),
            current,
        };
        let result = match result {
            Some(result) => result,
            None => match picker.begin(&browse) {
                music_file::Selection::Ready(result) => result,
                music_file::Selection::Pending(receiver) => {
                    self.music_picker_pending = Some(Pending { request, receiver });
                    return false;
                }
            },
        };
        let result = result.and_then(|path| {
            path.map(|path| music_file::selected_path_from_directory(&request.directory, &path))
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
        if matches!(request.target, super::text_fields::Target::Pack(_)) {
            self.commit_pack_field();
        }
        true
    }
}

#[cfg(test)]
mod untitled_resources_tests {
    use super::*;
    fn state(base: &std::path::Path) -> State {
        State::from_workspace(deadsync_editor::workspace::Workspace::new_song(base, 0.009).unwrap())
            .unwrap()
    }
    struct AsyncPicker(
        Option<
            std::sync::mpsc::Receiver<Result<Option<std::path::PathBuf>, deadsync_editor::Error>>,
        >,
    );
    impl Picker for AsyncPicker {
        fn choose(
            &mut self,
            _: &Browse,
        ) -> Result<Option<std::path::PathBuf>, deadsync_editor::Error> {
            panic!("synchronous selection must not run")
        }
        fn begin(&mut self, _: &Browse) -> music_file::Selection {
            music_file::Selection::Pending(self.0.take().unwrap())
        }
    }
    #[test]
    fn pending_picker_returns_immediately_and_revalidates_the_target() {
        for stale in [false, true] {
            crate::i18n::init_for_tests();
            let root = tempfile::tempdir().unwrap();
            let mut state = state(root.path());
            let before_revision = state.workspace.file().session.revision();
            let image = root.path().join("banner.png");
            std::fs::write(&image, []).unwrap();
            state.open_song_properties();
            state.open_song_text();
            let index = deadsync_editor::song_text::Field::ALL
                .iter()
                .position(|f| *f == deadsync_editor::song_text::Field::Banner)
                .unwrap();
            state.panel = Some(super::super::Panel::SongText(index));
            state.focus_text();
            state.queue_music_picker();
            assert!(state.music_picker_request.is_some());
            let (sender, receiver) = std::sync::mpsc::channel();
            let mut picker = AsyncPicker(Some(receiver));
            assert!(!state.process_music_picker(&mut picker));
            assert!(!state.process_music_picker(&mut picker));
            assert!(state.music_picker_pending.is_some());
            if stale {
                state.text_edit_serial = state.text_edit_serial.wrapping_add(1);
            }
            sender.send(Ok(Some(image.clone()))).unwrap();
            assert_eq!(state.process_music_picker(&mut picker), !stale);
            let value = state.text_buffer.as_ref().unwrap().value();
            if stale {
                assert_eq!(value, "");
            } else {
                assert_eq!(
                    state
                        .workspace
                        .file()
                        .resource_base()
                        .join(value)
                        .canonicalize()
                        .unwrap(),
                    image.canonicalize().unwrap()
                );
            }
            assert!(state.music_picker_pending.is_none());
            assert_eq!(state.workspace.file().session.revision(), before_revision);
        }
    }
    #[test]
    fn untitled_pack_operations_need_first_save_and_create_no_placeholder() {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let mut state = state(root.path());
        state.open_pack_song_list();
        assert!(state.pack_song_list.is_none());
        state.open_pack_properties();
        assert!(state.pack_properties.is_none());
        assert!(state.workspace.file().existing_path().is_none());
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    }
    #[test]
    fn untitled_save_as_prefills_resource_directory_without_inventing_filename() {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let mut state = state(root.path());
        state.open_save_as();
        assert!(matches!(
            state.panel,
            Some(super::super::Panel::SaveAsDestination(_))
        ));
        assert_eq!(
            std::path::Path::new(&state.save_as.destination),
            state.workspace.file().resource_base()
        );
        assert!(
            state
                .save_as
                .destination
                .ends_with(std::path::MAIN_SEPARATOR)
        );
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    }
    #[test]
    fn untitled_banner_preview_uses_real_resource_directory() {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let mut state = state(root.path());
        let mut draft = state.workspace.song_properties().unwrap();
        draft
            .set_text(deadsync_editor::song_text::Field::Banner, "art/banner.png")
            .unwrap();
        state.song_properties = Some(draft);
        state.panel = Some(super::super::Panel::SongText(0));
        let preview = state.image_preview_request().unwrap();
        assert_eq!(
            preview.paths[0],
            Some(root.path().canonicalize().unwrap().join("art/banner.png"))
        );
        assert!(preview.paths[1].is_none());
        assert!(state.workspace.file().existing_path().is_none());
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    }
}

#[cfg(test)]
#[path = "source_assets_tests.rs"]
mod source_assets_tests;
