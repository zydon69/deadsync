use super::*;
use crate::screens::edition::{KeyCode, Panel, RawKeyboardEvent};
use deadsync_editor::key_binding_preferences::{
    Preferences as Keys,
    scroll_modifiers::{Kind, Query},
};
fn key(s: &mut State, code: KeyCode, pressed: bool) {
    s.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        repeat: false,
        timestamp: std::time::Instant::now(),
        host_nanos: 0,
    });
}
fn press(s: &mut State, code: KeyCode) {
    key(s, code, true);
    key(s, code, false);
}
fn no_handoff(s: &mut State) {
    let commands = s.take_audio_commands();
    assert!(
        !commands.iter().any(|c| matches!(
            c,
            deadsync_gameplay::GameplayAudioCommand::StopMusic
                | deadsync_gameplay::GameplayAudioCommand::SeekMusic(_)
                | deadsync_gameplay::GameplayAudioCommand::PlayMusic { .. }
                | deadsync_gameplay::GameplayAudioCommand::EditorStopMusic { .. }
                | deadsync_gameplay::GameplayAudioCommand::EditorStartMusic { .. }
                | deadsync_gameplay::GameplayAudioCommand::EditorPauseMusic { .. }
                | deadsync_gameplay::GameplayAudioCommand::EditorResumeMusic { .. }
                | deadsync_gameplay::GameplayAudioCommand::EditorSwapMusicAsset { .. }
                | deadsync_gameplay::GameplayAudioCommand::EditorSeekMusic { .. }
                | deadsync_gameplay::GameplayAudioCommand::EditorEndAudio { .. }
                | deadsync_gameplay::GameplayAudioCommand::EditorPlayMusic { .. }
                | deadsync_gameplay::GameplayAudioCommand::EditorPlayPreview { .. }
                | deadsync_gameplay::GameplayAudioCommand::EditorResumePreview { .. }
                | deadsync_gameplay::GameplayAudioCommand::EditorClearMusicAsset { .. }
        )),
        "{commands:?}"
    );
}
#[test]
fn remapped_axis_query_drives_real_two_dimensional_draft_projection_save_history_and_keeps_main_pause_en_fr()
 {
    for language in ["en", "fr"] {
        for paused in [false, true] {
            deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
                854., 480.,
            ));
            let (_dir, mut s, _owner) =
                super::super::playback::song_properties_live_tests::fixture(language);
            let mut keys = (*s.command_bindings.preferences).clone();
            keys.scroll_modifiers.set(
                Kind::ReceptorAxis,
                Query::from_alternatives(vec![vec!["F13".into()]]),
            );
            s.configure_key_binding_preferences(Arc::new(keys));
            if paused {
                s.toggle_audio();
                assert!(s.paused_position().is_some());
            }
            s.take_audio_commands();
            let audio = (
                s.save_as_transport_stamp(),
                s.chart_audio_generation(),
                s.playback_asset_epoch(),
                s.audio_output_snapshot().leased,
            );
            let original = s.workspace.file().session.serialize().unwrap();
            let before = s.receptor_preferences();
            let canvas = s.receptor_canvas_position();
            s.open_receptor_settings();
            s.panel = Some(Panel::ReceptorSettings(12));
            press(&mut s, KeyCode::F2);
            for _ in 0..3 {
                press(&mut s, KeyCode::ArrowRight);
            }
            for _ in 0..2 {
                press(&mut s, KeyCode::ArrowDown);
            }
            assert_eq!(
                s.receptor_draft.unwrap().placement.x,
                before.placement.x + 3
            );
            assert_eq!(
                s.receptor_draft.unwrap().placement.y,
                before.placement.y + 2
            );
            key(&mut s, KeyCode::ShiftRight, true);
            assert_eq!(
                s.receptor_draft.unwrap().placement.y,
                before.placement.y + 2
            );
            key(&mut s, KeyCode::ShiftRight, false);
            key(&mut s, KeyCode::F13, true);
            assert_eq!(s.receptor_draft.unwrap().placement.y, before.placement.y);
            key(&mut s, KeyCode::F13, false);
            assert_eq!(
                s.receptor_draft.unwrap().placement.y,
                before.placement.y + 2
            );
            key(&mut s, KeyCode::F13, true);
            press(&mut s, KeyCode::Enter);
            assert!(!s.wants_text_input());
            key(&mut s, KeyCode::F13, false);
            let target = s.receptor_draft.unwrap();
            assert_eq!(target.placement.y, before.placement.y);
            s.panel = Some(Panel::ReceptorSettings(9));
            press(&mut s, KeyCode::Enter);
            assert_eq!(s.receptor_preferences(), target);
            assert_eq!(s.receptor_canvas_position(), (canvas.0 + 3., canvas.1));
            assert_eq!(
                s.capture_application_preferences_file()
                    .unwrap()
                    .receptors(),
                target
            );
            for (redo, expected) in [(false, before), (true, target)] {
                s.command(if redo { Command::Redo } else { Command::Undo });
                assert_eq!(s.receptor_preferences(), expected);
                assert_eq!(s.workspace.file().session.serialize().unwrap(), original);
                assert_eq!(
                    (
                        s.save_as_transport_stamp(),
                        s.chart_audio_generation(),
                        s.playback_asset_epoch(),
                        s.audio_output_snapshot().leased
                    ),
                    audio
                );
                if paused {
                    assert!(s.paused_position().is_some());
                }
                no_handoff(&mut s);
            }
        }
    }
}
#[test]
fn axis_move_cancel_focus_release_competing_draft_and_source_stale_do_not_publish() {
    crate::i18n::init_for_tests();
    let dir = tempfile::tempdir().unwrap();
    let mut s = State::without_song(dir.path()).unwrap();
    s.panel = None;
    let original = s.receptor_preferences();
    let owner = s.capture_application_preferences_file().unwrap();
    s.open_receptor_settings();
    press(&mut s, KeyCode::F2);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::Escape);
    assert_eq!(s.receptor_draft, Some(original));
    assert!(s.receptor_move.is_none());
    assert!(matches!(s.panel, Some(Panel::ReceptorSettings(_))));
    press(&mut s, KeyCode::F2);
    press(&mut s, KeyCode::ArrowDown);
    s.release_input();
    assert_eq!(s.receptor_draft, Some(original));
    assert!(s.receptor_move.is_none());
    assert_eq!(s.receptor_preferences(), original);
    assert_eq!(
        s.capture_application_preferences_file()
            .unwrap()
            .receptors(),
        owner.receptors()
    );
    press(&mut s, KeyCode::F2);
    let mut competing = original;
    competing.placement.x += 17;
    s.receptor_draft = Some(competing);
    s.update_receptor_move();
    assert!(s.receptor_move.is_none());
    assert_eq!(s.receptor_draft, Some(competing));
    assert_eq!(s.receptor_preferences(), original);
    let (_dir, mut changed, _owner) =
        super::super::playback::song_properties_live_tests::fixture("en");
    let before = changed.receptor_preferences();
    changed.open_receptor_settings();
    press(&mut changed, KeyCode::F2);
    press(&mut changed, KeyCode::ArrowRight);
    changed.workspace.execute(Command::Place(1)).unwrap();
    let competing = changed.workspace.file().session.serialize().unwrap();
    changed.update_receptor_move();
    assert!(changed.receptor_move.is_none());
    assert!(changed.receptor_draft.is_none());
    assert!(changed.panel.is_none());
    assert_eq!(changed.receptor_preferences(), before);
    assert_eq!(
        changed.workspace.file().session.serialize().unwrap(),
        competing
    );
}
#[test]
fn escape_can_be_an_axis_key_while_f2_confirms_and_ctrl_escape_cancels() {
    crate::i18n::init_for_tests();
    let dir = tempfile::tempdir().unwrap();
    let mut s = State::without_song(dir.path()).unwrap();
    s.panel = None;
    let mut prefs = Keys::default();
    prefs.scroll_modifiers.set(
        Kind::ReceptorAxis,
        Query::from_alternatives(vec![vec!["Escape".into()]]),
    );
    s.configure_key_binding_preferences(Arc::new(prefs));
    s.open_receptor_settings();
    let before = s.receptor_draft.unwrap();
    press(&mut s, KeyCode::F2);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::ArrowDown);
    key(&mut s, KeyCode::Escape, true);
    assert!(s.receptor_move.is_some());
    assert_eq!(s.receptor_draft.unwrap().placement.x, before.placement.x);
    assert_eq!(
        s.receptor_draft.unwrap().placement.y,
        before.placement.y + 1
    );
    press(&mut s, KeyCode::F2);
    key(&mut s, KeyCode::Escape, false);
    assert!(s.receptor_move.is_none());
    assert_eq!(s.receptor_draft.unwrap().placement.x, before.placement.x);
    press(&mut s, KeyCode::F2);
    press(&mut s, KeyCode::ArrowRight);
    key(&mut s, KeyCode::ControlLeft, true);
    press(&mut s, KeyCode::Escape);
    key(&mut s, KeyCode::ControlLeft, false);
    assert!(s.receptor_move.is_none());
    assert_eq!(s.receptor_preferences(), before);
}
