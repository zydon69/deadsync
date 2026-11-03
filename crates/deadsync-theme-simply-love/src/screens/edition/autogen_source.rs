//! Source UIAutogenChart owns a source chart independently from focused views.
use super::*;
use deadsync_editor::{
    chart_order::SessionBinding,
    generation::{job::ChartTask, metadata::Guid},
};
#[derive(Default)]
pub(super) struct Runtime {
    pub(super) showing: bool,
    pub(super) binding: Option<SessionBinding>,
    pub(super) selected: Option<Guid>,
    pub(super) expression_target: Option<Guid>,
    catalog_target: Option<Guid>,
    catalog_binding: Option<SessionBinding>,
    eligibility: Option<SessionBinding>,
    candidates: Vec<Guid>,
    order: Vec<Guid>,
    order_binding: Option<SessionBinding>,
    order_language: u64,
    order_error: bool,
    request: Option<OrderRequest>,
    groups: Vec<String>,
    layouts: Vec<String>,
    groups_binding: Option<SessionBinding>,
}
struct OrderRequest {
    binding: SessionBinding,
    language: u64,
}
impl State {
    pub(super) fn process_autogen_source_order(
        &mut self,
        orderer: &mut impl deadsync_editor::chart_order::Orderer,
    ) -> bool {
        let Some(request) = self.autogen_source.request.take() else {
            return false;
        };
        let current = |state: &State| {
            state.autogen_source.showing
                && request.language == i18n::revision()
                && request.binding.is_current(&state.workspace.file().session)
        };
        if !current(self) {
            return false;
        }
        let result = self
            .chart_list_cache
            .refresh(&mut self.workspace, request.language, orderer)
            .map(|rows| {
                rows.iter()
                    .map(|row| row.key.guid.clone())
                    .collect::<Vec<_>>()
            });
        if !current(self) {
            return false;
        }
        self.autogen_source.order_binding = Some(request.binding);
        self.autogen_source.order_language = request.language;
        match result {
            Ok(order) => {
                self.autogen_source.order = order;
                self.autogen_source.order_error = false;
                self.sync_autogen_source();
                if matches!(self.panel, Some(Panel::Generation(_))) {
                    self.status = i18n::tr("Edition", "GenerationOptionsHelp");
                }
            }
            Err(_) => {
                self.autogen_source.order_error = true;
                self.autogen_source.order.clear();
                self.status = i18n::tr("Edition", "ChartOrderFailed");
            }
        }
        true
    }
    fn autogen_source_order_current(&self) -> bool {
        !self.autogen_source.order_error
            && self.autogen_source.order_language == i18n::revision()
            && self
                .autogen_source
                .order_binding
                .as_ref()
                .is_some_and(|binding| binding.is_current(&self.workspace.file().session))
    }

