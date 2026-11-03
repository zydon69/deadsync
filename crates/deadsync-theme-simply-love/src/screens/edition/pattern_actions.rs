//! Optional source Pattern Selected/All actions, reusing existing background jobs.
use super::{GenerationJob, Panel, State};
use crate::i18n;
use deadlib_platform::input::KeyCode;
use deadsync_editor::pattern_actions::{Action, Plan, Refusal, Worker};
#[derive(Default)]
pub(super) struct Settings {
    action: usize,
    checked: bool,
    from_picker: bool,
    plan: Option<Plan>,
    seeds_committed: bool,
}
impl State {
    pub(super) fn command_pattern_action_review(&mut self, action: Action, checked: bool) {
        if self
            .marker
            .as_ref()
            .is_some_and(|draft| draft.has_changes())
        {
            self.status = i18n::tr("Edition", "MarkerPending");
            return;
        }
        if checked && self.event_selection.is_none() {
            self.status = i18n::tr("Edition", "PatternActionsNoSelection");
            return;
        }
        self.open_pattern_actions(checked);
        if !matches!(self.panel, Some(Panel::PatternActions(_))) {
            return;
        }
        self.pattern_actions.action = match action {
            Action::GenerateFixed => 0,
            Action::GenerateNewSeeds => 1,
            Action::Clear => 2,
        };
        self.pattern_actions.checked = checked;
        self.prepare_pattern_action_review();
        if matches!(self.panel, Some(Panel::PatternActions(_))) {
            self.panel = Some(Panel::PatternActions(2));
        }
    }

