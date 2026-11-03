//! Source LastSelectedPatternEvent and OnEvent(s)Added presentation callback.
//! No note-selection change, focus jump, seek, song edit or audio operation.
use super::*;
use deadsync_editor::{
    chart_order::SessionBinding,
    generation::{
        marker_edit::Draft,
        metadata::{ChartData, Guid},
    },
};
#[derive(Default)]
pub(super) struct Runtime {
    binding: Option<SessionBinding>,
    selected: std::collections::BTreeMap<Guid, u64>,
    focused: Option<Guid>,
    revision: u64,
}
impl State {
    pub(super) fn remember_selected_pattern(&mut self, row: u64) {
        let session = &self.workspace.file().session;
        if self
            .pattern_added
            .binding
            .as_ref()
            .is_none_or(|binding| !binding.belongs_to(session))
        {
            self.pattern_added = Runtime {
                binding: Some(SessionBinding::capture(session)),
                ..Default::default()
            };
        }
        if let Some(id) = session.chart_ids().get(self.workspace.chart()) {
            self.pattern_added.selected.insert(id.clone(), row);
        }
    }
    pub(super) fn sync_pattern_added_presentation(&mut self) {
        let effect = self.workspace.take_pattern_added_update();
        let session = &self.workspace.file().session;
        if self
            .pattern_added
            .binding
            .as_ref()
            .is_none_or(|binding| !binding.belongs_to(session))
        {
            self.pattern_added = Runtime {
                binding: Some(SessionBinding::capture(session)),
                ..Default::default()
            };
        }
        self.pattern_added
            .selected
            .retain(|id, _| session.chart_ids().contains(id));
        let mut callback = false;
        if let Some(effect) = effect.filter(|effect| effect.is_current(session)) {
            for (id, row) in effect.additions() {
                let Some(chart) = session
                    .chart_ids()
                    .iter()
                    .position(|candidate| candidate == id)
                else {
                    continue;
                };
                if ChartData::read(session.document(), chart)
                    .ok()
                    .flatten()
                    .and_then(|data| data.patterns)
                    .is_some_and(|patterns| patterns.contains_key(&row))
                {
                    self.pattern_added.selected.insert(id.clone(), row);
                    callback = true;
                }
            }
        }
        let focused = self
            .workspace
            .active_chart()
            .and_then(|chart| session.chart_ids().get(chart))
            .cloned();
        let changed = self.pattern_added.focused != focused
            || self.pattern_added.revision != session.content_revision();
        self.pattern_added.focused = focused.clone();
        self.pattern_added.revision = session.content_revision();
        if !callback && !(changed && matches!(self.panel, Some(Panel::Marker(_)))) {
            return;
        }
        // UIPatternEvent is global, but it displays the focused chart's remembered
        // Pattern. An addition in an unfocused chart must not steal chart focus.
        let row = focused
            .as_ref()
            .and_then(|id| self.pattern_added.selected.get(id))
            .copied();
        self.marker = row
            .and_then(|row| Draft::at_cursor(session, self.workspace.chart(), row).ok())
            .filter(|draft| Some(draft.start) == row);
        // The source inspector is a dock. Its notification must not close the
        // event picker that owns a multi-event edit or clipboard gesture.
        if !matches!(self.panel, Some(Panel::EventSelection)) {
            self.profile_selection = None;
            self.panel = Some(Panel::Marker(0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, State) {
        crate::i18n::init_for_tests();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("patterns.ssc");
        std::fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;").unwrap();
        let state = State::open(&path, 0).unwrap();
        (dir, state)
    }
    #[test]
    fn pattern_add_notification_updates_inspector_without_closing_event_picker() {
        let (_directory, mut state) = fixture();
        state.open_event_selection();
        assert!(matches!(state.panel, Some(Panel::EventSelection)));
        let plan = deadsync_editor::quick_add::Plan::prepare(
            &state.workspace.file().session,
            0,
            deadsync_editor::quick_add::Kind::Pattern,
            48.0,
        )
        .unwrap();
        state.workspace.apply_quick_add(&plan).unwrap();
        state.refresh();
        assert!(matches!(state.panel, Some(Panel::EventSelection)));
        assert!(state.event_selection.is_some());
        assert_eq!(state.marker.as_ref().unwrap().start, 48);
        assert_eq!(state.workspace.row(), 0);
    }
    #[test]
    fn confirmed_quick_add_opens_exact_inspector_and_history_readds_it_without_seeking() {
        let (_dir, mut state) = fixture();
        let before = state.workspace.file().session.serialize().unwrap();
        state.command_quick_add_review(deadsync_editor::quick_add::Kind::Pattern);
        assert!(state.marker.is_none());
        assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
        state.quick_add_key(4, KeyCode::Enter);
        assert!(state.marker.is_none());
        assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
        state.navigate_exact_position(48.75, true);
        let row = state.capture_song_property_presentation().unwrap().row();
        state.command_quick_add_review(deadsync_editor::quick_add::Kind::Pattern);
        assert!(state.marker.is_none());
        state.quick_add_key(3, KeyCode::Enter);
        assert!(matches!(state.panel, Some(Panel::Marker(0))));
        assert_eq!(state.marker.as_ref().unwrap().start, 49);
        assert_eq!(
            state.capture_song_property_presentation().unwrap().row(),
            row
        );
        let bytes = state.workspace.file().session.serialize().unwrap();
        state.sync_pattern_added_presentation();
        assert_eq!(state.workspace.file().session.serialize().unwrap(), bytes);
        state.command(Command::Undo);
        assert!(state.marker.is_none());
        state.command(Command::Redo);
        assert_eq!(state.marker.as_ref().unwrap().start, 49);
        assert_eq!(state.workspace.file().session.serialize().unwrap(), bytes);
        // Duplicate refusal does not select/open before any new commit.
        state.panel = None;
        state.marker = None;
        state.command_quick_add_review(deadsync_editor::quick_add::Kind::Pattern);
        assert!(state.marker.is_none());
        assert_eq!(state.workspace.file().session.serialize().unwrap(), bytes);
    }
    #[test]
    fn paste_multiple_equal_readds_and_unfocused_callback_follow_source_last_selected_pattern() {
        let (_dir, mut state) = fixture();
        for row in [48, 96] {
            let plan = deadsync_editor::quick_add::Plan::prepare(
                &state.workspace.file().session,
                0,
                deadsync_editor::quick_add::Kind::Pattern,
                row as f64,
            )
            .unwrap();
            state.workspace.apply_quick_add(&plan).unwrap();
        }
        state.refresh();
        assert_eq!(state.marker.as_ref().unwrap().start, 96);
        let mut selected = state.workspace.event_selection_between(48, 96).unwrap();
        selected.select_all(false);
        for i in 0..selected.events().len() {
            if matches!(
                selected.events()[i],
                deadsync_editor::event_clipboard::Event::Pattern { .. }
            ) {
                selected.set_selected(i, true).unwrap();
            }
        }
        state.workspace.copy_event_selection(&selected).unwrap();
        state.navigate_exact_position(192.75, true);
        let row = state.capture_song_property_presentation().unwrap().row();
        state.paste_selected_events();
        assert_eq!(state.marker.as_ref().unwrap().start, 241);
        assert_eq!(
            state.capture_song_property_presentation().unwrap().row(),
            row
        );
        let revision = state.workspace.file().session.revision();
        state.panel = None;
        state.marker = None;
        state.paste_selected_events();
        assert_eq!(state.marker.as_ref().unwrap().start, 241);
        assert_ne!(state.workspace.file().session.revision(), revision);
        let time = state.chart_focus_time().unwrap();
        assert!(state.focus_chart_at_time(1, time));
        assert!(state.marker.is_none());
        // An action on an unfocused chart stores its LastSelectedPattern but the
        // global window still shows the focused chart and does not steal focus.
        assert!(state.focus_chart_at_time(0, time));
        let plan = deadsync_editor::quick_add::Plan::prepare(
            &state.workspace.file().session,
            0,
            deadsync_editor::quick_add::Kind::Pattern,
            384.,
        )
        .unwrap();
        state.workspace.apply_quick_add(&plan).unwrap();
        state.workspace.execute(Command::SelectChart(1)).unwrap();
        state.refresh();
        assert_eq!(state.workspace.chart(), 1);
        assert!(matches!(state.panel, Some(Panel::Marker(0))));
        assert!(state.marker.is_none());
        assert!(state.focus_chart_at_time(0, time));
        assert_eq!(state.marker.as_ref().unwrap().start, 384);
    }
    #[test]
    fn moved_selected_pattern_tracks_real_callback_before_reopen_and_history_without_seek() {
        let (_root, mut state) = fixture();
        for row in [96., 192.] {
            let plan = deadsync_editor::quick_add::Plan::prepare(
                &state.workspace.file().session,
                0,
                deadsync_editor::quick_add::Kind::Pattern,
                row,
            )
            .unwrap();
            state.workspace.apply_quick_add(&plan).unwrap();
        }
        state.refresh();
        // Open B through the actual inspector entry, then place view A elsewhere.
        state.command(Command::Seek(96));
        state.open_marker();
        state.navigate_exact_position(0.25, true);
        let id = state.workspace.file().session.chart_ids()[0].clone();
        assert_eq!(state.pattern_added.selected.get(&id), Some(&96));
        let before = state.workspace.file().session.serialize().unwrap();
        state.marker.as_mut().unwrap().start = 120;
        state.commit_marker(false);
        assert_eq!(
            state.pattern_added.selected.get(&id),
            Some(&120),
            "addition callback must run before remembered-row lookup"
        );
        assert_eq!(state.marker.as_ref().unwrap().start, 120);
        assert_eq!(state.focus_position.as_ref().unwrap().row(), 0.25);
        let after = state.workspace.file().session.serialize().unwrap();
        for (redo, row, bytes) in [(false, 96, &before), (true, 120, &after)] {
            // Metadata history must preserve the exact current presentation.
            // An earlier explicit focus change may have reprojected its row.
            let anchor = state.capture_song_property_presentation().unwrap();
            let exact = (
                anchor.row().to_bits(),
                anchor.music_time().to_bits(),
                anchor.focus().chart_time().to_bits(),
            );
            // Close the native window as a user would before global history.
            state.marker = None;
            state.panel = None;
            state.command(if redo { Command::Redo } else { Command::Undo });
            assert_eq!(state.pattern_added.selected.get(&id), Some(&row));
            assert_eq!(state.marker.as_ref().unwrap().start, row);
            assert_eq!(state.workspace.file().session.serialize().unwrap(), *bytes);
            let restored = state.capture_song_property_presentation().unwrap();
            assert_eq!(
                (
                    restored.row().to_bits(),
                    restored.music_time().to_bits(),
                    restored.focus().chart_time().to_bits()
                ),
                exact,
                "metadata Undo/Redo must not navigate"
            );
            // The global window follows the focused chart's remembered event.
            // Source SetChartFocused passes SongTime to the next chart, whose
            // row is recomputed by its clock. It does not retain the prior row's
            // exact IEEE bits across this explicit navigation.
            let time = state.chart_focus_time().unwrap();
            assert!(state.focus_chart_at_time(1, time));
            assert!(state.marker.is_none());
            assert_eq!(
                state
                    .capture_song_property_presentation()
                    .unwrap()
                    .music_time()
                    .to_bits(),
                time.to_bits()
            );
            assert!(state.focus_chart_at_time(0, time));
            assert_eq!(state.marker.as_ref().unwrap().start, row);
            assert_eq!(
                state
                    .capture_song_property_presentation()
                    .unwrap()
                    .music_time()
                    .to_bits(),
                time.to_bits()
            );
            assert_eq!(state.pattern_added.selected.get(&id), Some(&row));
        }
    }
}

#[cfg(test)]
mod inspector_navigation_tests {
    use super::*;
    #[test]
    fn inspector_buttons_use_displayed_pattern_and_update_exact_focus_without_file_history_changes()
    {
        crate::i18n::init_for_tests();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("patterns.ssc");
        std::fs::write(
            &path,
            b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000,0100;",
        )
        .unwrap();
        let mut state = State::open(&path, 0).unwrap();
        for row in [48., 144., 96.] {
            let plan = deadsync_editor::quick_add::Plan::prepare(
                &state.workspace.file().session,
                0,
                deadsync_editor::quick_add::Kind::Pattern,
                row,
            )
            .unwrap();
            state.workspace.apply_quick_add(&plan).unwrap();
        }
        state.refresh();
        assert_eq!(state.marker.as_ref().unwrap().start, 96);
        state.navigate_exact_position(0.5, true);
        let bytes = state.workspace.file().session.serialize().unwrap();
        let revision = state.workspace.file().session.revision();
        let undo = state
            .workspace
            .file()
            .session
            .undo_label()
            .map(str::to_owned);
        // The callback selected the last addition, while the view is elsewhere.
        for (forward, expected) in [
            (true, 144),
            (true, 48),
            (false, 144),
            (false, 96),
            (false, 48),
            (false, 144),
        ] {
            state.navigate_exact_position(0., true);
            state.navigate_marker(forward);
            assert_eq!(state.marker.as_ref().unwrap().start, expected);
            assert_eq!(state.workspace.row(), expected);
            assert_eq!(
                state.focus_position.as_ref().unwrap().row(),
                expected as f64
            );
            assert_eq!(state.workspace.file().session.serialize().unwrap(), bytes);
            assert_eq!(state.workspace.file().session.revision(), revision);
            assert_eq!(
                state
                    .workspace
                    .file()
                    .session
                    .undo_label()
                    .map(str::to_owned),
                undo
            );
        }
        let seed = state.marker.as_ref().unwrap().definition.random_seed;
        state.marker.as_mut().unwrap().definition.random_seed = seed.wrapping_add(1);
        let row = state.focus_position.as_ref().unwrap().row();
        state.navigate_marker(true);
        assert_eq!(state.marker.as_ref().unwrap().start, 144);
        assert_eq!(state.focus_position.as_ref().unwrap().row(), row);
        assert_eq!(state.workspace.file().session.serialize().unwrap(), bytes);
        state.marker = None;
        state.navigate_marker(true);
        assert_eq!(state.focus_position.as_ref().unwrap().row(), row);
    }
}
