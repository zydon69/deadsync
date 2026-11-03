use super::super::{RawKeyboardEvent, playback::song_properties_live_tests::fixture};
use super::*;
use deadsync_editor::workspace::Command;
fn press(s: &mut State, code: KeyCode) {
    for pressed in [true, false] {
        s.raw_key(&RawKeyboardEvent {
            code,
            pressed,
            repeat: false,
            timestamp: std::time::Instant::now(),
            host_nanos: 0,
        });
    }
}
fn numeric(s: &mut State, row: usize, value: &str) {
    s.panel = Some(Panel::ZoomSettings(row));
    press(s, KeyCode::Enter);
    assert!(s.wants_text_input());
    assert!(s.text_input(value));
    press(s, KeyCode::Enter);
    assert!(s.preference_numeric.is_none());
}
fn save(s: &mut State) {
    s.panel = Some(Panel::ZoomSettings(5));
    press(s, KeyCode::Enter);
    assert!(s.zoom_preferences_draft.is_none(), "{}", s.status);
}
fn stamp(s: &State) -> (u64, Option<u64>, Option<u64>, u64, bool, bool, bool) {
    if s.paused_playback.is_some() {
        assert!(!s.chart_transport_is_playing());
        assert!(s.transport_music_time().is_some(), "Zoom publication must retain a valid paused content binding");
    }
    (
        s.editor_asset_owner(),
        s.chart_audio_generation(),
        s.playback_asset_epoch(),
        s.chart_focus_time().unwrap().to_bits(),
        s.audio_output_snapshot().leased,
        s.chart_transport_is_playing(),
        s.paused_playback.is_some(),
    )
}
fn sound(s: &mut State) {
    let commands = s.take_audio_commands();
    assert!(
        !commands.iter().any(|c| matches!(
            c,
            deadsync_gameplay::GameplayAudioCommand::EditorStopMusic { .. }
                | deadsync_gameplay::GameplayAudioCommand::EditorPauseMusic { .. }
                | deadsync_gameplay::GameplayAudioCommand::EditorStartMusic { .. }
                | deadsync_gameplay::GameplayAudioCommand::EditorResumeMusic { .. }
                | deadsync_gameplay::GameplayAudioCommand::EditorSeekMusic { .. }
                | deadsync_gameplay::GameplayAudioCommand::EditorSwapMusicAsset { .. }
                | deadsync_gameplay::GameplayAudioCommand::EditorClearMusicAsset { .. }
                | deadsync_gameplay::GameplayAudioCommand::EditorEndAudio { .. }
        )),
        "{commands:?}"
    );
}
#[test]
fn precise_zoom_mixed_apply_undo_redo_preserves_direct_cap_animation_and_actual_audio_en_fr() {
    for language in ["en", "fr"] {
        for chartless in [false, true] {
            for paused in [false, true] {
                let (_root, mut s, _owner) = fixture(language);
                if chartless {
                    s.chart_views_apply(super::super::chart_views::ViewAction::Close);
                }
                if paused {
                    s.toggle_audio();
                }
                let before = s.zoom_preferences;
                let audio = stamp(&s);
                let song = s.workspace.file().session.serialize().unwrap();
                s.take_audio_commands();
                s.zoom
                    .process_input(
                        0.,
                        1.,
                        false,
                        true,
                        deadsync_editor::zoom::SpacingMode::ConstantTime,
                    )
                    .unwrap();
                assert!(s.zoom.spacing_is_interpolating());
                s.open_zoom_settings();
                numeric(&mut s, 0, "0.25");
                numeric(&mut s, 1, "1.7");
                numeric(&mut s, 2, "0.75");
                s.panel = Some(Panel::ZoomSettings(3));
                press(&mut s, KeyCode::Enter);
                save(&mut s);
                assert_eq!(
                    (
                        s.zoom.interpolation_duration(),
                        s.zoom.zoom_multiplier(),
                        s.zoom.size_cap(),
                        s.zoom.limit_to_size()
                    ),
                    (0.25, 1.7, 0.75, true)
                );
                assert!(s.zoom.spacing_is_interpolating());
                assert_eq!(stamp(&s), audio);
                sound(&mut s);
                let revision = s.workspace.file().session.revision();
                let undo = s.workspace.file().session.undo_label().map(str::to_owned);
                s.open_zoom_settings();
                numeric(&mut s, 2, "0.5");
                save(&mut s);
                assert_eq!(s.workspace.file().session.revision(), revision);
                assert_eq!(s.workspace.file().session.undo_label(), undo.as_deref());
                for redo in [false, true] {
                    s.command(if redo { Command::Redo } else { Command::Undo });
                    let p = s.zoom_preferences;
                    assert_eq!(p.size_cap(), 0.5);
                    assert_eq!(
                        (p.duration(), p.multiplier(), p.limited()),
                        if redo {
                            (0.25, 1.7, true)
                        } else {
                            (before.duration(), before.multiplier(), before.limited())
                        }
                    );
                    assert!(s.zoom.spacing_is_interpolating());
                    assert_eq!(stamp(&s), audio);
                    assert_eq!(s.workspace.file().session.serialize().unwrap(), song);
                    let file = s.capture_application_preferences_file().unwrap();
                    assert_eq!(file.zoom(), p);
                    assert!(!file.has_changes());
                    sound(&mut s);
                }
            }
        }
    }
}
#[test]
fn zoom_cancel_noop_and_disk_or_runtime_conflict_do_not_publish_a_partial_mixed_edit_en_fr() {
    for language in ["en", "fr"] {
        for runtime in [false, true] {
            let (root, mut s, _owner) = fixture(language);
            let path = root.path().join("prefs.json");
            std::fs::write(&path, br#"{"Version":1}"#).unwrap();
            s.configure_audio_preferences(&path).unwrap();
            let original = s.zoom_preferences;
            let audio = stamp(&s);
            let revision = s.workspace.file().session.revision();
            let disk = std::fs::read(&path).unwrap();
            s.take_audio_commands();
            s.open_zoom_settings();
            numeric(&mut s, 0, "0.25");
            numeric(&mut s, 2, "0.75");
            press(&mut s, KeyCode::Escape);
            assert_eq!(s.zoom_preferences, original);
            assert_eq!(std::fs::read(&path).unwrap(), disk);
            assert_eq!(stamp(&s), audio);
            sound(&mut s);
            s.open_zoom_settings();
            save(&mut s);
            assert_eq!(s.workspace.file().session.revision(), revision);
            assert_eq!(std::fs::read(&path).unwrap(), disk);
            s.open_zoom_settings();
            numeric(&mut s, 0, "0.25");
            numeric(&mut s, 2, "0.75");
            s.zoom_preferences_draft.as_mut().unwrap().set_limited(true);
            if runtime {
                let mut p = original;
                p.set_duration(0.3).unwrap();
                p.apply(&mut s.zoom).unwrap();
                s.zoom_preferences = p;
            } else {
                std::fs::write(
                    &path,
                    br#"{"Version":1,"Zoom":{"ScrollInterpolationDuration":0.4}}"#,
                )
                .unwrap();
            }
            let disk = std::fs::read(&path).unwrap();
            let live = s.zoom_preferences;
            s.panel = Some(Panel::ZoomSettings(5));
            press(&mut s, KeyCode::Enter);
            assert!(s.zoom_preferences_draft.is_some());
            assert_eq!(
                s.status,
                i18n::tr_fmt(
                    "Edition",
                    "Error",
                    &[(
                        "message",
                        &i18n::editor_error(&deadsync_editor::Error::ExternalChange)
                    )]
                )
            );
            assert_eq!(s.zoom_preferences, live);
            assert_eq!(std::fs::read(&path).unwrap(), disk);
            assert_eq!(s.workspace.file().session.revision(), revision);
            assert_eq!(stamp(&s), audio);
            sound(&mut s);
        }
    }
}