    pub(super) fn open_pattern_actions(&mut self, from_picker: bool) {
        if self.is_generating() {
            return;
        }
        self.pattern_actions = Settings {
            checked: from_picker,
            from_picker,
            ..Default::default()
        };
        self.panel = Some(Panel::PatternActions(0));
        self.status = i18n::tr("Edition", "PatternActionsHelp");
    }
    pub(super) fn pattern_actions_key(&mut self, index: usize, code: KeyCode) {
        match code {
            KeyCode::Escape => self.close_pattern_actions(),
            KeyCode::ArrowUp => self.panel = Some(Panel::PatternActions((index + 5) % 6)),
            KeyCode::ArrowDown | KeyCode::Tab => {
                self.panel = Some(Panel::PatternActions((index + 1) % 6))
            }
            KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Enter if index < 2 => {
                if index == 0 {
                    self.pattern_actions.action = (self.pattern_actions.action
                        + if code == KeyCode::ArrowLeft { 2 } else { 1 })
                        % 3;
                } else if self.event_selection.is_some() {
                    self.pattern_actions.checked = !self.pattern_actions.checked;
                }
                self.pattern_actions.plan = None;
            }
            KeyCode::Enter if index == 2 => {
                self.prepare_pattern_action_review();
            }
            KeyCode::Enter if index == 4 => {
                let Some(plan) = self.pattern_actions.plan.take() else {
                    self.status = i18n::tr("Edition", "NoteOperationPrepareFirst");
                    return;
                };
                let review = plan.review().clone();
                let before = match self.capture_song_property_presentation() {
                    Ok(before) => before,
                    Err(error) => { self.status = i18n::editor_error(&error).into(); return; }
                };
                let admission = self.workspace.begin_pattern_action_with_presentation_receipt(
                    plan,
                    &review,
                    self.profile_directory.as_deref(),
                );
                if let Some(receipt) = admission.presentation_receipt() {
                    if let Err(error) = self.rebind_song_property_presentation(before, receipt) {
                        self.status = i18n::editor_error(&error).into();
                        return;
                    }
                }
                match admission.result {
                    Ok(started) => {
                        self.pattern_actions.seeds_committed = started.seeds_committed;
                        let job = match started.worker {
                            Worker::Generate(task) => task.spawn().map(GenerationJob::PatternSet),
                            Worker::Clear(task) => task.spawn().map(GenerationJob::Clear),
                        };
                        match job {
                            Ok(job) => {
                                self.event_selection = None;
                                self.preview = None;
                                self.job = Some(job);
                                self.panel = Some(Panel::Generating);
                                self.status = i18n::tr("Edition", "GenerationPreparing");
                                self.refresh();
                            }
                            Err(_) => {
                                self.status = i18n::tr(
                                    "Edition",
                                    if started.seeds_committed {
                                        "PatternActionsSeedsKept"
                                    } else {
                                        "PatternActionsFailed"
                                    },
                                );
                                self.pattern_actions.seeds_committed = false;
                            }
                        }
                    }
                    Err(e) => {
                        self.status = i18n::tr(
                            "Edition",
                            if e.seeds_committed {
                                "PatternActionsSeedsKept"
                            } else {
                                pattern_failure(e.reason)
                            },
                        );
                    }
                }
            }
            KeyCode::Enter if index == 5 => self.close_pattern_actions(),
            _ => {}
        }
    }
    fn prepare_pattern_action_review(&mut self) {
        let action = [
            Action::GenerateFixed,
            Action::GenerateNewSeeds,
            Action::Clear,
        ][self.pattern_actions.action];
        if action != Action::Clear && !self.current_timing_for_generation() {
            return;
        }
        let result = if self.pattern_actions.checked {
            match &self.event_selection {
                Some(s) => self
                    .workspace
                    .prepare_checked_pattern_action(&s.selection, action),
                None => {
                    self.status = i18n::tr("Edition", "PatternActionsNoSelection");
                    return;
                }
            }
        } else {
            self.workspace.prepare_all_pattern_action(action)
        };
        match result {
            Ok(plan) => {
                self.pattern_actions.plan = Some(plan);
                self.status = i18n::tr("Edition", "PatternActionsPrepared");
            }
            Err(e) => {
                self.pattern_actions.plan = None;
                self.status = i18n::tr("Edition", pattern_failure(e.reason));
            }
        }
    }
    fn close_pattern_actions(&mut self) {
        let from_picker = self.pattern_actions.from_picker;
        self.pattern_actions = Default::default();
        self.panel = Some(if from_picker {
            Panel::EventSelection
        } else {
            Panel::Regeneration(0)
        });
    }
    pub(super) fn pattern_action_job_finished(&mut self, failed: bool) {
        if failed && self.pattern_actions.seeds_committed {
            self.status = i18n::tr("Edition", "PatternActionsSeedsKept");
            self.pattern_actions.seeds_committed = false;
        }
    }
    pub(super) fn pattern_action_preview_discarded(&mut self) {
        if self.pattern_actions.seeds_committed {
            self.status = i18n::tr("Edition", "PatternActionsSeedsKept");
        }
        self.pattern_actions.seeds_committed = false;
    }
    pub(super) fn pattern_action_preview_applied(&mut self) {
        self.pattern_actions.seeds_committed = false;
    }
    pub(super) fn pattern_actions_label(&self, index: usize, key: &str) -> std::sync::Arc<str> {
        match index {
            0 => i18n::tr_fmt(
                "Edition",
                key,
                &[(
                    "value",
                    &i18n::tr(
                        "Edition",
                        [
                            "PatternActionsFixed",
                            "PatternActionsNew",
                            "PatternActionsClear",
                        ][self.pattern_actions.action],
                    ),
                )],
            ),
            1 => i18n::tr_fmt(
                "Edition",
                key,
                &[(
                    "value",
                    &i18n::tr(
                        "Edition",
                        if self.pattern_actions.checked {
                            "PatternActionsChecked"
                        } else {
                            "PatternActionsAll"
                        },
                    ),
                )],
            ),
            2 => i18n::tr_fmt(
                "Edition",
                key,
                &[(
                    "count",
                    &self
                        .pattern_actions
                        .plan
                        .as_ref()
                        .map_or(0, |p| p.review().rows.len())
                        .to_string(),
                )],
            ),
            3 => i18n::tr(
                "Edition",
                if self.pattern_actions.action == 1 {
                    "PatternActionsSeedHistory"
                } else {
                    "PatternActionsNoSeedChange"
                },
            ),
            _ => i18n::tr("Edition", key),
        }
    }
}
fn pattern_failure(reason: Refusal) -> &'static str {
    match reason {
        Refusal::Empty => "PatternActionsNoSelection",
        Refusal::Stale => "PatternActionsStale",
        Refusal::MissingReview => "NoteOperationPrepareFirst",
        Refusal::Invalid => "PatternActionsFailed",
    }
}

