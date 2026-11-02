use super::*;
use deadsync_editor::snap::Level;
fn open(state: &mut State) {
    bind(state, Action::OpenMenu);
    press(state, KeyCode::F12);
    let index = if !state.has_song() {
        assert!(matches!(state.panel,Some(Panel::FileMenu(0))));15
    } else if state.workspace.has_chart() {
        assert!(matches!(state.panel,Some(Panel::Menu(0))));32
    } else {
        assert!(matches!(state.panel,Some(Panel::EmptyMenu(0))));18
    };
    for _ in 0..index {
        press(state, KeyCode::ArrowDown);
    }
    press(state, KeyCode::Enter);
    assert!(
        matches!(state.panel, Some(Panel::EditorHelp(0))),
        "{}",
        state.status
    );
    for _ in 0..11 {
        press(state, KeyCode::ArrowDown);
    }
    press(state, KeyCode::Enter);
    assert!(
        matches!(state.panel, Some(Panel::Hotbar(0))),
        "{}",
        state.status
    );
}
fn forbidden(commands: &[GameplayAudioCommand]) {
    assert!(
        !commands.iter().any(|c| matches!(
            c,
            GameplayAudioCommand::EditorStopMusic { .. }
                | GameplayAudioCommand::EditorPauseMusic { .. }
                | GameplayAudioCommand::EditorStartMusic { .. }
                | GameplayAudioCommand::EditorResumeMusic { .. }
                | GameplayAudioCommand::EditorSeekMusic { .. }
                | GameplayAudioCommand::EditorSwapMusicAsset { .. }
                | GameplayAudioCommand::EditorEndAudio { .. }
        )),
        "{commands:?}"
    );
}
#[test]
fn physical_hotbar_signed_fractional_idle_main_and_paused_read_exact_current_focus_en_fr() {
    for language in ["en", "fr"] {
        for mode in 0..3 {
            for row in [-12.75_f64, 60.75] {
                let (_root, _path, mut state) = fixture(false, false);
                crate::i18n::set_locale(deadsync_assets::language::load_for_tests(language));
                if mode > 0 {
                    start(&mut state, false);
                    let mut snapshot = GameplayAudioSnapshot::default();
                    snapshot.editor.owner = state.editor_asset_owner();
                    snapshot.editor.output = state.audio_output_snapshot();
                    snapshot.stream_clock.valid_at = Instant::now();
                    snapshot.editor.logical_music =
                        Some(deadsync_gameplay::EditorLogicalMusicClock {
                            generation: 17,
                            asset_epoch: 29,
                            music_nanos: (row / 96. * 1e9) as i64,
                        });
                    state.update_audio(snapshot);
                    if mode == 2 {
                        state.pause_audio();
                    }
                } else {
                    state
                        .workspace
                        .seek_preserving_selection(row.max(0.).floor() as u64)
                        .unwrap();
                    state.focus_position = Some(
                        deadsync_editor::chart_focus::Position::at_fractional_row(
                            &state.workspace.file().session,
                            0,
                            row,
                        )
                        .unwrap(),
                    );
                }
                state.take_audio_commands();
                let before = state.capture_song_property_presentation().unwrap();
                let source = state.workspace.file().session.serialize().unwrap();
                let history = state
                    .workspace
                    .file()
                    .session
                    .undo_label()
                    .map(str::to_owned);
                let expected_screen = state.receptor_canvas_position();
                open(&mut state);
                state.capture_hotbar_frame();
                let view = state.hotbar_readout().unwrap();
                assert_eq!(view.row.to_bits(), row.to_bits());
                assert_eq!(view.beat, row / 48.);
                assert_eq!(view.music.to_bits(), before.music_time().to_bits());
                assert_eq!(view.chart.to_bits(), before.focus().chart_time().to_bits());
                assert_eq!(view.measure, row / 192.);
                assert_eq!(view.screen, [expected_screen.0, expected_screen.1]);
                let mut actors = Vec::new();
                state.panel_actors(Panel::Hotbar(0), &mut actors);
                assert!(!actors.is_empty());
                assert_eq!(state.workspace.file().session.serialize().unwrap(), source);
                assert_eq!(
                    state.workspace.file().session.undo_label(),
                    history.as_deref()
                );
                forbidden(&state.take_audio_commands());
                if mode == 1 {
                    assert!(state.chart_transport_is_playing());
                    assert_eq!(state.chart_audio_generation(), Some(17));
                } else if mode == 2 {
                    assert!(state.paused_position().is_some());
                }
                press(&mut state, KeyCode::Escape);
                assert!(matches!(state.panel, Some(Panel::EditorHelp(11))));
                forbidden(&state.take_audio_commands());
            }
        }
    }
}
#[test]
fn physical_hotbar_chartless_has_no_invented_chart_position_and_zoom_entry_keeps_transport_en_fr() {
    for language in ["en", "fr"] {
        crate::i18n::init_for_tests();
        crate::i18n::set_locale(deadsync_assets::language::load_for_tests(language));
        let root = tempfile::tempdir().unwrap();
        let mut empty = State::without_song(root.path()).unwrap();
        empty.panel = None;
        open(&mut empty);
        empty.capture_hotbar_frame();
        assert!(empty.hotbar_readout().is_none());
        let mut actors = Vec::new();
        empty.panel_actors(Panel::Hotbar(0), &mut actors);
        assert!(!actors.is_empty());
        // A loaded song with its final chart view closed uses EmptyMenu,
        // while NoSong above uses FileMenu. Both must reach the real panel.
        let (_closed_root, _closed_path, mut closed) = fixture(false, false);
        closed.chart_views_apply(crate::screens::edition::chart_views::ViewAction::Close);
        assert!(closed.has_song());assert!(!closed.workspace.has_chart());
        open(&mut closed);closed.capture_hotbar_frame();assert!(closed.hotbar_readout().is_none());
        let (_root, _path, mut state) = fixture(false, false);
        start(&mut state, false);
        let owner = state.editor_asset_owner();
        open(&mut state);
        state.take_audio_commands();
        for _ in 0..7 {
            press(&mut state, KeyCode::ArrowDown);
        }
        assert!(matches!(state.panel, Some(Panel::Hotbar(7))));
        state.command_zoom(Some(1.), Instant::now());
        let original = state.zoom.clone();
        press(&mut state, KeyCode::Enter);
        assert!(state.wants_text_input());
        assert_eq!(
            state.zoom, original,
            "entering text cannot cancel desired interpolation"
        );
        press(&mut state, KeyCode::Escape);
        assert_eq!(state.zoom, original, "cancel cannot change the manager");
        press(&mut state, KeyCode::Enter);
        state.text_buffer.as_mut().unwrap().select_all();
        assert!(state.text_input("0.732142"));
        press(&mut state, KeyCode::Enter);
        assert_eq!(
            state.spacing_zoom().to_bits(),
            f64::from(0.732142f32).to_bits()
        );
        assert!(state.chart_transport_is_playing());
        assert_eq!(state.editor_asset_owner(), owner);
        forbidden(&state.take_audio_commands());
    }
}
#[test]
fn hotbar_snap_limit_matches_all_original_reconciliations_without_moving_or_stopping() {
    let (_root, _path, mut state) = fixture(false, false);
    start(&mut state, false);
    let owner = state.editor_asset_owner();
    let position = state.capture_song_property_presentation().unwrap();
    let bytes = state.workspace.file().session.serialize().unwrap();
    let undo = state
        .workspace
        .file()
        .session
        .undo_label()
        .map(str::to_owned);
    let cases: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../deadsync-editor/tests/fixtures/hotbar/snap-lock.json"
    )))
    .unwrap();
    for case in cases.as_array().unwrap() {
        let value = |name: &str| case[name].as_u64().unwrap() as usize;
        let level = Level::source(value("level")).unwrap();
        let old_lock = Level::source(value("old_lock")).unwrap();
        let new_lock = Level::source(value("new_lock")).unwrap();
        let mut file = state.capture_application_preferences_file().unwrap();
        file.set_snap(deadsync_editor::snap::Preferences {
            level,
            lock: old_lock,
        });
        state.publish_application_preferences_owner(file);
        state
            .workspace
            .execute(Command::SetSnap(level.native_index()))
            .unwrap();
        state.take_audio_commands();
        state.set_hotbar_snap_lock(new_lock).unwrap();
        let next = Level::native(state.workspace.snap()).unwrap();
        assert_eq!(next.source_index(), value("result"), "{case}");
        assert_eq!(
            state.application_preferences_owner().unwrap().snap().lock,
            new_lock
        );
        assert_eq!(
            state
                .capture_song_property_presentation()
                .unwrap()
                .row()
                .to_bits(),
            position.row().to_bits()
        );
        assert_eq!(
            state
                .capture_song_property_presentation()
                .unwrap()
                .music_time()
                .to_bits(),
            position.music_time().to_bits()
        );
        assert_eq!(state.editor_asset_owner(), owner);
        assert_eq!(state.chart_audio_generation(), Some(17));
        assert!(state.chart_transport_is_playing());
        forbidden(&state.take_audio_commands());
    }
    assert_eq!(state.workspace.file().session.serialize().unwrap(), bytes);
    assert_eq!(state.workspace.file().session.undo_label(), undo.as_deref());
}