    pub(super) fn open_generation(&mut self) {
        if self.is_generating() {
            return;
        }
        self.autogen_source.showing = true;
        self.autogen_source.selected = None;
        self.autogen_source.expression_target = None;
        self.autogen_source.request = None;
        self.sync_autogen_source();
        self.panel = Some(Panel::Generation(0));
        self.status = i18n::tr("Edition", "GenerationOptionsHelp");
    }
    pub(super) fn close_generation_source(&mut self) {
        self.autogen_source.showing = false;
        self.autogen_source.selected = None;
        self.autogen_source.expression_target = None;
        self.autogen_source.catalog_target = None;
        self.autogen_source.catalog_binding = None;
        self.autogen_source.request = None;
    }
    pub(super) fn autogen_source_chart(&self) -> Option<usize> {
        if !self.has_song() {
            return None;
        }
        let session = &self.workspace.file().session;
        let cached = self
            .autogen_source
            .eligibility
            .as_ref()
            .is_some_and(|binding| binding.is_current(session));
        let valid = |chart| {
            if cached {
                session
                    .chart_ids()
                    .get(chart)
                    .is_some_and(|id| self.autogen_source.candidates.contains(id))
            } else {
                ChartTask::supports_source(session, chart)
            }
        };
        self.autogen_source
            .binding
            .as_ref()
            .filter(|owner| owner.belongs_to(session))
            .and_then(|_| self.autogen_source.selected.as_ref())
            .and_then(|id| {
                session
                    .chart_ids()
                    .iter()
                    .position(|candidate| candidate == id)
            })
            .filter(|chart| valid(*chart))
            .or_else(|| self.workspace.active_chart().filter(|chart| valid(*chart)))
            .or_else(|| {
                if !self.autogen_source_order_current() {
                    return None;
                }
                // Utils GetCharts keeps type groups in first-insertion order, with
                // ChartComparer ordering inside each group.
                let layout = self.autogen_source.groups.iter().find(|layout| {
                    self.autogen_source
                        .layouts
                        .iter()
                        .enumerate()
                        .any(|(chart, candidate)| candidate == *layout && valid(chart))
                })?;
                self.autogen_source
                    .order
                    .iter()
                    .filter_map(|id| {
                        session
                            .chart_ids()
                            .iter()
                            .position(|candidate| candidate == id)
                    })
                    .find(|chart| {
                        valid(*chart) && self.autogen_source.layouts.get(*chart) == Some(layout)
                    })
            })
    }
    pub(super) fn sync_autogen_source(&mut self) {
        let showing = self.autogen_source.showing;
        if !self.has_song() {
            self.autogen_source = Runtime {
                showing,
                ..Default::default()
            };
            return;
        }
        let session = &self.workspace.file().session;
        if self
            .autogen_source
            .binding
            .as_ref()
            .is_none_or(|binding| !binding.belongs_to(session))
        {
            self.autogen_source = Runtime {
                showing,
                binding: Some(SessionBinding::capture(session)),
                ..Default::default()
            };
        }
        if self
            .autogen_source
            .groups_binding
            .as_ref()
            .is_none_or(|binding| !binding.is_current(session))
        {
            // EditorSong keeps first-insertion type groups, removes an empty group,
            // and appends a re-added group. Preserve that order through UI close/Undo.
            let layouts = (0..session.chart_ids().len())
                .map(|chart| {
                    deadsync_editor::chart_order::Key::read(
                        session.document(),
                        chart,
                        0,
                        session.chart_ids()[chart].clone(),
                    )
                    .map(|key| key.steps_type)
                    .unwrap_or_else(|_| session.document().charts()[chart].steps_type.clone())
                })
                .collect::<Vec<_>>();
            self.autogen_source
                .groups
                .retain(|layout| layouts.contains(layout));
            for layout in &layouts {
                if !self.autogen_source.groups.contains(layout) {
                    self.autogen_source.groups.push(layout.clone());
                }
            }
            self.autogen_source.layouts = layouts;
            self.autogen_source.groups_binding = Some(SessionBinding::capture(session));
        }
        if !showing {
            return;
        }
        if self
            .autogen_source
            .eligibility
            .as_ref()
            .is_none_or(|binding| !binding.is_current(session))
        {
            self.autogen_source.candidates = (0..session.chart_ids().len())
                .filter(|chart| ChartTask::supports_source(session, *chart))
                .map(|chart| session.chart_ids()[chart].clone())
                .collect();
            self.autogen_source.eligibility = Some(SessionBinding::capture(session));
        }
        let language = i18n::revision();
        if self.autogen_source.order_language != language
            || self
                .autogen_source
                .order_binding
                .as_ref()
                .is_none_or(|binding| !binding.is_current(session))
        {
            self.autogen_source.request = Some(OrderRequest {
                binding: SessionBinding::capture(session),
                language,
            });
        }
        let selected = self
            .autogen_source_chart()
            .and_then(|chart| self.workspace.file().session.chart_ids().get(chart))
            .cloned();
        // Commit fallback to the UI owner: a removed chart must not be resurrected as
        // selected merely because Undo later restores its stable chart identity.
        self.autogen_source.selected = selected;
    }
    pub(super) fn adjust_autogen_source(&mut self, forward: bool) {
        if self.chart_properties.is_some() {
            self.status = i18n::tr("Edition", "AutogenSourceExpressionPending");
            return;
        }
        self.autogen_source.showing = true;
        self.sync_autogen_source();
        if !self.autogen_source_order_current() {
            if self.autogen_source.order_error {
                self.autogen_source.order_binding = None;
                self.sync_autogen_source();
            }
            self.status = i18n::tr("Edition", "AutogenSourceLoading");
            return;
        }
        let session = &self.workspace.file().session;
        let choices = self
            .autogen_source
            .order
            .iter()
            .filter_map(|id| {
                session
                    .chart_ids()
                    .iter()
                    .position(|candidate| candidate == id)
            })
            .filter(|chart| {
                self.autogen_source
                    .candidates
                    .contains(&session.chart_ids()[*chart])
            })
            .collect::<Vec<_>>();
        if choices.is_empty() {
            return;
        }
        let index = self
            .autogen_source_chart()
            .and_then(|chart| choices.iter().position(|candidate| *candidate == chart))
            .unwrap_or(0);
        let next = (index + if forward { 1 } else { choices.len() - 1 }) % choices.len();
        self.autogen_source.selected = Some(session.chart_ids()[choices[next]].clone());
    }
    pub(super) fn autogen_source_label(&self) -> String {
        let Some(chart) = self.autogen_source_chart() else {
            return i18n::tr(
                "Edition",
                if self.autogen_source.request.is_some() {
                    "AutogenSourceLoading"
                } else if self.autogen_source.order_error {
                    "ChartOrderFailed"
                } else {
                    "AutogenNoSource"
                },
            )
            .to_string();
        };
        let document = self.workspace.file().session.document();
        let descriptor = &document.charts()[chart];
        let description = deadsync_editor::chart_text::Text::read(document, chart)
            .ok()
            .map(|text| {
                text.get(deadsync_editor::chart_text::Field::Description)
                    .to_owned()
            })
            .unwrap_or_default();
        let identity = deadsync_editor::chart_identity::Identity::read(document, chart).ok();
        let difficulty = identity
            .as_ref()
            .map(|identity| identity.raw_difficulty())
            .unwrap_or_default();
        let meter = identity
            .as_ref()
            .map(|identity| identity.rating().to_string())
            .unwrap_or_default();
        format!(
            "#{} {} {} {} {}",
            chart + 1,
            descriptor.steps_type,
            difficulty,
            meter,
            description
        )
        .trim()
        .to_owned()
    }
    /// Catalog and ChartProperties drafts remain bound to their chosen source even
    /// if chart focus changes. Missing/replaced documents fail, never retarget.
    pub(super) fn capture_autogen_catalog_chart(
        &mut self,
        target: ProfileTarget,
    ) -> Result<usize, deadsync_editor::Error> {
        self.autogen_source.catalog_target = None;
        self.autogen_source.catalog_binding = None;
        if matches!(target, ProfileTarget::Conversion) && self.autogen_source.showing {
            let chart = self.autogen_source_chart().ok_or_else(|| {
                deadsync_editor::Error::InvalidEdit("No eligible autogen source chart".into())
            })?;
            let session = &self.workspace.file().session;
            self.autogen_source.catalog_target = Some(session.chart_ids()[chart].clone());
            self.autogen_source.catalog_binding = Some(SessionBinding::capture(session));
            return Ok(chart);
        }
        self.profile_catalog_chart()
    }
    pub(super) fn profile_catalog_chart(&self) -> Result<usize, deadsync_editor::Error> {
        let session = &self.workspace.file().session;
        if self.autogen_expression_back.is_none() {
            if let Some(id) = self
                .autogen_source
                .catalog_target
                .as_ref()
                .filter(|_| matches!(self.catalog_target, ProfileTarget::Conversion))
            {
                return self
                    .autogen_source
                    .catalog_binding
                    .as_ref()
                    .filter(|binding| binding.belongs_to(session))
                    .and_then(|_| {
                        session
                            .chart_ids()
                            .iter()
                            .position(|candidate| candidate == id)
                    })
                    .ok_or_else(|| {
                        deadsync_editor::Error::InvalidEdit(
                            "Autogen profile source no longer exists in this document".into(),
                        )
                    });
            }
            return Ok(self.workspace.chart());
        }
        self.autogen_source
            .binding
            .as_ref()
            .filter(|binding| binding.belongs_to(session))
            .and_then(|_| self.autogen_source.expression_target.as_ref())
            .and_then(|id| {
                session
                    .chart_ids()
                    .iter()
                    .position(|candidate| candidate == id)
            })
            .ok_or_else(|| {
                deadsync_editor::Error::InvalidEdit(
                    "Autogen source expression chart no longer exists in this document".into(),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadsync_editor::generation::metadata::ChartData;
    struct Host;
    impl deadsync_editor::chart_order::Orderer for Host {
        fn order(
            &mut self,
            snapshot: &deadsync_editor::chart_order::Snapshot,
        ) -> Result<Vec<usize>, deadsync_editor::Error> {
            Ok(snapshot.source_indices().to_vec())
        }
    }
    fn open(state: &mut State) {
        state.open_generation();
        state.process_chart_order(&mut Host);
    }
    fn fixture() -> (tempfile::TempDir, State) {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.ssc");
        std::fs::write(&path,b"#VERSION:0.83;#TITLE:Independent;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:Focused A;#DIFFICULTY:Hard;#METER:9;#NOTES:1000\n0100\n0010\n0001;#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:Chosen B;#DIFFICULTY:Hard;#METER:9;#NOTES:1000\n0000\n0100\n0000;").unwrap();
        (root, State::open(&path, 0).unwrap())
    }
    fn wait_catalog(state: &mut State) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while state.catalog_job.is_some() {
            state.update();
            assert!(std::time::Instant::now() < deadline, "catalog timed out");
            std::thread::yield_now();
        }
        assert!(state.profile_selection.is_some(), "{}", state.status);
    }
    fn wait_generation(state: &mut State) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while state.job.is_some() {
            state.update();
            assert!(std::time::Instant::now() < deadline, "generation timed out");
            std::thread::yield_now();
        }
        assert!(state.preview.is_some(), "{}", state.status);
    }
    #[test]
    fn actual_picker_expression_generation_preview_apply_and_history_keep_independent_source() {
        let (_root, mut state) = fixture();
        state.navigate_exact_position(24.5, true);
        state.take_audio_commands();
        let bytes = state.workspace.file().session.serialize().unwrap();
        let revision = state.workspace.file().session.revision();
        open(&mut state);
        state.panel = Some(Panel::Generation(7));
        state.panel_key(KeyCode::ArrowRight);
        assert_eq!(state.autogen_source_chart(), Some(1));
        assert_eq!(state.workspace.chart(), 0);
        assert_eq!(state.focus_position.as_ref().unwrap().row(), 24.5);
        assert_eq!(state.workspace.file().session.serialize().unwrap(), bytes);
        assert_eq!(state.workspace.file().session.revision(), revision);
        assert!(state.workspace.file().session.undo_label().is_none());
        assert!(state.audio_commands.is_empty());
        state.open_autogen_source_expression(Panel::Generation(6));
        wait_catalog(&mut state);
        let chosen: Guid = "0c0ba200-8f90-4060-8912-e9ea65831ebc".parse().unwrap();
        let settings = state.profile_selection.as_mut().unwrap();
        settings.expressed = Some(
            settings
                .catalog
                .entries(ProfileKind::Expressed)
                .iter()
                .position(|entry| entry.guid == chosen)
                .unwrap(),
        );
        state.stage_chart_expression();
        assert!(state.chart_properties.is_none(), "{}", state.status);
        assert_eq!(state.workspace.chart(), 0);
        assert_eq!(state.focus_position.as_ref().unwrap().row(), 24.5);
        assert_eq!(
            ChartData::existing_or_initial(state.workspace.file().session.document(), 1)
                .unwrap()
                .expressed_chart_config,
            chosen
        );
        assert_ne!(
            ChartData::existing_or_initial(state.workspace.file().session.document(), 0)
                .unwrap()
                .expressed_chart_config,
            chosen
        );
        state.command(Command::Undo);
        assert_eq!(state.workspace.file().session.serialize().unwrap(), bytes);
        state.command(Command::Redo);
        let configured = state.workspace.file().session.serialize().unwrap();
        state.autogen_preferences.target =
            deadsync_editor::autogen_preferences::Target::from_layout("dance-double").unwrap();
        state.start_generation();
        wait_generation(&mut state);
        let Some(GenerationPreview::Chart(preview)) = &state.preview else {
            panic!("chart preview");
        };
        assert_eq!(preview.source_chart(), 1);
        assert_eq!(preview.report().source_notes, 2);
        assert_eq!(state.workspace.chart(), 0);
        assert_eq!(
            state.workspace.file().session.serialize().unwrap(),
            configured
        );
        state.panel = Some(Panel::GenerationPreview(1));
        state.panel_key(KeyCode::Enter);
        assert!(state.preview.is_none());
        assert_eq!(state.workspace.chart(), 0);
        assert_eq!(
            state.workspace.file().session.serialize().unwrap(),
            configured
        );
        open(&mut state);
        state.adjust_autogen_source(true);
        state.start_generation();
        wait_generation(&mut state);
        assert!(
            matches!(state.preview,Some(GenerationPreview::Chart(ref preview))if preview.source_chart()==1)
        );
        state.panel = Some(Panel::GenerationPreview(0));
        state.panel_key(KeyCode::Enter);
        assert!(state.preview.is_none(), "{}", state.status);
        assert_eq!(state.workspace.chart(), 2);
        assert_eq!(state.workspace.file().session.document().charts().len(), 3);
        state.command(Command::Undo);
        assert_eq!(state.workspace.chart(), 0);
        assert_eq!(
            state.workspace.file().session.serialize().unwrap(),
            configured
        );
        state.command(Command::Redo);
        assert_eq!(state.workspace.chart(), 2);
    }
    #[test]
    fn stable_source_identity_survives_reindex_but_removal_fallback_does_not_resurrect_and_new_document_rebinds()
     {
        let (root, mut state) = fixture();
        open(&mut state);
        state.adjust_autogen_source(true);
        let original = state.autogen_source.selected.clone().unwrap();
        assert_eq!(state.autogen_source_chart(), Some(1));
        let draft = state.workspace.deletion_draft().unwrap();
        state.workspace.delete_chart(&draft).unwrap();
        state.refresh();
        state.process_chart_order(&mut Host);
        assert_eq!(state.autogen_source.selected, Some(original.clone()));
        assert_eq!(state.autogen_source_chart(), Some(0));
        state.workspace.execute(Command::Undo).unwrap();
        state.refresh();
        state.process_chart_order(&mut Host);
        assert_eq!(state.autogen_source_chart(), Some(1));
        state.workspace.execute(Command::SelectChart(1)).unwrap();
        let draft = state.workspace.deletion_draft().unwrap();
        state.workspace.delete_chart(&draft).unwrap();
        state.refresh();
        state.process_chart_order(&mut Host);
        let fallback = state.autogen_source.selected.clone().unwrap();
        assert_ne!(fallback, original);
        state.workspace.execute(Command::Undo).unwrap();
        state.refresh();
        state.process_chart_order(&mut Host);
        assert_eq!(state.autogen_source.selected, Some(fallback));
        assert_eq!(state.autogen_source_chart(), Some(0));
        state.workspace.clear_active_chart();
        open(&mut state);
        assert_eq!(state.autogen_source_chart(), Some(0));
        assert!(!state.workspace.has_chart());
        let path = root.path().join("another.ssc");
        std::fs::write(
            &path,
            b"#VERSION:0.83;#BPMS:0=140;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;",
        )
        .unwrap();
        state.workspace = Workspace::open(&path, 0).unwrap();
        state.sync_autogen_source();
        state.process_chart_order(&mut Host);
        assert_ne!(state.autogen_source.selected, Some(original));
        assert!(
            state
                .autogen_source
                .binding
                .as_ref()
                .unwrap()
                .belongs_to(&state.workspace.file().session)
        );
        assert_eq!(state.autogen_source_chart(), Some(0));
    }
    #[test]
    fn source_fallback_skips_multiplayer_and_cancel_or_pending_expression_never_changes_focus() {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.ssc");
        std::fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:00000000;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        let mut state = State::open(&path, 0).unwrap();
        let bytes = state.workspace.file().session.serialize().unwrap();
        open(&mut state);
        assert_eq!(state.autogen_source_chart(), Some(1));
        assert_eq!(state.workspace.chart(), 0);
        state.open_autogen_source_expression(Panel::Generation(6));
        wait_catalog(&mut state);
        let target = state.autogen_source.selected.clone();
        state.adjust_autogen_source(true);
        assert_eq!(state.autogen_source.selected, target);
        assert!(state.cancel_autogen_expression());
        assert_eq!(state.workspace.file().session.serialize().unwrap(), bytes);
        assert_eq!(state.workspace.chart(), 0);
        state.panel = Some(Panel::Generation(4));
        state.panel_key(KeyCode::Enter);
        assert!(state.panel.is_none());
        open(&mut state);
        assert_eq!(state.autogen_source_chart(), Some(1));
        let path = root.path().join("none.ssc");
        std::fs::write(
            &path,
            b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:00000000;",
        )
        .unwrap();
        let mut none = State::open(&path, 0).unwrap();
        let bytes = none.workspace.file().session.serialize().unwrap();
        none.open_generation();
        none.process_chart_order(&mut Host);
        assert_eq!(none.autogen_source_chart(), None);
        none.start_generation();
        assert!(none.job.is_none());
        assert_eq!(none.workspace.file().session.serialize().unwrap(), bytes);
    }
    #[test]
    fn stale_expression_of_independent_source_refuses_without_retargeting_focused_chart() {
        let (_root, mut state) = fixture();
        open(&mut state);
        state.adjust_autogen_source(true);
        state.open_autogen_source_expression(Panel::Generation(6));
        wait_catalog(&mut state);
        let chosen: Guid = "0c0ba200-8f90-4060-8912-e9ea65831ebc".parse().unwrap();
        let settings = state.profile_selection.as_mut().unwrap();
        settings.expressed = Some(
            settings
                .catalog
                .entries(ProfileKind::Expressed)
                .iter()
                .position(|entry| entry.guid == chosen)
                .unwrap(),
        );
        state.workspace.execute(Command::Place(2)).unwrap();
        let bytes = state.workspace.file().session.serialize().unwrap();
        state.stage_chart_expression();
        assert!(state.chart_properties.is_some());
        assert_eq!(state.workspace.chart(), 0);
        assert_eq!(state.workspace.file().session.serialize().unwrap(), bytes);
        assert_ne!(
            ChartData::existing_or_initial(state.workspace.file().session.document(), 1)
                .unwrap()
                .expressed_chart_config,
            chosen
        );
        assert!(state.cancel_autogen_expression());
        assert_eq!(state.workspace.file().session.serialize().unwrap(), bytes);
    }
    #[test]
    fn chosen_source_expression_applies_without_creating_a_chart_view_when_none_is_focused() {
        let (_root, mut state) = fixture();
        state.workspace.clear_active_chart();
        open(&mut state);
        state.adjust_autogen_source(true);
        assert_eq!(state.autogen_source_chart(), Some(1));
        state.open_autogen_source_expression(Panel::Generation(6));
        wait_catalog(&mut state);
        let chosen: Guid = "0c0ba200-8f90-4060-8912-e9ea65831ebc".parse().unwrap();
        let settings = state.profile_selection.as_mut().unwrap();
        settings.expressed = Some(
            settings
                .catalog
                .entries(ProfileKind::Expressed)
                .iter()
                .position(|entry| entry.guid == chosen)
                .unwrap(),
        );
        state.stage_chart_expression();
        assert!(state.chart_properties.is_none(), "{}", state.status);
        assert!(!state.workspace.has_chart());
        assert_eq!(
            ChartData::existing_or_initial(state.workspace.file().session.document(), 1)
                .unwrap()
                .expressed_chart_config,
            chosen
        );
    }

    #[test]
    fn first_type_group_survives_removing_its_first_chart_and_empty_group_readd_moves_to_end() {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("groups.ssc");
        std::fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;#NOTEDATA:;#STEPSTYPE:dance-double;#NOTES:00000000;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;").unwrap();
        let mut state = State::open(&path, 0).unwrap();
        state.open_chart_deletion();
        state.process_chart_order(&mut Host);
        state.deletion_key(1, KeyCode::Enter);
        assert!(state.deletion.is_none(), "{}", state.status);
        state.workspace.clear_active_chart();
        open(&mut state);
        assert_eq!(
            state.autogen_source_chart(),
            Some(1),
            "retained single group precedes double although its first source block was deleted"
        );
        state.close_generation_source();
        state.workspace.execute(Command::SelectChart(1)).unwrap();
        state.open_chart_deletion();
        state.process_chart_order(&mut Host);
        state.deletion_key(1, KeyCode::Enter);
        assert!(state.deletion.is_none(), "{}", state.status);
        state.workspace.execute(Command::Undo).unwrap();
        state.refresh();
        state.workspace.clear_active_chart();
        open(&mut state);
        assert_eq!(
            state.autogen_source_chart(),
            Some(0),
            "source removes empty group, then re-add appends group after remaining double"
        );
    }
    #[test]
    fn picker_cycle_uses_actual_host_order_without_changing_focus_or_selection_when_order_is_pending()
     {
        struct Reverse;
        impl deadsync_editor::chart_order::Orderer for Reverse {
            fn order(
                &mut self,
                snapshot: &deadsync_editor::chart_order::Snapshot,
            ) -> Result<Vec<usize>, deadsync_editor::Error> {
                Ok(snapshot.source_indices().iter().rev().copied().collect())
            }
        }
        let (_root, mut state) = fixture();
        state.workspace.clear_active_chart();
        state.open_generation();
        let bytes = state.workspace.file().session.serialize().unwrap();
        state.adjust_autogen_source(true);
        assert_eq!(state.autogen_source_chart(), None);
        state.process_chart_order(&mut Reverse);
        assert_eq!(state.autogen_source_chart(), Some(1));
        state.adjust_autogen_source(true);
        assert_eq!(state.autogen_source_chart(), Some(0));
        assert!(!state.workspace.has_chart());
        assert_eq!(state.workspace.file().session.serialize().unwrap(), bytes);
        assert!(state.workspace.file().session.undo_label().is_none());
    }

    struct ProfileOrder;
    impl profile_catalog::Orderer for ProfileOrder {
        fn order(
            &mut self,
            catalog: &profile_catalog::Catalog,
        ) -> Result<Vec<(ProfileKind, Vec<(Guid, String)>)>, deadsync_editor::Error> {
            Ok(profile_catalog::KINDS
                .into_iter()
                .map(|kind| {
                    (
                        kind,
                        catalog
                            .entries(kind)
                            .iter()
                            .rev()
                            .map(|entry| {
                                (entry.guid.clone(), entry.name.clone().unwrap_or_default())
                            })
                            .collect(),
                    )
                })
                .collect())
        }
    }
    fn selected_performed(state: &State) -> Guid {
        let settings = state.profile_selection.as_ref().unwrap();
        settings.catalog.entries(ProfileKind::Performed)[settings.performed.unwrap() - 1]
            .guid
            .clone()
    }
    #[test]
    fn independent_source_performed_catalog_keyboard_sort_stage_and_generate_without_active_view() {
        for no_active in [false, true] {
            let (_root, mut state) = fixture();
            state.navigate_exact_position(24.5, true);
            if no_active {
                state.workspace.clear_active_chart();
            }
            let before = state.workspace.file().session.serialize().unwrap();
            let revision = state.workspace.file().session.revision();
            state.take_audio_commands();
            open(&mut state);
            state.adjust_autogen_source(true);
            assert_eq!(state.autogen_source_chart(), Some(1));
            let source = state.autogen_source.selected.clone();
            state.panel = Some(Panel::Generation(5));
            state.panel_key(KeyCode::Enter);
            wait_catalog(&mut state);
            assert_eq!(state.profile_catalog_chart().unwrap(), 1);
            assert!(
                state
                    .profile_selection
                    .as_ref()
                    .unwrap()
                    .catalog
                    .is_current(&state.workspace.file().session, 1)
            );
            assert!(state.process_profile_order("en", &mut ProfileOrder));
            for _ in 0..=state
                .profile_selection
                .as_ref()
                .unwrap()
                .catalog
                .entries(ProfileKind::Performed)
                .len()
            {
                if state.profile_selection.as_ref().unwrap().performed == Some(0) {
                    break;
                }
                state.panel_key(KeyCode::ArrowRight);
            }
            state.panel_key(KeyCode::ArrowRight);
            let chosen = selected_performed(&state);
            state.panel_key(KeyCode::F9);
            assert_eq!(state.profile_sort.specs[&ProfileKind::Performed].len(), 1);
            assert!(state.process_profile_order("en", &mut ProfileOrder));
            assert_eq!(selected_performed(&state), chosen);
            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
            assert_eq!(state.workspace.file().session.revision(), revision);
            assert_eq!(state.workspace.has_chart(), !no_active);
            assert_eq!(state.autogen_source.selected, source);
            state.panel = Some(Panel::ConversionProfiles(2));
            state.panel_key(KeyCode::Enter);
            assert!(state.profile_selection.is_none(), "{}", state.status);
            assert_eq!(state.generation.profiles.performed, Some(chosen));
            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
            assert!(state.workspace.file().session.undo_label().is_none());
            assert_eq!(state.workspace.has_chart(), !no_active);
            assert!(state.take_audio_commands().is_empty());
            state.start_generation();
            wait_generation(&mut state);
            assert!(
                matches!(state.preview,Some(GenerationPreview::Chart(ref preview))if preview.source_chart()==1)
            );
            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
            assert_eq!(state.workspace.has_chart(), !no_active);
            state.panel = Some(Panel::GenerationPreview(0));
            state.panel_key(KeyCode::Enter);
            assert!(state.preview.is_none(), "{}", state.status);
            assert_eq!(state.workspace.chart(), 2);
            assert_eq!(state.workspace.file().session.document().charts().len(), 3);
            state.command(Command::Undo);
            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
            assert_eq!(state.workspace.has_chart(), !no_active);
        }
    }
    #[test]
    fn independent_catalog_owner_survives_focus_change_but_stale_and_foreign_catalogs_refuse_stage()
    {
        let (root, mut state) = fixture();
        open(&mut state);
        state.adjust_autogen_source(true);
        state.open_conversion_profiles();
        wait_catalog(&mut state);
        assert_eq!(state.profile_catalog_chart().unwrap(), 1);
        state.workspace.execute(Command::SelectChart(1)).unwrap();
        state.workspace.clear_active_chart();
        state.panel = Some(Panel::ConversionProfiles(0));
        for _ in 0..=state
            .profile_selection
            .as_ref()
            .unwrap()
            .catalog
            .entries(ProfileKind::Performed)
            .len()
        {
            if state.profile_selection.as_ref().unwrap().performed == Some(0) {
                break;
            }
            state.panel_key(KeyCode::ArrowRight);
        }
        state.panel_key(KeyCode::ArrowRight);
        let chosen = selected_performed(&state);
        state.panel_key(KeyCode::F9);
        assert!(state.process_profile_order("en", &mut ProfileOrder));
        assert_eq!(selected_performed(&state), chosen);
        let before = state.workspace.file().session.serialize().unwrap();
        state.stage_conversion_profiles();
        assert!(state.profile_selection.is_none(), "{}", state.status);
        assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
        state.open_conversion_profiles();
        wait_catalog(&mut state);
        state.workspace.execute(Command::SelectChart(0)).unwrap();
        state.workspace.execute(Command::Place(2)).unwrap();
        let changed = state.workspace.file().session.serialize().unwrap();
        let profiles = state.generation.profiles.clone();
        state.stage_conversion_profiles();
        assert!(state.profile_selection.is_some());
        assert_eq!(state.generation.profiles.performed, profiles.performed);
        assert_eq!(state.workspace.file().session.serialize().unwrap(), changed);
        let other = root.path().join("foreign.ssc");
        std::fs::write(
            &other,
            b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
        )
        .unwrap();
        state.workspace = Workspace::open(&other, 0).unwrap();
        let foreign = state.workspace.file().session.serialize().unwrap();
        assert!(state.profile_catalog_chart().is_err());
        state.stage_conversion_profiles();
        assert!(state.profile_selection.is_some());
        assert_eq!(state.workspace.file().session.serialize().unwrap(), foreign);
    }
}