#[cfg(test)]
mod legacy_clear_tests {
    use super::super::GenerationPreview;
    use super::*;
    use deadsync_editor::{
        document::{Document, Format},
        event_clipboard::Event,
        generation::metadata::{ChartData, Definition, PATTERN_BALANCED, SIXTEENTHS},
        history::Session,
        workspace::Command,
    };
    fn fixture() -> (tempfile::TempDir, State) {
        crate::i18n::init_for_tests();
        let text=b"#VERSION:0.60;#BPMS:0=120,1=130;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Edit;#BPMS:2=150;#NOTES:1000\n0100\n0010\n0001;";
        let mut session = Session::new(Document::parse(text.as_slice(), Format::Ssc).unwrap());
        ChartData {
            patterns: Some(
                [0, 96]
                    .into_iter()
                    .map(|row| {
                        (
                            row,
                            Definition {
                                length: 48,
                                start_position_inclusive: true,
                                pattern_config_guid: SIXTEENTHS.parse().unwrap(),
                                performed_chart_config_guid: PATTERN_BALANCED.parse().unwrap(),
                                ..Default::default()
                            },
                        )
                    })
                    .collect(),
            ),
            ..Default::default()
        }
        .save(&mut session, 0)
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("legacy.ssc");
        std::fs::write(&path, session.serialize().unwrap()).unwrap();
        let mut state = State::open(&path, 0).unwrap();
        state.panel = None;
        assert!(
            deadsync_editor::timing_edit::migration::has_legacy_chart_timing(
                state.workspace.file().session.document()
            )
        );
        (dir, state)
    }
    #[test]
    fn legacy_ssc_clear_selected_and_all_need_no_migration_and_undo_restores() {
        for checked in [false, true] {
            let (_dir, mut state) = fixture();
            let before = state.workspace.file().session.serialize().unwrap();
            let inventory=state.workspace.event_selection_between(0,192).unwrap();
            assert!(inventory.events().iter().any(|e|matches!(e,Event::Note(_))));
            assert!(inventory.events().iter().any(|e|matches!(e,Event::Pattern{row:0})));
            assert!(inventory.events().iter().any(|e|matches!(e,Event::Timing{row:48,kind:deadsync_editor::event_clipboard::TimingKind::Tempo})));
            assert!(!inventory.events().iter().any(|e|matches!(e,Event::Timing{row:96,kind:deadsync_editor::event_clipboard::TimingKind::Tempo})),"opaque legacy chart timing is not effective playback timing");
            assert!(deadsync_editor::timing_edit::TempoDraft::capture(&state.workspace.file().session,0,deadsync_editor::timing_edit::Scope::Song).is_err(),"selection must not unlock timing mutations");
            state.workspace.copy_event_selection(&inventory).unwrap();
            assert!(state.workspace.cut_event_selection(&inventory).is_err(),"mixed timing mutation must retain migration admission");
            assert_eq!(state.workspace.file().session.serialize().unwrap(),before);
            if checked {
                let mut selection = state.workspace.event_selection_between(0, 192).unwrap();
                selection.select_all(false);
                for i in 0..selection.events().len() {
                    if matches!(selection.events()[i], Event::Pattern { row: 0 }) {
                        selection.set_selected(i, true).unwrap();
                    }
                }
                state.replace_region_event_selection(selection);
            }
            state.command_pattern_action_review(Action::Clear, checked);
            assert!(matches!(state.panel, Some(Panel::PatternActions(2))));
            assert!(state.timing_migration.is_none());
            let plan = state
                .pattern_actions
                .plan
                .take()
                .expect("clear review without migration");
            assert_eq!(
                plan.review().rows,
                if checked { vec![0] } else { vec![0, 96] }
            );
            let review = plan.review().clone();
            let started = state
                .workspace
                .begin_pattern_action(plan, &review, None)
                .unwrap();
            let Worker::Clear(task) = started.worker else {
                panic!("clear worker")
            };
            let preview = task
                .run(&std::sync::atomic::AtomicBool::new(false), |_, _| {})
                .unwrap();
            state.preview = Some(GenerationPreview::Clear(preview));
            state.panel = Some(Panel::GenerationPreview(0));
            state.panel_key(KeyCode::Enter);
            let after = state.workspace.file().session.serialize().unwrap();
            assert_ne!(after, before);
            assert!(state.timing_migration.is_none());
            assert_eq!(
                state
                    .workspace
                    .file()
                    .session
                    .document()
                    .text(None, "VERSION")
                    .as_deref(),
                Some("0.60")
            );
            state.command(Command::Undo);
            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
            state.command(Command::Redo);
            assert_eq!(state.workspace.file().session.serialize().unwrap(), after);
        }
    }
    #[test]
    fn legacy_ssc_generate_still_routes_to_timing_review() {
        let (_dir, mut state) = fixture();
        let before = state.workspace.file().session.serialize().unwrap();
        state.command_pattern_action_review(Action::GenerateFixed, false);
        assert!(matches!(state.panel, Some(Panel::TimingMigration(_))));
        assert!(state.pattern_actions.plan.is_none());
        assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
    }
}

