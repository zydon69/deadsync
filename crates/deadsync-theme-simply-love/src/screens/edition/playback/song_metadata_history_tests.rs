use super::super::{Panel, RawKeyboardEvent};
use super::song_properties_live_tests::fixture;
use super::*;
use deadlib_platform::input::KeyCode;
use deadsync_editor::{
    key_binding_preferences::{Action, Preferences},
    song_text::Field,
};
use deadsync_gameplay::{EditorAssetKind, EditorPreviewCursor};
fn press(s: &mut State, code: KeyCode) {
    for pressed in [true, false] {
        s.raw_key(&RawKeyboardEvent {
            code,
            pressed,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
    }
}
fn open(s: &mut State) {
    press(s, KeyCode::F11);
    assert!(
        matches!(s.panel, Some(Panel::SongProperties(0))),
        "{}",
        s.status
    );
}
fn apply(s: &mut State) {
    s.panel = Some(Panel::SongProperties(1));
    press(s, KeyCode::Enter);
    assert!(s.song_properties.is_none(), "{}", s.status);
}
fn history(s: &mut State, redo: bool) {
    press(s, if redo { KeyCode::F14 } else { KeyCode::F13 });
}
#[derive(Debug, PartialEq)]
struct Stamp {
    intent: (u64, Option<Instant>, bool, bool),
    generation: Option<u64>,
    epoch: Option<u64>,
    lease: bool,
    focus: Option<(u64, u64, u64)>,
    sample: Option<GameplayAudioCommand>,
    cursor: Option<EditorPreviewCursor>,
    registration: Option<u64>,
    playing: bool,
}
fn stamp(s: &State) -> Stamp {
    let focus = s.workspace.has_chart().then(|| {
        let position = s.capture_song_property_presentation().unwrap().focus;
        assert!(position.is_current(&s.workspace));
        (
            position.row().to_bits(),
            position.music_time().to_bits(),
            position.chart_time().to_bits(),
        )
    });
    if s.paused_playback.is_some() {
        assert!(
            s.paused_position().is_some(),
            "metadata must restamp the actual pause source/revision"
        );
    }
    Stamp {
        intent: s.save_as_transport_stamp(),
        generation: s.chart_audio_generation(),
        epoch: s.playback_asset_epoch(),
        lease: s.audio_output_snapshot().leased,
        focus,
        sample: s
            .playback
            .as_ref()
            .and_then(|p| p.sample_descriptor.clone()),
        cursor: s.playback.as_ref().and_then(|p| p.sample_cursor),
        registration: s
            .playback
            .as_ref()
            .filter(|p| p.sample_descriptor.is_some())
            .map(|p| p.sample_registration),
        playing: s.is_playing(),
    }
}
fn unchanged_transport(commands: &[GameplayAudioCommand]) {
    assert!(
        !commands.iter().any(|c| matches!(
            c,
            GameplayAudioCommand::StopMusic
                | GameplayAudioCommand::SeekMusic(_)
                | GameplayAudioCommand::PlayMusic { .. }
                | GameplayAudioCommand::EditorStopMusic { .. }
                | GameplayAudioCommand::EditorPauseMusic { .. }
                | GameplayAudioCommand::EditorResumeMusic { .. }
                | GameplayAudioCommand::EditorStartMusic { .. }
                | GameplayAudioCommand::EditorPlayPreview { .. }
                | GameplayAudioCommand::EditorResumePreview { .. }
                | GameplayAudioCommand::EditorSeekMusic { .. }
                | GameplayAudioCommand::EditorSwapMusicAsset { .. }
                | GameplayAudioCommand::EditorClearMusicAsset { .. }
                | GameplayAudioCommand::EditorEndAudio { .. }
        )),
        "{commands:?}"
    );
}
fn state(language: &str, mode: usize) -> (tempfile::TempDir, State) {
    let (root, mut s, _owner) = fixture(language);
    let mut keys = Preferences::default();
    keys.commands
        .insert(Action::Undo.id().into(), vec![vec!["F13".into()]]);
    keys.commands
        .insert(Action::Redo.id().into(), vec![vec!["F14".into()]]);
    s.configure_key_binding_preferences(Arc::new(keys));
    if mode == 3 {
        std::fs::write(root.path().join("dedicated.wav"), b"existing descriptor").unwrap();
        let mut d = s.workspace.song_properties().unwrap();
        d.set_text(Field::PreviewMusic, "dedicated.wav").unwrap();
        s.commit_song_properties_presented(&d).unwrap();
        s.refresh();
    }
    match mode {
        1 => {
            s.toggle_audio();
            assert!(s.paused_playback.is_some());
        }
        2 | 3 => {
            s.stop_audio();
            let owner = s.editor_asset_owner();
            s.observe_editor_assets(deadsync_gameplay::EditorAssetReadiness {
                owner,
                registration: s.editor_asset_registration(),
                main: true,
                dedicated_preview: true,
            });
            s.command_toggle_sample_audio();
            assert!(s.is_playing());
            assert!(!s.chart_transport_is_playing());
            let kind = if mode == 2 {
                EditorAssetKind::Main
            } else {
                EditorAssetKind::DedicatedPreview
            };
            let cursor = EditorPreviewCursor {
                suspended: false,
                rendered: true,
                owner,
                kind,
                registration: 11,
                revision: 12,
                generation: 13,
                asset_epoch: 14,
                next_music_seconds: 0.75,
                rendered_frame: 200,
                source_frame: None,
                sample_rate_hz: 48000,
            };
            s.observe_rendered_preview_cursor(Some(cursor));
            assert_eq!(s.playback.as_ref().unwrap().sample_cursor, Some(cursor));
        }
        4 => {
            s.stop_audio();
            s.navigate_exact_position(-0.25, true);
        }
        5 | 6 => {
            s.chart_views_apply(super::super::chart_views::ViewAction::Close);
            assert!(!s.workspace.has_chart());
            if mode == 6 {
                s.toggle_audio();
                assert!(s.paused_playback.is_some());
            }
        }
        _ => {}
    }
    s.take_audio_commands();
    (root, s)
}
#[test]
fn physical_f11_metadata_and_auto_images_lyrics_apply_undo_redo_keep_main_pause_preview_and_chartless_en_fr()
 {
    for language in ["en", "fr"] {
        for mode in 0..7 {
            for automatic in [false, true] {
                let (_root, mut s) = state(language, mode);
                let before = stamp(&s);
                let original = s.workspace.file().session.serialize().unwrap();
                if automatic {
                    let directory = s.workspace.file().resource_base();
                    image::RgbImage::new(418, 164)
                        .save(directory.join("banner.png"))
                        .unwrap();
                    std::fs::write(directory.join("lyrics.lrc"), b"[00:01.00]lyric").unwrap();
                    s.command_auto_song_assets(false);
                    assert_eq!(
                        s.workspace.song_properties().unwrap().text(Field::Banner),
                        "banner.png"
                    );
                    assert_eq!(
                        s.workspace.song_properties().unwrap().text(Field::Lyrics),
                        "lyrics.lrc"
                    );
                } else {
                    open(&mut s);
                    s.song_properties
                        .as_mut()
                        .unwrap()
                        .set_text(Field::Title, "Precise metadata")
                        .unwrap();
                    apply(&mut s);
                }
                assert!(
                    s.workspace
                        .file()
                        .session
                        .history_preserves_song_properties_metadata(false)
                );
                let after = s.workspace.file().session.serialize().unwrap();
                assert_ne!(after, original);
                assert_eq!(stamp(&s), before);
                unchanged_transport(&s.take_audio_commands());
                for (redo, expected) in [(false, &original), (true, &after)] {
                    history(&mut s, redo);
                    assert_eq!(
                        s.workspace.file().session.serialize().unwrap(),
                        *expected,
                        "{}",
                        s.status
                    );
                    assert_eq!(stamp(&s), before);
                    unchanged_transport(&s.take_audio_commands());
                }
            }
        }
    }
}
#[test]
fn preview_path_metadata_reloads_dedicated_role_and_stops_only_audition_on_apply_and_history_en_fr()
{
    for language in ["en", "fr"] {
        for mode in [0, 1, 2, 3, 5, 6] {
            let (_root, mut s) = state(language, mode);
            std::fs::write(
                s.workspace.file().resource_base().join("new-preview.wav"),
                b"existing descriptor",
            )
            .unwrap();
            let original = s.workspace.file().session.serialize().unwrap();
            let before = stamp(&s);
            open(&mut s);
            s.song_properties
                .as_mut()
                .unwrap()
                .set_text(Field::PreviewMusic, "new-preview.wav")
                .unwrap();
            apply(&mut s);
            assert!(
                s.editor_asset_paths()[1]
                    .as_ref()
                    .unwrap()
                    .ends_with("new-preview.wav")
            );
            let after = s.workspace.file().session.serialize().unwrap();
            let commands = s.take_audio_commands();
            assert!(commands.iter().any(|c|matches!(c,GameplayAudioCommand::EditorRegisterAsset{kind:EditorAssetKind::DedicatedPreview,path:Some(path),..}if path.ends_with("new-preview.wav"))));
            if matches!(mode, 2 | 3) {
                assert!(!s.is_playing());
                assert!(
                    commands
                        .iter()
                        .any(|c| matches!(c, GameplayAudioCommand::EditorStopMusic { .. }))
                );
            } else {
                assert_eq!(stamp(&s), before);
                unchanged_transport(&commands);
            }
            for (redo, bytes) in [(false, &original), (true, &after)] {
                history(&mut s, redo);
                assert_eq!(s.workspace.file().session.serialize().unwrap(), *bytes);
                let commands = s.take_audio_commands();
                if !matches!(mode, 2 | 3) {
                    assert_eq!(stamp(&s), before);
                    unchanged_transport(&commands);
                } else {
                    assert!(!s.is_playing());
                    assert!(!commands.iter().any(|c| matches!(
                        c,
                        GameplayAudioCommand::EditorStartMusic { .. }
                            | GameplayAudioCommand::EditorPlayPreview { .. }
                    )));
                }
            }
        }
    }
}
#[test]
fn sample_metadata_history_updates_only_main_preview_region_without_resetting_callback_cursor_en_fr()
 {
    for language in ["en", "fr"] {
        for mode in [0, 1, 2, 3] {
            let (_root, mut s) = state(language, mode);
            let original = s.workspace.file().session.serialize().unwrap();
            let before = stamp(&s);
            open(&mut s);
            s.song_properties
                .as_mut()
                .unwrap()
                .set_sample_start(0.375)
                .unwrap();
            s.song_properties
                .as_mut()
                .unwrap()
                .set_sample_length(1.25)
                .unwrap();
            apply(&mut s);
            let after = s.workspace.file().session.serialize().unwrap();
            for phase in 0..3 {
                if phase > 0 {
                    history(&mut s, phase == 2);
                    assert_eq!(
                        s.workspace.file().session.serialize().unwrap(),
                        if phase == 1 {
                            original.clone()
                        } else {
                            after.clone()
                        }
                    );
                }
                let commands = s.take_audio_commands();
                unchanged_transport(&commands);
                let actual = stamp(&s);
                if mode == 2 {
                    assert_eq!(actual.cursor, before.cursor);
                    assert_eq!(actual.registration, before.registration);
                    assert_eq!(actual.intent, before.intent);
                    assert_eq!(actual.focus, before.focus);
                    let (start, length) = if phase == 1 { (0., 2.) } else { (0.375, 1.25) };
                    assert!(commands.iter().any(|c|matches!(c,GameplayAudioCommand::EditorSetPreviewRegion{start:a,length:b,..}if *a==start&&*b==length)));
                    match actual.sample.unwrap() {
                        GameplayAudioCommand::EditorPlayPreview { cut, .. } => {
                            assert_eq!(cut.start_sec, start);
                            assert_eq!(cut.length_sec, length);
                        }
                        _ => panic!("main preview descriptor"),
                    }
                } else {
                    assert_eq!(actual, before);
                    assert!(
                        !commands.iter().any(|c| matches!(
                            c,
                            GameplayAudioCommand::EditorSetPreviewRegion { .. }
                        ))
                    );
                }
            }
        }
    }
}
#[test]
fn physical_metadata_noop_cancel_and_stale_draft_refusal_preserve_actual_history_and_transport() {
    for language in ["en", "fr"] {
        for mode in [0, 1, 2, 5, 6] {
            let (_root, mut s) = state(language, mode);
            let before = stamp(&s);
            let original = s.workspace.file().session.serialize().unwrap();
            let revision = s.workspace.file().session.revision();
            open(&mut s);
            apply(&mut s);
            assert_eq!(s.workspace.file().session.serialize().unwrap(), original);
            assert_eq!(s.workspace.file().session.revision(), revision);
            assert_eq!(stamp(&s), before);
            unchanged_transport(&s.take_audio_commands());
            open(&mut s);
            s.song_properties
                .as_mut()
                .unwrap()
                .set_text(Field::Title, "Cancel")
                .unwrap();
            s.panel = Some(Panel::SongProperties(2));
            press(&mut s, KeyCode::Enter);
            assert!(s.song_properties.is_none());
            assert_eq!(s.workspace.file().session.serialize().unwrap(), original);
            assert_eq!(stamp(&s), before);
            unchanged_transport(&s.take_audio_commands());
            open(&mut s);
            s.song_properties
                .as_mut()
                .unwrap()
                .set_text(Field::Title, "Stale")
                .unwrap();
            let mut other = s.workspace.song_properties().unwrap();
            other.set_text(Field::Credit, "Competing edit").unwrap();
            s.commit_song_properties_presented(&other).unwrap();
            s.refresh();
            let bytes = s.workspace.file().session.serialize().unwrap();
            let revision = s.workspace.file().session.revision();
            let undo = s.workspace.file().session.undo_label().map(str::to_owned);
            let after = stamp(&s);
            s.take_audio_commands();
            s.panel = Some(Panel::SongProperties(1));
            press(&mut s, KeyCode::Enter);
            assert!(s.song_properties.is_some());
            assert_eq!(s.workspace.file().session.serialize().unwrap(), bytes);
            assert_eq!(s.workspace.file().session.revision(), revision);
            assert_eq!(s.workspace.file().session.undo_label(), undo.as_deref());
            assert_eq!(stamp(&s), after);
            unchanged_transport(&s.take_audio_commands());
        }
    }
}
