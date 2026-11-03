//! Product entry without any gameplay song, chart or fabricated simfile.
use super::*;
use deadsync_editor::{Error, audio_preferences::StartupTarget, recent_songs::Song};

#[derive(Clone, Debug)]
pub enum EditorEntryRequest {
    Startup,
    Empty,
    Explicit(PathBuf),
    Recent(Song),
}
impl State {
    pub fn without_song(resource_base: &Path) -> Result<Self, Error> {
        let mut state = Self::from_workspace(Workspace::new_song(resource_base, 0.009)?)?;
        state.document_presence = DocumentPresence::NoSong;
        state.refresh();
        if state.status == i18n::tr("Edition", "Ready") {
            state.status = i18n::tr("Edition", "FileNoSongHelp");
        }
        Ok(state)
    }
    pub fn prepare_entry(resource_base: &Path, request: EditorEntryRequest) -> Result<Self, Error> {
        let mut state = Self::without_song(resource_base)?;
        let target = match request {
            EditorEntryRequest::Empty => Ok(StartupTarget::New),
            EditorEntryRequest::Explicit(path) => Ok(StartupTarget::Explicit(path)),
            EditorEntryRequest::Recent(song) => Ok(StartupTarget::Recent(song)),
            EditorEntryRequest::Startup => state
                .audio_preferences_file
                .as_ref()
                .map(|preferences| preferences.startup_target(None))
                .transpose()
                .map(|target| target.unwrap_or(StartupTarget::New)),
        };
        match target {
            Ok(StartupTarget::New) => {} // No launch target; File/New stays a true Untitled.
            Ok(StartupTarget::Explicit(path)) => state.begin_entry_open(path, None),
            Ok(StartupTarget::Recent(song)) => {
                state.begin_entry_open(song.file.clone(), Some(song))
            }
            Err(error) => {
                state.status = i18n::tr_fmt("Edition", "Error", &[("message", &crate::i18n::editor_error(&error))])
            }
        }
        Ok(state)
    }
    /// Drain transport mutations before accepting a snapshot of the old request.
    pub fn update_host_audio(
        &mut self,
        snapshot: deadsync_gameplay::GameplayAudioSnapshot,
    ) -> Vec<deadsync_gameplay::GameplayAudioCommand> {
        use deadsync_gameplay::GameplayAudioCommand;
        self.update_general_startup_graphs();
        self.update();
        let mut commands = self.take_audio_commands();
        let changing_transport = commands.iter().any(|command| {
            matches!(
                command,
                GameplayAudioCommand::PlayMusic { .. }
                    | GameplayAudioCommand::StopMusic
                    | GameplayAudioCommand::SeekMusic(_)
                    | GameplayAudioCommand::SetMusicRate(_)
                    | GameplayAudioCommand::EditorPlayMusic { .. }
                    | GameplayAudioCommand::EditorPlayPreview { .. }
                    | GameplayAudioCommand::EditorResumePreview { .. }
                    | GameplayAudioCommand::EditorSuspendPreview { .. }
                    | GameplayAudioCommand::EditorRegisterAsset { .. }
                    | GameplayAudioCommand::EditorSwapMusicAsset { .. }
                    | GameplayAudioCommand::EditorStartMusic { .. }
                    | GameplayAudioCommand::EditorClearMusicAsset { .. }
                    | GameplayAudioCommand::EditorPauseMusic { .. }
                    | GameplayAudioCommand::EditorResumeMusic { .. }
                    | GameplayAudioCommand::EditorStopMusic { .. }
                    | GameplayAudioCommand::EditorSeekMusic { .. }
                    | GameplayAudioCommand::EditorSetMusicRate { .. }
                    | GameplayAudioCommand::EditorEndAudio { .. }
            )
        });
        // New commands have not reached the backend, so this snapshot still belongs
        // to the preceding request even if its timestamp was sampled just now.
        if !changing_transport {
            self.update_audio(snapshot);
        }
        commands.extend(self.take_audio_commands());
        commands
    }
    pub fn host_close_pending(&self) -> bool {
        self.entry_close_pending() || self.audio_output_close_pending()
    }
    pub fn has_open_song(&self) -> bool {
        self.has_song()
    }
    pub fn request_host_close(&mut self) -> bool {
        self.request_file_close()
    }
    pub fn take_host_close(&mut self) -> bool {
        self.take_editor_audio_close()
    }
    pub fn release_host_audio(&mut self) {
        self.suspend_performance_frame(std::time::Instant::now());
        self.release_editor_audio_scope();
    }
    pub fn request_host_application_exit(&mut self) -> ApplicationExit {
        self.request_application_exit()
    }
    pub fn take_host_application_exit(&mut self) -> Option<ApplicationExit> {
        self.take_application_exit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_entry_needs_no_gameplay_or_song_and_creates_no_files_en_fr() {
        for language in ["en", "fr"] {
            crate::i18n::init_for_tests();
            crate::i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            let root = tempfile::tempdir().unwrap();
            let mut state = State::prepare_entry(root.path(), EditorEntryRequest::Empty).unwrap();
            assert!(!state.has_open_song());
            assert_eq!(state.document_presence, DocumentPresence::NoSong);
            assert!(state.workspace.file().existing_path().is_none());
            assert!(state.workspace.active_chart().is_none());
            assert!(state.capture_recent_song().unwrap().is_none());
            state.autosave();
            assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
            assert_eq!(state.chart_label, i18n::tr("Edition", "FileNoSong"));
            state.command_file_new();
            assert!(state.has_open_song());
            assert!(state.workspace.file().existing_path().is_none());
        }
    }
    #[test]
    fn entry_explicit_uses_guarded_worker_and_retains_empty_state_until_publish() {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let missing = root.path().join("missing.ssc");
        let mut state =
            State::prepare_entry(root.path(), EditorEntryRequest::Explicit(missing)).unwrap();
        assert!(!state.has_open_song());
        assert!(state.workspace.file().existing_path().is_none());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        struct NoSort;
        impl deadsync_editor::chart_order::Orderer for NoSort {
            fn order(
                &mut self,
                _: &deadsync_editor::chart_order::Snapshot,
            ) -> Result<Vec<usize>, Error> {
                panic!("missing file has no charts to order")
            }
        }
        while state.file_io_active() && std::time::Instant::now() < deadline {
            state.process_file_menu(&mut NoSort);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(!state.file_io_active());
        loop {
            if let Ok(permit) = super::super::file_open_permit::Permit::acquire() {
                drop(permit);
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "worker permit was not released"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(!state.has_open_song());
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    }
}

#[cfg(test)]
mod host_close_tests {
    use super::*;
    #[test]
    fn close_intent_is_distinct_from_file_open_and_cancellation_revokes_it_en_fr() {
        crate::i18n::init_for_tests();
        for language in ["en", "fr"] {
            crate::i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            let root = tempfile::tempdir().unwrap();
            let mut state = State::prepare_entry(root.path(), EditorEntryRequest::Empty).unwrap();
            state.audio_preferences_file = None;
            assert!(!state.host_close_pending());
            state.command_file_new();
            assert!(!state.request_host_close());
            assert!(state.host_close_pending());
            assert!(matches!(state.panel, Some(Panel::FileUnsaved(_))));
            state.file_menu_key(0, KeyCode::Escape);
            assert!(!state.host_close_pending());
            assert!(!state.take_host_close());
            assert!(state.has_open_song());
            assert!(state.workspace.file().has_unsaved_document());
        }
    }
}
