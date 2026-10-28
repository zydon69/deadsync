//! Keyboard/IME Save As. No path write occurs before an explicit review action.
use super::profile_edit::save_as::ProfileHistoryRelocation;
use super::*;
use deadsync_editor::{
    Error,
    storage::{PendingDurability, PreviousRecovery, SaveAsOutcome, SaveAttempt},
    workspace::WorkspaceSaveAsReview,
};
#[derive(Default)]
pub(super) struct Ui {
    pub(super) destination: String,
    review: Option<Review>,
    pending: Option<Pending>,
    previous: Vec<PreviousRecovery>,
    published_issue: bool,
}
#[derive(PartialEq)]
struct PresentationGuard {
    chart: Option<usize>,
    row: u64,
    selection: Option<(u64, u64)>,
    profile_serial: u64,
    focused: Option<deadsync_editor::generation::metadata::Guid>,
    shared_time: Option<u64>,
    focus: Option<(u64, u64)>,
    views: Vec<(
        deadsync_editor::generation::metadata::Guid,
        bool,
        Option<u64>,
        u64,
        u64,
    )>,
}
enum Pending {
    Core(PendingDurability),
    #[cfg(test)]
    Boundary(std::sync::Arc<std::sync::Mutex<PendingBoundary>>),
}
#[cfg(test)]
#[derive(Default)]
struct PendingBoundary {
    calls: usize,
    fail: bool,
}
impl Pending {
    fn retry(&self, workspace: &mut Workspace) -> Result<(), Error> {
        match self {
            Self::Core(receipt) => workspace.retry_save_as_durability(receipt),
            #[cfg(test)]
            Self::Boundary(boundary) => {
                let mut boundary = boundary.lock().unwrap();
                boundary.calls += 1;
                if boundary.fail {
                    Err(Error::InvalidEdit("Injected UI receipt failure".into()))
                } else {
                    Ok(())
                }
            }
        }
    }
}
struct Review {
    presentation: PresentationGuard,
    preferences:super::file_menu::PublishedPreferences,
    file: WorkspaceSaveAsReview,
    profiles: ProfileHistoryRelocation,
    views: Option<deadsync_editor::chart_views::ActiveChartViews>,
    focus: Option<deadsync_editor::chart_focus::Position>,
}
impl State {
    fn save_as_presentation_guard(&self) -> PresentationGuard {
        PresentationGuard {
            chart: self.workspace.active_chart(),
            row: self.workspace.row(),
            selection: self.workspace.selection(),
            profile_serial: self.profile_preference_serial,
            focused: self
                .active_views
                .as_ref()
                .and_then(|views| views.focused())
                .map(|entry| entry.identity().clone()),
            shared_time: self
                .active_views
                .as_ref()
                .map(|views| views.music_time().to_bits()),
            focus: self
                .focus_position
                .as_ref()
                .filter(|position| position.is_current(&self.workspace))
                .map(|position| (position.row().to_bits(), position.music_time().to_bits())),
            views: self
                .active_views
                .as_ref()
                .map(|views| {
                    views
                        .entries()
                        .iter()
                        .map(|entry| {
                            (
                                entry.identity().clone(),
                                entry.dedicated(),
                                entry.selection_anchor(),
                                entry.position().row().to_bits(),
                                entry.position().music_time().to_bits(),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
    pub(super) fn save_as_pending(&self) -> bool {
        self.save_as.pending.is_some()
    }
    pub(super) fn save_as_modal(&self) -> bool {
        matches!(
            self.panel,
            Some(Panel::OutputSettings(_) | Panel::OutputLoss(_) | Panel::SaveAsDestination(_) | Panel::SaveAsReview(_) | Panel::SaveAsPending(_))
        )
    }
    pub(super) fn save_as_guard(&mut self) -> bool {
        if self.file_io_active()||self.pack_file_io_active()||self.analysis_job.is_some(){self.status=i18n::tr("Edition","FileBusy");return false;}
        if self.pack_has_unsaved_changes() || self.pack_has_pending_durability() {
            self.open_pack_properties();
            self.status = i18n::tr("Edition", "SaveAsPackGuard");
            return false;
        }
        if self.lane_input.model.as_ref().is_some_and(|input| {
            input.drafts().next().is_some() || input.deleted_heads().next().is_some()
        }) {
            self.status = i18n::tr("Edition", "SaveAsBusy");
            return false;
        }
        if self.auto_recovery.is_pending()
            || self.job.is_some()
            || self.catalog_job.is_some()
            || self.preview.is_some()
        {
            self.status = i18n::tr("Edition", "SaveAsBusy");
            return false;
        }
        true
    }
    pub(super) fn open_save_as(&mut self) {self.open_save_as_impl(false);}
    pub(super) fn open_save_as_for_output(&mut self) {self.open_save_as_impl(true);}
    fn open_save_as_impl(&mut self,preserve_output:bool) {
        if self.save_as_pending() {
            self.panel = Some(Panel::SaveAsPending(0));
            return;
        }
        if !self.save_as_guard() {
            return;
        }
        if !preserve_output {self.output_clear();}
        let presented = self.playback_position();
        let music_time = self.chart_focus_time().ok();
        let exact = self
            .focus_position
            .as_ref()
            .filter(|position| {
                position.is_current(&self.workspace)
                    && presented.is_none_or(|(chart_time, row)| {
                        chart_time.to_bits() == position.chart_time().to_bits()
                            && row.to_bits() == position.row().to_bits()
                    })
                    && music_time
                        .is_none_or(|time| time.to_bits() == position.music_time().to_bits())
            })
            .cloned();
        self.stop_audio();
        // A pending/resumed transport can retain an intentional row inside a
        // warp. Keep that exact current anchor if it is the position presented.
        if exact.is_some() {
            self.focus_position = exact;
        }
        self.minimap_release_input();
        self.lane_entry_reconcile();
        self.save_as.destination = self.workspace.file().existing_path()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.save_as.review = None;
        self.panel = Some(Panel::SaveAsDestination(0));
        self.focus_text();
        self.status = i18n::tr("Edition", "SaveAsDestinationHelp");
    }
    pub(super) fn save_as_prepare_review(&mut self) {
        if !self.sync_text() || !self.label_preedit.is_empty() || !self.save_as_guard() {
            return;
        }
        let result = (|| -> Result<Review, Error> {
            let stamps = self.save_as_profile_stamps();
            let file = self
                .workspace
                .prepare_save_as_with_policy_and_revisions(Path::new(&self.save_as.destination),self.output_candidate_policy()?, &stamps)?;
            let preferences=self.prepare_published_preferences(file.destination(),self.output_global_options())?;
            let receipt = file.relocation();
            let focus = self
                .focus_position
                .as_ref()
                .filter(|position| position.is_current(&self.workspace))
                .cloned();
            if focus
                .as_ref()
                .is_some_and(|position| !position.can_rebind_relocation(receipt))
                || self
                    .active_views
                    .as_ref()
                    .is_some_and(|views| !views.can_rebind_relocation(receipt))
            {
                return Err(Error::InvalidEdit(
                    "Open positions are stale; refresh before Save As".into(),
                ));
            }
            let profiles = ProfileHistoryRelocation::capture(self, receipt)?;
            Ok(Review {
                presentation: self.save_as_presentation_guard(),
                preferences,
                file,
                profiles,
                focus,
                views: self.active_views.clone(),
            })
        })();
        match result {
            Ok(review) => {
                self.save_as.review = Some(review);
                self.text_buffer = None;
                self.label_preedit.clear();
                if !self.save_as_issues().is_empty(){self.output_review_save_as();}else{
                    self.output_ui.acknowledged.clear();
                    self.panel = Some(Panel::SaveAsReview(1));
                    self.status = i18n::tr("Edition", "SaveAsReviewHelp");
                }
            }
            Err(error) => self.save_as_error(error),
        }
    }
    pub(super) fn save_as_error(&mut self, error: impl std::fmt::Display) {
        log::warn!("Editor Save As: {error}");
        self.status = i18n::tr("Edition", "SaveAsFailed");
    }
    fn save_as_publish(&mut self) {
        if !self.save_as_guard() {
            return;
        }
        if self.save_as.previous.try_reserve(1).is_err() {
            self.save_as_error("Cannot reserve recovery receipt");
            return;
        }
        let Some(mut review) = self.save_as.review.take() else {
            return;
        };
        if review.presentation != self.save_as_presentation_guard() {
            self.panel = Some(Panel::SaveAsDestination(0));
            self.focus_text();
            self.save_as_error("Editor presentation changed since review");
            return;
        }
        match self.workspace.save_as_reviewed(review.file,&self.output_ui.acknowledged) {
            Err(error) => {
                self.panel = Some(Panel::SaveAsDestination(0));
                self.focus_text();
                self.save_as_error(error);
            }
            Ok(outcome) => {
                let (relocation, previous, pending) = match outcome {
                    SaveAsOutcome::Durable {
                        relocation,
                        previous_recovery,
                    } => (relocation, previous_recovery, None),
                    SaveAsOutcome::WrittenPendingDurability {
                        receipt,
                        relocation,
                        previous_recovery,
                        error,
                    } => {
                        log::warn!("Save As output awaits directory sync: {error}");
                        (relocation, previous_recovery, Some(Pending::Core(receipt)))
                    }
                };
                self.output_clear();
                self.publish_output_preferences(review.preferences);
                self.file_action_written(&relocation);
                // These prevalidated media-only proofs preserve exact row/time,
                // GUID/order/dedication/selection. They never replay history.
                let session = &self.workspace.file().session;
                let rebound_focus = review
                    .focus
                    .as_ref()
                    .map(|position| position.rebind_relocation(session, &relocation))
                    .transpose();
                let rebound_views = review
                    .views
                    .as_mut()
                    .map(|views| views.rebind_relocation(session, &relocation))
                    .transpose();
                self.save_as.published_issue = false;
                match (rebound_focus, rebound_views) {
                    (Ok(focus), Ok(_)) => {
                        self.focus_position = focus;
                        self.active_views = review.views;
                    }
                    (Err(error), _) | (_, Err(error)) => {
                        // Output already exists. Preserve the prior positions for
                        // diagnosis; do not turn this into a false write failure.
                        log::error!("Published Save As position rebind failed: {error}");
                        self.save_as.published_issue = true;
                        self.focus_position = None;
                    }
                }
                review.profiles.publish(self);
                if let Some(previous)=previous {self.save_as.previous.push(previous);}
                self.save_as.pending = pending;
                self.text_buffer = None;
                self.label_preedit.clear();
                self.text_edit_serial = self.text_edit_serial.wrapping_add(1);
                self.pack_properties = None;
                self.pack_song_list = None;
                self.chart_order_request = None;
                self.music_picker_request = None;
                self.text_clipboard_request = None;
                self.auto_recovery = Default::default();
                self.image_preview_identity = image_previews::next_document_identity();
                self.receptor_views.reset();
                self.refresh();
                if self.save_as.published_issue {
                    self.panel = Some(Panel::SaveAsPending(2));
                    self.file_save_cancelled();
                    self.status = i18n::tr("Edition", "SaveAsRebindFailed");
                } else if self.save_as_pending() {
                    self.panel = Some(Panel::SaveAsPending(0));
                    self.status = i18n::tr("Edition", "SaveAsWrittenPending");
                } else {
                    self.panel = None;
                    self.status = i18n::tr("Edition", "SaveAsSaved");
                    self.file_action_saved();
                }
            }
        }
    }
    pub(super) fn save_as_review_destination(&self)->Option<&Path>{self.save_as.review.as_ref().map(|review|review.file.destination())}
    pub(super) fn save_as_review_policy(&self)->Option<&deadsync_editor::output_policy::OutputPolicy>{self.save_as.review.as_ref().map(|review|review.file.output_policy())}
    pub(super) fn save_as_issues(&self)->&[deadsync_editor::advanced_export::Issue]{self.save_as.review.as_ref().map(|review|review.file.issues()).unwrap_or(&[])}
    pub(super) fn save_as_cancel_output_review(&mut self){self.save_as.review=None;self.text_buffer=None;self.label_preedit.clear();}
    pub(super) fn save_as_back_to_destination(&mut self){self.save_as.review=None;self.output_ui.acknowledged.clear();self.panel=Some(Panel::SaveAsDestination(0));self.focus_text();}
    pub(super) fn save_output_completed(&mut self,attempt:SaveAttempt){
        match attempt{
            SaveAttempt::Saved=>{self.save_as.pending=None;self.panel=None;self.refresh();self.status=i18n::tr("Edition","Saved");self.file_action_saved();},
            SaveAttempt::WrittenPendingDurability{receipt,error}=>{log::warn!("Editor output awaits directory sync: {error}");self.save_as.pending=Some(Pending::Core(receipt));self.panel=Some(Panel::SaveAsPending(0));self.refresh();self.status=i18n::tr("Edition","SaveAsWrittenPending");},
            SaveAttempt::NeedsReview(review)=>self.output_save_attempt(SaveAttempt::NeedsReview(review)),
        }
    }
    pub(super) fn save_as_pending_guard(&mut self) -> bool {
        if self.save_as_pending() {
            self.panel = Some(Panel::SaveAsPending(0));
            self.status = i18n::tr("Edition", "SaveAsPendingGuard");
            true
        } else {
            false
        }
    }
    fn save_as_output_notice(&self) -> &'static str {
        if self.save_as.pending.is_some() {
            "SaveAsWrittenPending"
        } else if self.save_as.published_issue {
            "SaveAsRebindFailed"
        } else if self.workspace.file().session.is_dirty() {
            "SaveAsDurableEditsPending"
        } else {
            "SaveAsSaved"
        }
    }
    pub(super) fn save_as_key(&mut self, index: usize, code: KeyCode) {
        let enter = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter);
        let destination = matches!(self.panel, Some(Panel::SaveAsDestination(_)));
        let pending = matches!(self.panel, Some(Panel::SaveAsPending(_)));
        if code == KeyCode::Escape {
            self.file_save_cancelled();
            self.save_as.review = None;
            self.output_clear();
            self.text_buffer = None;
            self.label_preedit.clear();
            self.panel = None;
            self.status = i18n::tr(
                "Edition",
                if pending {
                    self.save_as_output_notice()
                } else {
                    "SaveAsCancelled"
                },
            );
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            if destination && !self.sync_text() {
                return;
            }
            let backward = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            let next = (index + if backward { 2 } else { 1 }) % 3;
            self.panel = Some(if destination {
                Panel::SaveAsDestination(next)
            } else if pending {
                Panel::SaveAsPending(next)
            } else {
                Panel::SaveAsReview(next)
            });
            if destination {
                self.focus_text();
            }
            return;
        }
        if destination && index == 0 && !enter {
            self.text_field_key(0, code);
            return;
        }
        if !enter {
            return;
        }
        if destination {
            if index <= 1 {
                self.save_as_prepare_review();
            } else {
                self.panel = None;
                self.text_buffer = None;
                self.save_as.review = None;
                self.output_clear();
                self.file_save_cancelled();
                self.status = i18n::tr("Edition", "SaveAsCancelled");
            }
        } else if pending {
            match index {
                0 => {
                    let result = self
                        .save_as
                        .pending
                        .as_ref()
                        .map(|receipt| receipt.retry(&mut self.workspace));
                    match result {
                        Some(Ok(())) => {
                            self.save_as.pending = None;
                            self.panel = None;
                            self.refresh();
                            self.status = i18n::tr("Edition", self.save_as_output_notice());
                            self.file_action_saved();
                        }
                        Some(Err(error)) => self.save_as_error(error),
                        None => {}
                    }
                }
                1 => self.output_save_current(false),
                _ => {
                    self.file_save_cancelled();
                    let notice = self.save_as_output_notice();
                    self.save_as.published_issue = false;
                    self.panel = None;
                    self.status = i18n::tr("Edition", notice);
                }
            }
        } else {
            match index {
                0 => self.save_as_publish(),
                1 => {
                    self.save_as.review = None;
                    self.panel = Some(Panel::SaveAsDestination(0));
                    self.focus_text();
                }
                _ => {
                    self.save_as.review = None;
                    self.output_clear();
                    self.panel = None;
                    self.file_save_cancelled();
                self.status = i18n::tr("Edition", "SaveAsCancelled");
                }
            }
        }
    }
    pub(super) fn save_as_actors(&self, index: usize, actors: &mut Vec<Actor>) {
        let x = screen_width() * 0.5;
        let y = (screen_height() * 0.5 - 105.0).max(134.0);
        actors.push(
            act!(quad: xy(x,y+80.0): setsize(660.0,220.0): diffuse(0.04,0.06,0.1,0.98): z(8050)),
        );
        let title = if matches!(self.panel, Some(Panel::SaveAsReview(_))) {
            "SaveAsReviewTitle"
        } else if self.save_as_pending() {
            "SaveAsPendingTitle"
        } else {
            "SaveAsTitle"
        };
        actors.push(act!(text: font("miso"): settext(i18n::tr("Edition",title)): align(0.0,0.5): xy(x-300.0,y-20.0): zoom(0.65): maxwidth(600.0): z(8051)));
        let destination = self
            .save_as
            .review
            .as_ref()
            .map(|review| review.file.destination())
            .unwrap_or_else(|| Path::new(&self.save_as.destination));
        let path = if matches!(self.panel, Some(Panel::SaveAsDestination(0))) {
            self.text_field_label(0).to_string()
        } else {
            destination.to_string_lossy().into_owned()
        };
        actors.push(act!(text: font("miso"): settext(path): align(0.0,0.5): xy(x-300.0,y+10.0): zoom(0.55): maxwidth(600.0): z(8051)));
        let destination_panel = matches!(self.panel, Some(Panel::SaveAsDestination(_)));
        let review = matches!(self.panel, Some(Panel::SaveAsReview(_)));
        let notice = if review
            && self
                .save_as
                .review
                .as_ref()
                .is_some_and(|review| review.file.replaces_existing())
        {
            "SaveAsWillReplace"
        } else if review {
            "SaveAsWillCreate"
        } else {
            "OutputDestinationNotice"
        };
        actors.push(act!(text: font("miso"): settext(i18n::tr("Edition",notice)): align(0.0,0.5): xy(x-300.0,y+43.0): zoom(0.55): maxwidth(600.0): diffuse(1.0,0.85,0.4,1.0): z(8051)));
        let keys = if destination_panel {
            [
                "SaveAsEditDestination",
                "SaveAsReviewAction",
                "ScrollSettingsCancel",
            ]
        } else if review {
            ["SaveAsConfirm", "SaveAsBack", "ScrollSettingsCancel"]
        } else {
            ["SaveAsRetry", "SaveAsSaveCurrent", "KeepEditing"]
        };
        for (row, key) in keys.into_iter().enumerate() {
            actors.push(act!(text: font("miso"): settext(i18n::tr("Edition",key)): xy(x,y+78.0+row as f32*29.0): zoom(0.65): maxwidth(600.0): diffuse(1.0,if row==index {0.8}else{1.0},if row==index {0.3}else{1.0},1.0): z(8051)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state(language: &str) -> (tempfile::TempDir, State) {
        i18n::init(deadsync_assets::language::load_for_tests(language));
        deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
            854.0, 480.0,
        ));
        let root = tempfile::tempdir().unwrap();
        let song = root.path().join("Song");
        std::fs::create_dir(&song).unwrap();
        let path = song.join("song.ssc");
        std::fs::write(&path,b"#VERSION:0.83;#TITLE:Keep;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;").unwrap();
        let state = State::open(&path, 0).unwrap();
        (root, state)
    }
    #[test]
    fn pending_boundary_guards_close_open_recovery_and_retains_retry_after_failure_en_fr() {
        // This is a native UI boundary test, not simulated filesystem durability.
        // Real rename/sync/dirty correctness belongs to storage tests.
        for language in ["en", "fr"] {
            let (_root, mut state) = state(language);
            let source = state.workspace.file().require_existing_path().unwrap().to_owned();
            let bytes = std::fs::read(&source).unwrap();
            let calls = std::sync::Arc::new(std::sync::Mutex::new(PendingBoundary {
                calls: 0,
                fail: true,
            }));
            state.save_as.pending = Some(Pending::Boundary(calls.clone()));
            assert!(
                !state.workspace.file().session.is_dirty(),
                "pending guard must not depend on ordinary dirty alone"
            );
            assert!(!state.request_close());
            assert!(matches!(state.panel, Some(Panel::SaveAsPending(0))));
            state.save_as_key(2, KeyCode::Enter);
            assert!(state.save_as_pending());
            assert!(state.panel.is_none());
            state.open_pack_song_list();
            assert!(matches!(state.panel, Some(Panel::SaveAsPending(0))));
            state.save_as_key(0, KeyCode::Enter);
            assert!(state.save_as_pending());
            assert_eq!(calls.lock().unwrap().calls, 1);
            state.save_as_key(2, KeyCode::Enter);
            state.command(Command::Recover);
            assert!(state.save_as_pending());
            assert!(matches!(state.panel, Some(Panel::SaveAsPending(0))));
            calls.lock().unwrap().fail = false;
            state.save_as_key(0, KeyCode::Enter);
            assert!(!state.save_as_pending());
            assert_eq!(calls.lock().unwrap().calls, 2);
            assert!(state.panel.is_none());
            assert_eq!(std::fs::read(source).unwrap(), bytes);
        }
    }
    #[test]
    fn modal_autosave_poll_cannot_start_new_recovery_and_dirty_pack_blocks_entry_en_fr() {
        for language in ["en", "fr"] {
            let (_root, mut state) = state(language);
            state.workspace.execute(Command::Place(1)).unwrap();
            assert!(state.workspace.file().session.is_dirty());
            state.open_save_as();
            assert!(state.save_as_modal());
            state.autosave();
            assert!(!state.recovery_pending());
            state.update_with_time(std::time::Instant::now() + std::time::Duration::from_secs(90));
            assert!(!state.recovery_pending());
            assert!(!state.workspace.file().recovery_path().unwrap().exists());
            state.save_as_key(0, KeyCode::Escape);
            std::fs::write(
                _root.path().join("Pack.ini"),
                b"[Group]\nVersion=1\nDisplayTitle=Source pack\n",
            )
            .unwrap();
            state.open_pack_properties();
            assert!(state.text_input("Working pack title"));
            assert!(state.pack_has_unsaved_changes());
            state.open_save_as();
            assert!(state.pack_has_unsaved_changes());
            assert!(matches!(state.panel, Some(Panel::PackProperties)));
        }
    }
    #[test]
    fn successful_durability_retry_does_not_claim_new_working_edits_were_saved_en_fr() {
        for language in ["en", "fr"] {
            let (_root, mut state) = state(language);
            let source = state.workspace.file().require_existing_path().unwrap().to_owned();
            let original = std::fs::read(&source).unwrap();
            state.workspace.execute(Command::Place(1)).unwrap();
            let calls = std::sync::Arc::new(std::sync::Mutex::new(PendingBoundary::default()));
            state.save_as.pending = Some(Pending::Boundary(calls.clone()));
            state.panel = Some(Panel::SaveAsPending(0));
            state.save_as_key(0, KeyCode::Enter);
            assert_eq!(calls.lock().unwrap().calls, 1);
            assert!(!state.save_as_pending());
            assert!(state.workspace.file().session.is_dirty());
            assert_eq!(
                state.status,
                i18n::tr("Edition", "SaveAsDurableEditsPending")
            );
            assert_eq!(std::fs::read(source).unwrap(), original);
        }
    }
    #[test]
    fn acknowledged_view_rebind_diagnostic_never_invents_pending_disk_durability_en_fr() {
        for language in ["en", "fr"] {
            for dismiss in [KeyCode::Escape, KeyCode::Enter] {
                let (_root, mut state) = state(language);
                state.save_as.published_issue = true;
                state.panel = Some(Panel::SaveAsPending(2));
                state.save_as_key(2, dismiss);
                assert!(!state.save_as_pending());
                assert!(state.panel.is_none());
                assert_eq!(state.status, i18n::tr("Edition", "SaveAsRebindFailed"));
                assert!(!state.workspace.file().session.is_dirty());
            }
        }
    }
}