#[cfg(test)]
mod publication_focus_tests {
    use super::*;
    use super::super::GenerationPreview;
    use deadsync_editor::{document::{Document, Format}, generation::metadata::{ChartData, Definition, PATTERN_BALANCED, SIXTEENTHS}, history::Session, workspace::Command};
    use std::time::{Duration, Instant};

    fn fixture(row: f64) -> (tempfile::TempDir, State) {
        i18n::init_for_tests();
        let mut session = Session::new(Document::parse(b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Edit;#METER:9;#NOTES:1000\n0100\n0010\n0001;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Edit;#METER:9;#NOTES:0001;".as_slice(), Format::Ssc).unwrap());
        ChartData { patterns:Some([0,96].into_iter().map(|row| (row, Definition { length:48, random_seed:42, start_position_inclusive:true, end_position_inclusive:true, pattern_config_guid:SIXTEENTHS.parse().unwrap(), performed_chart_config_guid:PATTERN_BALANCED.parse().unwrap(), ..Default::default() })).collect()), ..Default::default() }.save(&mut session,0).unwrap();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("patterns.ssc");
        std::fs::write(&path, session.serialize().unwrap()).unwrap();
        let mut state = State::open(&path,0).unwrap();
        for chart in state.workspace.file().session.document().charts() {
            assert_eq!(chart.meter.parse::<f64>().unwrap(),9.0,"generation difficulty precondition");
        }
        state.panel = None;
        state.navigate_exact_position(row,true);
        state.take_audio_commands();
        (root,state)
    }
    fn bytes(state: &State) -> Vec<u8> { state.workspace.file().session.serialize().unwrap() }
    fn exact(state: &State) -> (u64,u64,u64) {
        let focus = state.focus_position.as_ref().expect("exact editor focus");
        assert!(focus.is_current(&state.workspace), "focus must stay current");
        (state.workspace.row(),focus.row().to_bits(),focus.music_time().to_bits())
    }
    fn start(state: &mut State, action: Action, checked: bool) {
        if checked {
            let mut selection = state.workspace.event_selection_between(0,192).unwrap();
            selection.select_all(false);
            let index = selection.events().iter().position(|event|matches!(event,deadsync_editor::event_clipboard::Event::Pattern {row:96})).unwrap();
            selection.set_selected(index,true).unwrap();
            state.replace_region_event_selection(selection);
        }
        state.command_pattern_action_review(action,checked);
        assert!(matches!(state.panel,Some(Panel::PatternActions(2))),"{}",state.status);
        assert!(state.pattern_actions.plan.is_some());
        if action == Action::GenerateNewSeeds {
            // Inject source-valid deterministic entropy through the public Plan
            // seam; do not assert that Random.Next cannot redraw an old seed.
            let rows = state.pattern_actions.plan.as_ref().unwrap().review().rows.clone();
            let seeds = (0..rows.len()).map(|index| 142 + index as i32).collect();
            state.pattern_actions.plan = Some(Plan::rows_with_seeds(&state.workspace.file().session, state.workspace.chart(), rows, seeds).unwrap());
        }
        state.pattern_actions_key(4,KeyCode::Enter);
        assert!(state.job.is_some(),"{}",state.status);
    }
    fn publish(state: &mut State) {
        let deadline = Instant::now()+Duration::from_secs(10);
        while state.job.is_some() {
            state.update();
            assert!(Instant::now()<deadline,"{}",state.status);
            std::thread::yield_now();
        }
        assert!(matches!(state.preview,Some(GenerationPreview::PatternSet {..})),"{}",state.status);
        assert!(matches!(state.panel,Some(Panel::GenerationPreview(0))));
    }
    fn finish(state: &mut State, mode:usize) {
        match mode {
            0 => {state.panel_key(KeyCode::Enter);}
            1 => {state.panel_key(KeyCode::ArrowDown);state.panel_key(KeyCode::Enter);}
            _ => {state.panel_key(KeyCode::Escape);}
        }
        assert!(state.preview.is_none(),"{}",state.status);
    }
    #[test]
    fn real_worker_publication_apply_discard_and_escape_preserve_fractional_and_negative_focus() {
        for row in [192.25,-0.75,0.25] { for checked in [false,true] { for mode in 0..3 {
            let (_root,mut state)=fixture(row);
            let anchor=exact(&state);
            let before=bytes(&state);
            let revision=state.workspace.file().session.revision();
            start(&mut state,Action::GenerateFixed,checked);
            assert_eq!(exact(&state),anchor);
            assert_eq!(bytes(&state),before);
            publish(&mut state);
            assert_eq!(exact(&state),anchor,"job publication is not navigation");
            assert_eq!(bytes(&state),before);
            assert_eq!(state.workspace.file().session.revision(),revision);
            finish(&mut state,mode);
            assert_eq!(exact(&state),anchor);
            if mode==0 {
                let after=bytes(&state);
                assert_ne!(after,before,"generated notes must be committed");
                state.command(Command::Undo);assert_eq!(bytes(&state),before);assert_eq!(exact(&state),anchor);
                state.command(Command::Redo);assert_eq!(bytes(&state),after);assert_eq!(exact(&state),anchor);
            } else {assert_eq!(bytes(&state),before);assert_eq!(state.workspace.file().session.revision(),revision);}
        }}}
    }
    #[test]
    fn reseeding_keeps_its_separate_undo_on_discard_escape_and_applied_generation() {
        for mode in 0..3 {
            let (_root,mut state)=fixture(192.25);
            let anchor=exact(&state);let before=bytes(&state);
            start(&mut state,Action::GenerateNewSeeds,false);
            let seeded=bytes(&state);assert_ne!(seeded,before);
            publish(&mut state);assert_eq!(bytes(&state),seeded);assert_eq!(exact(&state),anchor);
            finish(&mut state,mode);assert_eq!(exact(&state),anchor);
            if mode==0 {assert_ne!(bytes(&state),seeded);state.command(Command::Undo);assert_eq!(bytes(&state),seeded);assert_eq!(exact(&state),anchor);}
            else {assert_eq!(bytes(&state),seeded,"discard keeps the separate source seed action");}
            state.command(Command::Undo);assert_eq!(bytes(&state),before);assert_eq!(exact(&state),anchor);
        }
    }
    #[test]
    fn explicit_pattern_review_entry_navigation_still_seeks_requested_marker() {
        let (_root,mut state)=fixture(192.25);
        start(&mut state,Action::GenerateFixed,false);publish(&mut state);
        assert_eq!(state.workspace.row(),192);
        state.panel_key(KeyCode::ArrowRight);
        assert_eq!(state.pattern_set_preview().unwrap().1,1);
        assert_eq!(state.workspace.row(),96);
        state.panel_key(KeyCode::ArrowLeft);
        assert_eq!(state.pattern_set_preview().unwrap().1,0);
        assert_eq!(state.workspace.row(),0);
    }
}
