//! Raw keyboard publication plus real scoped snapshot ownership. These fixtures
//! do not execute a hardware DSP; backend seek/swap commands remain observable.
use super::*;
#[derive(Clone, Copy, Debug)]
enum Mode {
    Main,
    Paused,
    Idle,
    Sample,
}
fn setup(language: &str, mode: Mode) -> (tempfile::TempDir, State) {
    let (root, _, mut state) = fixture(false, false);
    crate::i18n::set_locale(deadsync_assets::language::load_for_tests(language));
    match mode {
        Mode::Main | Mode::Paused => {
            start(&mut state, false);
            let mut snapshot = GameplayAudioSnapshot::default();
            snapshot.editor.owner = state.editor_asset_owner();
            snapshot.editor.output = state.audio_output_snapshot();
            snapshot.stream_clock.valid_at = Instant::now();
            snapshot.editor.logical_music = Some(deadsync_gameplay::EditorLogicalMusicClock {
                generation: 17,
                asset_epoch: 29,
                music_nanos: 632_812_500,
            });
            state.update_audio(snapshot);
            if matches!(mode, Mode::Paused) {
                state.pause_audio();
                assert!(state.paused_position().is_some());
            }
        }
        Mode::Sample => {
            start(&mut state, true);
        }
        Mode::Idle => {}
    }
    if matches!(mode, Mode::Idle | Mode::Sample) {
        state.workspace.seek_preserving_selection(0).unwrap();
        state.focus_position = Some(
            deadsync_editor::chart_focus::Position::at_fractional_row(
                &state.workspace.file().session,
                0,
                -12.75,
            )
            .unwrap(),
        );
    }
    bind(&mut state, Action::OpenChartProperties);
    state.take_audio_commands();
    (root, state)
}
fn stage_offset(state: &mut State) {
    press(state, KeyCode::F12);
    assert!(
        matches!(state.panel, Some(Panel::ChartProperties(0))),
        "{}",
        state.status
    );
    press(state, KeyCode::ArrowDown);
    press(state, KeyCode::ArrowRight);
    press(state, KeyCode::ArrowDown);
    press(state, KeyCode::ArrowRight);
    press(state, KeyCode::ArrowDown);
    press(state, KeyCode::ArrowDown);
    assert!(matches!(state.panel, Some(Panel::ChartProperties(4))));
}
fn assert_mode(state: &State, mode: Mode) {
    match mode {
        Mode::Main => assert!(state.chart_transport_is_playing()),
        Mode::Paused => {
            assert!(!state.is_playing());
            assert!(state.paused_position().is_some());
        }
        Mode::Idle => assert!(!state.is_playing()),
        Mode::Sample => {
            assert!(state.is_playing());
            assert!(!state.chart_transport_is_playing());
        }
    }
}
fn commands(state: &mut State, mode: Mode, owner: u64, time: f64) {
    let commands = state.take_audio_commands();
    let seeks = commands
        .iter()
        .filter_map(|command| match command {
            GameplayAudioCommand::EditorSeekMusic { owner, seconds } => Some((*owner, *seconds)),
            _ => None,
        })
        .collect::<Vec<_>>();
    if matches!(mode, Mode::Main) {
        assert_eq!(seeks.len(), 1, "{commands:?}");
        assert_eq!(seeks[0].0, owner);
        assert_eq!(
            seeks[0].1.to_bits(),
            (time + state.audio_preferences.effective().offset).to_bits()
        );
    } else {
        assert!(seeks.is_empty(), "{commands:?}");
    }
    assert!(
        !commands.iter().any(|command| matches!(
            command,
            GameplayAudioCommand::EditorStopMusic { .. }
                | GameplayAudioCommand::EditorPauseMusic { .. }
                | GameplayAudioCommand::EditorStartMusic { .. }
                | GameplayAudioCommand::EditorResumeMusic { .. }
                | GameplayAudioCommand::EditorBeginAudio { .. }
                | GameplayAudioCommand::EditorEndAudio { .. }
                | GameplayAudioCommand::EditorSuspendPreview { .. }
                | GameplayAudioCommand::EditorResumePreview { .. }
        )),
        "{commands:?}"
    );
}
#[test]
fn physical_chart_offset_apply_undo_redo_keep_double_row_and_transport_modes_en_fr() {
    for language in ["en", "fr"] {
        for mode in [Mode::Main, Mode::Paused, Mode::Idle, Mode::Sample] {
            let (_root, mut state) = setup(language, mode);
            let before = state.capture_song_property_presentation().unwrap();
            let row = before.row();
            let time = before.music_time();
            let owner = state.editor_asset_owner();
            let generation = state.chart_audio_generation();
            let sample = state.playback.as_ref().and_then(|p| p.sample_cursor);
            let bytes = state.workspace.file().session.serialize().unwrap();
            stage_offset(&mut state);
            assert_mode(&state, mode);
            assert_eq!(state.workspace.file().session.serialize().unwrap(), bytes);
            press(&mut state, KeyCode::Enter);
            assert!(state.panel.is_none(), "{}", state.status);
            assert_mode(&state, mode);
            let focus = state.capture_song_property_presentation().unwrap();
            assert_eq!(focus.row().to_bits(), row.to_bits());
            assert_eq!(focus.music_time(), time - 0.0001);
            assert_eq!(state.editor_asset_owner(), owner);
            assert_eq!(state.chart_audio_generation(), generation);
            commands(&mut state, mode, owner, focus.music_time());
            bind(&mut state, Action::Undo);
            press(&mut state, KeyCode::F12);
            assert_mode(&state, mode);
            let undo = state.capture_song_property_presentation().unwrap();
            assert_eq!(undo.row().to_bits(), row.to_bits());
            assert_eq!(undo.music_time().to_bits(), time.to_bits());
            commands(&mut state, mode, owner, undo.music_time());
            bind(&mut state, Action::Redo);
            press(&mut state, KeyCode::F12);
            assert_mode(&state, mode);
            let redo = state.capture_song_property_presentation().unwrap();
            assert_eq!(redo.row().to_bits(), row.to_bits());
            assert_eq!(redo.music_time(), time - 0.0001);
            commands(&mut state, mode, owner, redo.music_time());
            if matches!(mode, Mode::Sample) {
                assert_eq!(state.playback.as_ref().unwrap().sample_cursor, sample);
            }
        }
    }
}
#[test]
fn physical_offset_cancel_keeps_bytes_history_and_transport_en_fr() {
    for language in ["en", "fr"] {
        let (_root, mut state) = setup(language, Mode::Main);
        let before = witness(&state);
        stage_offset(&mut state);
        press(&mut state, KeyCode::Escape);
        assert!(state.panel.is_none());
        unchanged(&mut state, &before, "Chart offset Cancel");
    }
}
#[test]
fn nonfocused_chart_offset_history_keeps_main_exact_time_and_no_seek_en_fr() {
    for language in ["en", "fr"] {
        let (_root, mut state) = setup(language, Mode::Main);
        let before = state.capture_song_property_presentation().unwrap();
        let row = before.row();
        let time = before.music_time();
        let owner = state.editor_asset_owner();
        state.autogen_expression_back = Some(Panel::Generation(6));
        state.autogen_source.binding = Some(deadsync_editor::chart_order::SessionBinding::capture(
            &state.workspace.file().session,
        ));
        state.autogen_source.expression_target =
            Some(state.workspace.file().session.chart_ids()[1].clone());
        state.chart_properties = Some(
            deadsync_editor::chart_properties::Draft::capture(&state.workspace.file().session, 1)
                .unwrap(),
        );
        state.panel = Some(Panel::ChartProperties(1));
        press(&mut state, KeyCode::ArrowRight);
        press(&mut state, KeyCode::ArrowDown);
        press(&mut state, KeyCode::ArrowRight);
        press(&mut state, KeyCode::ArrowDown);
        press(&mut state, KeyCode::ArrowDown);
        press(&mut state, KeyCode::Enter);
        assert!(state.chart_transport_is_playing());
        assert_eq!(state.workspace.chart(), 0);
        let after = state.capture_song_property_presentation().unwrap();
        assert_eq!(
            (after.row().to_bits(), after.music_time().to_bits()),
            (row.to_bits(), time.to_bits())
        );
        let emitted = state.take_audio_commands();
        assert!(
            !emitted.iter().any(|c| matches!(
                c,
                GameplayAudioCommand::EditorSeekMusic { .. }
                    | GameplayAudioCommand::EditorStopMusic { .. }
                    | GameplayAudioCommand::EditorPauseMusic { .. }
            )),
            "{emitted:?}"
        );
        for redo in [false, true] {
            state.command(if redo { Command::Redo } else { Command::Undo });
            assert_eq!(state.editor_asset_owner(), owner);
            let after = state.capture_song_property_presentation().unwrap();
            assert_eq!(
                (after.row().to_bits(), after.music_time().to_bits()),
                (row.to_bits(), time.to_bits())
            );
            let emitted = state.take_audio_commands();
            assert!(
                !emitted.iter().any(|c| matches!(
                    c,
                    GameplayAudioCommand::EditorSeekMusic { .. }
                        | GameplayAudioCommand::EditorStopMusic { .. }
                        | GameplayAudioCommand::EditorPauseMusic { .. }
                )),
                "{emitted:?}"
            );
        }
    }
}
#[test]
fn physical_mixed_music_and_offset_apply_undo_redo_swap_then_seek_without_stop_en_fr() {
    for language in ["en", "fr"] {
        let (_root, mut state) = setup(language, Mode::Main);
        let path = state.workspace.file().resource_base().join("alternate.wav");
        fs::write(&path, b"scoped alternate fixture").unwrap();
        let before = state.capture_song_property_presentation().unwrap();
        let row = before.row();
        let time = before.music_time();
        let owner = state.editor_asset_owner();
        stage_offset(&mut state);
        for _ in 0..5 {
            press(&mut state, KeyCode::ArrowDown);
        }
        press(&mut state, KeyCode::Enter);
        assert!(matches!(state.panel, Some(Panel::ChartText(0))));
        for _ in 0..4 {
            press(&mut state, KeyCode::ArrowDown);
        }
        assert!(matches!(state.panel, Some(Panel::ChartText(4))));
        state.text_buffer.as_mut().unwrap().select_all();
        assert!(state.edit_text_input("alternate.wav"));
        press(&mut state, KeyCode::Escape);
        for _ in 0..5 {
            press(&mut state, KeyCode::ArrowUp);
        }
        assert!(matches!(state.panel, Some(Panel::ChartProperties(4))));
        press(&mut state, KeyCode::Enter);
        for step in 0..3 {
            if step > 0 {
                bind(
                    &mut state,
                    if step == 1 {
                        Action::Undo
                    } else {
                        Action::Redo
                    },
                );
                press(&mut state, KeyCode::F12);
            }
            assert!(state.chart_transport_is_playing());
            assert_eq!(state.editor_asset_owner(), owner);
            let focus = state.capture_song_property_presentation().unwrap();
            assert_eq!(focus.row().to_bits(), row.to_bits());
            let expected = if step == 1 { time } else { time - 0.0001 };
            assert_eq!(focus.music_time(), expected);
            let emitted = state.take_audio_commands();
            let swap = emitted
                .iter()
                .position(|c| matches!(c, GameplayAudioCommand::EditorSwapMusicAsset { .. }))
                .expect("effective MUSIC change swaps the real asset");
            let seek = emitted
                .iter()
                .position(|c| matches!(c, GameplayAudioCommand::EditorSeekMusic { .. }))
                .expect("effective offset change seeks Main");
            assert!(swap < seek, "{emitted:?}");
            assert!(
                matches!(&emitted[seek],GameplayAudioCommand::EditorSeekMusic{owner:actual,seconds}if *actual==owner&&seconds.to_bits()==(expected+state.audio_preferences.effective().offset).to_bits())
            );
            assert!(
                !emitted.iter().any(|c| matches!(
                    c,
                    GameplayAudioCommand::EditorPauseMusic { .. }
                        | GameplayAudioCommand::EditorStopMusic { .. }
                        | GameplayAudioCommand::EditorStartMusic { .. }
                )),
                "{emitted:?}"
            );
        }
    }
}
