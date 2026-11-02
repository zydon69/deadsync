use super::*;
use std::fs;
fn fixture() -> (tempfile::TempDir, State) {
    i18n::init_for_tests();
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("song.ssc");
    fs::write(
        &path,
        b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
    )
    .unwrap();
    let mut s = State::open(&path, 0).unwrap();
    s.panel = None;
    s.take_audio_commands();
    (root, s)
}
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
#[test]
fn shown_modal_alerts_once_navigation_does_not_repeat_and_close_reopen_is_new() {
    let (_root, mut s) = fixture();
    s.command(Command::Place(1));
    s.take_audio_commands();
    let bytes = s.workspace.file().session.serialize().unwrap();
    let revision = s.workspace.file().session.revision();
    assert!(!s.request_close());
    assert!(matches!(s.panel, Some(Panel::FileUnsaved(0))));
    assert_eq!(s.take_editor_system_alert(), Some(SystemAlert::Exclamation));
    for code in [KeyCode::ArrowDown, KeyCode::ArrowUp, KeyCode::Tab] {
        press(&mut s, code);
        assert_eq!(s.take_editor_system_alert(), None);
    }
    press(&mut s, KeyCode::Escape);
    assert_eq!(s.take_editor_system_alert(), None);
    assert_eq!(s.workspace.file().session.serialize().unwrap(), bytes);
    assert_eq!(s.workspace.file().session.revision(), revision);
    assert!(!s.request_close());
    assert_eq!(s.take_editor_system_alert(), Some(SystemAlert::Exclamation));
    assert_eq!(s.take_editor_system_alert(), None);
}
#[test]
fn ftue_rebind_preferences_help_and_performance_have_no_source_modal_sound() {
    let (_root, mut s) = fixture();
    let bytes = s.workspace.file().session.serialize().unwrap();
    for panel in [
        Panel::Ftue(0),
        Panel::Performance(0),
        Panel::EditorHelp(0),
        Panel::WindowLayout(0),
        Panel::Log,
    ] {
        s.panel = Some(panel);
        s.system_alert_modal_transition();
        assert_eq!(s.take_editor_system_alert(), None);
    }
    s.panel = None;
    s.open_key_binding_settings();
    assert_eq!(s.take_editor_system_alert(), None);
    assert_eq!(s.workspace.file().session.serialize().unwrap(), bytes);
    assert!(s.take_audio_commands().is_empty());
}
#[test]
fn refused_save_queues_once_and_keeps_loaded_document_and_external_disk() {
    let (root, mut s) = fixture();
    let before = s.workspace.file().session.serialize().unwrap();
    let label = s.workspace.file().session.undo_label().map(str::to_owned);
    let external = b"externally replaced";
    fs::write(root.path().join("song.ssc"), external).unwrap();
    s.command(Command::Save);
    assert_eq!(s.take_editor_system_alert(), Some(SystemAlert::Exclamation));
    assert_eq!(s.take_editor_system_alert(), None);
    assert_eq!(s.workspace.file().session.serialize().unwrap(), before);
    assert_eq!(s.workspace.file().session.undo_label(), label.as_deref());
    assert_eq!(fs::read(root.path().join("song.ssc")).unwrap(), external);
}
#[test]
fn source_action_policy_excludes_preferences_navigation_and_read_only_copy() {
    for action in [
        Action::Arrow0,
        Action::Paste,
        Action::Undo,
        Action::RegenerateAllPatternsNewSeeds,
        Action::AddEventWarp,
    ] {
        assert_eq!(blocked_action(action), Some(Refusal::EditBusy));
    }
    for action in [
        Action::FileOpen,
        Action::FileReload,
        Action::Save,
        Action::SaveAs,
        Action::Exit,
    ] {
        assert_eq!(blocked_action(action), Some(Refusal::FileIoBusy));
    }
    for action in [
        Action::OpenSongProperties,
        Action::OpenKeyBindings,
        Action::OpenReceptors,
        Action::OpenAnalysis,
        Action::OpenMenu,
        Action::MoveUp,
        Action::Copy,
        Action::PlayPause,
    ] {
        assert_eq!(blocked_action(action), None);
    }
}
#[test]
fn source_modal_classes_and_refusal_coalesce_without_per_frame_or_selection_repetition() {
    let mut r = Runtime::default();
    for modal in [
        Modal::Update,
        Modal::ExternalSong,
        Modal::ExternalPack,
        Modal::DeletePack,
        Modal::Unsaved,
    ] {
        r.observe(None);
        r.observe(Some(modal));
        r.refusal = Some(Refusal::FileIoBusy);
        assert_eq!(r.take(), Some(SystemAlert::Exclamation));
        for _ in 0..10 {
            r.observe(Some(modal));
            assert_eq!(r.take(), None);
        }
    }
    r.observe(None);
    r.refusal = Some(Refusal::EditBusy);
    assert_eq!(r.take(), Some(SystemAlert::Exclamation));
    assert_eq!(r.take(), None);
}
