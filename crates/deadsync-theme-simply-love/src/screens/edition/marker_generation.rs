//! UIPatternEvent's Generate button targets its displayed event, not Position.
use super::*;
use deadsync_editor::{
    chart_order::SessionBinding,
    generation::{marker_edit::Draft, metadata::Guid},
};
struct Origin {
    binding: SessionBinding,
    chart: Guid,
    row: u64,
}
#[derive(Default)]
pub(super) struct Runtime {
    origin: Option<Origin>,
}
impl Runtime {
    pub(super) fn active(&self) -> bool {
        self.origin.is_some()
    }
}
impl State {
    pub(super) fn new_inspected_marker_seed(&mut self,generate:bool){
        let seed=deadsync_editor::pattern_actions::new_random_seed();
        self.reseed_inspected_marker(seed,generate);
    }
    fn reseed_inspected_marker(&mut self,seed:i32,generate:bool){
        if self.is_generating(){return;}
        let Some(draft)=self.marker.as_ref() else{return;};
        if draft.has_changes(){self.status=i18n::tr("Edition","MarkerPending");return;}
        let row=draft.start;
        let result=(||->Result<(),deadsync_editor::Error>{
            let before=self.capture_song_property_presentation()?;
            let draft=self.marker.as_ref().unwrap();
            let application=self.workspace.reseed_inspected_marker(draft,seed)?;
            self.rebind_song_property_presentation(before,application.presentation_receipt().expect("typed seed receipt"))?;
            self.refresh();
            self.marker=Some(Draft::at_cursor(&self.workspace.file().session,self.workspace.chart(),row)?);
            self.panel=Some(Panel::Marker(if generate{15}else{14}));
            Ok(())
        })();
        if let Err(error)=result{self.status=i18n::editor_error(&error).into();return;}
        if generate{self.generate_inspected_marker();}
    }
    pub(super) fn generate_inspected_marker(&mut self) {
        if self.is_generating() {
            return;
        }
        let Some(draft) = self.marker.as_ref() else {
            return;
        };
        if draft.has_changes() {
            self.status = i18n::tr("Edition", "MarkerPending");
            return;
        }
        // Validate identity/revision before migration or worker admission.
        let task = match draft.generation_task(
            &self.workspace.file().session,
            self.workspace.chart(),
            self.profile_directory.as_deref(),
        ) {
            Ok(task) => task,
            Err(error) => {
                self.status = crate::i18n::editor_error(&error).into();
                return;
            }
        };
        let row = draft.start;
        if !self.current_timing_for_generation() {
            return;
        }
        match task.spawn() {
            Ok(job) => {
                let session = &self.workspace.file().session;
                self.marker_generation.origin = Some(Origin {
                    binding: SessionBinding::capture(session),
                    chart: session.chart_ids()[self.workspace.chart()].clone(),
                    row,
                });
                self.preview = None;
                self.job = Some(GenerationJob::Pattern(job));
                self.panel = Some(Panel::Generating);
                self.status = i18n::tr("Edition", "GenerationPreparing");
            }
            Err(error) => self.status = crate::i18n::editor_error(&error).into(),
        }
    }
    pub(super) fn finish_marker_generation(&mut self, applied: bool) {
        let Some(origin) = self.marker_generation.origin.take() else {
            return;
        };
        let session = &self.workspace.file().session;
        // Only successful token-validated Apply may advance the content revision.
        if !origin.binding.belongs_to(session)
            || (!applied && !origin.binding.is_current(session))
            || self
                .workspace
                .active_chart()
                .and_then(|chart| session.chart_ids().get(chart))
                != Some(&origin.chart)
        {
            self.marker = None;
            return;
        }
        self.marker = Draft::at_cursor(session, self.workspace.chart(), origin.row)
            .ok()
            .filter(|draft| draft.start == origin.row);
        if self.marker.is_some() {
            self.remember_selected_pattern(origin.row);
            self.panel = Some(Panel::Marker(13));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    pub(super) fn fixture() -> (tempfile::TempDir, State) {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("markers.ssc");
        std::fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#METER:9;#NOTES:1000\n0100\n0010\n0001;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        let mut state = State::open(&path, 0).unwrap();
        for row in [0, 96] {
            let plan = deadsync_editor::quick_add::Plan::prepare(
                &state.workspace.file().session,
                0,
                deadsync_editor::quick_add::Kind::Pattern,
                row as f64,
            )
            .unwrap();
            state.workspace.apply_quick_add(&plan).unwrap();
            state.refresh();
            let mut draft = Draft::at_cursor(&state.workspace.file().session, 0, row).unwrap();
            draft.definition.length = 48;
            draft.definition.random_seed = 42;
            draft.definition.start_position_inclusive = true;
            draft.definition.end_position_inclusive = true;
            state.workspace.apply_marker_edit(&draft, false).unwrap();
            state.refresh();
        }
        assert_eq!(
            state.marker.as_ref().unwrap().start,
            96,
            "actual PatternAdded opens B"
        );
        state.navigate_exact_position(0.25, true);
        state.take_audio_commands();
        (root, state)
    }
    pub(super) fn notes(state: &State, chart: usize) -> deadsync_editor::notes::Grid {
        let session = &state.workspace.file().session;
        let document = deadsync_editor::document::Document::parse_with_context(
            std::sync::Arc::<[u8]>::from(session.serialize().unwrap()),
            session.document().format,
            session.document().load_context(),
        )
        .unwrap();
        let descriptor = &document.charts()[chart];
        deadsync_editor::notes::Grid::parse_layout(
            document.value(descriptor.notes.clone()),
            &descriptor.steps_type,
        )
        .unwrap()
    }
    pub(super) fn press(state: &mut State, code: KeyCode) {
        for pressed in [true, false] {
            state.raw_key(&RawKeyboardEvent {
                code,
                pressed,
                repeat: false,
                timestamp: Instant::now(),
                host_nanos: 0,
            });
        }
    }
    pub(super) fn generate(state: &mut State) {
        state.panel = Some(Panel::Marker(0));
        for _ in 0..13 {
            press(state, KeyCode::ArrowDown);
        }
        assert!(matches!(state.panel, Some(Panel::Marker(13))));
        press(state, KeyCode::Enter);
    }
    pub(super) fn wait(state: &mut State) {
        let deadline = Instant::now() + std::time::Duration::from_secs(10);
        while state.job.is_some() {
            state.update();
            assert!(Instant::now() < deadline, "{}", state.status);
            std::thread::yield_now();
        }
        assert!(
            matches!(state.preview, Some(GenerationPreview::Pattern(_))),
            "{}",
            state.status
        );
    }
    pub(super) fn position(state: &State, phase: &str) {
        assert_eq!(state.workspace.row(), 0,"{phase}: integral cursor");
        assert_eq!(state.focus_position.as_ref().unwrap().row(), 0.25,"{phase}: exact focus");
        if !phase.contains("refusal"){assert!(state.focus_position.as_ref().unwrap().is_current(&state.workspace),"{phase}: focus receipt must be current");}
    }
    #[test]
    fn raw_inspector_generate_targets_b_without_seek_cancel_apply_and_history() {
        let (_root, mut state) = fixture();
        let before = state.workspace.file().session.serialize().unwrap();
        let revision = state.workspace.file().session.revision();
        let old_label = state
            .workspace
            .file()
            .session
            .undo_label()
            .map(str::to_owned);
        let grid = notes(&state, 0);
        let peer = notes(&state, 1);
        generate(&mut state);
        assert!(state.job.is_some());
        position(&state,"admission");
        wait(&mut state);
        position(&state,"preview");
        let Some(GenerationPreview::Pattern(preview)) = &state.preview else {
            unreachable!()
        };
        assert_eq!(preview.generated().start, 96);
        assert_eq!(preview.generated().end, 144);
        assert_ne!(preview.grid(), &grid);
        assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
        assert_eq!(state.workspace.file().session.revision(), revision);
        assert_eq!(
            state
                .workspace
                .file()
                .session
                .undo_label()
                .map(str::to_owned),
            old_label
        );
        assert!(state.take_audio_commands().is_empty());
        press(&mut state, KeyCode::ArrowDown);
        press(&mut state, KeyCode::Enter);
        assert!(state.preview.is_none());
        assert!(matches!(state.panel, Some(Panel::Marker(13))));
        assert_eq!(state.marker.as_ref().unwrap().start, 96);
        position(&state,"Cancel");
        assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
        press(&mut state, KeyCode::Enter);
        wait(&mut state);
        let expected = match &state.preview {
            Some(GenerationPreview::Pattern(p)) => p.grid().clone(),
            _ => unreachable!(),
        };
        press(&mut state, KeyCode::Enter);
        assert!(state.preview.is_none(), "{}", state.status);
        assert!(matches!(state.panel, Some(Panel::Marker(13))));
        assert_eq!(state.marker.as_ref().unwrap().start, 96);
        position(&state,"Apply");
        assert_eq!(notes(&state, 0), expected);
        assert_eq!(notes(&state, 1), peer);
        for (pos, note) in grid.iter().filter(|(pos, _)| pos.row < 96 || pos.row > 144) {
            assert_eq!(expected.get(pos), Some(note));
        }
        let after = state.workspace.file().session.serialize().unwrap();
        state.command(Command::Undo);
        assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
        position(&state,"Undo");
        state.command(Command::Redo);
        assert_eq!(state.workspace.file().session.serialize().unwrap(), after);
        position(&state,"Redo");
    }
    #[test]
    fn pending_deleted_stale_foreign_and_stale_preview_never_retarget_cursor_or_recreate_marker() {
        let (root, mut state) = fixture();
        let before = state.workspace.file().session.serialize().unwrap();
        state.marker.as_mut().unwrap().start = 108;
        generate(&mut state);
        assert!(state.job.is_none());
        assert_eq!(state.status, i18n::tr("Edition", "MarkerPending"));
        assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
        position(&state,"pending refusal");
        state.marker.as_mut().unwrap().start = 96;
        let draft = state.marker.take().unwrap();
        state.workspace.apply_marker_edit(&draft, true).unwrap();
        state.marker = Some(draft);
        let removed = state.workspace.file().session.serialize().unwrap();
        generate(&mut state);
        assert!(state.job.is_none());
        assert_eq!(state.workspace.file().session.serialize().unwrap(), removed);
        position(&state,"deleted refusal");
        let (_second, mut stale) = fixture();
        stale.workspace.execute(Command::Place(2)).unwrap();
        let changed = stale.workspace.file().session.serialize().unwrap();
        generate(&mut stale);
        assert!(stale.job.is_none());
        assert_eq!(stale.workspace.file().session.serialize().unwrap(), changed);
        let other = root.path().join("other.ssc");
        std::fs::write(
            &other,
            b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
        )
        .unwrap();
        stale.workspace = Workspace::open(&other, 0).unwrap();
        let foreign = stale.workspace.file().session.serialize().unwrap();
        generate(&mut stale);
        assert!(stale.job.is_none());
        assert_eq!(stale.workspace.file().session.serialize().unwrap(), foreign);
        let (_third, mut preview) = fixture();
        generate(&mut preview);
        wait(&mut preview);
        preview.workspace.execute(Command::Place(2)).unwrap();
        let changed = preview.workspace.file().session.serialize().unwrap();
        press(&mut preview, KeyCode::Enter);
        assert!(preview.preview.is_some());
        assert_eq!(
            preview.workspace.file().session.serialize().unwrap(),
            changed
        );
        press(&mut preview, KeyCode::Escape);
        assert!(preview.preview.is_none());
        assert!(preview.marker.is_none());
        assert_eq!(
            preview.workspace.file().session.serialize().unwrap(),
            changed
        );
    }
}

#[cfg(test)]
mod note_preview_presentation_tests {
    use super::*;
    use super::tests::{fixture, notes, press, generate, wait, position};
    #[test]
    fn common_pattern_clear_and_set_apply_rebind_fractional_signed_focus_and_history(){
        for row in [-0.25,0.25,120.5] {
            for operation in 0..3 {
                let (_root,mut state)=super::tests::fixture();
                state.navigate_exact_position(row,true);state.take_audio_commands();
                let before=state.workspace.file().session.serialize().unwrap();
                let cancel=std::sync::atomic::AtomicBool::new(false);
                let preview=match operation {
                    0=>GenerationPreview::Pattern(deadsync_editor::generation::pattern_job::Task::capture_saved(&state.workspace.file().session,0,96,&deadsync_editor::generation::metadata::Registry::builtin()).unwrap().run(&cancel,|_,_|{}).unwrap()),
                    1=>GenerationPreview::Clear(deadsync_editor::generation::clear_patterns::Task::capture(&state.workspace.file().session,0,[96],None).unwrap().run(&cancel,|_,_|{}).unwrap()),
                    _=>GenerationPreview::PatternSet{preview:state.workspace.all_patterns_task(None).unwrap().run(&cancel,|_|{}).unwrap(),entry:0},
                };
                state.preview=Some(preview);state.panel=Some(Panel::GenerationPreview(0));
                state.marker_generation=Runtime::default(); // Common F5/review path, no inspector origin.
                super::tests::press(&mut state,KeyCode::Enter);
                assert!(state.preview.is_none(),"operation {operation}: {}",state.status);
                for phase in ["Apply","Undo","Redo"] {
                    if phase=="Undo"{state.command(Command::Undo);assert_eq!(state.workspace.file().session.serialize().unwrap(),before);}
                    if phase=="Redo"{state.command(Command::Redo);}
                    let focus=state.focus_position.as_ref().unwrap();
                    assert_eq!(focus.row().to_bits(),row.to_bits(),"{operation} {phase}");assert!(focus.is_current(&state.workspace),"{operation} {phase}: current token");
                }
            }
        }
    }
    fn seed(state:&State,row:u64)->i32{
        deadsync_editor::generation::metadata::ChartData::read(state.workspace.file().session.document(),0).unwrap().unwrap().patterns.unwrap()[&row].random_seed
    }
    #[test]
    fn displayed_marker_seed_callbacks_preserve_cursor_and_split_generation_history(){
        let (_root,mut state)=fixture();let original=state.workspace.file().session.serialize().unwrap();let grid=notes(&state,0);
        state.reseed_inspected_marker(7,false);
        assert_eq!(seed(&state,0),42);assert_eq!(seed(&state,96),7);position(&state,"new seed");
        assert_eq!(notes(&state,0),grid);assert_eq!(state.marker.as_ref().unwrap().start,96);assert!(!state.marker.as_ref().unwrap().has_changes());
        state.command(Command::Undo);position(&state,"undo seed");assert_eq!(state.workspace.file().session.serialize().unwrap(),original);
        state.command(Command::Redo);position(&state,"redo seed");assert_eq!(seed(&state,96),7);
        state.marker=Some(Draft::at_cursor(&state.workspace.file().session,0,96).unwrap());
        state.reseed_inspected_marker(8,true);wait(&mut state);position(&state,"reseed preview");
        assert_eq!(seed(&state,96),8);assert_eq!(notes(&state,0),grid);
        state.panel=Some(Panel::GenerationPreview(1));press(&mut state,KeyCode::Enter);
        assert_eq!(seed(&state,96),8);assert_eq!(notes(&state,0),grid);position(&state,"cancel keeps seed");
        generate(&mut state);wait(&mut state);state.panel=Some(Panel::GenerationPreview(0));press(&mut state,KeyCode::Enter);
        position(&state,"apply after reseed");assert_ne!(notes(&state,0),grid);
        state.command(Command::Undo);position(&state,"undo notes only");assert_eq!(seed(&state,96),8);assert_eq!(notes(&state,0),grid);
        state.command(Command::Undo);position(&state,"undo reseed separately");assert_eq!(seed(&state,96),7);
        state.command(Command::Redo);state.command(Command::Redo);position(&state,"redo both");assert_eq!(seed(&state,96),8);assert_ne!(notes(&state,0),grid);
    }
    #[test]
    fn seed_ui_buttons_target_displayed_marker_and_pending_draft_refuses(){
        for regenerate in [false,true]{
            let (_root,mut state)=fixture();state.panel=Some(Panel::Marker(0));
            for _ in 0..if regenerate{15}else{14}{press(&mut state,KeyCode::ArrowDown);}
            assert!(matches!(state.panel,Some(Panel::Marker(i)) if i==if regenerate{15}else{14}));
            let revision=state.workspace.file().session.revision();press(&mut state,KeyCode::Enter);
            assert_ne!(state.workspace.file().session.revision(),revision);assert_eq!(seed(&state,0),42);assert!((0..i32::MAX).contains(&seed(&state,96)));position(&state,"raw seed button");
            if regenerate{wait(&mut state);state.panel=Some(Panel::GenerationPreview(1));press(&mut state,KeyCode::Enter);}
            state.marker.as_mut().unwrap().definition.length+=1;
            let before=state.workspace.file().session.serialize().unwrap();let revision=state.workspace.file().session.revision();
            state.reseed_inspected_marker(99,regenerate);
            assert_eq!(state.workspace.file().session.serialize().unwrap(),before);assert_eq!(state.workspace.file().session.revision(),revision);assert!(state.job.is_none());assert_eq!(state.status,i18n::tr("Edition","MarkerPending"));
        }
    }
    #[test]
    fn repeated_seed_is_explicit_history_without_fake_byte_mutation_and_stale_refuses(){
        let (_root,mut state)=fixture();let before=state.workspace.file().session.serialize().unwrap();let revision=state.workspace.file().session.revision();
        state.reseed_inspected_marker(42,false);assert_eq!(state.workspace.file().session.serialize().unwrap(),before);assert_ne!(state.workspace.file().session.revision(),revision);position(&state,"repeated seed");
        state.command(Command::Undo);assert_eq!(state.workspace.file().session.serialize().unwrap(),before);state.command(Command::Redo);position(&state,"repeated seed redo");
        let stale=Draft::at_cursor(&state.workspace.file().session,0,96).unwrap();
        state.workspace.apply_marker_edit(&stale,true).unwrap();state.refresh();state.marker=Some(stale);
        let before=state.workspace.file().session.serialize().unwrap();let revision=state.workspace.file().session.revision();
        state.reseed_inspected_marker(9,true);assert_eq!(state.workspace.file().session.serialize().unwrap(),before);assert_eq!(state.workspace.file().session.revision(),revision);assert!(state.job.is_none());
        let (_other_root,other)=fixture();state.marker=Some(Draft::at_cursor(&other.workspace.file().session,0,96).unwrap());
        state.reseed_inspected_marker(9,false);assert_eq!(state.workspace.file().session.serialize().unwrap(),before);assert_eq!(state.workspace.file().session.revision(),revision);
    }

    #[test]
    fn new_seed_save_reopen_preserves_inspected_marker_and_other_marker(){
        let (root,mut state)=fixture();let grid=notes(&state,0);
        state.reseed_inspected_marker(123,false);state.command(Command::Save);
        assert!(!state.workspace.file().session.is_dirty(),"{}",state.status);
        let reopened=State::open(&root.path().join("markers.ssc"),0).unwrap();
        assert_eq!(seed(&reopened,96),123);assert_eq!(seed(&reopened,0),42);assert_eq!(notes(&reopened,0),grid);
        position(&state,"saved seed");state.command(Command::Undo);position(&state,"saved seed undo");assert_eq!(seed(&state,96),42);
        state.command(Command::Redo);position(&state,"saved seed redo");assert_eq!(seed(&state,96),123);
    }

}
