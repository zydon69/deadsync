//! New from an existing selector preserves the parent draft and commits the
//! source chart/marker binding together with the new profile file.
use super::*;
use deadsync_editor::generation::profiles::{Kind, Profile};
impl State {
    pub(super) fn create_inline_profile(&mut self, kind: Kind) {
        if matches!(self.catalog_target, ProfileTarget::Application) {
            self.create_application_profile(kind);
            return;
        }
        let result = (|| -> Result<profile_edit::Choices, deadsync_editor::Error> {
            let invalid = || {
                deadsync_editor::Error::InvalidEdit(
                    "Profile selector context is missing or stale".into(),
                )
            };
            let settings = self.profile_selection.as_ref().ok_or_else(invalid)?;
            if !self.profile_catalog_current(&settings.catalog) {
                return Err(invalid());
            }
            let offset = usize::from(matches!(
                self.catalog_target,
                ProfileTarget::Conversion | ProfileTarget::Batch
            ));
            let selected = |kind, index: Option<usize>| {
                index
                    .and_then(|i| i.checked_sub(offset))
                    .and_then(|i| settings.catalog.entries(kind).get(i))
                    .map(|entry| entry.guid.clone())
            };
            let mut choices = profile_edit::Choices {
                pattern: selected(Kind::Pattern, settings.pattern),
                performed: selected(Kind::Performed, settings.performed),
                expressed: selected(Kind::Expressed, settings.expressed),
            };
            let plan = match self.catalog_target {
                ProfileTarget::Chart => {
                    if kind != Kind::Expressed {
                        return Err(invalid());
                    }
                    Some(
                        self.chart_properties
                            .as_ref()
                            .ok_or_else(invalid)?
                            .new_profile_plan(&self.workspace.file().session)?,
                    )
                }
                ProfileTarget::Marker => Some(
                    self.marker
                        .as_ref()
                        .ok_or_else(invalid)?
                        .new_profile_plan(&self.workspace.file().session, kind)?,
                ),
                ProfileTarget::Conversion | ProfileTarget::Batch => None,
                ProfileTarget::Application => unreachable!(),
            };
            let before = if plan.is_some() && self.workspace.has_chart() {
                Some(self.capture_song_property_presentation()?)
            } else {
                None
            };
            let mut profile = Profile::new_editor_default(kind)?;
            profile.rename(&i18n::tr("Edition", "ProfileManagerNewName"))?;
            let directory = self.profile_directory.as_deref().ok_or_else(|| {
                deadsync_editor::Error::InvalidEdit(
                    i18n::tr("Edition", "ExpressionEditUnavailable").to_string(),
                )
            })?;
            if let Some(plan) = plan {
                let committed = self
                    .workspace
                    .create_bound_profile(&plan, directory, &profile)?;
                match self.catalog_target {
                    ProfileTarget::Chart => self
                        .chart_properties
                        .as_mut()
                        .expect("validated parent")
                        .after_inline_profile_creation(
                            &self.workspace.file().session,
                            &committed,
                        )?,
                    ProfileTarget::Marker => self
                        .marker
                        .as_mut()
                        .expect("validated parent")
                        .after_inline_profile_creation(
                            &self.workspace.file().session,
                            &committed,
                        )?,
                    _ => unreachable!(),
                }
                if let (Some(before), Some(receipt)) =
                    (before, committed.application().presentation_receipt())
                {
                    self.rebind_song_property_presentation(before, receipt)?;
                }
                // Refresh may alter panel presentation. Parent fields have been
                // explicitly rebound, so retain these drafts across it.
                let chart = self.chart_properties.take();
                let marker = self.marker.take();
                self.refresh();
                self.chart_properties = chart;
                self.marker = marker;
            } else {
                self.workspace.create_profile(directory, &profile)?;
            }
            let id = profile.guid().clone();
            self.profile_creations
                .insert(self.workspace.file().session.revision(), (kind, id.clone()));
            match kind {
                Kind::Pattern => choices.pattern = Some(id),
                Kind::Performed => choices.performed = Some(id),
                Kind::Expressed => choices.expressed = Some(id),
            };
            Ok(choices)
        })();
        match result {
            Ok(choices) => {
                match self.catalog_target {
                    ProfileTarget::Chart => self.open_chart_expression(),
                    ProfileTarget::Marker => self.open_marker_profiles(),
                    target => self.open_conversion_profiles_for(target),
                }
                if self.catalog_job.is_some() {
                    self.profile_saved_choices = Some(choices);
                    self.application_profile_created = Some(kind);
                }
                self.sync_profile_edit_notice();
            }
            Err(error) => self.status = i18n::editor_error(&error).into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadsync_editor::generation::{marker_edit::Draft, metadata::ChartData};
    fn fixture() -> (tempfile::TempDir, State) {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.ssc");
        std::fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0100;").unwrap();
        let mut state = State::open(&path, 0).unwrap();
        let plan = deadsync_editor::quick_add::Plan::prepare(
            &state.workspace.file().session,
            0,
            deadsync_editor::quick_add::Kind::Pattern,
            96.,
        )
        .unwrap();
        state.workspace.apply_quick_add(&plan).unwrap();
        state.refresh();
        let mut marker = Draft::at_cursor(&state.workspace.file().session, 0, 96).unwrap();
        marker.definition.length = 48;
        state.workspace.apply_marker_edit(&marker, false).unwrap();
        state.refresh();
        state.navigate_exact_position(0.25, true);
        state.take_audio_commands();
        (root, state)
    }
    fn press(state: &mut State, code: KeyCode) {
        for pressed in [true, false] {
            state.raw_key(&RawKeyboardEvent {
                code,
                pressed,
                repeat: false,
                timestamp: std::time::Instant::now(),
                host_nanos: 0,
            });
        }
    }
    fn poll(state: &mut State) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while state.catalog_job.is_some() {
            state.update();
            assert!(std::time::Instant::now() < deadline, "{}", state.status);
            std::thread::yield_now();
        }
    }
    #[test]
    fn marker_new_all_kinds_keeps_pending_draft_and_published_binding_survives_cancel_then_atomic_undo()
     {
        for kind in [Kind::Pattern, Kind::Performed, Kind::Expressed] {
            let (root, mut state) = fixture();
            let configs = root.path().join("configs");
            std::fs::create_dir(&configs).unwrap();
            state.profile_directory = Some(configs.clone());
            state.marker.as_mut().unwrap().definition.length = 192;
            state.open_marker_profiles();
            poll(&mut state);
            let before = state.workspace.file().session.serialize().unwrap();
            let row = match kind {
                Kind::Pattern => 0,
                Kind::Performed => 1,
                Kind::Expressed => 2,
            };
            state.panel = Some(Panel::Profiles(row));
            press(&mut state, KeyCode::KeyN);
            poll(&mut state);
            assert!(
                matches!(state.panel, Some(Panel::ProfileEdit(_))),
                "{}",
                state.status
            );
            assert_eq!(state.marker.as_ref().unwrap().definition.length, 192);
            assert!(state.marker.as_ref().unwrap().has_changes());
            assert_eq!(state.focus_position.as_ref().unwrap().row(), 0.25);
            let files: Vec<_> = std::fs::read_dir(&configs)
                .unwrap()
                .map(|e| e.unwrap().path())
                .collect();
            assert_eq!(files.len(), 1);
            let profile = Profile::parse(kind, &std::fs::read(&files[0]).unwrap()).unwrap();
            let data = ChartData::read(state.workspace.file().session.document(), 0)
                .unwrap()
                .unwrap();
            let marker = &data.patterns.as_ref().unwrap()[&96];
            assert_ne!(marker.length, 192);
            let id = match kind {
                Kind::Pattern => &marker.pattern_config_guid,
                Kind::Performed => &marker.performed_chart_config_guid,
                Kind::Expressed => &data.expressed_chart_config,
            };
            assert_eq!(id, profile.guid());
            state.panel_key(KeyCode::Escape);
            state.marker = None;
            state.profile_selection = None;
            state.panel = None;
            let published = state.workspace.file().session.serialize().unwrap();
            state.command(Command::Undo);
            assert!(!files[0].exists());
            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
            assert_eq!(state.focus_position.as_ref().unwrap().row(), 0.25);
            state.command(Command::Redo);
            assert!(files[0].exists());
            assert_eq!(
                state.workspace.file().session.serialize().unwrap(),
                published
            );
            state.command(Command::Save);
            assert!(
                !state.workspace.file().session.is_dirty(),
                "{}",
                state.status
            );
            let reopened =
                State::open_with_profiles(&root.path().join("song.ssc"), 0, &configs).unwrap();
            let reopened_data = ChartData::read(reopened.workspace.file().session.document(), 0)
                .unwrap()
                .unwrap();
            let reopened_marker = &reopened_data.patterns.as_ref().unwrap()[&96];
            let reopened_id = match kind {
                Kind::Pattern => &reopened_marker.pattern_config_guid,
                Kind::Performed => &reopened_marker.performed_chart_config_guid,
                Kind::Expressed => &reopened_data.expressed_chart_config,
            };
            assert_eq!(reopened_id, profile.guid());
            state.marker = Some(Draft::at_cursor(&state.workspace.file().session, 0, 96).unwrap());
            state.marker.as_mut().unwrap().definition.length = 192;
            state.commit_marker(false);
            assert_eq!(
                ChartData::read(state.workspace.file().session.document(), 0)
                    .unwrap()
                    .unwrap()
                    .patterns
                    .unwrap()[&96]
                    .length,
                192
            );
        }
    }
    #[test]
    fn chart_new_during_independent_source_expression_edit_keeps_parent_and_focus() {
        let (root, mut state) = fixture();
        let configs = root.path().join("configs");
        std::fs::create_dir(&configs).unwrap();
        state.profile_directory = Some(configs.clone());
        state.open_autogen_source_expression(Panel::Generation(6));
        poll(&mut state);
        assert!(state.autogen_expression_back.is_some());
        state.panel = Some(Panel::ChartExpression(0));
        press(&mut state, KeyCode::KeyN);
        poll(&mut state);
        assert!(state.profile_edit.is_some(), "{}", state.status);
        assert!(state.chart_properties.is_some());
        assert!(state.autogen_expression_back.is_some());
        assert_eq!(state.focus_position.as_ref().unwrap().row(), 0.25);
        let id = state
            .chart_properties
            .as_ref()
            .unwrap()
            .expression()
            .clone();
        assert_eq!(
            ChartData::read(state.workspace.file().session.document(), 0)
                .unwrap()
                .unwrap()
                .expressed_chart_config,
            id
        );
        state.panel_key(KeyCode::Escape);
        assert!(matches!(state.panel, Some(Panel::ChartExpression(_))));
        assert!(state.cancel_autogen_expression());
        assert_eq!(
            ChartData::read(state.workspace.file().session.document(), 0)
                .unwrap()
                .unwrap()
                .expressed_chart_config,
            id
        );
    }
    #[test]
    fn conversion_new_is_unbound_until_explicit_stage_and_preserves_document() {
        let (root, mut state) = fixture();
        let configs = root.path().join("configs");
        std::fs::create_dir(&configs).unwrap();
        state.profile_directory = Some(configs.clone());
        state.open_conversion_profiles();
        poll(&mut state);
        let before = state.workspace.file().session.serialize().unwrap();
        let profiles = state.generation.profiles.clone();
        state.panel = Some(Panel::ConversionProfiles(0));
        press(&mut state, KeyCode::KeyN);
        poll(&mut state);
        assert!(state.profile_edit.is_some(), "{}", state.status);
        assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
        assert_eq!(state.generation.profiles, profiles);
        assert_eq!(std::fs::read_dir(&configs).unwrap().count(), 1);
    }
}
