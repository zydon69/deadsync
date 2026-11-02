//! Guarded keyboard File menu. Parsing prepares a detached candidate; failed or
//! cancelled requests retain the original document, history and settings.
use super::file_controller::{Controller, Phase};
use super::*;
use deadsync_editor::{
    Error, history::file_intent::IntentGuard, recent_song_preferences::Preferences,
    recent_songs::Song,
};
enum Action {
    OpenPreparedPackSong {
        candidate: Box<State>,
        row: Box<deadsync_editor::pack::song_list::Row>,
        source_publication: Option<deadsync_editor::storage::ExternalSnapshot>,
    },
    CloseSong,
    New,
    NewMedia {
        path: PathBuf,
    },
    Open {
        path: PathBuf,
        saved: Option<Song>,
    },
    OpenPublished {
        witness: deadsync_editor::storage::PublishedOpenWitness,
        saved: Option<Song>,
    },
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
    requested_witness: Option<deadsync_editor::storage::PublishedOpenWitness>,
    cancelled: bool,
    recent: Preferences,
    close_ready: bool,
    application_exit: Option<ApplicationExit>,
    application_exit_guard: Option<(IntentGuard, u64, Option<pack_properties::CouplingGuard>)>,
    pack_guard: Option<pack_properties::CouplingGuard>,
    pack_review: bool,
    pack_discard: bool,
    pack_save_selected: bool,
}
pub(super) struct PublishedPreferences {
    runtime: Preferences,
    persisted: Option<Preferences>,
    advanced: Option<deadsync_editor::advanced_export::Options>,
}
impl Ui {
    pub(super) fn recent_preferences(&self) -> &Preferences {
        &self.recent
    }
    pub(super) fn recent_maximum(&self) -> usize {
        self.recent.maximum
    }
    pub(super) fn set_recent_maximum(&mut self, maximum: usize) {
        self.recent.maximum = maximum;
    }
    pub(super) fn install_recent(&mut self, preferences: Preferences) {
        self.recent = preferences;
    }
}
impl State {
    pub(in crate::screens) fn request_application_exit(&mut self) -> ApplicationExit {
        if !self.wheel_intent_guard() {
            return ApplicationExit::Cancelled;
        }
        if self.audio_output_busy() {
            self.status = i18n::tr("Edition", "AudioOutputBusy");
            return if self.file_menu.application_exit == Some(ApplicationExit::Pending) {
                ApplicationExit::Pending
            } else {
                ApplicationExit::Cancelled
            };
        }
        if self.file_menu.application_exit == Some(ApplicationExit::Pending) {
            return ApplicationExit::Pending;
        }
        self.file_menu.application_exit = Some(ApplicationExit::Pending);
        if self.save_as_pending() {
            self.file_menu.application_exit_guard = Some((
                IntentGuard::capture(&self.workspace.file().session),
                self.workspace.file().output_policy_revision(),
                self.capture_pack_coupling(),
            ));
            self.save_as_pending_guard();
            return ApplicationExit::Pending;
        }
        if self.request_close() {
            self.file_menu.application_exit = Some(ApplicationExit::Ready);
        }
        if !self.audio_output_busy()
            && self.file_menu.controller.phase().is_none()
            && self.file_menu.application_exit == Some(ApplicationExit::Pending)
        {
            self.file_menu.application_exit = Some(ApplicationExit::Cancelled);
        }
        self.file_menu.application_exit.unwrap()
    }
    pub(in crate::screens) fn take_application_exit(&mut self) -> Option<ApplicationExit> {
        match self.file_menu.application_exit {
            Some(ApplicationExit::Pending) => Some(ApplicationExit::Pending),
            Some(ApplicationExit::Ready) => {
                if !self.retry_preferences_durability() {
                    self.file_menu.application_exit = None;
                    self.file_menu.application_exit_guard = None;
                    return Some(ApplicationExit::Cancelled);
                }
                let current = self.file_menu.application_exit_guard.as_ref().is_some_and(
                    |(guard, policy, pack)| {
                        guard.is_current(&self.workspace.file().session)
                            && *policy == self.workspace.file().output_policy_revision()
                            && self.pack_coupling_current(pack)
                    },
                );
                self.file_menu.application_exit = None;
                self.file_menu.application_exit_guard = None;
                Some(if current {
                    ApplicationExit::Ready
                } else {
                    ApplicationExit::Cancelled
                })
            }
            other => {
                self.file_menu.application_exit = None;
                self.file_menu.application_exit_guard = None;
                other
            }
        }
    }
    pub(super) fn command_request_close(&mut self) {
        if self.request_close() {
            self.file_menu.close_ready = true;
        }
    }

