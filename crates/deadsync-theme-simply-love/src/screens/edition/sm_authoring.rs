use super::playback::SongPropertyPresentation;
use super::*;
use deadsync_editor::{history::authoring::Plan, quick_add::Kind};
pub(super) struct Settings {
    plan: Plan,
    resume: Option<Kind>,
    return_panel: Option<Panel>,
}
impl State {
    pub(super) fn request_sm_authoring(&mut self, resume: Option<Kind>) {
        if !self.save_as_guard() || self.save_as_pending_guard() || !self.wheel_intent_guard() {
            return;
        }
        if self
            .timing
            .as_ref()
            .is_some_and(|draft| draft.has_changes())
            || self
                .song_properties
                .as_ref()
                .is_some_and(|draft| draft.has_changes())
            || self
                .chart_properties
                .as_ref()
                .is_some_and(|draft| draft.has_changes())
        {
            self.status = i18n::tr("Edition", "TimingConversionPending");
            return;
        }
        self.refresh_chart_views();
        match self.workspace.prepare_sm_authoring() {
            Ok(plan) => {
                self.sm_authoring = Some(Settings {
                    plan,
                    resume,
                    return_panel: self.panel,
                });
                self.panel = Some(Panel::SmAuthoring(0));
                self.status = i18n::tr("Edition", "SmAuthoringHelp");
            }
            Err(error) => {
                self.status = i18n::tr_fmt(
                    "Edition",
                    "Error",
                    &[("message", &i18n::editor_error(&error))],
                )
            }
        }
    }
    fn rebind_sm_authoring(
        &mut self,
        before: Option<SongPropertyPresentation>,
        application: deadsync_editor::song_properties::Application,
    ) -> Result<(), deadsync_editor::Error> {
        if let Some(receipt) = application.presentation_receipt() {
            if let Some(views) = self.active_views.as_mut() {
                views.rebind_preserved_song_properties(&self.workspace.file().session, receipt)?;
            }
            if let Some(before) = before {
                self.rebind_song_property_presentation(before, receipt)?;
            }
        }
        self.invalidate_source_output_request();
        self.timing = None;
        self.refresh();
        Ok(())
    }
    pub(super) fn sm_authoring_key(&mut self, index: usize, code: KeyCode) {
        if code == KeyCode::Escape
            || (index == 1 && matches!(code, KeyCode::Enter | KeyCode::NumpadEnter))
        {
            if let Some(settings) = self.sm_authoring.take() {
                self.panel = settings.return_panel;
            }
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            self.panel = Some(Panel::SmAuthoring(1 - index));
            return;
        }
        if !matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) {
            return;
        }
        if !self.save_as_guard() || self.save_as_pending_guard() || !self.wheel_intent_guard() {
            return;
        }
        let Some(settings) = self.sm_authoring.take() else {
            return;
        };
        let result = (|| -> Result<(), deadsync_editor::Error> {
            let before = if self.workspace.has_chart() {
                Some(self.capture_song_property_presentation()?)
            } else {
                None
            };
            let options = self
                .application_preferences_owner()
                .map(|file| file.advanced_save())
                .unwrap_or_default();
            let application = self.workspace.apply_sm_authoring(settings.plan, options)?;
            self.rebind_sm_authoring(before, application)?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.panel = None;
                if let Some(kind) = settings.resume {
                    self.command_quick_add_review(kind);
                } else {
                    self.open_timing();
                }
            }
            Err(error) => {
                self.panel = settings.return_panel;
                self.status = i18n::tr_fmt(
                    "Edition",
                    "Error",
                    &[("message", &i18n::editor_error(&error))],
                );
            }
        }
    }
    pub(super) fn sm_authoring_history(&mut self, redo: bool) {
        let result = (|| -> Result<(), deadsync_editor::Error> {
            let before = if self.workspace.has_chart() {
                Some(self.capture_song_property_presentation()?)
            } else {
                None
            };
            let application = self.workspace.apply_sm_authoring_history(redo)?;
            self.rebind_sm_authoring(before, application)
        })();
        if let Err(error) = result {
            self.status = i18n::tr_fmt(
                "Edition",
                "Error",
                &[("message", &i18n::editor_error(&error))],
            );
        }
    }
    pub(super) fn sm_authoring_actors(&self, actors: &mut Vec<Actor>, index: usize) {
        let x = screen_width() * 0.5;
        let y = screen_height() * 0.5;
        actors.push(act!(quad:xy(x,y):setsize(730.0,220.0):diffuse(0.04,0.06,0.1,0.99):z(8050)));
        actors.push(act!(text:font("miso"):settext(i18n::tr("Edition","SmAuthoringHelp")):xy(x,y-65.0):zoom(0.6):wrapwidthpixels(1100.0):maxwidth(680.0):z(8051)));
        for (row, key) in ["SmAuthoringConfirm", "KeepEditing"]
            .into_iter()
            .enumerate()
        {
            actors.push(act!(text:font("miso"):settext(i18n::tr("Edition",key)):xy(x,y+15.0+row as f32*35.0):zoom(0.6):diffuse(1.0,if row==index{0.8}else{1.0},if row==index{0.3}else{1.0},1.0):z(8051)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state(language: &str) -> (tempfile::TempDir, State) {
        i18n::init_for_tests();
        i18n::set_locale(deadsync_assets::language::load_for_tests(language));
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("source.sm");
        std::fs::write(&path,b"#TITLE:Original;#OFFSET:0.25;#BPMS:0=120;#NOTES:dance-single:first:Challenge:9:0,0,0,0,0:1000;#NOTES:dance-single:second:Hard:7:0,0,0,0,0:0100;").unwrap();
        let mut state = State::open(&path, 0).unwrap();
        let owner = state
            .take_audio_commands()
            .iter()
            .find_map(|command| match command {
                deadsync_gameplay::GameplayAudioCommand::EditorBeginAudio { owner, .. } => {
                    Some(*owner)
                }
                _ => None,
            })
            .unwrap();
        let mut ack = deadsync_gameplay::GameplayAudioSnapshot::default();
        ack.editor.owner = owner;
        ack.editor.output.available = true;
        ack.editor.output.leased = true;
        ack.editor.output.period_control = deadsync_gameplay::EditorBufferControl::Adjustable;
        ack.editor.output.requested_period = Some(512);
        ack.editor.output.negotiated_period = Some(512);
        state.update_audio(ack);
        state.take_audio_commands();
        (root, state)
    }
    #[test]
    fn explicit_format_promotion_requires_confirmation_and_preserves_views_focus_and_undo_en_fr() {
        for language in ["en", "fr"] {
            let (root, mut s) = state(language);
            let source = root.path().join("source.sm");
            let disk = std::fs::read(&source).unwrap();
            s.workspace.execute(Command::Seek(160)).unwrap();
            s.workspace.execute(Command::Place(1)).unwrap();
            s.focus_position = Some(
                deadsync_editor::chart_focus::Position::at_fractional_row(
                    &s.workspace.file().session,
                    0,
                    160.5,
                )
                .unwrap(),
            );
            s.refresh();
            let mut views = s.active_views.clone().unwrap();
            views.show(&s.workspace.file().session, 1, true).unwrap();
            s.active_views = Some(views);
            let ids = s.workspace.file().session.chart_ids().to_vec();
            let before = s.workspace.file().session.serialize().unwrap();
            let focus = s.chart_focus_time().unwrap();
            let viewids = s
                .active_views
                .as_ref()
                .unwrap()
                .entries()
                .iter()
                .map(|entry| entry.identity().clone())
                .collect::<Vec<_>>();
            s.request_sm_authoring(Some(Kind::Warp));
            assert!(matches!(s.panel, Some(Panel::SmAuthoring(0))));
            assert_eq!(
                s.workspace.file().session.document().format,
                deadsync_editor::document::Format::Sm
            );
            let mut actors = vec![];
            s.sm_authoring_actors(&mut actors, 0);
            assert!(!actors.is_empty());
            s.sm_authoring_key(0, KeyCode::Escape);
            assert_eq!(s.workspace.file().session.serialize().unwrap(), before);
            s.request_sm_authoring(Some(Kind::Warp));
            s.sm_authoring_key(0, KeyCode::Enter);
            assert_eq!(
                s.workspace.file().session.document().format,
                deadsync_editor::document::Format::Ssc
            );
            assert!(matches!(s.panel, Some(Panel::QuickAdd(2))));
            assert_eq!(s.focus_position.as_ref().unwrap().row(), 160.5);
            assert_eq!(s.chart_focus_time().unwrap(), focus);
            assert_eq!(s.workspace.file().session.chart_ids(), ids);
            assert_eq!(
                s.active_views
                    .as_ref()
                    .unwrap()
                    .entries()
                    .iter()
                    .map(|entry| entry.identity().clone())
                    .collect::<Vec<_>>(),
                viewids
            );
            assert_eq!(std::fs::read(&source).unwrap(), disk);
            s.panel = None;
            s.sm_authoring_history(false);
            assert_eq!(
                s.workspace.file().session.document().format,
                deadsync_editor::document::Format::Sm
            );
            assert_eq!(s.focus_position.as_ref().unwrap().row(), 160.5);
            assert!(matches!(
                s.workspace.file().output_policy(),
                deadsync_editor::output_policy::OutputPolicy::NativeLossless
            ));
            s.sm_authoring_history(true);
            assert_eq!(
                s.workspace.file().session.document().format,
                deadsync_editor::document::Format::Ssc
            );
            assert_eq!(s.focus_position.as_ref().unwrap().row(), 160.5);
            s.command_quick_add_review(Kind::Warp);
            s.quick_add_key(3, KeyCode::Enter);
            assert_eq!(
                s.workspace
                    .file()
                    .session
                    .document()
                    .text(Some(0), "WARPS")
                    .as_deref(),
                Some("3.3333333333333335=1")
            );
            s.command(Command::Undo);
            s.command(Command::Undo);
            assert_eq!(
                s.workspace.file().session.document().format,
                deadsync_editor::document::Format::Sm
            );
            assert_eq!(s.focus_position.as_ref().unwrap().row(), 160.5);
            s.command(Command::Redo);
            assert_eq!(
                s.workspace.file().session.document().format,
                deadsync_editor::document::Format::Ssc
            );
            assert_eq!(s.focus_position.as_ref().unwrap().row(), 160.5);
            assert_eq!(std::fs::read(source).unwrap(), disk);
        }
    }
}
