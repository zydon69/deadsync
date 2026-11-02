//! File explorer requests use the existing argument-based platform API.
use super::*;
impl State {
    pub(super) fn initial_file_destination(&self) -> Result<String, deadsync_editor::Error> {
        let selected = self
            .application_preferences_owner()
            .map(|file| file.initial_file_directory())
            .transpose()?
            .flatten();
        let mut directory =
            selected.unwrap_or_else(|| self.workspace.file().resource_base().to_owned());
        if !directory.is_absolute() {
            directory = self.workspace.file().resource_base().join(directory);
        }
        directory.push("");
        directory.to_str().map(str::to_owned).ok_or_else(|| {
            deadsync_editor::Error::Unsupported("The initial file directory is not Unicode".into())
        })
    }
    /// Confirmed File/Open changes the next dialog directory even if loading
    /// subsequently fails. A pathless owner stays memory-only and participates
    /// in source-preference history authentication, never acquiring a disk path.
    pub(super) fn remember_open_dialog_directory(&mut self,path:&Path)->Result<(),deadsync_editor::Error>{
        let directory=path.parent().ok_or_else(||deadsync_editor::Error::InvalidEdit("Selected file has no parent directory".into()))?;
        if let Some(file)=self.audio_preferences_file.as_mut(){file.set_initial_file_directory(directory)?;}
        else{if self.source_preferences_import.memory.is_none(){self.source_preferences_import.memory=Some(deadsync_editor::audio_preferences::File::memory()?);}self.source_preferences_import.memory.as_mut().unwrap().set_initial_file_directory(directory)?;}
        Ok(())
    }
    pub(super) fn command_open_containing_folder(&mut self) {
        let result = self.open_containing_folder_with(deadlib_platform::open_path::reveal);
        self.status = i18n::tr(
            "Edition",
            if result.is_ok() {
                "FileFolderRequested"
            } else {
                "FileFolderUnavailable"
            },
        );
        if let Err(error) = result {
            log::warn!("Editor containing folder: {error}");
        }
    }
    fn open_containing_folder_with(
        &self,
        reveal: impl FnOnce(&Path) -> std::io::Result<()>,
    ) -> Result<(), deadsync_editor::Error> {
        let file = self.workspace.file().require_existing_path()?;
        let directory = file
            .parent()
            .filter(|path| path.is_absolute() && path.is_dir())
            .ok_or_else(|| {
                deadsync_editor::Error::InvalidEdit("The song directory is unavailable".into())
            })?;
        reveal(directory)?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn state() -> (tempfile::TempDir, State) {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let song = root.path().join("Song $() 'quoted' space");
        std::fs::create_dir(&song).unwrap();
        let path = song.join("song.ssc");
        std::fs::write(&path, b"#VERSION:0.83;#TITLE:Quoted;").unwrap();
        let state = State::open(&path, 0).unwrap();
        (root, state)
    }
    #[test]
    fn folder_request_passes_one_exact_existing_absolute_directory_and_preserves_edits() {
        let (_root, state) = state();
        let expected = state
            .workspace
            .file()
            .require_existing_path()
            .unwrap()
            .parent()
            .unwrap()
            .to_owned();
        let before = state.workspace.file().session.serialize().unwrap();
        let mut actual = None;
        state
            .open_containing_folder_with(|path| {
                actual = Some(path.to_owned());
                Ok(())
            })
            .unwrap();
        assert_eq!(actual, Some(expected));
        assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
    }
    #[test]
    fn untitled_never_fabricates_or_creates_a_folder_request() {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let state =
            State::from_workspace(Workspace::new_song(root.path(), 0.009).unwrap()).unwrap();
        let mut called = false;
        assert!(
            state
                .open_containing_folder_with(|_| {
                    called = true;
                    Ok(())
                })
                .is_err()
        );
        assert!(!called);
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    }
    #[test]
    fn initial_directory_prefills_open_and_untitled_save_as_without_writing_en_fr() {
        for language in ["en", "fr"] {
            i18n::init(deadsync_assets::language::load_for_tests(language));
            let (root, mut state) = state();
            i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            let path = root.path().join("prefs.json");
            let selected = root.path().join("Not created");
            std::fs::write(
                &path,
                serde_json::to_vec(
                    &serde_json::json!({"Version":1,"OpenFileDialogInitialDirectory":selected}),
                )
                .unwrap(),
            )
            .unwrap();
            state.audio_preferences_file =
                Some(deadsync_editor::audio_preferences::File::open(&path).unwrap());
            state.command_file_open();
            assert_eq!(Path::new(&state.file_menu.destination), selected.as_path());
            assert!(!selected.exists());
            let owner = state.audio_preferences_file.take();
            let mut new =
                State::from_workspace(Workspace::new_song(root.path(), 0.009).unwrap()).unwrap();
            new.audio_preferences_file = owner;
            new.open_save_as();
            assert_eq!(Path::new(&new.save_as.destination), selected.as_path());
            assert!(!selected.exists());
        }
    }
}
