//! Guarded keyboard File menu. Parsing prepares a detached candidate; failed or
//! cancelled requests retain the original document, history and settings.
use super::file_controller::{Controller, Phase};
use super::*;
use deadsync_editor::{
    Error, history::file_intent::IntentGuard, recent_song_preferences::Preferences,
    recent_songs::Song,
};
#[derive(Clone)]
enum Action {
    New,
    Open { path: PathBuf, saved: Option<Song> },
    Close,
}
#[derive(Default)]
pub(super) struct Ui {
    pub(super) destination: String,
    controller: Controller<Action>,
    loading: Option<pack_song_list::file_open::Job>,
    prepared: Option<pack_song_preference::Prepared>,
    guard: Option<IntentGuard>,
    restore: Option<Song>,
    requested: Option<PathBuf>,
    cancelled: bool,
    recent: Preferences,
    close_ready: bool,
}
pub(super) struct PublishedPreferences {
    runtime:Preferences,
    persisted:Option<Preferences>,
    advanced:Option<deadsync_editor::advanced_export::Options>,
}
impl State {
    pub(super) fn prepare_published_preferences(&self,destination:&Path,advanced:Option<deadsync_editor::advanced_export::Options>)->Result<PublishedPreferences,Error>{
        let song=self.capture_recent_song_at(destination)?;
        let mut recent=self.audio_preferences_file.as_ref().map(|file|file.recent_songs()).unwrap_or(&self.file_menu.recent).clone();
        recent.record(&song)?;recent.validate()?;
        let persisted=self.audio_preferences_file.as_ref().map(|_|recent.clone());
        Ok(PublishedPreferences{runtime:recent,persisted,advanced})
    }
    pub(super) fn publish_output_preferences(&mut self,prepared:PublishedPreferences){
        // Every fallible descriptor/JSON preparation preceded the real output.
        // This runs only after namespace publication, including pending sync.
        if let(Some(file),Some(recent))=(self.audio_preferences_file.as_mut(),prepared.persisted){
            if let Err(error)=file.set_recent_songs(recent){self.file_error(error);self.file_save_cancelled();return;}
            if let Some(options)=prepared.advanced{file.set_advanced_save(options);}
        }
        self.file_menu.recent=prepared.runtime;
    }
    fn file_error(&mut self, error: impl std::fmt::Display) {
        self.status = i18n::tr_fmt("Edition", "FileFailed", &[("message", &error.to_string())]);
    }
    pub(super) fn file_modal(&self) -> bool {
        matches!(
            self.panel,
            Some(
                Panel::FileMenu(_)
                    | Panel::FileOpen(_)
                    | Panel::FileRecent(_)
                    | Panel::FileUnsaved(_)
                    | Panel::FileDiscard
                    | Panel::FileLoading
            )
        )
    }
    pub(super) fn file_io_active(&self) -> bool {
        self.file_menu.loading.is_some() || self.file_menu.prepared.is_some()
    }
    fn file_publish_allowed(&self) -> bool {
        !self.save_as_pending()
            && !self.pack_file_io_active()
            && !self.pack_has_unsaved_changes()
            && !self.pack_has_pending_durability()
            && !self.auto_recovery.is_pending()
            && self.job.is_none()
            && self.catalog_job.is_none()
            && self.analysis_job.is_none()
            && self.preview.is_none()
            && !self.lane_input.model.as_ref().is_some_and(|input| {
                input.drafts().next().is_some() || input.deleted_heads().next().is_some()
            })
    }
    pub(super) fn open_file_menu(&mut self) {
        if self.save_as_pending_guard() {
            return;
        }
        if self.file_io_active() {
            self.status = i18n::tr("Edition", "FileBusy");
            return;
        }
        if self.pack_file_io_active() {
            self.status = i18n::tr("Edition", "FileBusy");
            return;
        }
        self.cancel_file_action();
        self.stop_audio();
        if let Some(file) = &self.audio_preferences_file {
            self.file_menu.recent = file.recent_songs().clone();
        }
        self.panel = Some(Panel::FileMenu(0));
        self.status = i18n::tr("Edition", "FileMenuHelp");
    }
    pub(super) fn command_file_new(&mut self) {
        self.begin_file_action(Action::New);
    }
    pub(super) fn command_file_open(&mut self) {
        self.open_file_menu();
        if matches!(self.panel, Some(Panel::FileMenu(_))) {
            self.file_menu.destination.clear();
            self.panel = Some(Panel::FileOpen(0));
            self.focus_text();
        }
    }
    pub(super) fn command_file_recent(&mut self) {
        self.open_file_menu();
        if matches!(self.panel, Some(Panel::FileMenu(_))) {
            self.panel = Some(Panel::FileRecent(0));
        }
    }
    pub(super) fn command_file_reload(&mut self) {
        self.open_file_menu();
        if !matches!(self.panel, Some(Panel::FileMenu(_))) {
            return;
        }
        let saved = (|| -> Result<Option<Song>, Error> {
            let Some(record) = self.file_menu.recent.entries.first() else {
                // CLI/open-with does not publish MRU until a successful file
                // transition. Explicit native adaptation: reload current.
                return self.capture_recent_song();
            };
            let saved = record.song()?;
            if self
                .workspace
                .file()
                .existing_path()
                .is_some_and(|path| path.as_os_str() == saved.file.as_os_str())
            {
                // Keep live fractional position/zoom when MRU0 is this song.
                return Ok(self.capture_recent_song()?.or(Some(saved)));
            }
            Ok(Some(saved))
        })();
        match saved {
            Ok(Some(song)) => self.begin_file_action(Action::Open {
                path: song.file.clone(),
                saved: Some(song),
            }),
            Ok(None) => {
                self.panel = None;
                self.status = i18n::tr("Edition", "FileReloadUnavailable");
            }
            Err(error) => self.file_error(error),
        }
    }
    fn cancel_file_action(&mut self) {
        self.file_menu.controller.cancel();
        self.file_menu.close_ready = false;
        self.file_menu.cancelled = true;
        if let Some(job) = &self.file_menu.loading {
            job.cancel();
        }
        self.file_menu.prepared = None;
        self.file_menu.guard = None;
        self.file_menu.restore = None;
        self.file_menu.requested = None;
    }
    fn begin_file_action(&mut self, action: Action) {
        if self.save_as_pending_guard() {
            return;
        }
        if !self.file_publish_allowed() || self.file_io_active() {
            self.status = i18n::tr("Edition", "FileBusy");
            return;
        }
        self.stop_audio();
        self.text_buffer = None;
        self.label_preedit.clear();
        self.music_picker_request = None;
        self.text_clipboard_request = None;
        self.text_edit_serial = self.text_edit_serial.wrapping_add(1);
        if let Some(file) = &self.audio_preferences_file {
            self.file_menu.recent = file.recent_songs().clone();
        }
        self.file_menu.cancelled = false;
        self.file_menu
            .controller
            .begin(action, self.workspace.file());
        if self.file_menu.controller.phase() == Some(Phase::Review) {
            self.panel = Some(Panel::FileUnsaved(0));
            self.status = i18n::tr("Edition", "FileUnsavedHelp");
        } else {
            self.run_file_action();
        }
    }
    fn run_file_action(&mut self) {
        let allowed = self.file_publish_allowed();
        let action = match self
            .file_menu
            .controller
            .take_ready(self.workspace.file(), allowed)
        {
            Ok(action) => action,
            Err(error) => {
                self.file_error(error);
                return;
            }
        };
        let Some(action) = action else {
            return;
        };
        match action {
            Action::Close => {
                let song = match self.capture_recent_song() {
                    Ok(song) => song,
                    Err(error) => {
                        self.file_error(error);
                        return;
                    }
                };
                if let Some(file) = self.audio_preferences_file.as_mut() {
                    let before = file.recent_songs().clone();
                    let result = (|| {
                        if let Some(song) = song.as_ref() {
                            file.record_recent_song(song)?;
                        }
                        file.save()?;
                        Ok::<(), Error>(())
                    })();
                    if let Err(error) = result {
                        let _ = file.set_recent_songs(before);
                        self.file_error(error);
                        return;
                    }
                }
                self.file_menu.close_ready = true;
                self.panel = None;
            }
            Action::New => {
                let offset = match self
                    .audio_preferences_file
                    .as_ref()
                    .map(|file| file.new_song_sync_offset())
                    .transpose()
                {
                    Ok(value) => value.unwrap_or(0.009),
                    Err(error) => {
                        self.file_error(error);
                        return;
                    }
                };
                let result = Workspace::new_song(self.workspace.file().resource_base(), offset)
                    .and_then(State::from_workspace)
                    .and_then(|mut next| {
                        pack_song_list::application_preferences::Snapshot::capture(self)
                            .install(&mut next)?;
                        Ok(next)
                    });
                match result {
                    Ok(next) => {
                        if let Err(error) = self.publish_file_candidate(next) {
                            self.file_error(error);
                        }
                    }
                    Err(error) => self.file_error(error),
                }
            }
            Action::Open { path, saved } => {
                let preferences = self
                    .audio_preferences_file
                    .as_ref()
                    .map(|file| file.opening())
                    .transpose()
                    .map(|value| value.unwrap_or_default());
                let result = preferences.and_then(|preferences| {
                    pack_song_list::file_open::Job::spawn(path.clone(), preferences)
                });
                match result {
                    Ok(job) => {
                        self.file_menu.guard =
                            Some(IntentGuard::capture(&self.workspace.file().session));
                        self.file_menu.loading = Some(job);
                        self.file_menu.requested = Some(path);
                        self.file_menu.restore = saved;
                        self.file_menu.cancelled = false;
                        self.panel = Some(Panel::FileLoading);
                        self.status = i18n::tr("Edition", "FileLoading");
                    }
                    Err(error) => self.file_error(error),
                }
            }
        }
    }
    fn publish_file_candidate(&mut self, mut next: State) -> Result<(), Error> {
        if !self.file_publish_allowed() {
            return Err(Error::InvalidEdit("File publication is busy".into()));
        }
        let mut recent = self.file_menu.recent.clone();
        if let Some(song) = self.capture_recent_song()? {
            recent.record(&song)?;
        }
        if let Some(song) = next.capture_recent_song()? {
            recent.record(&song)?;
        }
        if let Some(file) = self.audio_preferences_file.as_mut() {
            file.set_recent_songs(recent.clone())?;
        }
        self.stop_audio();
        next.audio_preferences_file = self.audio_preferences_file.take();
        next.file_menu.recent = recent;
        next.audio_commands.extend(self.take_audio_commands());
        next.status = i18n::tr(
            "Edition",
            if next.workspace.file().requires_save_as() {
                "FileNewCreated"
            } else {
                "FileOpened"
            },
        );
        *self = next;
        Ok(())
    }
    pub(super) fn process_file_menu(
        &mut self,
        orderer: &mut impl deadsync_editor::chart_order::Orderer,
    ) -> bool {
        let result = self.file_menu.loading.as_mut().and_then(|job| job.poll());
        if let Some(result) = result {
            self.file_menu.loading = None;
            match result {
                Ok(prepared) if !self.file_menu.cancelled => {
                    self.file_menu.prepared = Some(prepared)
                }
                Ok(_) => {}
                Err(error) if !self.file_menu.cancelled => {
                    self.panel = Some(Panel::FileMenu(0));
                    self.file_error(error);
                }
                Err(_) => {}
            }
        }
        let Some(prepared) = self.file_menu.prepared.take() else {
            return false;
        };
        if !self
            .file_menu
            .guard
            .as_ref()
            .is_some_and(|guard| guard.is_current(&self.workspace.file().session))
        {
            self.cancel_file_action();
            self.file_error("Source history changed while opening");
            return true;
        }
        let requested = self.file_menu.requested.clone();
        let restore = self.file_menu.restore.clone();
        let result = pack_song_list::file_open::finish(self, prepared, restore.as_ref(), orderer)
            .and_then(|next| {
                if requested
                    .as_ref()
                    .map(|path| path.canonicalize())
                    .transpose()?
                    .as_deref()
                    != next.workspace.file().existing_path()
                {
                    return Err(Error::ExternalChange);
                }
                if next.workspace.file().has_external_change()? {
                    return Err(Error::ExternalChange);
                }
                self.publish_file_candidate(next)
            });
        if let Err(error) = result {
            self.cancel_file_action();
            self.panel = Some(Panel::FileMenu(0));
            self.file_error(error);
        }
        true
    }
    pub(super) fn file_action_written(
        &mut self,
        receipt: &deadsync_editor::storage::RelocationReceipt,
    ) {
        if self.file_menu.controller.phase() == Some(Phase::Saving) {
            if let Err(error) = self
                .file_menu
                .controller
                .relocated(self.workspace.file(), receipt)
            {
                self.file_error(error);
            }
        }
    }
    pub(super) fn file_action_saved(&mut self) {
        if self.file_menu.controller.phase() == Some(Phase::Saving) {
            match self.file_menu.controller.saved(self.workspace.file(), None) {
                Ok(()) => self.run_file_action(),
                Err(error) => self.file_error(error),
            }
        }
    }
    pub(super) fn take_file_close(&mut self) -> bool {
        std::mem::take(&mut self.file_menu.close_ready)
    }
    pub(super) fn request_file_close(&mut self) -> bool {
        self.begin_file_action(Action::Close);
        self.take_file_close()
    }
    pub(super) fn file_save_cancelled(&mut self) {
        self.file_menu.controller.cancel();
        self.file_menu.close_ready = false;
    }
    pub(super) fn file_menu_key(&mut self, index: usize, code: KeyCode) -> bool {
        if code == KeyCode::Escape {
            self.cancel_file_action();
            self.panel = None;
            self.text_buffer = None;
            self.label_preedit.clear();
            self.status = i18n::tr("Edition", "FileCancelled");
            return false;
        }
        if matches!(self.panel, Some(Panel::FileLoading)) {
            return false;
        }
        let count = match self.panel {
            Some(Panel::FileMenu(_)) => 7,
            Some(Panel::FileOpen(_) | Panel::FileUnsaved(_)) => 3,
            Some(Panel::FileRecent(_)) => self.file_menu.recent.entries.len() + 1,
            Some(Panel::FileDiscard) => 1,
            _ => return false,
        };
        if matches!(
            code,
            KeyCode::ArrowUp
                | KeyCode::ArrowDown
                | KeyCode::Tab
                | KeyCode::PageUp
                | KeyCode::PageDown
        ) {
            if !self.sync_text() {
                return false;
            }
            let backwards = matches!(code, KeyCode::ArrowUp | KeyCode::PageUp)
                || (code == KeyCode::Tab && self.shift);
            let next = (index + if backwards { count - 1 } else { 1 }) % count;
            self.panel = Some(match self.panel.unwrap() {
                Panel::FileMenu(_) => Panel::FileMenu(next),
                Panel::FileOpen(_) => Panel::FileOpen(next),
                Panel::FileRecent(_) => Panel::FileRecent(next),
                Panel::FileUnsaved(_) => Panel::FileUnsaved(next),
                _ => Panel::FileDiscard,
            });
            self.focus_text();
            return false;
        }
        if !matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) {
            if matches!(self.panel, Some(Panel::FileOpen(0))) {
                self.text_field_key(0, code);
            }
            return false;
        }
        if !self.sync_text() || !self.label_preedit.is_empty() {
            return false;
        }
        match self.panel {
            Some(Panel::FileMenu(_)) => match index {
                0 => self.command_file_new(),
                1 => self.command_file_open(),
                2 => self.command_file_recent(),
                3 => self.command_file_reload(),
                4 => self.command(Command::Save),
                5 => self.open_save_as(),
                _ => {
                    self.panel = None;
                }
            },
            Some(Panel::FileOpen(_)) => {
                if index <= 1 {
                    let path = PathBuf::from(&self.file_menu.destination);
                    if !path.is_absolute() {
                        self.status = i18n::tr("Edition", "FileAbsolutePathRequired");
                    } else {
                        self.begin_file_action(Action::Open { path, saved: None });
                    }
                } else {
                    self.cancel_file_action();
                    self.panel = None;
                }
            }
            Some(Panel::FileRecent(_)) => {
                if let Some(record) = self.file_menu.recent.entries.get(index) {
                    match record.song() {
                        Ok(song) => self.begin_file_action(Action::Open {
                            path: song.file.clone(),
                            saved: Some(song),
                        }),
                        Err(error) => self.file_error(error),
                    }
                } else {
                    self.panel = None;
                }
            }
            Some(Panel::FileUnsaved(_)) => match index {
                1 => {
                    if !self.file_publish_allowed() {
                        self.status = i18n::tr("Edition", "FileBusy");
                        return false;
                    }
                    if let Err(error) = self.file_menu.controller.choose_save(self.workspace.file())
                    {
                        self.file_error(error);
                        return false;
                    }
                    if self.workspace.file().requires_save_as() {
                        self.open_save_as();
                    } else {
                        self.output_save_current(false);
                    }
                }
                2 => self.panel = Some(Panel::FileDiscard),
                _ => {
                    self.cancel_file_action();
                    self.panel = None;
                }
            },
            Some(Panel::FileDiscard) => match self
                .file_menu
                .controller
                .confirm_discard(self.workspace.file())
            {
                Ok(()) => self.run_file_action(),
                Err(error) => self.file_error(error),
            },
            _ => {}
        }
        self.take_file_close()
    }
    pub(super) fn file_menu_actors(&self, index: usize, actors: &mut Vec<Actor>) {
        let x = screen_width() * 0.5;
        let top = (screen_height() * 0.5 - 140.0).max(110.0);
        actors.push(
            act!(quad: xy(x,top+120.0): setsize(670.0,310.0): diffuse(0.04,0.06,0.1,0.98): z(8050)),
        );
        actors.push(act!(text: font("miso"): settext(i18n::tr("Edition","FileMenuTitle")): xy(x,top-15.0): zoom(0.7): z(8051)));
        let keys: &[&str] = match self.panel {
            Some(Panel::FileMenu(_)) => &[
                "FileNew",
                "FileOpen",
                "FileRecent",
                "FileReload",
                "Save",
                "SaveAsOpen",
                "KeepEditing",
            ],
            Some(Panel::FileOpen(_)) => &["FileOpenPath", "FileOpenConfirm", "KeepEditing"],
            Some(Panel::FileUnsaved(_)) => {
                &["KeepEditing", "FileSaveContinue", "FileDiscardReview"]
            }
            Some(Panel::FileDiscard) => &["FileDiscardConfirm"],
            Some(Panel::FileLoading) => &["FileLoading"],
            _ => &[],
        };
        if matches!(self.panel, Some(Panel::FileRecent(_))) {
            let first = index.saturating_sub(4);
            for (row, record) in self
                .file_menu
                .recent
                .entries
                .iter()
                .enumerate()
                .skip(first)
                .take(7)
            {
                let text = record
                    .song()
                    .map(|song| song.file.to_string_lossy().into_owned())
                    .unwrap_or_else(|error| error.to_string());
                actors.push(act!(text: font("miso"): settext(text): align(0.0,0.5): xy(x-310.0,top+20.0+(row-first) as f32*30.0): zoom(0.55): maxwidth(620.0): diffuse(1.0,if row==index{0.8}else{1.0},if row==index{0.3}else{1.0},1.0): z(8051)));
            }
            let back = self.file_menu.recent.entries.len();
            actors.push(act!(text: font("miso"): settext(i18n::tr("Edition","KeepEditing")): xy(x,top+245.0): zoom(0.6): diffuse(1.0,if index==back{0.8}else{1.0},0.5,1.0): z(8051)));
        } else {
            for (row, key) in keys.iter().enumerate() {
                let text = if matches!(self.panel, Some(Panel::FileOpen(_))) && row == 0 {
                    self.text_field_label(0).to_string()
                } else {
                    i18n::tr("Edition", key).to_string()
                };
                actors.push(act!(text: font("miso"): settext(text): xy(x,top+20.0+row as f32*32.0): zoom(0.6): maxwidth(620.0): diffuse(1.0,if row==index{0.8}else{1.0},if row==index{0.3}else{1.0},1.0): z(8051)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
    struct NoSort;
    impl deadsync_editor::chart_order::Orderer for NoSort {
        fn order(
            &mut self,
            _: &deadsync_editor::chart_order::Snapshot,
        ) -> Result<Vec<usize>, Error> {
            panic!("one chart needs no cultural sort")
        }
    }
    const SONG: &str = "#VERSION:0.83;#TITLE:Original;#BPMS:0=120;#WARPS:2=4;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Challenge;#METER:9;#NOTES:1000;";
    fn state(language: &str) -> (tempfile::TempDir, State) {
        i18n::init_for_tests();
        i18n::set_locale(deadsync_assets::language::load_for_tests(language));
        deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
            854.0, 480.0,
        ));
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.ssc");
        std::fs::write(&source, SONG).unwrap();
        (root, State::open(&source, 0).unwrap())
    }
    fn press(state: &mut State, code: KeyCode) -> bool {
        let mut close = false;
        for pressed in [true, false] {
            close |= state.raw_key(&RawKeyboardEvent {
                code,
                pressed,
                repeat: false,
                timestamp: std::time::Instant::now(),
                host_nanos: 0,
            });
        }
        close
    }
    fn pump(state: &mut State) {
        let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while state.file_io_active() {
            state.process_file_menu(&mut NoSort);
            assert!(
                std::time::Instant::now() < end,
                "File worker did not finish"
            );
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
    #[test]
    fn keyboard_menu_creates_true_untitled_with_no_starter_chart_in_both_languages() {
        for language in ["en", "fr"] {
            let (root, mut state) = state(language);
            press(&mut state, KeyCode::F1);
            press(&mut state, KeyCode::ArrowUp);
            press(&mut state, KeyCode::Enter);
            assert!(matches!(state.panel, Some(Panel::FileMenu(0))));
            let mut actors = Vec::new();
            state.file_menu_actors(0, &mut actors);
            assert!(!actors.is_empty());
            press(&mut state, KeyCode::Enter);
            assert!(state.workspace.file().requires_save_as());
            assert!(!state.workspace.has_chart());
            assert_eq!(
                std::fs::read(root.path().join("source.ssc")).unwrap(),
                SONG.as_bytes()
            );
            assert!(!root.path().join("Untitled.ssc").exists());
        }
    }
    #[test]
    fn dirty_new_review_cancel_retains_session_guids_and_undo_in_both_languages() {
        for language in ["en", "fr"] {
            let (_root, mut state) = state(language);
            let binding = deadsync_editor::chart_order::SessionBinding::capture(
                &state.workspace.file().session,
            );
            let ids = state.workspace.file().session.chart_ids().to_vec();
            state.workspace.execute(Command::Place(0)).unwrap();
            state.begin_file_action(Action::New);
            assert!(matches!(state.panel, Some(Panel::FileUnsaved(0))));
            press(&mut state, KeyCode::Escape);
            assert!(binding.belongs_to(&state.workspace.file().session));
            assert_eq!(state.workspace.file().session.chart_ids(), ids);
            state.workspace.execute(Command::Undo).unwrap();
            assert!(!state.workspace.file().session.is_dirty());
        }
    }
    #[test]
    fn open_path_ime_and_failed_load_retain_original_in_both_languages() {
        for language in ["en", "fr"] {
            let (root, mut state) = state(language);
            let binding = deadsync_editor::chart_order::SessionBinding::capture(
                &state.workspace.file().session,
            );
            state.open_file_menu();
            press(&mut state, KeyCode::ArrowDown);
            press(&mut state, KeyCode::Enter);
            assert!(state.wants_text_input());
            state.text_preedit("音");
            assert!(!press(&mut state, KeyCode::Enter));
            assert!(!state.file_io_active());
            state.text_preedit("");
            let invalid = root.path().join("absent été.ssc");
            assert!(state.text_input(invalid.to_str().unwrap()));
            press(&mut state, KeyCode::Enter);
            pump(&mut state);
            assert!(binding.belongs_to(&state.workspace.file().session));
            assert_eq!(
                state
                    .workspace
                    .file()
                    .session
                    .document()
                    .text(None, "TITLE")
                    .as_deref(),
                Some("Original")
            );
        }
    }
    #[test]
    fn open_recent_and_reload_preserve_fractional_row_zoom_in_both_languages() {
        for language in ["en", "fr"] {
            let (root, mut state) = state(language);
            let target = root.path().join("deuxième.ssc");
            std::fs::write(&target, SONG.replace("Original", "Second")).unwrap();
            state.begin_file_action(Action::Open {
                path: target.clone(),
                saved: None,
            });
            pump(&mut state);
            assert_eq!(
                state.workspace.file().existing_path(),
                Some(target.canonicalize().unwrap().as_path())
            );
            assert_eq!(state.file_menu.recent.entries.len(), 2);
            state.workspace.seek_preserving_selection(160).unwrap();
            state.focus_position = Some(
                deadsync_editor::chart_focus::Position::at_fractional_row(
                    &state.workspace.file().session,
                    0,
                    160.5,
                )
                .unwrap(),
            );
            state.zoom.set_zoom(2.5).unwrap();
            state.open_file_menu();
            for _ in 0..3 {
                press(&mut state, KeyCode::ArrowDown);
            }
            press(&mut state, KeyCode::Enter);
            pump(&mut state);
            assert_eq!(state.focus_position.as_ref().unwrap().row(), 160.5);
            assert_eq!(state.zoom.spacing_zoom(), 2.5);
            state.open_file_menu();
            for _ in 0..2 {
                press(&mut state, KeyCode::ArrowDown);
            }
            press(&mut state, KeyCode::Enter);
            press(&mut state, KeyCode::ArrowDown);
            press(&mut state, KeyCode::Enter);
            pump(&mut state);
            assert_eq!(
                state
                    .workspace
                    .file()
                    .session
                    .document()
                    .text(None, "TITLE")
                    .as_deref(),
                Some("Original")
            );
        }
    }
    #[test]
    fn cancelling_save_as_revokes_close_and_first_save_can_resume_close_in_both_languages() {
        for language in ["en", "fr"] {
            let (root, mut state) = state(language);
            state.begin_file_action(Action::New);
            assert!(!state.request_close());
            press(&mut state, KeyCode::ArrowDown);
            press(&mut state, KeyCode::Enter);
            assert!(matches!(state.panel, Some(Panel::SaveAsDestination(0))));
            press(&mut state, KeyCode::Escape);
            state.command(Command::Save);
            let destination = root.path().join("new.ssc");
            state.text_input(destination.to_str().unwrap());
            press(&mut state, KeyCode::Enter);
            assert!(matches!(state.panel,Some(Panel::SaveAsReview(1))));
            assert!(!destination.exists(),"the default Back action must not publish");
            press(&mut state,KeyCode::ArrowUp);
            assert!(!press(&mut state, KeyCode::Enter));
            assert!(state.workspace.file().existing_path().is_some());
            state.begin_file_action(Action::New);
            assert!(!state.request_close());
            press(&mut state, KeyCode::ArrowDown);
            press(&mut state, KeyCode::Enter);
            let destination = root.path().join("new2.ssc");
            state.text_input(destination.to_str().unwrap());
            press(&mut state, KeyCode::Enter);
            assert!(matches!(state.panel,Some(Panel::SaveAsReview(1))));
            assert!(!destination.exists());
            press(&mut state,KeyCode::ArrowUp);
            assert!(press(&mut state, KeyCode::Enter));
            assert!(!state.workspace.file().has_unsaved_document());
        }
    }
    #[test]
    fn cancelled_open_keeps_worker_slot_until_drained_and_never_reorders_recent_in_both_languages()
    {
        for language in ["en", "fr"] {
            let (root, mut state) = state(language);
            let target = root.path().join("target.ssc");
            std::fs::write(&target, SONG).unwrap();
            let binding = deadsync_editor::chart_order::SessionBinding::capture(
                &state.workspace.file().session,
            );
            state.begin_file_action(Action::Open {
                path: target,
                saved: None,
            });
            assert!(state.file_io_active());
            press(&mut state, KeyCode::Escape);
            assert!(state.file_io_active());
            pump(&mut state);
            assert!(binding.belongs_to(&state.workspace.file().session));
            assert!(state.file_menu.recent.entries.is_empty());
        }
    }
    #[test]
    fn source_analysis_blocks_file_publication_until_drained_in_both_languages() {
        for language in ["en", "fr"] {
            let (_root, mut state) = state(language);
            let binding = deadsync_editor::chart_order::SessionBinding::capture(
                &state.workspace.file().session,
            );
            state.analysis_job = Some(
                deadsync_editor::analysis::task::Task::capture(
                    &state.workspace.file().session,
                    state.workspace.chart(),
                )
                .unwrap()
                .spawn()
                .unwrap(),
            );
            state.begin_file_action(Action::New);
            assert!(binding.belongs_to(&state.workspace.file().session));
            assert!(!state.workspace.file().requires_save_as());
            let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while state.analysis_job.is_some() || deadsync_editor::analysis::task::worker_busy() {
                state.poll_analysis();
                assert!(std::time::Instant::now() < end);
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            state.begin_file_action(Action::New);
            assert!(state.workspace.file().requires_save_as());
        }
    }
    #[test]
    fn reload_uses_mru_zero_when_it_is_not_the_current_song() {
        for language in ["en", "fr"] {
            let (root, mut state) = state(language);
            let target = root.path().join("mru-first.ssc");
            std::fs::write(&target, SONG.replace("Original", "MRU first")).unwrap();
            let mut recent = State::open(&target, 0).unwrap();
            recent.workspace.seek_preserving_selection(160).unwrap();
            recent.focus_position = Some(
                deadsync_editor::chart_focus::Position::at_fractional_row(
                    &recent.workspace.file().session,
                    0,
                    160.5,
                )
                .unwrap(),
            );
            recent.zoom.set_zoom(2.5).unwrap();
            state
                .file_menu
                .recent
                .record(&recent.capture_recent_song().unwrap().unwrap())
                .unwrap();
            state.command_file_reload();
            pump(&mut state);
            assert_eq!(
                state.workspace.file().existing_path(),
                Some(target.canonicalize().unwrap().as_path())
            );
            assert_eq!(state.focus_position.as_ref().unwrap().row(), 160.5);
            assert_eq!(state.zoom.spacing_zoom(), 2.5);
        }
    }
    #[test]
    fn reload_cli_with_empty_recent_explicitly_keeps_current_view() {
        for language in ["en", "fr"] {
            let (_root, mut state) = state(language);
            assert!(state.file_menu.recent.entries.is_empty());
            let path = state.workspace.file().existing_path().unwrap().to_owned();
            state.workspace.seek_preserving_selection(160).unwrap();
            state.focus_position = Some(
                deadsync_editor::chart_focus::Position::at_fractional_row(
                    &state.workspace.file().session,
                    0,
                    160.5,
                )
                .unwrap(),
            );
            state.zoom.set_zoom(2.5).unwrap();
            state.command_file_reload();
            pump(&mut state);
            assert_eq!(state.workspace.file().existing_path(), Some(path.as_path()));
            assert_eq!(state.focus_position.as_ref().unwrap().row(), 160.5);
            assert_eq!(state.zoom.spacing_zoom(), 2.5);
        }
    }
    #[test]
    fn untitled_save_as_publishes_new_mru_then_reload_targets_new_song_en_fr(){
        for language in ["en","fr"]{
            let(root,old)=state(language);let old_song=old.capture_recent_song().unwrap().unwrap();let preferences=root.path().join("preferences.json");std::fs::write(&preferences,b"{\"Version\":1,\"OtherOwner\":{\"keep\":true}}").unwrap();
            let mut state=State::from_workspace(Workspace::new_song(root.path(),0.009).unwrap()).unwrap();state.audio_preferences_file=Some(deadsync_editor::audio_preferences::File::open(&preferences).unwrap());state.file_menu.recent.record(&old_song).unwrap();state.audio_preferences_file.as_mut().unwrap().record_recent_song(&old_song).unwrap();let before=state.file_menu.recent.clone();
            let destination=root.path().join("New Song.ssc");state.open_save_as();state.save_as.destination=destination.to_string_lossy().into_owned();state.text_buffer=None;state.save_as_prepare_review();assert_eq!(state.file_menu.recent,before,"prepare must not reorder MRU");assert!(!destination.exists());state.save_as_key(1,KeyCode::ArrowUp);state.save_as_key(0,KeyCode::Enter);assert!(destination.exists());let destination=destination.canonicalize().unwrap();assert_eq!(state.file_menu.recent.entries[0].song().unwrap().file,destination);assert_eq!(state.audio_preferences_file.as_ref().unwrap().recent_songs(),&state.file_menu.recent);
            state.command_file_reload();pump(&mut state);assert_eq!(state.workspace.file().existing_path(),Some(destination.as_path()));assert!(!state.workspace.has_chart());
        }
    }
    #[test]
    fn ordinary_publication_records_current_fractional_views_and_zoom_en_fr(){
        for language in ["en","fr"]{
            let(_root,mut state)=state(language);state.workspace.seek_preserving_selection(160).unwrap();state.focus_position=Some(deadsync_editor::chart_focus::Position::at_fractional_row(&state.workspace.file().session,0,160.5).unwrap());state.zoom.set_zoom(2.5).unwrap();let expected=state.capture_recent_song().unwrap().unwrap();state.command(Command::Save);assert_eq!(state.file_menu.recent.entries[0].song().unwrap(),expected);assert!(!state.save_as_pending());
        }
    }

}