    pub(super) fn prepare_published_preferences(
        &self,
        destination: &Path,
        advanced: Option<deadsync_editor::advanced_export::Options>,
    ) -> Result<PublishedPreferences, Error> {
        let song = self.capture_recent_song_at(destination)?;
        let mut recent = self
            .audio_preferences_file
            .as_ref()
            .map(|file| file.recent_songs())
            .unwrap_or(&self.file_menu.recent)
            .clone();
        recent.record(&song)?;
        recent.validate()?;
        let persisted = self.audio_preferences_file.as_ref().map(|_| recent.clone());
        Ok(PublishedPreferences {
            runtime: recent,
            persisted,
            advanced,
        })
    }
    pub(super) fn publish_output_preferences(&mut self, prepared: PublishedPreferences) {
        // Every fallible descriptor/JSON preparation preceded the real output.
        // This runs only after namespace publication, including pending sync.
        if let (Some(file), Some(recent)) =
            (self.audio_preferences_file.as_mut(), prepared.persisted)
        {
            if let Err(error) = file.set_recent_songs(recent) {
                self.file_error(error);
                self.file_save_cancelled();
                return;
            }
            if let Some(options) = prepared.advanced {
                file.set_advanced_save(options);
            }
        }
        if let (Some(memory), Some(options)) = (
            self.source_preferences_import.memory.as_mut(),
            prepared.advanced,
        ) {
            memory.set_advanced_save(options);
        }
        self.file_menu.recent = prepared.runtime;
    }
    fn file_error(&mut self, error: impl crate::i18n::EditorDiagnostic) {
        self.status = i18n::tr_fmt(
            "Edition",
            "FileFailed",
            &[("message", &crate::i18n::editor_error(&error))],
        );
    }
    pub(super) fn file_modal(&self) -> bool {
        matches!(
            self.panel,
            Some(
                Panel::ExternalFile(_)
                    | Panel::FileMenu(_)
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
        !self.audio_output_busy()
            && !self.save_as_pending()
            && !self.pack_file_io_active()
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
        if let Some(file) = &self.audio_preferences_file {
            self.file_menu.recent = file.recent_songs().clone();
        }
        self.panel = Some(Panel::FileMenu(0));
        self.status = i18n::tr("Edition", "FileMenuHelp");
    }
    pub(super) fn command_open_prepared_pack_song(
        &mut self,
        mut candidate: State,
        row: deadsync_editor::pack::song_list::Row,
    ) {
        if let Err(error) =
            pack_song_list::application_preferences::Snapshot::capture(self).install(&mut candidate)
        {
            self.file_error(error);
            return;
        }
        self.begin_file_action(Action::OpenPreparedPackSong {
            candidate: Box::new(candidate),
            row: Box::new(row),
            source_publication: self.workspace.file().external_snapshot(),
        });
    }
    pub(super) fn command_file_close(&mut self) {
        if !self.has_song() {
            self.status = i18n::tr("Edition", "FileNoSongHelp");
            return;
        }
        self.begin_file_action(Action::CloseSong);
    }
    pub(super) fn command_file_new(&mut self) {
        self.begin_file_action(Action::New);
    }
    pub(super) fn command_file_open(&mut self) {
        if self.file_io_active() || self.pack_file_io_active() {
            self.queue_system_alert(super::system_alert::Refusal::FileIoBusy);
        }
        self.open_file_menu();
        if matches!(self.panel, Some(Panel::FileMenu(_))) {
            self.file_menu.destination = match self.initial_file_destination() {
                Ok(path) => path,
                Err(error) => {
                    self.file_error(error);
                    return;
                }
            };
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
    pub(super) fn command_file_reload_external(
        &mut self,
        snapshot: &deadsync_editor::storage::ExternalSnapshot,
    ) {
        if !snapshot.is_current(self.workspace.file()) {
            self.status = i18n::tr("Edition", "FileExternalStale");
            return;
        }
        let saved = match self.capture_recent_song() {
            Ok(saved) => saved,
            Err(error) => {
                self.file_error(error);
                return;
            }
        };
        self.begin_file_action(Action::Open {
            path: snapshot.path().to_owned(),
            saved,
        });
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
        self.invalidate_source_output_request();
        if self.file_menu.application_exit == Some(ApplicationExit::Pending) {
            self.file_menu.application_exit = Some(ApplicationExit::Cancelled);
        }
        self.file_menu.controller.cancel();
        self.file_menu.pack_guard = None;
        self.file_menu.pack_review = false;
        self.file_menu.pack_discard = false;
        self.file_menu.pack_save_selected = false;
        self.file_menu.close_ready = false;
        self.file_menu.cancelled = true;
        if let Some(job) = &self.file_menu.loading {
            job.cancel();
        }
        self.file_menu.prepared = None;
        self.file_menu.guard = None;
        self.file_menu.restore = None;
        self.file_menu.requested = None;
        self.file_menu.requested_witness = None;
    }
    fn begin_file_action(&mut self, action: Action) {
        if !self.retry_preferences_durability() {
            return;
        }
        if self.save_as_pending_guard() {
            return;
        }
        if self.pack_has_unsaved_changes() && !self.pack_source_current() {
            self.open_pack_properties();
            self.status = i18n::tr("Edition", "PackSourceChanged");
            return;
        }
        if self.pack_has_pending_durability() {
            self.open_pack_properties();
            self.status = i18n::tr("Edition", "PackCoupledPending");
            return;
        }
        if !self.file_publish_allowed() || self.file_io_active() {
            self.queue_system_alert(super::system_alert::Refusal::FileIoBusy);
            self.status = i18n::tr("Edition", "FileBusy");
            return;
        }
        if !self.deactivate_pack_before_file_intent() {
            return;
        }
        self.invalidate_source_output_request();
        self.stop_audio();
        self.text_buffer = None;
        self.label_preedit.clear();
        self.music_picker_request = None;
        self.text_clipboard_request = None;
        self.text_edit_serial = self.text_edit_serial.wrapping_add(1);
        if let Some(file) = &self.audio_preferences_file {
            self.file_menu.recent = file.recent_songs().clone();
        }
        self.file_menu.pack_guard = self.capture_pack_coupling();
        self.file_menu.pack_review = self.pack_has_unsaved_changes()
            && !match &action {
                Action::Open { path, .. } => self.target_is_same_pack(path),
                Action::OpenPreparedPackSong { candidate, .. } => candidate
                    .workspace
                    .file()
                    .existing_path()
                    .is_some_and(|p| self.target_is_same_pack(p)),
                _ => false,
            };
        self.file_menu.pack_discard = false;
        self.file_menu.cancelled = false;
        let unsaved = (self.has_song() && self.workspace.file().has_unsaved_document())
            || self.file_menu.pack_review;
        self.file_menu
            .controller
            .begin_with_unsaved(action, self.workspace.file(), unsaved);
        if self.file_menu.controller.phase() == Some(Phase::Review) {
            self.panel = Some(Panel::FileUnsaved(0));
            self.status = i18n::tr(
                "Edition",
                if self.file_menu.pack_review {
                    "PackCoupledUnsaved"
                } else {
                    "FileUnsavedHelp"
                },
            );
        } else {
            self.run_file_action();
        }
    }
    fn run_file_action(&mut self) {
        if !self.pack_coupling_current(&self.file_menu.pack_guard) {
            self.cancel_file_action();
            self.status = i18n::tr("Edition", "PackCoupledStale");
            return;
        }
        if self.file_menu.pack_review
            && self.pack_has_unsaved_changes()
            && !self.file_menu.pack_discard
        {
            self.panel = Some(Panel::FileUnsaved(0));
            return;
        }
        let allowed = self.file_publish_allowed();
        if allowed
            && self.file_menu.controller.phase() == Some(Phase::Ready)
            && self
                .file_menu
                .controller
                .is_action(|action| matches!(action, Action::Close))
        {
            if !self.wheel_intent_guard() {
                return;
            }
            self.freeze_wheel_spacing();
        }
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
            Action::OpenPreparedPackSong { candidate, row, .. } => {
                let result = (|| {
                    if candidate.workspace.file().existing_path() != Some(row.path.as_path())
                        || !row.is_current()?
                        || candidate.workspace.file().has_external_change()?
                    {
                        return Err(Error::ExternalChange);
                    }
                    self.publish_file_candidate(*candidate)
                })();
                if let Err(error) = result {
                    self.cancel_file_action();
                    self.panel = Some(Panel::FileMenu(0));
                    self.file_error(error);
                } else {
                    self.status = if let Some(first) = self.song_open_warnings.first() {
                        i18n::tr_fmt(
                            "Edition",
                            "PackSongsOpenedWarnings",
                            &[
                                ("count", &self.song_open_warnings.len().to_string()),
                                ("chart", &(first.source_index + 1).to_string()),
                                ("message", &first.message),
                            ],
                        )
                    } else {
                        i18n::tr("Edition", "PackSongsOpened")
                    };
                }
            }
            Action::CloseSong => self.run_close_song(),
            Action::Close => {
                if self.prepare_audio_output_close() {
                    self.finish_authorized_audio_close();
                }
            }
            Action::New => self.run_new_song(),
            Action::NewMedia { path } => self.run_new_media(path),
            Action::Open { path, saved } => self.start_file_open(path, saved, None),
            Action::OpenPublished { witness, saved } => {
                self.start_file_open(witness.path().to_owned(), saved, Some(witness))
            }
        }
    }
    // Keep replacement construction separate from command dispatch.
    fn run_close_song(&mut self) {
        // Memory-only support workspace is explicitly absent in the UI;
        // no simfile/destination is created or represented as Untitled.
        let prepared = Workspace::new_song(self.workspace.file().resource_base(), 0.009)
            .and_then(State::from_workspace)
            .and_then(|mut next| {
                pack_song_list::application_preferences::Snapshot::capture(self)
                    .install(&mut next)?;
                next.zoom.set_zoom(1.0)?;
                Ok(next)
            });
        match prepared {
            Ok(mut next) => {
                next.document_presence = DocumentPresence::NoSong;
                next.refresh();
                if let Err(error) = self.publish_file_candidate(next) {
                    self.file_error(error);
                } else {
                    self.status = i18n::tr("Edition", "FileNoSongHelp");
                }
            }
            Err(error) => self.file_error(error),
        }
    }
    fn run_new_song(&mut self) {
        let offset = self.general_options.new_sync;
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
    fn start_file_open(
        &mut self,
        path: PathBuf,
        saved: Option<Song>,
        witness: Option<deadsync_editor::storage::PublishedOpenWitness>,
    ) {
        let preferences = self.application_opening_preferences();
        let result = preferences.and_then(|preferences| match witness.as_ref() {
            Some(witness) => pack_song_list::file_open::Job::spawn_published_with_profiles(
                witness.clone(),
                preferences,
                self.profile_directory.clone(),
            ),
            None => pack_song_list::file_open::Job::spawn_with_profiles(
                path.clone(),
                preferences,
                self.profile_directory.clone(),
            ),
        });
        match result {
            Ok(job) => {
                self.file_menu.guard = Some(IntentGuard::capture(&self.workspace.file().session));
                self.file_menu.loading = Some(job);
                self.file_menu.requested = Some(path);
                self.file_menu.requested_witness = witness;
                self.file_menu.restore = saved;
                self.file_menu.cancelled = false;
                self.panel = Some(Panel::FileLoading);
                self.status = i18n::tr("Edition", "FileLoading");
            }
            Err(error) => self.file_error(error),
        }
    }
    fn publish_file_candidate(&mut self, mut next: State) -> Result<(), Error> {
        deadsync_editor::audio_preferences::durability::retry_all()?;
        self.flush_wheel_settings()?;
        next.install_scroll_preferences(self.scroll_preferences)?;
        self.transfer_wheel_spacing(&mut next);
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
        if !self.pack_coupling_current(&self.file_menu.pack_guard) {
            return Err(Error::InvalidEdit("Pack review became stale".into()));
        }
        // Final barrier before moving the pack, preference owner or source State.
        if let Some(witness) = self.file_menu.requested_witness.as_ref() {
            witness.validate_source(self.workspace.file())?;
            witness.validate_opened(
                next.workspace.file(),
                &std::sync::atomic::AtomicBool::new(false),
            )?;
        }
        next.workspace.file().validate_generation_profile_context(
            self.profile_directory.as_deref(),
            &std::sync::atomic::AtomicBool::new(false),
        )?;
        if let Some(file) = self.audio_preferences_file.as_mut() {
            file.set_recent_songs(recent.clone())?;
        }
        self.retain_pack_for(&mut next);
        self.stop_audio();
        next.audio_preferences_file = self.audio_preferences_file.take();
        next.file_menu.recent = recent;

        next.status = i18n::tr(
            "Edition",
            if next.workspace.file().requires_save_as() {
                "FileNewCreated"
            } else {
                "FileOpened"
            },
        );
        self.transfer_editor_audio_session(&mut next);
        *self = next;
        Self::report_profile_load_repairs(self.workspace.file());
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
    pub(super) fn file_pack_durability_completed(&mut self) {
        if !self.file_menu.pack_save_selected
            || self.file_menu.controller.phase() != Some(Phase::Review)
        {
            return;
        }
        if !self.pack_coupling_current(&self.file_menu.pack_guard) {
            self.cancel_file_action();
            self.status = i18n::tr("Edition", "PackCoupledStale");
            return;
        }
        if self.pack_has_unsaved_changes() || self.pack_has_pending_durability() {
            return;
        }
        match self
            .file_menu
            .controller
            .confirm_discard(self.workspace.file())
        {
            Ok(()) => self.run_file_action(),
            Err(error) => {
                self.file_save_cancelled();
                self.file_error(error);
            }
        }
    }
    pub(super) fn file_pack_relocated(&mut self) {
        if self.file_menu.controller.phase() == Some(Phase::Saving) {
            self.file_menu.pack_guard = self.capture_pack_coupling();
        }
    }
    pub(super) fn file_action_saved(&mut self) {
        if !self.audio_output_busy()
            && self.file_menu.controller.phase().is_none()
            && self.file_menu.application_exit == Some(ApplicationExit::Pending)
        {
            let current = self.file_menu.application_exit_guard.as_ref().is_some_and(
                |(guard, policy, pack)| {
                    guard.is_current(&self.workspace.file().session)
                        && *policy == self.workspace.file().output_policy_revision()
                        && self.pack_coupling_current(pack)
                },
            );
            if current
                && !self.workspace.file().has_unsaved_document()
                && !self.pack_has_unsaved_changes()
                && !self.pack_has_pending_durability()
            {
                self.begin_file_action(Action::Close);
            } else {
                self.file_menu.application_exit = Some(ApplicationExit::Cancelled);
            }
            return;
        }
        if self.file_menu.controller.phase() == Some(Phase::Saving) {
            match self.file_menu.controller.saved(self.workspace.file(), None) {
                Ok(()) => {
                    let saved = match self.capture_recent_song() {
                        Ok(saved) => saved,
                        Err(error) => {
                            self.file_save_cancelled();
                            self.file_error(error);
                            return;
                        }
                    };
                    let file = self.workspace.file();
                    let mapped = self.file_menu.controller.map_ready(file, |action| {
                        if let Action::OpenPreparedPackSong {
                            candidate,
                            row,
                            source_publication: Some(publication),
                        } = action
                        {
                            // Only our own successful namespace publication can
                            // advance this epoch; external bytes cannot do so.
                            if file.existing_path() == Some(publication.path())
                                && candidate.workspace.file().existing_path()
                                    == file.existing_path()
                                && row.path.as_path() == publication.path()
                                && !publication.is_current(file)
                            {
                                *action = Action::OpenPublished {
                                    witness: file.published_open_witness()?,
                                    saved,
                                };
                            }
                        }
                        Ok(())
                    });
                    match mapped {
                        Ok(()) => self.run_file_action(),
                        Err(error) => self.file_error(error),
                    }
                }
                Err(error) => self.file_error(error),
            }
        }
    }
    pub(super) fn take_file_close(&mut self) -> bool {
        if self.file_menu.close_ready && !self.retry_preferences_durability() {
            self.file_menu.close_ready = false;
            return false;
        }
        std::mem::take(&mut self.file_menu.close_ready)
    }
    pub(super) fn request_file_close(&mut self) -> bool {
        if !self.retry_preferences_durability() {
            return false;
        }
        if !self.wheel_intent_guard() {
            return false;
        }
        self.begin_file_action(Action::Close);
        self.take_file_close()
    }
    pub(super) fn file_save_cancelled(&mut self) {
        self.invalidate_source_output_request();
        if self.file_menu.application_exit == Some(ApplicationExit::Pending) {
            self.file_menu.application_exit = Some(ApplicationExit::Cancelled);
        }
        self.file_menu.controller.cancel();
        self.file_menu.pack_guard = None;
        self.file_menu.pack_review = false;
        self.file_menu.pack_discard = false;
        self.file_menu.pack_save_selected = false;
        self.file_menu.close_ready = false;
    }
    pub(super) fn file_menu_key(&mut self, index: usize, code: KeyCode) -> bool {
        if code == KeyCode::F9 && matches!(self.panel, Some(Panel::FileMenu(_))) {
            self.open_general_options();
            return false;
        }
        if code == KeyCode::F10 && matches!(self.panel, Some(Panel::FileMenu(_))) {
            self.open_display_options();
            return false;
        }
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
            Some(Panel::FileMenu(_)) => {
                if self.has_song() {
                    9
                } else {
                    18
                }
            }
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
            Some(Panel::FileMenu(_)) if !self.has_song() => match index {
                0 => self.command_file_new(),
                1 => self.command_file_open(),
                2 => self.command_file_recent(),
                3 => self.open_key_binding_settings(),
                4 => self.open_density_settings(),
                5 => self.open_dark_settings(),
                6 => self.open_stream_settings(),
                7 => self.open_receptor_settings(),
                8 => self.open_note_color_settings(),
                9 => self.open_waveform_settings(),
                10 => self.open_scroll_settings(),
                11 => return self.request_close(),
                12 => {
                    self.request_application_exit();
                }
                14 => {
                    self.panel = None;
                    self.open_log();
                }
                15 => {
                    self.panel = None;
                    self.open_editor_help();
                }
                16 => {
                    self.panel = None;
                    self.open_performance();
                }
                17 => {
                    self.panel = None;
                    self.open_profile_manager();
                }
                _ => self.panel = None,
            },
            Some(Panel::FileMenu(_)) => match index {
                0 => self.command_file_new(),
                1 => self.command_file_open(),
                2 => self.command_file_recent(),
                3 => self.command_file_reload(),
                4 => self.command(Command::Save),
                5 => self.open_save_as(),
                6 => self.command_open_containing_folder(),
                7 => self.command_file_close(),
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
                        if !self.try_open_direct_media(path.clone()) {
                            match self.remember_open_dialog_directory(&path) {
                                Ok(()) => {
                                    self.begin_file_action(Action::Open { path, saved: None })
                                }
                                Err(error) => self.file_error(error),
                            }
                        }
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
                    if !self.pack_coupling_current(&self.file_menu.pack_guard) {
                        self.cancel_file_action();
                        self.status = i18n::tr("Edition", "PackCoupledStale");
                        return false;
                    }
                    if !self.workspace.file().has_unsaved_document() {
                        self.file_menu.pack_save_selected = true;
                        if !self.save_coupled_pack() {
                            return false;
                        }
                        if let Err(error) = self
                            .file_menu
                            .controller
                            .confirm_discard(self.workspace.file())
                        {
                            self.file_error(error);
                            return false;
                        }
                        self.run_file_action();
                        return self.take_file_close();
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
                Ok(()) => {
                    self.file_menu.pack_discard = true;
                    self.run_file_action();
                }
                Err(error) => self.file_error(error),
            },
            _ => {}
        }
        self.take_file_close()
    }
    pub(super) fn file_menu_actors(&self, index: usize, actors: &mut Vec<Actor>) {
        // This modal owns pagination. Reserve the title, actual row pitch and
        // footer inside the same card, including the last row of every page.
        let width = (screen_width() - 24.).min(670.).max(1.);
        let height = (screen_height() - 24.).min(360.).max(1.);
        let x = screen_width() * 0.5;
        let top = (screen_height() - height) * 0.5;
        let pitch = 32.;
        let first_row = 62.;
        let capacity = (((height - first_row - 38.) / pitch).floor().max(0.) as usize + 1).min(9);
        actors.push(act!(quad:xy(x,top+height*0.5):setsize(width,height):diffuse(0.04,0.06,0.1,0.98):z(8050)));
        actors.push(act!(text:font("miso"):settext(i18n::tr("Edition","FileMenuTitle")):xy(x,top+24.):zoom(0.7):maxwidth(width-24.):z(8051)));
        let keys: &[&str] = match self.panel {
            Some(Panel::FileMenu(_)) if !self.has_song() => &[
                "FileNew",
                "FileOpen",
                "FileRecent",
                "KeyBindingSettingsOpen",
                "DensitySettingsOpen",
                "DarkSettingsOpen",
                "StreamSettingsOpen",
                "ReceptorSettingsOpen",
                "NoteColorSettingsOpen",
                "KeyBindingActionOpenWaveformSettings",
                "KeyBindingActionOpenScrollSettings",
                "Close",
                "KeyBindingActionExit",
                "KeepEditing",
                "LogOpenPanel",
                "EditorHelpOpen",
                "PerformanceOpen",
                "ProfileManagerOpen",
            ],
            Some(Panel::FileMenu(_)) => &[
                "FileNew",
                "FileOpen",
                "FileRecent",
                "FileReload",
                "Save",
                "SaveAsOpen",
                "FileFolderOpen",
                "FileCloseSong",
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

        let recent = matches!(self.panel, Some(Panel::FileRecent(_)));
        let total = if recent {
            self.file_menu.recent.entries.len() + 1
        } else {
            keys.len()
        };
        let mut rows = Vec::with_capacity(capacity);
        if recent {
            let back = self.file_menu.recent.entries.len();
            // Keep the existing always-visible Back control whenever there is
            // room for both it and a record; Escape remains available too.
            let records = if capacity > 1 {
                capacity - 1
            } else {
                usize::from(index < back)
            };
            let first = index
                .min(back.saturating_sub(1))
                .saturating_sub(records / 2)
                .min(back.saturating_sub(records));
            for (row, record) in self
                .file_menu
                .recent
                .entries
                .iter()
                .enumerate()
                .skip(first)
                .take(records)
            {
                let label = record
                    .song()
                    .map(|song| song.file.to_string_lossy().into_owned())
                    .unwrap_or_else(|error| crate::i18n::editor_error(&error));
                rows.push((row, label));
            }
            if rows.len() < capacity {
                rows.push((back, i18n::tr("Edition", "KeepEditing").to_string()));
            }
        } else {
            let first = index
                .saturating_sub(capacity / 2)
                .min(keys.len().saturating_sub(capacity));
            for (row, key) in keys.iter().enumerate().skip(first).take(capacity) {
                let label = if matches!(self.panel, Some(Panel::FileOpen(_))) && row == 0 {
                    self.text_field_label(0).to_string()
                } else {
                    i18n::tr("Edition", key).to_string()
                };
                rows.push((row, label));
            }
        }
        for (local, (row, label)) in rows.into_iter().enumerate() {
            let start = actors.len();
            actors.push(act!(text:font("miso"):settext(label):xy(x,top+first_row+local as f32*pitch):zoom(0.6):maxwidth(width-48.):diffuse(1.,if row==index{0.8}else{1.},if row==index{0.3}else{1.},1.):z(8051)));
            self.ui_row(actors, start, row == index, pitch);
        }
        if total > 0 {
            let position = i18n::tr_fmt(
                "Edition",
                "FileMenuPosition",
                &[
                    ("current", &(index.min(total - 1) + 1).to_string()),
                    ("total", &total.to_string()),
                ],
            );
            actors.push(act!(text:font("miso"):settext(position):xy(x,top+height-18.):zoom(0.4):maxwidth(width-24.):z(8051)));
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
            assert!(matches!(state.panel, Some(Panel::Menu(0))));
            // Density, Dark and Stream follow File in the expanded main menu.
            for _ in 0..25 {
                press(&mut state, KeyCode::ArrowDown);
            }
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
            assert!(matches!(state.panel, Some(Panel::SaveAsReview(1))));
            assert!(
                !destination.exists(),
                "the default Back action must not publish"
            );
            press(&mut state, KeyCode::ArrowUp);
            assert!(!press(&mut state, KeyCode::Enter));
            assert!(state.workspace.file().existing_path().is_some());
            state.begin_file_action(Action::New);
            assert!(!state.request_close());
            press(&mut state, KeyCode::ArrowDown);
            press(&mut state, KeyCode::Enter);
            let destination = root.path().join("new2.ssc");
            state.text_input(destination.to_str().unwrap());
            press(&mut state, KeyCode::Enter);
            assert!(matches!(state.panel, Some(Panel::SaveAsReview(1))));
            assert!(!destination.exists());
            press(&mut state, KeyCode::ArrowUp);
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
    fn untitled_save_as_publishes_new_mru_then_reload_targets_new_song_en_fr() {
        for language in ["en", "fr"] {
            let (root, old) = state(language);
            let old_song = old.capture_recent_song().unwrap().unwrap();
            let preferences = root.path().join("preferences.json");
            std::fs::write(
                &preferences,
                b"{\"Version\":1,\"OtherOwner\":{\"keep\":true}}",
            )
            .unwrap();
            let mut state =
                State::from_workspace(Workspace::new_song(root.path(), 0.009).unwrap()).unwrap();
            state.audio_preferences_file =
                Some(deadsync_editor::audio_preferences::File::open(&preferences).unwrap());
            state.file_menu.recent.record(&old_song).unwrap();
            state
                .audio_preferences_file
                .as_mut()
                .unwrap()
                .record_recent_song(&old_song)
                .unwrap();
            let before = state.file_menu.recent.clone();
            let destination = root.path().join("New Song.ssc");
            state.open_save_as();
            state.save_as.destination = destination.to_string_lossy().into_owned();
            state.text_buffer = None;
            state.save_as_prepare_review();
            assert_eq!(
                state.file_menu.recent, before,
                "prepare must not reorder MRU"
            );
            assert!(!destination.exists());
            state.save_as_key(1, KeyCode::ArrowUp);
            state.save_as_key(0, KeyCode::Enter);
            assert!(destination.exists());
            let destination = destination.canonicalize().unwrap();
            assert_eq!(
                state.file_menu.recent.entries[0].song().unwrap().file,
                destination
            );
            assert_eq!(
                state
                    .audio_preferences_file
                    .as_ref()
                    .unwrap()
                    .recent_songs(),
                &state.file_menu.recent
            );
            state.command_file_reload();
            pump(&mut state);
            assert_eq!(
                state.workspace.file().existing_path(),
                Some(destination.as_path())
            );
            assert!(!state.workspace.has_chart());
            let descriptor = state.capture_recent_song().unwrap();
            assert!(state.request_host_close());
            let expected = state
                .audio_preferences_file
                .as_ref()
                .unwrap()
                .recent_songs()
                .clone();
            assert_eq!(expected.entries[0].song().ok(), descriptor);
            let reopened = deadsync_editor::audio_preferences::File::open(&preferences).unwrap();
            assert_eq!(reopened.recent_songs(), &expected);
            assert_eq!(
                reopened.recent_songs().entries[0].song().unwrap().file,
                destination
            );
        }
    }
    #[test]
    fn clean_preferences_close_does_not_probe_or_rewrite_external_file() {
        let (root, _song) = state("en");
        let mut state = State::without_song(root.path()).unwrap();
        let preferences = root.path().join("clean-preferences.json");
        std::fs::write(&preferences, b"{\"Version\":1}").unwrap();
        state.configure_audio_preferences(&preferences).unwrap();
        assert!(!state.audio_preferences_file.as_ref().unwrap().has_changes());
        let external = b"{\"Version\":1,\"Vendor\":123}";
        std::fs::write(&preferences, external).unwrap();
        assert!(state.request_host_close());
        assert_eq!(std::fs::read(&preferences).unwrap(), external);
    }
    #[test]
    fn saved_mru_close_reopens_and_conflict_keeps_saved_song_retryable_en_fr() {
        for language in ["en", "fr"] {
            let (root, mut state) = state(language);
            let preferences = root.path().join("durable-preferences.json");
            let initial = b"{\"Version\":1,\"OtherOwner\":{\"keep\":true}}";
            std::fs::write(&preferences, initial).unwrap();
            state.configure_audio_preferences(&preferences).unwrap();
            state.command(Command::Save);
            let song = state.workspace.file().existing_path().unwrap().to_owned();
            let published = std::fs::read(&song).unwrap();
            let expected = state.file_menu.recent.clone();
            assert!(!expected.entries.is_empty());
            assert!(state.audio_preferences_file.as_ref().unwrap().has_changes());
            let changed = b"{\"Version\":1,\"OtherOwner\":{\"keep\":\"external\"}}";
            std::fs::write(&preferences, changed).unwrap();
            assert!(
                !state.request_host_close(),
                "conflicting preferences must block departure"
            );
            assert_eq!(std::fs::read(&preferences).unwrap(), changed);
            assert_eq!(std::fs::read(&song).unwrap(), published);
            assert!(!state.workspace.file().has_unsaved_document());
            assert_eq!(
                state
                    .audio_preferences_file
                    .as_ref()
                    .unwrap()
                    .recent_songs(),
                &expected
            );
            assert!(state.audio_preferences_file.as_ref().unwrap().has_changes());
            // Resolve the conflict externally, then retry Close, not Save Song.
            std::fs::write(&preferences, initial).unwrap();
            assert!(state.request_host_close());
            assert_eq!(std::fs::read(&song).unwrap(), published);
            let reopened = deadsync_editor::audio_preferences::File::open(&preferences).unwrap();
            assert_eq!(reopened.recent_songs(), &expected);
            assert!(!reopened.has_changes());
            let bytes = std::fs::read(&preferences).unwrap();
            let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(value["OtherOwner"]["keep"], true);
        }
    }
    #[test]
    fn ordinary_publication_records_current_fractional_views_and_zoom_en_fr() {
        for language in ["en", "fr"] {
            let (_root, mut state) = state(language);
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
            let expected = state.capture_recent_song().unwrap().unwrap();
            state.command(Command::Save);
            assert_eq!(state.file_menu.recent.entries[0].song().unwrap(), expected);
            assert!(!state.save_as_pending());
        }
    }

    fn coupled(language: &str) -> (tempfile::TempDir, State, std::path::PathBuf) {
        let (root, _) = state(language);
        let pack = root.path().join("Pack");
        let song = pack.join("Song");
        std::fs::create_dir_all(&song).unwrap();
        std::fs::write(
            pack.join("Pack.ini"),
            b"[Group]\nVersion=1\nDisplayTitle=Original pack\n",
        )
        .unwrap();
        let path = song.join("song.ssc");
        std::fs::write(&path, SONG).unwrap();
        let mut state = State::open(&path, 0).unwrap();
        state.open_pack_properties();
        state
            .pack_properties
            .as_mut()
            .unwrap()
            .set_text(pack_properties::Field::Title, "Changed pack")
            .unwrap();
        state.panel = None;
        (root, state, pack.join("Pack.ini"))
    }
    #[test]
    fn coupled_pack_close_cancel_discard_and_save_are_explicit_en_fr() {
        for language in ["en", "fr"] {
            for choice in [0, 1, 2] {
                let (_root, mut state, pack) = coupled(language);
                let before = std::fs::read(&pack).unwrap();
                let song = state
                    .workspace
                    .file()
                    .require_existing_path()
                    .unwrap()
                    .to_owned();
                let song_before = std::fs::read(&song).unwrap();
                assert!(!state.request_close());
                assert!(matches!(state.panel, Some(Panel::FileUnsaved(0))));
                let close = state.file_menu_key(choice, KeyCode::Enter);
                match choice {
                    0 => {
                        assert!(!close);
                        assert!(state.pack_has_unsaved_changes());
                        assert_eq!(std::fs::read(&pack).unwrap(), before);
                    }
                    1 => {
                        assert!(close);
                        assert!(!state.pack_has_unsaved_changes());
                        assert_ne!(std::fs::read(&pack).unwrap(), before);
                    }
                    _ => {
                        assert!(!close);
                        assert!(matches!(state.panel, Some(Panel::FileDiscard)));
                        assert!(state.file_menu_key(0, KeyCode::Enter));
                        assert_eq!(std::fs::read(&pack).unwrap(), before);
                    }
                }
                assert_eq!(
                    std::fs::read(song).unwrap(),
                    song_before,
                    "pack-only save never writes the song"
                );
            }
        }
    }
    #[test]
    fn coupled_pack_edit_then_return_revokes_old_intent_en_fr() {
        for language in ["en", "fr"] {
            let (_root, mut state, pack) = coupled(language);
            let before = std::fs::read(&pack).unwrap();
            assert!(!state.request_close());
            let settings = state.pack_properties.as_mut().unwrap();
            settings
                .set_text(pack_properties::Field::Title, "Other title")
                .unwrap();
            settings
                .set_text(pack_properties::Field::Title, "Changed pack")
                .unwrap();
            assert!(!state.file_menu_key(1, KeyCode::Enter));
            assert_eq!(std::fs::read(&pack).unwrap(), before);
            assert!(state.pack_has_unsaved_changes());
            assert!(!state.take_file_close());
        }
    }
    #[test]
    fn same_pack_transition_retains_exact_dirty_pack_and_rebinds_to_new_song() {
        let (root, mut state, _) = coupled("en");
        let nextsong = root.path().join("Pack/Other");
        std::fs::create_dir(&nextsong).unwrap();
        let path = nextsong.join("next.ssc");
        std::fs::write(&path, SONG).unwrap();
        let mut next = State::open(&path, 0).unwrap();
        let guard = state.capture_pack_coupling();
        assert!(state.target_is_same_pack(next.workspace.file().require_existing_path().unwrap()));
        state.retain_pack_for(&mut next);
        assert!(next.pack_coupling_current(&guard));
        assert!(next.pack_has_unsaved_changes());
        assert!(next.pack_text_current());
        assert!(state.pack_properties.is_none());
    }
    #[test]
    fn pinned_pack_unsaved_modal_oracle_has_all_sixty_source_cases() {
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../deadsync-editor/tests/fixtures/pack-coupling");
        let cases: serde_json::Value =
            serde_json::from_slice(&std::fs::read(base.join("request.json")).unwrap()).unwrap();
        let expected: serde_json::Value =
            serde_json::from_slice(&std::fs::read(base.join("expected.json")).unwrap()).unwrap();
        assert_eq!(cases.as_array().unwrap().len(), 60);
        for (case, out) in cases
            .as_array()
            .unwrap()
            .iter()
            .zip(expected.as_array().unwrap())
        {
            assert_eq!(case["name"], out["name"]);
            let song = case["Song"].as_bool().unwrap() && case["Intent"] != "pack-reload";
            let pack = case["Pack"].as_bool().unwrap() && case["Intent"] != "same-pack-open";
            assert_eq!(!out["title"].as_str().unwrap().is_empty(), song || pack);
            if (song || pack) && case["Choice"] == "cancel" {
                assert!(out["trace"].as_array().unwrap().is_empty());
                assert_eq!(out["continuation_pending"], false);
            }
            if (song || pack) && case["Choice"] == "save" {
                let trace = out["trace"].as_array().unwrap();
                if song {
                    assert_eq!(trace[0], "song-save");
                    assert_eq!(trace[1], "pack-save");
                } else {
                    assert_eq!(trace[0], "pack-save");
                }
            }
        }
    }

    #[test]
    fn application_exit_is_distinct_from_editor_close_and_cancel_revokes_it() {
        let (_root, mut state, _) = coupled("en");
        assert_eq!(state.request_application_exit(), ApplicationExit::Pending);
        assert!(!state.file_menu_key(0, KeyCode::Enter));
        assert_eq!(
            state.take_application_exit(),
            Some(ApplicationExit::Cancelled)
        );
        assert_eq!(state.take_application_exit(), None);
        assert!(!state.request_close());
        state.file_menu_key(2, KeyCode::Enter);
        assert!(state.file_menu_key(0, KeyCode::Enter));
        assert_eq!(
            state.take_application_exit(),
            None,
            "closing the editor after cancelling Exit must not exit application"
        );
    }
    #[test]
    fn application_exit_becomes_ready_only_after_explicit_durable_pack_save() {
        let (_root, mut state, _) = coupled("fr");
        assert_eq!(state.request_application_exit(), ApplicationExit::Pending);
        assert_eq!(
            state.take_application_exit(),
            Some(ApplicationExit::Pending)
        );
        assert!(
            !state.file_menu_key(1, KeyCode::Enter),
            "host exit must not remove editor through editor-close bool"
        );
        assert_eq!(state.take_application_exit(), Some(ApplicationExit::Ready));
        assert_eq!(state.take_application_exit(), None);
        assert!(!state.pack_has_unsaved_changes());
    }

    #[test]
    fn new_edit_after_exit_ready_revokes_host_authority_before_event_loop_consumes_it() {
        let (_root, mut state, _) = coupled("en");
        assert_eq!(state.request_application_exit(), ApplicationExit::Pending);
        state.file_menu_key(1, KeyCode::Enter);
        state.workspace.execute(Command::Place(1)).unwrap();
        assert_eq!(
            state.take_application_exit(),
            Some(ApplicationExit::Cancelled)
        );
        assert!(state.workspace.file().session.is_dirty());
    }

    #[test]
    fn source_close_enters_explicit_no_song_without_creating_file_or_leaving_editor_en_fr() {
        for language in ["en", "fr"] {
            let (root, mut state) = state(language);
            let source = root.path().join("source.ssc");
            let before = std::fs::read(&source).unwrap();
            state.command_file_close();
            assert!(!state.has_song());
            assert_eq!(state.document_presence, DocumentPresence::NoSong);
            assert!(!state.take_file_close());
            assert!(state.pack_properties.is_none());
            assert_eq!(std::fs::read(&source).unwrap(), before);
            assert!(!root.path().join("Untitled.ssc").exists());
            assert_eq!(
                state.chart_label.as_ref(),
                i18n::tr("Edition", "FileNoSong").as_ref()
            );
            let mut actors = Vec::new();
            state.actors(&mut actors);
            assert!(!actors.is_empty());
        }
    }
    #[test]
    fn source_close_dirty_cancel_and_discard_preserve_original_disk_en_fr() {
        for language in ["en", "fr"] {
            let (root, mut state) = state(language);
            let before = std::fs::read(root.path().join("source.ssc")).unwrap();
            state.workspace.execute(Command::Place(1)).unwrap();
            state.command_file_close();
            assert!(matches!(state.panel, Some(Panel::FileUnsaved(0))));
            state.file_menu_key(0, KeyCode::Enter);
            assert!(state.has_song());
            assert!(state.workspace.file().session.is_dirty());
            state.command_file_close();
            state.file_menu_key(2, KeyCode::Enter);
            assert!(!state.file_menu_key(0, KeyCode::Enter));
            assert!(!state.has_song());
            assert_eq!(
                std::fs::read(root.path().join("source.ssc")).unwrap(),
                before
            );
            assert!(!state.take_file_close());
        }
    }
    #[test]
    fn no_song_rejects_save_recovery_and_notes_but_keeps_global_settings_and_exit() {
        let (root, mut state) = state("en");
        state.command_file_close();
        let native = state.workspace.file().session.serialize().unwrap();
        for command in [
            Command::Save,
            Command::SaveRecovery,
            Command::Recover,
            Command::Place(1),
            Command::Undo,
        ] {
            state.command(command);
            assert_eq!(state.workspace.file().session.serialize().unwrap(), native);
            assert!(!state.has_song());
        }
        state.autosave();
        state.update_with_time(std::time::Instant::now() + std::time::Duration::from_secs(90));
        assert!(!state.recovery_pending());
        state.open_save_as();
        assert!(!state.save_as_modal());
        state.open_output_settings();
        assert!(!state.save_as_modal());
        state.open_new_chart();
        assert!(!matches!(state.panel, Some(Panel::NewChart(_))));
        state.open_density_settings();
        assert!(matches!(state.panel, Some(Panel::DensitySettings(_))));
        state.panel = None;
        state.open_key_binding_settings();
        assert!(matches!(state.panel, Some(Panel::KeyBindingSettings(_))));
        state.panel = None;
        assert!(!root.path().join("Untitled.ssc").exists());
        assert_eq!(state.request_application_exit(), ApplicationExit::Ready);
        assert_eq!(state.take_application_exit(), Some(ApplicationExit::Ready));
        assert!(!state.take_file_close());
    }
    #[test]
    fn no_song_new_is_a_real_untitled_document_and_open_restores_loaded_song() {
        let (root, mut state) = state("fr");
        state.command_file_close();
        assert!(!state.has_song());
        state.command_file_new();
        assert!(state.has_song());
        assert!(state.workspace.file().requires_save_as());
        assert!(!matches!(state.panel, Some(Panel::FileUnsaved(_))));
        state.open_new_chart();
        assert!(matches!(state.panel, Some(Panel::NewChart(_))));
        state.panel = None;
        state.command_file_close();
        assert!(matches!(state.panel, Some(Panel::FileUnsaved(_))));
        state.file_menu_key(2, KeyCode::Enter);
        state.file_menu_key(0, KeyCode::Enter);
        assert!(!state.has_song());
        state.begin_file_action(Action::Open {
            path: root.path().join("source.ssc"),
            saved: None,
        });
        pump(&mut state);
        assert!(state.has_song());
        assert!(state.workspace.has_chart());
        assert!(state.workspace.file().existing_path().is_some());
        assert!(!state.take_file_close());
    }
    #[test]
    fn no_song_f1_opens_file_menu_and_source_close_is_distinct_from_close_editor() {
        let (_root, mut state) = state("en");
        state.command_file_close();
        press(&mut state, KeyCode::F1);
        assert!(matches!(state.panel, Some(Panel::FileMenu(0))));
        press(&mut state, KeyCode::Escape);
        assert!(!state.has_song());
        assert!(
            state.request_close(),
            "native CloseEditor may return to Practice even when no song is open"
        );
        assert_eq!(state.take_application_exit(), None);
    }

    #[test]
    fn source_close_preserves_application_preferences_but_resets_song_zoom() {
        let (_root, mut state) = state("en");
        let keys = std::sync::Arc::clone(&state.command_bindings.preferences);
        let mut density = state.density_preferences();
        density.visible = !density.visible;
        state.configure_density_preferences(density).unwrap();
        state.zoom.set_zoom(2.0).unwrap();
        state.command_file_close();
        assert!(!state.has_song());
        assert_eq!(state.density_preferences(), density);
        assert!(std::sync::Arc::ptr_eq(
            &keys,
            &state.command_bindings.preferences
        ));
        assert_eq!(state.zoom.spacing_zoom(), 1.0);
        state.command_file_new();
        assert!(state.has_song());
        assert_eq!(state.density_preferences(), density);
        assert!(std::sync::Arc::ptr_eq(
            &keys,
            &state.command_bindings.preferences
        ));
    }
    #[test]
    fn source_close_save_writes_song_then_enters_no_song_without_returning_to_practice() {
        let (root, mut state) = state("fr");
        state.workspace.execute(Command::Place(1)).unwrap();
        let view = state.workspace.file().session.serialize().unwrap();
        let expected = state
            .workspace
            .file()
            .session
            .serialize_for_output()
            .unwrap();
        assert_eq!(state.workspace.file().session.serialize().unwrap(), view);
        state.command_file_close();
        assert!(matches!(state.panel, Some(Panel::FileUnsaved(0))));
        assert!(!state.file_menu_key(1, KeyCode::Enter));
        assert!(!state.has_song());
        assert_eq!(
            std::fs::read(root.path().join("source.ssc")).unwrap(),
            expected
        );
        assert!(!state.take_file_close());
    }

    #[test]
    fn prepared_pack_candidate_busy_refusal_retains_source_and_worker_lease_until_completion() {
        let (root, mut state, _) = coupled("en");
        assert!(state.save_coupled_pack());
        state.panel = None;
        let source = state
            .workspace
            .file()
            .require_existing_path()
            .unwrap()
            .to_owned();
        let target = root.path().join("Pack/B/song.ssc");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, SONG).unwrap();
        let row =
            deadsync_editor::pack::song_list::Task::from_song_file(&target, Default::default())
                .unwrap()
                .run(
                    &deadsync_editor::pack::song_list::HeaderReader,
                    &std::sync::atomic::AtomicBool::new(false),
                    &mut |_| {},
                )
                .unwrap()
                .rows
                .into_iter()
                .find(|r| r.path == target.canonicalize().unwrap())
                .unwrap();
        state.begin_file_action(Action::Open {
            path: target.clone(),
            saved: None,
        });
        assert!(state.file_io_active());
        state.command_open_prepared_pack_song(State::open(&target, 0).unwrap(), row);
        assert_eq!(
            state.workspace.file().existing_path(),
            Some(source.as_path())
        );
        assert_eq!(state.status, i18n::tr("Edition", "FileBusy"));
        assert!(state.file_io_active());
        assert_eq!(
            state.take_editor_system_alert(),
            Some(deadlib_platform::system_alert::SystemAlert::Exclamation)
        );
        assert_eq!(state.take_editor_system_alert(), None);
        state.cancel_file_action();
        assert_eq!(
            state.workspace.file().existing_path(),
            Some(source.as_path())
        );
        // Cancellation drops the receiver; only the original worker may release
        // the shared parser lease. The dedicated Permit tests prove the blocked interval.
        let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            if let Ok(permit) = super::super::file_open_permit::Permit::acquire() {
                drop(permit);
                break;
            }
            assert!(std::time::Instant::now() < end);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    fn current_pack_row(state: &State) -> deadsync_editor::pack::song_list::Row {
        let path = state.workspace.file().require_existing_path().unwrap();
        deadsync_editor::pack::song_list::Task::from_song_file(path, Default::default())
            .unwrap()
            .run(
                &deadsync_editor::pack::song_list::HeaderReader,
                &std::sync::atomic::AtomicBool::new(false),
                &mut |_| {},
            )
            .unwrap()
            .rows
            .into_iter()
            .find(|row| row.path == path)
            .unwrap()
    }
    #[test]
    fn prepared_current_pack_song_after_own_save_reopens_fresh_bytes_through_common_job() {
        let (_root, mut state, _) = coupled("en");
        let path = state
            .workspace
            .file()
            .require_existing_path()
            .unwrap()
            .to_owned();
        let row = current_pack_row(&state);
        let candidate = State::open(&path, 0).unwrap();
        state.workspace.execute(Command::Place(1)).unwrap();
        let view = state.workspace.file().session.serialize().unwrap();
        let native = state
            .workspace
            .file()
            .session
            .serialize_for_output()
            .unwrap();
        assert_ne!(native, view);
        assert_eq!(state.workspace.file().session.serialize().unwrap(), view);
        let publication = state.workspace.file().external_snapshot().unwrap();
        state.command_open_prepared_pack_song(candidate, row);
        assert!(matches!(state.panel, Some(Panel::FileUnsaved(0))));
        assert!(!state.file_menu_key(1, KeyCode::Enter));
        assert!(
            !publication.is_current(state.workspace.file()),
            "proof is a real publication epoch, never a test flag"
        );
        assert!(state.file_io_active());
        pump(&mut state);
        assert_eq!(state.workspace.file().existing_path(), Some(path.as_path()));
        assert_eq!(state.workspace.file().session.serialize().unwrap(), native);
        assert!(!state.workspace.file().session.is_dirty());
        assert_eq!(std::fs::read(&path).unwrap(), native);
    }
    #[test]
    fn prepared_current_pack_song_external_change_does_not_reclassify_discard_as_own_save() {
        let (_root, mut state, _) = coupled("fr");
        let path = state
            .workspace
            .file()
            .require_existing_path()
            .unwrap()
            .to_owned();
        let row = current_pack_row(&state);
        let candidate = State::open(&path, 0).unwrap();
        state.workspace.execute(Command::Place(1)).unwrap();
        let original = state.workspace.file().session.serialize().unwrap();
        let publication = state.workspace.file().external_snapshot().unwrap();
        state.command_open_prepared_pack_song(candidate, row);
        let external = SONG.replace("Original", "External replacement");
        std::fs::write(&path, &external).unwrap();
        state.file_menu_key(2, KeyCode::Enter);
        state.file_menu_key(0, KeyCode::Enter);
        assert!(
            publication.is_current(state.workspace.file()),
            "external writes cannot advance our publication epoch"
        );
        assert!(!state.file_io_active());
        assert_eq!(
            state.workspace.file().session.serialize().unwrap(),
            original
        );
        assert!(state.workspace.file().session.is_dirty());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), external);
    }

    #[test]
    fn published_reopen_final_barrier_rejects_identical_inode_replacement_without_moving_pack() {
        use std::io::Write;
        let (_root, mut state, _) = coupled("en");
        state.workspace.execute(Command::Place(1)).unwrap();
        state.output_save_current(false);
        assert!(!state.workspace.file().session.is_dirty());
        let witness = state.workspace.file().published_open_witness().unwrap();
        let path = witness.path().to_owned();
        let opened = witness
            .open_bounded(1024 * 1024, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();
        let next = State::from_workspace(Workspace::from_file(opened, 0).unwrap()).unwrap();
        let binding =
            deadsync_editor::chart_order::SessionBinding::capture(&state.workspace.file().session);
        let pack = state.capture_pack_coupling();
        state.file_menu.pack_guard = pack.clone();
        state.file_menu.requested_witness = Some(witness);
        let bytes = std::fs::read(&path).unwrap();
        let mut replacement = tempfile::NamedTempFile::new_in(path.parent().unwrap()).unwrap();
        replacement.write_all(&bytes).unwrap();
        replacement.as_file().sync_all().unwrap();
        replacement.persist(&path).unwrap();
        assert!(!state.workspace.file().has_external_change().unwrap());
        assert!(state.publish_file_candidate(next).is_err());
        assert!(binding.belongs_to(&state.workspace.file().session));
        assert!(state.pack_coupling_current(&pack));
        assert!(state.pack_properties.is_some());
    }
}

impl State {
    pub(super) fn finish_authorized_audio_close(&mut self) {
        let song = match self.capture_recent_song() {
            Ok(song) => song,
            Err(error) => {
                self.file_error(error);
                return;
            }
        };
        if let Some(file) = self.audio_preferences_file.as_mut() {
            let result = (|| {
                if let Some(song) = song.as_ref() {
                    file.record_recent_song(song)?;
                }
                if file.has_changes() {
                    file.save()?;
                }
                Ok::<(), Error>(())
            })();
            if let Err(error) = result {
                // Preserve the real owner's pending mutations and, after
                // rename, its updated publication/durability bookkeeping.
                self.file_error(error);
                return;
            }
        }
        self.release_editor_audio_scope();
        if self.file_menu.application_exit == Some(ApplicationExit::Pending) {
            self.file_menu.application_exit_guard = Some((
                IntentGuard::capture(&self.workspace.file().session),
                self.workspace.file().output_policy_revision(),
                self.capture_pack_coupling(),
            ));
            self.file_menu.application_exit = Some(ApplicationExit::Ready);
        } else {
            self.file_menu.close_ready = true;
        }
        self.panel = None;
    }
}

impl State {
    pub(super) fn begin_entry_open(&mut self, path: PathBuf, saved: Option<Song>) {
        self.begin_file_action(Action::Open { path, saved });
    }
    pub(super) fn entry_close_pending(&self) -> bool {
        self.file_menu
            .controller
            .is_action(|action| matches!(action, Action::Close))
    }
}

/// Source OnOpenAudio/Image/Video/LyricsFile destinations. Keep exact extension
/// membership (case sensitive); image classification reuses the source owner.
fn direct_media_field(path: &std::path::Path) -> Option<deadsync_editor::song_text::Field> {
    use deadsync_editor::{music_file, song_text::Field};
    let extension = path.extension()?.to_str()?;
    if music_file::AUDIO_EXTENSIONS.contains(&extension) {
        Some(Field::Music)
    } else if music_file::IMAGE_EXTENSIONS.contains(&extension) {
        Some(
            deadsync_editor::song_assets::best_image_type(path, |path| {
                let (w, h) = image::image_dimensions(path).ok()?;
                Some((i32::try_from(w).ok()?, i32::try_from(h).ok()?))
            })
            .field(),
        )
    } else if music_file::VIDEO_EXTENSIONS.contains(&extension) {
        Some(Field::Background)
    } else if extension == "lrc" {
        Some(Field::Lyrics)
    } else {
        None
    }
}
impl State {
    /// Returns false only for a non-media extension, leaving simfile Open to its
    /// existing worker. This is reached from the real keyboard File/Open field.
    fn try_open_direct_media(&mut self, path: PathBuf) -> bool {
        let Some(field) = direct_media_field(&path) else {
            return false;
        };
        if !path.is_absolute() {
            self.status = i18n::tr("Edition", "FileAbsolutePathRequired");
            return true;
        }
        if self.save_as_pending_guard() {
            return true;
        }
        if !self.file_publish_allowed() || self.file_io_active() {
            self.status = i18n::tr("Edition", "FileBusy");
            return true;
        }
        if self
            .song_properties
            .as_ref()
            .is_some_and(|draft| draft.has_changes())
        {
            self.status = i18n::tr("Edition", "MediaOpenDraftPending");
            return true;
        }
        if !self.wheel_intent_guard() {
            return true;
        }
        // Shared lexical-path validator checks existence/Unicode/control chars;
        // no canonicalization or move/copy/delete of the selected asset occurs.
        if let Err(error) = deadsync_editor::music_file::selected_path_from_directory(
            self.workspace.file().resource_base(),
            &path,
        ) {
            self.file_error(error);
            return true;
        }
        if !self.has_song() {
            self.begin_file_action(Action::NewMedia { path });
            return true;
        }
        let result = (|| -> Result<(), Error> {
            let relative = deadsync_editor::music_file::selected_path_from_directory(
                self.workspace.file().resource_base(),
                &path,
            )?;
            // Source Untitled has no FileDirectory; Fumen.Path keeps absolute.
            let selected = if self.workspace.file().requires_save_as() {
                path.to_str()
                    .ok_or_else(|| Error::Unsupported("Media path is not valid Unicode".into()))?
                    .to_owned()
            } else {
                relative
            };
            let mut draft = self.workspace.song_properties()?;
            draft.set_text(field, &selected)?;
            self.commit_song_properties_presented(&draft)?;
            self.invalidate_source_output_request();
            self.song_properties = None;
            self.text_buffer = None;
            self.label_preedit.clear();
            self.music_picker_request = None;
            self.text_clipboard_request = None;
            self.text_edit_serial = self.text_edit_serial.wrapping_add(1);
            self.panel = None;
            self.refresh();
            self.status = i18n::tr("Edition", "MediaOpened");
            Ok(())
        })();
        if let Err(error) = result {
            self.file_error(error);
        }
        true
    }
    fn run_new_media(&mut self, path: PathBuf) {
        let result = (|| -> Result<State, Error> {
            let field = direct_media_field(&path)
                .ok_or_else(|| Error::Unsupported("Unsupported song media extension".into()))?;
            deadsync_editor::music_file::selected_path_from_directory(
                self.workspace.file().resource_base(),
                &path,
            )?;
            let mut candidate = deadsync_editor::file_origin::NewSong::prepare(
                self.workspace.file().resource_base(),
                self.general_options.new_sync,
            )?;
            let mut draft = deadsync_editor::song_properties::Draft::capture(&candidate.session)?;
            // OnNewNoSave installs the pending absolute path before the first
            // song history baseline. Saved songs use relative paths above.
            draft.set_text(
                field,
                path.to_str()
                    .ok_or_else(|| Error::Unsupported("Media path is not valid Unicode".into()))?,
            )?;
            draft.apply(&mut candidate.session)?;
            candidate.session =
                deadsync_editor::history::Session::new(candidate.session.snapshot_document()?);
            let workspace = Workspace::from_file_unfocused(
                deadsync_editor::storage::FileSession::from_new(candidate)?,
            );
            let mut next = State::from_workspace(workspace)?;
            pack_song_list::application_preferences::Snapshot::capture(self).install(&mut next)?;
            Ok(next)
        })();
        match result {
            Ok(next) => {
                if let Err(error) = self.publish_file_candidate(next) {
                    self.file_error(error);
                } else {
                    self.status = i18n::tr("Edition", "MediaOpenedUntitled");
                }
            }
            Err(error) => self.file_error(error),
        }
    }
}

#[cfg(test)]
mod direct_media_tests {
    use super::*;
    use deadsync_editor::song_text::Field;
    fn fixture(language: &str) -> (tempfile::TempDir, State) {
        i18n::init_for_tests();
        i18n::set_locale(deadsync_assets::language::load_for_tests(language));
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("source.ssc");
        std::fs::write(&path,b"#VERSION:0.83;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#TITLE:Retain;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        let mut state = State::open(&path, 0).unwrap();
        ack(&mut state);
        (root, state)
    }
    fn ack(state: &mut State) {
        let commands = state.take_audio_commands();
        if let Some(owner) = commands.iter().find_map(|command| match command {
            deadsync_gameplay::GameplayAudioCommand::EditorBeginAudio { owner, .. } => Some(*owner),
            _ => None,
        }) {
            let mut ack = deadsync_gameplay::GameplayAudioSnapshot::default();
            ack.editor.owner = owner;
            ack.editor.output.available = true;
            ack.editor.output.leased = true;
            ack.editor.output.period_control = deadsync_gameplay::EditorBufferControl::Adjustable;
            ack.editor.output.requested_period = Some(512);
            ack.editor.output.negotiated_period = Some(512);
            state.update_audio(ack);
            state.take_audio_commands();
        }
    }
    fn open(state: &mut State, path: &std::path::Path) {
        state.command_file_open();
        assert!(matches!(state.panel, Some(Panel::FileOpen(0))));
        state.file_menu.destination = path.to_str().unwrap().to_owned();
        state.focus_text();
        state.file_menu_key(0, KeyCode::Enter);
    }
    #[test]
    fn keyboard_media_open_without_song_creates_real_untitled_and_preserves_originals_en_fr() {
        for language in ["en", "fr"] {
            for (extension, field) in [
                ("ogg", Field::Music),
                ("mp4", Field::Background),
                ("lrc", Field::Lyrics),
                ("png", Field::Banner),
            ] {
                let (root, mut state) = fixture(language);
                let source = std::fs::read(root.path().join("source.ssc")).unwrap();
                let path = root.path().join(format!("banner.{extension}"));
                if extension == "png" {
                    image::RgbImage::new(418, 164).save(&path).unwrap();
                } else {
                    std::fs::write(&path, b"original media").unwrap();
                }
                let media = std::fs::read(&path).unwrap();
                state.command_file_close();
                assert!(!state.has_song());
                ack(&mut state);
                open(&mut state, &path);
                assert!(state.has_song());
                assert!(state.workspace.file().requires_save_as());
                assert!(state.workspace.file().existing_path().is_none());
                assert!(!state.workspace.has_chart());
                assert_eq!(
                    state.workspace.song_properties().unwrap().text(field),
                    path.to_str().unwrap()
                );
                assert!(
                    state.workspace.file().session.undo_label().is_none(),
                    "pending source media is installed in the new baseline"
                );
                assert_eq!(std::fs::read(&path).unwrap(), media);
                assert_eq!(
                    std::fs::read(root.path().join("source.ssc")).unwrap(),
                    source
                );
                assert!(!root.path().join("Untitled.ssc").exists());
                let lyrics = root.path().join("second.lrc");
                std::fs::write(&lyrics, b"keep second media").unwrap();
                ack(&mut state);
                open(&mut state, &lyrics);
                assert_eq!(
                    state
                        .workspace
                        .song_properties()
                        .unwrap()
                        .text(Field::Lyrics),
                    lyrics.to_str().unwrap(),
                    "already Untitled retains source absolute-path semantics"
                );
            }
        }
    }
    #[test]
    fn opening_media_into_dirty_song_is_one_undoable_edit_not_a_replacement_en_fr() {
        for language in ["en", "fr"] {
            let (root, mut state) = fixture(language);
            state.workspace.execute(Command::Place(1)).unwrap();
            let before = state.workspace.file().session.serialize().unwrap();
            let original = std::fs::read(root.path().join("source.ssc")).unwrap();
            // The opened song has a canonical resource base (/private/var on macOS).
            // Use that same directory spelling to exercise the source relative-path contract.
            let assets = state.workspace.file().resource_base().join("assets");
            std::fs::create_dir(&assets).unwrap();
            let path = assets.join("music.ogg");
            std::fs::write(&path, b"keep audio").unwrap();
            open(&mut state, &path);
            assert!(state.has_song());
            assert!(!state.workspace.file().requires_save_as());
            assert_eq!(
                state
                    .workspace
                    .song_properties()
                    .unwrap()
                    .text(Field::Music),
                "assets/music.ogg"
            );
            assert!(state.workspace.file().session.is_dirty());
            state.workspace.execute(Command::Undo).unwrap();
            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
            assert_eq!(
                std::fs::read(root.path().join("source.ssc")).unwrap(),
                original
            );
            assert_eq!(std::fs::read(path).unwrap(), b"keep audio");
        }
    }
    #[test]
    fn media_validation_and_staged_draft_refusal_do_not_replace_document_en_fr() {
        for language in ["en", "fr"] {
            let (root, mut state) = fixture(language);
            let before = state.workspace.file().session.serialize().unwrap();
            assert!(state.try_open_direct_media(root.path().join("missing.ogg")));
            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
            let path = root.path().join("music.ogg");
            std::fs::write(&path, b"keep").unwrap();
            let mut draft = state.workspace.song_properties().unwrap();
            draft.set_text(Field::Title, "staged").unwrap();
            state.song_properties = Some(draft);
            assert!(state.try_open_direct_media(path));
            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
            assert_eq!(
                state.song_properties.as_ref().unwrap().text(Field::Title),
                "staged"
            );
            state.song_properties = None;
            state.command_file_close();
            assert!(!state.has_song());
            ack(&mut state);
            let absent = state.workspace.file().session.serialize().unwrap();
            state.command_file_open();
            state.file_menu_key(2, KeyCode::Enter);
            assert!(!state.has_song());
            assert_eq!(state.workspace.file().session.serialize().unwrap(), absent);
            assert!(state.try_open_direct_media(root.path().join("still-missing.ogg")));
            assert!(!state.has_song());
            assert_eq!(state.workspace.file().session.serialize().unwrap(), absent);
            assert!(direct_media_field(&root.path().join("x.OGG")).is_none());
            assert!(direct_media_field(&root.path().join("x.exe")).is_none());
        }
    }
}

#[cfg(test)]
mod direct_media_extension_tests {
    use super::*;
    #[test]
    fn exact_source_extension_membership_reaches_concrete_destinations() {
        use deadsync_editor::{music_file, song_text::Field};
        let original: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/direct-media.expected.json"
        )))
        .unwrap();
        for case in original["cases"].as_array().unwrap() {
            let props = &case["properties"];
            let expected = [
                ("MusicPath", Field::Music),
                ("BackgroundPath", Field::Background),
                ("BannerPath", Field::Banner),
                ("CDTitlePath", Field::CdTitle),
                ("JacketPath", Field::Jacket),
                ("CDImagePath", Field::CdImage),
                ("DiscImagePath", Field::DiscImage),
                ("LyricsPath", Field::Lyrics),
            ]
            .into_iter()
            .find_map(|(name, field)| props[name].is_string().then_some(field));
            let path = format!("/media/banner.{}", case["extension"].as_str().unwrap());
            assert_eq!(
                direct_media_field(std::path::Path::new(&path)),
                expected,
                "{path}"
            );
        }

        for extension in music_file::AUDIO_EXTENSIONS {
            assert_eq!(
                direct_media_field(std::path::Path::new(&format!("plain.{extension}"))),
                Some(Field::Music)
            );
        }
        for extension in music_file::IMAGE_EXTENSIONS {
            assert_eq!(
                direct_media_field(std::path::Path::new(&format!("banner.{extension}"))),
                Some(Field::Banner)
            );
        }
        for extension in music_file::VIDEO_EXTENSIONS {
            assert_eq!(
                direct_media_field(std::path::Path::new(&format!("plain.{extension}"))),
                Some(Field::Background)
            );
        }
        assert_eq!(
            direct_media_field(std::path::Path::new("plain.lrc")),
            Some(Field::Lyrics)
        );
        for path in [
            "plain.OGG",
            "plain.PNG",
            "plain.MP4",
            "plain.LRC",
            "plain.flac",
            "plain.txt",
            "plain",
        ] {
            assert_eq!(direct_media_field(std::path::Path::new(path)), None);
        }
    }
}

#[cfg(test)]
mod directory_open_tests {
    use super::*;
    #[test]
    fn confirmed_file_open_remembers_directory_in_pathless_owner_before_load_and_authenticates_history()
     {
        i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let mut state = State::without_song(root.path()).unwrap();
        state.audio_preferences_file = None;
        state.audio_preferences_path = None;
        let before = state.capture_application_preferences_file().unwrap();
        let draft = state
            .workspace
            .prepare_source_preferences(
                &before,
                br#"{"OpenFileDialogInitialDirectory":"/source/imported"}"#,
                deadsync_editor::key_binding_preferences::source_profile::Platform::Mac,
            )
            .unwrap();
        state
            .workspace
            .import_source_preferences(draft, &before, true)
            .unwrap();
        state.sync_source_preferences_history().unwrap();
        let selected = root.path().join("chosen").join("missing.ssc");
        state.command_file_open();
        state.file_menu.destination = selected.to_str().unwrap().into();
        state.focus_text();
        state.file_menu_key(0, KeyCode::Enter);
        let current = state.capture_application_preferences_file().unwrap();
        assert_eq!(
            current.initial_file_directory().unwrap(),
            Some(selected.parent().unwrap().to_owned())
        );
        assert!(state.audio_preferences_file.is_none());
        assert!(state.audio_preferences_path.is_none());
        assert_eq!(
            state.application_preferences_owner().unwrap().path(),
            Path::new("")
        );
        state.workspace.capture_source_preferences_context(&current);
        assert!(
            state.workspace.execute(Command::Undo).is_err(),
            "direct dialog directory change invalidates overlapping reviewed import Undo"
        );
        assert!(state.workspace.take_source_preferences_update().is_none());
        let mut next = State::without_song(root.path()).unwrap();
        super::super::pack_song_list::application_preferences::Snapshot::capture(&state)
            .install(&mut next)
            .unwrap();
        assert_eq!(
            next.application_preferences_owner()
                .unwrap()
                .initial_file_directory()
                .unwrap(),
            current.initial_file_directory().unwrap()
        );
        assert!(next.audio_preferences_file.is_none());
        assert!(!root.path().join("preferences.json").exists());
    }
}

#[cfg(test)]
mod published_audio_binding_tests {
    use super::*;
    #[test]
    fn publishing_chartless_document_rebinds_main_even_with_identical_song_music() {
        i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let mut states = Vec::new();
        for (directory, chart_music) in [("old", "b.wav"), ("new", "c.wav")] {
            let base = root.path().join(directory);
            std::fs::create_dir(&base).unwrap();
            let path = base.join("song.ssc");
            std::fs::write(&path,format!("#VERSION:0.83;#MUSIC:a.wav;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#MUSIC:{chart_music};#NOTES:1000;")).unwrap();
            let mut state = State::open(&path, 0).unwrap();
            state.chart_views_apply(super::super::chart_views::ViewAction::Close);
            assert!(!state.workspace.has_chart());
            states.push(state);
        }
        let next = states.pop().unwrap();
        let expected = next.editor_asset_paths()[0].clone().unwrap();
        let mut current = states.pop().unwrap();
        let owner = current.editor_asset_owner();
        let old = current.editor_asset_paths()[0].clone().unwrap();
        assert_ne!(old, expected);
        current.take_audio_commands();
        current.publish_file_candidate(next).unwrap();
        assert!(!current.workspace.has_chart());
        assert_eq!(current.editor_asset_owner(), owner);
        assert_eq!(current.editor_asset_paths()[0].as_ref(), Some(&expected));
        let commands = current.take_audio_commands();
        assert!(commands.iter().any(|command|matches!(command,deadsync_gameplay::GameplayAudioCommand::EditorRegisterAsset{owner:actual,kind:deadsync_gameplay::EditorAssetKind::Main,path:Some(path)}if *actual==owner&&path==&expected)));
        assert!(!commands.iter().any(|command|matches!(command,deadsync_gameplay::GameplayAudioCommand::EditorRegisterAsset{path:Some(path),..}if path==&old)));
    }
}

#[cfg(test)]
mod live_media_transport_tests {
    use super::super::playback::song_properties_live_tests::{fixture, no_stop, row};
    use super::*;
    #[test]
    fn direct_media_open_and_automatic_assets_use_the_live_song_publication_boundary_en_fr() {
        for language in ["en", "fr"] {
            for automatic in [false, true] {
                let (_root, mut state, owner) = fixture(language);
                let original_row = row(&state);
                let original_time = state.chart_focus_time().unwrap();
                let before = state.workspace.file().session.serialize().unwrap();
                if automatic {
                    // Source auto-assets discovers images and lyrics, never MUSIC.
                    // Real dimensions exercise the production classifier/decoder.
                    let path = state.workspace.file().resource_base().join("banner.png");
                    image::RgbImage::new(418, 164).save(&path).unwrap();
                    state.command_auto_song_assets(false);
                    assert_eq!(
                        state
                            .workspace
                            .file()
                            .session
                            .document()
                            .text(None, "BANNER")
                            .as_deref(),
                        Some("banner.png"),
                        "{}",
                        state.status
                    );
                    assert_eq!(
                        state
                            .workspace
                            .file()
                            .session
                            .document()
                            .text(None, "MUSIC")
                            .as_deref(),
                        Some("main.wav")
                    );
                    assert!(
                        !state
                            .workspace
                            .file()
                            .session
                            .history_changes_song_properties_live(false),
                        "image-only action retains the exact metadata producer"
                    );
                } else {
                    let path = state.workspace.file().resource_base().join("selected.ogg");
                    std::fs::write(&path, b"existing media descriptor").unwrap();
                    state.command_file_open();
                    assert!(matches!(state.panel, Some(Panel::FileOpen(0))));
                    state.file_menu.destination = path.to_str().unwrap().to_owned();
                    state.focus_text();
                    state.file_menu_key(0, KeyCode::Enter);
                    assert_eq!(
                        state
                            .workspace
                            .file()
                            .session
                            .document()
                            .text(None, "MUSIC")
                            .as_deref(),
                        Some("selected.ogg"),
                        "{}",
                        state.status
                    );
                }
                assert_eq!(state.audio_session.scope.owner, owner);
                assert!(state.chart_transport_is_playing());
                assert_eq!(row(&state).to_bits(), original_row.to_bits());
                assert_eq!(
                    state.chart_focus_time().unwrap().to_bits(),
                    original_time.to_bits()
                );
                let commands = state.take_audio_commands();
                no_stop(&commands);
                if automatic {
                    assert!(!commands.iter().any(|c| matches!(
                        c,
                        deadsync_gameplay::GameplayAudioCommand::EditorSwapMusicAsset { .. }
                            | deadsync_gameplay::GameplayAudioCommand::EditorClearMusicAsset { .. }
                    )));
                    let revision = state.workspace.file().session.revision();
                    state.command_auto_song_assets(false);
                    assert_eq!(state.workspace.file().session.revision(), revision);
                    assert!(state.chart_transport_is_playing());
                    no_stop(&state.take_audio_commands());
                } else {
                    assert!(commands.iter().any(|c|matches!(c,deadsync_gameplay::GameplayAudioCommand::EditorSwapMusicAsset{path,..}if path.ends_with("selected.ogg"))));
                    state.command(Command::Undo);
                    assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
                    assert!(state.chart_transport_is_playing());
                    no_stop(&state.take_audio_commands());
                }
            }
        }
    }
}
