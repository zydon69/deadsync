use super::State;
use crate::i18n;
use deadsync_editor::chart_focus::Transition;
impl State {
    /// Exposes the exact music-file instant for chart/view presentation checks.
    /// It does not claim that a decoder is playing or an audio mapping exists.
    pub fn chart_focus_time(&self) -> Result<f64, deadsync_editor::Error> {
        if !self.workspace.has_chart() {
            if let Some(time) = self.transport_music_time() {
                return Ok(time);
            }
            return Ok(self
                .active_views
                .as_ref()
                .map_or(0.0, |views| views.music_time()));
        }
        self.chart_focus_music_time()
    }
    pub(super) fn focus_chart_at_time(&mut self, chart: usize, music_time: f64) -> bool {
        self.focus_chart_at_time_with_placement(chart, music_time, true)
    }
    pub(super) fn focus_chart_at_time_with_placement(
        &mut self,
        chart: usize,
        music_time: f64,
        preserve_left: bool,
    ) -> bool {
        // Source SetChartFocused returns before touching selection on same focus.
        if self.workspace.active_chart() == Some(chart) {
            return true;
        }
        let previous_left = if preserve_left && self.workspace.active_chart() != Some(chart) {
            self.first_chart_canvas_left()
        } else {
            None
        };
        let prepared = Transition::open_at_time(&self.workspace, chart, music_time);
        let mut success = false;
        match prepared {
            Ok(transition) => {
                // Prepare the target clock before changing focus. The loaded
                // same-file transport retains its owner, generation and lease.
                let audio = self.prepare_chart_focus_audio(chart);
                let paused = self.paused_playback.is_some();
                match transition.apply(&mut self.workspace) {
                    Ok(position) => {
                        self.clear_unfocused_chart_selection();
                        self.apply_chart_focus_audio(audio, paused, &position);
                        self.focus_position = Some(position);
                        success = true;
                        self.status = i18n::tr("Edition", "Ready");
                    }
                    Err(_) => self.status = i18n::tr("Edition", "ChartOrderFailed"),
                }
            }
            Err(_) => self.status = i18n::tr("Edition", "ChartOrderFailed"),
        }
        self.refresh();
        if success {
            if let Err(error) = self.preserve_chart_canvas_left(previous_left) {
                self.status = crate::i18n::editor_error(&error).into();
            }
        }
        success
    }
}

impl State {
    /// ActiveEditorChart.SetFocused(false): selection, region gesture, row
    /// selection and pending lane edits cannot migrate to the next chart.
    pub(super) fn clear_unfocused_chart_selection(&mut self) {
        self.event_selection = None;
        self.region_selection = None;
        self.note_operation_review = Default::default();
        self.selected_transform_review = Default::default();
        self.shift_all = Default::default();
        self.lane_entry_release_input();
        if matches!(
            self.panel,
            Some(
                super::Panel::EventSelection
                    | super::Panel::SelectionPresets(_)
                    | super::Panel::NoteOperationReview(_)
                    | super::Panel::SelectedTransformReview(_)
                    | super::Panel::ShiftAll(_)
            )
        ) {
            self.panel = None;
        }
    }
}
#[cfg(test)]
mod selection_focus_tests {
    use super::*;
    #[test]
    fn real_focus_clears_selection_and_gesture_but_failed_or_same_focus_keeps_them() {
        crate::i18n::init_for_tests();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("two.ssc");
        std::fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        let mut state = State::open(&path, 0).unwrap();
        let before = state.workspace.file().session.serialize().unwrap();
        let mut selection = state.workspace.event_selection_between(0, 192).unwrap();
        selection.select_all(true);
        state.replace_region_event_selection(selection);
        let count = state
            .event_selection
            .as_ref()
            .unwrap()
            .selection
            .selected_count();
        assert!(count > 0);
        state
            .workspace
            .toggle_source_row_selection_at(-12.5)
            .unwrap();
        state.open_region_selection();
        let time = state.chart_focus_time().unwrap();
        assert!(!state.focus_chart_at_time(999, time));
        assert!(state.region_selection.is_some());
        assert_eq!(
            state
                .event_selection
                .as_ref()
                .unwrap()
                .selection
                .selected_count(),
            count
        );
        assert!(state.focus_chart_at_time(0, time));
        assert!(state.region_selection.is_some());
        assert!(
            state
                .workspace
                .source_row_selection_at(12.5)
                .unwrap()
                .is_some()
        );
        assert!(state.focus_chart_at_time(1, time));
        assert!(state.event_selection.is_none());
        assert!(state.region_selection.is_none());
        assert!(
            state
                .workspace
                .source_row_selection_at(12.5)
                .unwrap()
                .is_none()
        );
        assert!(
            state
                .lane_entry_model()
                .is_none_or(|model| model.drafts().next().is_none())
        );
        let mut actors = Vec::new();
        state.actors(&mut actors);
        assert!(!actors.iter().any(|actor|matches!(actor,deadlib_present::actors::Actor::Sprite{z:8003,tint,..}if *tint==[0.2,0.6,0.85,0.85])),"B has no borrowed selected-widget highlight");
        state.copy_source_region_selection(false);
        assert_eq!(
            state.status,
            crate::i18n::tr("Edition", "EventSelectionNoChoice")
        );
        assert!(state.focus_chart_at_time(0, time));
        assert!(state.event_selection.is_none());
        assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
        assert!(state.workspace.file().session.undo_label().is_none());
        // A fresh selection in B works normally; no stale-A refusal survives.
        assert!(state.focus_chart_at_time(1, time));
        let mut selected = state.workspace.event_selection_between(0, 192).unwrap();
        selected.select_all(true);
        state.replace_region_event_selection(selected);
        state.copy_source_region_selection(false);
        assert_eq!(
            state.status,
            crate::i18n::tr("Edition", "EventSelectionCopied")
        );
        state.command_selected_transform_review(deadsync_editor::transform::Transform::Mirror);
        assert!(matches!(
            state.panel,
            Some(super::super::Panel::SelectedTransformReview(_))
        ));
        assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
    }
}
