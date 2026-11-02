use super::{Panel, State};
use crate::i18n;
use deadsync_editor::chart_order::{Binding, Orderer, SessionBinding};

pub(super) struct Request {
    binding: Option<Binding>,
    session: SessionBinding,
    backwards: bool,
    opening_list: bool,
    source_row: u64,
    source_focus: Option<(u64, u64)>,
    language_revision: u64,
}
impl State {
    pub(super) fn queue_chart_order(&mut self, backwards: bool) {
        let count = self.workspace.file().session.document().charts().len();
        if count == 0 || (self.workspace.has_chart() && count == 1) {
            return;
        }
        // No focused chart is a genuine source state, even when Workspace's
        // legacy chart index is still zero. Do not bind that placeholder.
        let binding = if self.workspace.has_chart() {
            match Binding::capture(&self.workspace.file().session, self.workspace.chart()) {
                Ok(binding) => Some(binding),
                Err(_) => return,
            }
        } else {
            None
        };
        if self.chart_focus_time().is_err() {
            self.status = i18n::tr("Edition", "ChartOrderFailed");
            return;
        }
        self.chart_order_request = Some(Request {
            binding,
            session: SessionBinding::capture(&self.workspace.file().session),
            backwards,
            opening_list: false,
            source_row: self.workspace.row(),
            source_focus: self.chart_order_focus_anchor(),
            language_revision: i18n::revision(),
        });
    }
    fn chart_order_focus_anchor(&self) -> Option<(u64, u64)> {
        self.focus_position
            .as_ref()
            .map(|position| (position.row().to_bits(), position.music_time().to_bits()))
    }
    pub(super) fn queue_chart_list(&mut self) {
        self.chart_order_request = Some(Request {
            binding: self
                .workspace
                .has_chart()
                .then(|| {
                    Binding::capture(&self.workspace.file().session, self.workspace.chart()).ok()
                })
                .flatten(),
            session: SessionBinding::capture(&self.workspace.file().session),
            backwards: false,
            opening_list: true,
            source_row: self.workspace.row(),
            source_focus: self.chart_order_focus_anchor(),
            language_revision: i18n::revision(),
        });
    }
    pub fn process_chart_order(&mut self, orderer: &mut impl Orderer) -> bool {
        let copy_processed=self.process_chart_event_copy_order(orderer);
        let autogen_processed = self.process_autogen_source_order(orderer)||copy_processed;
        let output_processed = self.process_source_output_order(orderer) || autogen_processed;
        let timing_processed = self.process_song_timing_order(orderer) || output_processed;
        let Some(request) = self.chart_order_request.take() else {
            return timing_processed;
        };
        let current = |state: &Self| {
            state.panel.is_none()
                && request.source_focus == state.chart_order_focus_anchor()
                && (request.source_row == state.workspace.row()
                    // update_audio advances the integral editing cursor from
                    // the scoped callback clock. This is not a manual seek.
                    || (state.chart_transport_is_playing()
                        && state.playback_position().is_some_and(|(_, row)|
                            row.max(0.0).min(i32::MAX as f64).floor() as u64 == state.workspace.row())))
                && state.label_preedit.is_empty()
                && request.language_revision == i18n::revision()
                && request.session.is_current(&state.workspace.file().session)
                && request
                    .binding
                    .as_ref()
                    .map_or(!state.workspace.has_chart(), |binding| {
                        state.workspace.has_chart()
                            && binding.is_current(
                                &state.workspace.file().session,
                                state.workspace.chart(),
                            )
                    })
        };
        if !current(self) {
            return false;
        }
        let result = self
            .chart_list_cache
            .refresh(&mut self.workspace, request.language_revision, orderer)
            .map(|_| ());
        // The native boundary may reenter the application or change locale.
        // Never apply results belonging to the former song/focus/language.
        if !current(self) {
            return false;
        }
        match result {
            Ok(()) if request.opening_list => {
                self.chart_list_selection
                    .reconcile(self.chart_list_cache.rows(), self.workspace.chart());
                self.panel = Some(Panel::ChartList);
                self.status = i18n::tr("Edition", "ChartListHelp");
            }
            Ok(()) => {
                let next = if self.workspace.has_chart() {
                    self.chart_list_cache
                        .adjacent(self.workspace.chart(), request.backwards)
                } else if request.backwards {
                    self.chart_list_cache
                        .rows()
                        .last()
                        .map(|row| row.source_index)
                } else {
                    self.chart_list_cache
                        .rows()
                        .first()
                        .map(|row| row.source_index)
                };
                let Some(next) = next else {
                    self.status = i18n::tr("Edition", "ChartOrderFailed");
                    return true;
                };
                // Capture after the deferred cultural sort: the callback may
                // have advanced meanwhile. Chartless uses the retained song
                // clock/LastKnownSongTime, never the sample-preview clock.
                match self.chart_focus_time() {
                    Ok(time) => {
                        self.focus_chart_at_time(next, time);
                    }
                    Err(_) => self.status = i18n::tr("Edition", "ChartOrderFailed"),
                }
            }
            Err(_) => self.status = i18n::tr("Edition", "ChartOrderFailed"),
        }
        true
    }
}

