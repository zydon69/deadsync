//! Reuse the real raw-key and backend protocol fixtures from the opener tests.
use super::*;
#[derive(Clone, Copy, Debug)]
enum Mode {
    Main,
    Paused,
    Preview,
}
fn legacy(mode: Mode) -> (tempfile::TempDir, PathBuf, State) {
    let (root, path, state) = fixture(false, false);
    drop(state);
    fs::write(&path,b"#VERSION:0.60;#TITLE:Legacy;#MUSIC:main.wav;#PREVIEW:preview.wav;#SAMPLESTART:0.25;#SAMPLELENGTH:1;#OFFSET:0;#BPMS:0=120;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=180;#NOTES:1000,0100,0010,0001;").unwrap();
    let mut state = State::open(&path, 0).unwrap();
    start(&mut state, matches!(mode, Mode::Preview));
    if matches!(mode, Mode::Paused) {
        state.pause_audio();
        assert!(state.paused_position().is_some());
    }
    state.take_audio_commands();
    (root, path, state)
}
fn audio(state: &State) -> String {
    if state.is_playing() {
        return witness(state).audio;
    }
    let paused = state
        .paused_position()
        .expect("typed migration must retain the exact pause");
    format!(
        "{:?}",
        (
            state.editor_asset_owner(),
            state.editor_asset_registration(),
            paused.music_time.to_bits(),
            paused.chart_time.to_bits(),
            paused.row.to_bits()
        )
    )
}
fn uninterrupted(state: &mut State, expected: &str, step: &str) {
    assert_eq!(audio(state), expected, "{step}");
    for command in state.take_audio_commands() {
        assert!(
            !matches!(
                command,
                GameplayAudioCommand::EditorPauseMusic { .. }
                    | GameplayAudioCommand::EditorStopMusic { .. }
                    | GameplayAudioCommand::EditorStartMusic { .. }
                    | GameplayAudioCommand::EditorResumeMusic { .. }
                    | GameplayAudioCommand::EditorSeekMusic { .. }
                    | GameplayAudioCommand::EditorSwapMusicAsset { .. }
                    | GameplayAudioCommand::EditorClearMusicAsset { .. }
                    | GameplayAudioCommand::EditorPlayPreview { .. }
                    | GameplayAudioCommand::EditorResumePreview { .. }
                    | GameplayAudioCommand::EditorSuspendPreview { .. }
                    | GameplayAudioCommand::EditorBeginAudio { .. }
                    | GameplayAudioCommand::EditorEndAudio { .. }
                    | GameplayAudioCommand::SeekMusic(_)
            ),
            "{step}: {command:?}"
        );
    }
}
#[test]
fn physical_migration_apply_undo_redo_keep_main_pause_and_actual_preview_cursor() {
    deadlib_present::space::with_metrics(
        deadlib_present::space::Metrics::centered(1100., 760.),
        || {
            for mode in [Mode::Main, Mode::Paused, Mode::Preview] {
                let (_root, path, mut state) = legacy(mode);
                let before = state.workspace.file().session.serialize().unwrap();
                let disk_before = fs::read(&path).unwrap();
                let expected = audio(&state);
                press(&mut state, KeyCode::F10);
                assert!(matches!(state.panel, Some(Panel::TimingMigration(0))));
                uninterrupted(&mut state, &expected, "open");
                press(&mut state, KeyCode::Enter);
                assert!(state.panel.is_none(), "{mode:?}: {}", state.status);
                uninterrupted(&mut state, &expected, "apply");
                let after = state.workspace.file().session.serialize().unwrap();
                assert_ne!(after, before);
                for (action, bytes) in [(Action::Undo, &before), (Action::Redo, &after)] {
                    bind(&mut state, action);
                    press(&mut state, KeyCode::F12);
                    assert_eq!(
                        &state.workspace.file().session.serialize().unwrap(),
                        bytes,
                        "{action:?}"
                    );
                    uninterrupted(&mut state, &expected, &format!("{mode:?} {action:?}"));
                }
                assert_eq!(
                    fs::read(path).unwrap(),
                    disk_before,
                    "migration and history remain unsaved"
                );
            }
        },
    );
}
#[test]
fn physical_migration_cancel_and_stale_apply_keep_document_and_audio() {
    deadlib_present::space::with_metrics(
        deadlib_present::space::Metrics::centered(1100., 760.),
        || {
            for mode in [Mode::Main, Mode::Paused, Mode::Preview] {
                let (_root, _path, mut state) = legacy(mode);
                let before = state.workspace.file().session.serialize().unwrap();
                let expected = audio(&state);
                press(&mut state, KeyCode::F10);
                press(&mut state, KeyCode::Escape);
                assert!(state.panel.is_none());
                assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
                uninterrupted(&mut state, &expected, "cancel");
                press(&mut state, KeyCode::F10);
                assert!(matches!(state.panel, Some(Panel::TimingMigration(0))));
                // A genuine committed title edit makes the already-open migration
                // stale; use its existing typed receipt to keep pause provenance.
                let presentation = state.capture_song_property_presentation().unwrap();
                let mut draft = state.workspace.song_properties().unwrap();
                draft
                    .set_text(
                        deadsync_editor::song_text::Field::Title,
                        "Edited after review",
                    )
                    .unwrap();
                let application = state
                    .workspace
                    .apply_song_properties_with_presentation_receipt(&draft)
                    .unwrap();
                state
                    .rebind_song_property_presentation(
                        presentation,
                        application.presentation_receipt().unwrap(),
                    )
                    .unwrap();
                let edited = state.workspace.file().session.serialize().unwrap();
                let revision = state.workspace.file().session.revision();
                press(&mut state, KeyCode::Enter);
                assert!(matches!(state.panel, Some(Panel::TimingMigration(0))));
                assert_eq!(state.workspace.file().session.serialize().unwrap(), edited);
                assert_eq!(state.workspace.file().session.revision(), revision);
                uninterrupted(&mut state, &expected, "stale refusal");
                press(&mut state, KeyCode::Escape);
                uninterrupted(&mut state, &expected, "cancel stale review");
            }
        },
    );
}