#[cfg(test)]
mod transport_navigation_tests {
    use super::*;
    use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
    use deadsync_gameplay::{EditorLogicalMusicClock, GameplayAudioCommand, GameplayAudioSnapshot};
    use std::time::Instant;

    struct SourceOrder;
    impl Orderer for SourceOrder {
        fn order(
            &mut self,
            snapshot: &deadsync_editor::chart_order::Snapshot,
        ) -> Result<Vec<usize>, deadsync_editor::Error> {
            snapshot.indices_with(str::cmp)
        }
    }
    fn fixture(language: &str, count: usize) -> (tempfile::TempDir, State, u64) {
        i18n::init(deadsync_assets::language::load_for_tests(language));
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.ssc");
        // Source indices are deliberately opposite the difficulty sort order.
        let mut song = String::from(
            "#VERSION:0.83;#MUSIC:music.wav;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#BPMS:0=120;",
        );
        for index in 0..count {
            song.push_str(&format!("#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:{};#OFFSET:{};#BPMS:0={};#NOTES:1000;", if index == 0 { "Hard" } else { "Easy" }, index as f64 * 0.5, if index == 0 { 120 } else { 60 }));
        }
        std::fs::write(&path, song).unwrap();
        let mut state = State::open(&path, 0).unwrap();
        let owner = state
            .take_audio_commands()
            .iter()
            .find_map(|command| match command {
                GameplayAudioCommand::EditorBeginAudio { owner, .. } => Some(*owner),
                _ => None,
            })
            .unwrap();
        let mut snapshot = GameplayAudioSnapshot::default();
        snapshot.editor.owner = owner;
        snapshot.editor.output.available = true;
        snapshot.editor.output.leased = true;
        snapshot.editor.output.period_control = deadsync_gameplay::EditorBufferControl::Adjustable;
        snapshot.editor.output.requested_period = Some(512);
        snapshot.editor.output.negotiated_period = Some(512);
        state.update_audio(snapshot);
        state.take_audio_commands();
        (root, state, owner)
    }
    fn start(state: &mut State, owner: u64) {
        state.toggle_audio();
        assert!(
            matches!(state.take_audio_commands().as_slice(), [GameplayAudioCommand::EditorStartMusic {owner: actual,..}] if *actual == owner)
        );
        state.bind_decoder_generation(Some(7));
        state.bind_decoder_asset_epoch(Some(owner), Some(13));
        logical(state, owner, 1.25);
    }
    fn logical(state: &mut State, owner: u64, time: f64) {
        let mut snapshot = GameplayAudioSnapshot::default();
        snapshot.editor.owner = owner;
        snapshot.editor.output = state.audio_output_snapshot();
        snapshot.stream_clock.valid_at = Instant::now();
        snapshot.editor.logical_music = Some(EditorLogicalMusicClock {
            generation: 7,
            asset_epoch: 13,
            music_nanos: (time * 1e9) as i64,
        });
        state.update_audio(snapshot);
    }
    fn shortcut(state: &mut State, backwards: bool) {
        let mut keys = vec![KeyCode::ControlLeft];
        if backwards {
            keys.push(KeyCode::ShiftLeft);
        }
        keys.push(KeyCode::Tab);
        for key in keys.iter().copied() {
            state.raw_key(&RawKeyboardEvent {
                code: key,
                pressed: true,
                repeat: false,
                timestamp: Instant::now(),
                host_nanos: 0,
            });
        }
        for key in keys.iter().rev().copied() {
            state.raw_key(&RawKeyboardEvent {
                code: key,
                pressed: false,
                repeat: false,
                timestamp: Instant::now(),
                host_nanos: 0,
            });
        }
    }
    fn assert_uninterrupted(state: &mut State, owner: u64) {
        assert!(state.chart_transport_is_playing());
        assert_eq!(state.chart_audio_generation(), Some(7));
        assert_eq!(state.playback_asset_epoch(), Some(13));
        assert_eq!(state.editor_asset_owner(), owner);
        assert!(state.audio_output_snapshot().leased);
        assert!(
            state.take_audio_commands().iter().all(|command| !matches!(
                command,
                GameplayAudioCommand::EditorStopMusic { .. }
                    | GameplayAudioCommand::EditorPauseMusic { .. }
                    | GameplayAudioCommand::EditorStartMusic { .. }
                    | GameplayAudioCommand::EditorResumeMusic { .. }
                    | GameplayAudioCommand::EditorSwapMusicAsset { .. }
                    | GameplayAudioCommand::EditorClearMusicAsset { .. }
                    | GameplayAudioCommand::EditorEndAudio { .. }
            )),
            "same asset keeps owner {owner}, decoder and lease"
        );
    }
    #[test]
    fn physical_next_previous_chart_keeps_scoped_transport_and_consumes_current_callback_phase() {
        for language in ["en", "fr"] {
            for backwards in [false, true] {
                let (_root, mut state, owner) = fixture(language, 2);
                start(&mut state, owner);
                let before = state.workspace.file().session.serialize().unwrap();
                shortcut(&mut state, backwards);
                assert!(state.chart_order_request.is_some());
                let queued_row = state.workspace.row();
                logical(&mut state, owner, 1.75);
                assert_ne!(state.workspace.row(), queued_row);
                assert!(state.process_chart_order(&mut SourceOrder));
                assert_eq!(state.workspace.active_chart(), Some(1));
                assert_eq!(
                    state.chart_focus_time().unwrap(),
                    1.75 - state.audio_preferences.effective().offset
                );
                assert_uninterrupted(&mut state, owner);
                logical(&mut state, owner, 2.0);
                assert_eq!(
                    state.chart_focus_time().unwrap(),
                    2.0 - state.audio_preferences.effective().offset
                );
                assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
                assert!(state.workspace.file().session.undo_label().is_none());
            }
        }
    }
    #[test]
    fn queued_playing_navigation_rejects_explicit_fractional_seek_even_if_next_snapshot_advances() {
        let (_root, mut state, owner) = fixture("en", 2);
        start(&mut state, owner);
        state.scroll_preferences.set_stop_playback(false);
        state.queue_chart_order(false);
        let row = state.playback_position().unwrap().1;
        state.navigate_exact_position(row + 0.125, true);
        logical(&mut state, owner, 1.75);
        assert!(!state.process_chart_order(&mut SourceOrder));
        assert_eq!(state.workspace.active_chart(), Some(0));
        assert!(state.chart_transport_is_playing());
    }
    #[test]
    fn chart_list_allows_callback_progress_but_rejects_a_manual_exact_anchor_change() {
        for manual_seek in [false, true] {
            let (_root, mut state, owner) = fixture("en", 2);
            start(&mut state, owner);
            state.queue_chart_list();
            let old_row = state.workspace.row();
            if manual_seek {
                state.scroll_preferences.set_stop_playback(false);
                state.navigate_exact_position(state.playback_position().unwrap().1 + 0.125, true);
            }
            logical(&mut state, owner, 1.75);
            assert_ne!(state.workspace.row(), old_row);
            assert_eq!(state.process_chart_order(&mut SourceOrder), !manual_seek);
            assert_eq!(matches!(state.panel, Some(Panel::ChartList)), !manual_seek);
            assert_eq!(state.workspace.active_chart(), Some(0));
            assert!(state.chart_transport_is_playing());
            if !manual_seek {
                assert_uninterrupted(&mut state, owner);
            }
        }
    }
    #[test]
    fn unique_chart_shortcut_is_noop_without_stop_or_pause() {
        let (_root, mut state, owner) = fixture("en", 1);
        start(&mut state, owner);
        for backwards in [false, true] {
            shortcut(&mut state, backwards);
            assert!(state.chart_order_request.is_none());
            assert_uninterrupted(&mut state, owner);
        }
    }
    #[test]
    fn chartless_opens_first_or_last_sorted_chart_including_unique_at_retained_song_instant() {
        for count in [1, 2] {
            for backwards in [false, true] {
                for playing in [false, true] {
                    let (_root, mut state, owner) = fixture("en", count);
                    if playing {
                        start(&mut state, owner);
                    } else {
                        state.navigate_exact_time(1.25, true).unwrap();
                    }
                    state.chart_views_apply(super::super::chart_views::ViewAction::Close);
                    assert!(!state.workspace.has_chart());
                    state.take_audio_commands();
                    shortcut(&mut state, backwards);
                    assert!(state.chart_order_request.is_some());
                    if playing {
                        logical(&mut state, owner, 1.75);
                    }
                    let time = state.chart_focus_time().unwrap();
                    assert!(state.process_chart_order(&mut SourceOrder));
                    let expected = if count == 1 || backwards { 0 } else { 1 };
                    assert_eq!(state.workspace.active_chart(), Some(expected));
                    assert_eq!(state.chart_focus_time().unwrap(), time);
                    if playing {
                        assert_uninterrupted(&mut state, owner);
                    }
                }
            }
        }
    }
    #[test]
    fn deferred_navigation_rejects_manual_fractional_seek_focus_and_locale_changes() {
        for change in 0..4 {
            let (_root, mut state, _owner) = fixture("en", 2);
            state.navigate_exact_position(50.25, true);
            state.queue_chart_order(false);
            match change {
                0 => state.navigate_exact_position(50.75, true), // same integral row
                1 => {
                    let time = state.chart_focus_time().unwrap();
                    assert!(state.focus_chart_at_time(1, time));
                }
                2 => i18n::init(deadsync_assets::language::load_for_tests("fr")),
                _ => state
                    .workspace
                    .execute(deadsync_editor::workspace::Command::Place(1))
                    .unwrap(),
            }
            assert!(!state.process_chart_order(&mut SourceOrder));
            assert_eq!(
                state.workspace.active_chart(),
                Some(if change == 1 { 1 } else { 0 })
            );
        }
    }
}
