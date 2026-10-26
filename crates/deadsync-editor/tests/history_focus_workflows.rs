use deadsync_editor::{
    audio_preferences::Audio,
    document::{Document, Format},
    history::Session,
    workspace::{Command, Workspace},
};
use std::fs;
fn setup() -> (tempfile::TempDir, Workspace) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("history.ssc");
    fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#VENDOR:keep;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0000\n0000\n0000;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0100\n0000\n0000\n0000;").unwrap();
    let workspace = Workspace::open(&path, 0).unwrap();
    (directory, workspace)
}
#[test]
fn undo_and_redo_notes_on_closed_chart_update_document_without_reopening_any_view() {
    let (_directory, mut workspace) = setup();
    let before = workspace.file().session.serialize().unwrap();
    workspace.execute(Command::Place(1)).unwrap();
    let after = workspace.file().session.serialize().unwrap();
    assert_ne!(after, before);
    workspace.clear_active_chart();
    workspace.execute(Command::Undo).unwrap();
    assert!(!workspace.has_chart());
    assert_eq!(workspace.file().session.serialize().unwrap(), before);
    workspace.execute(Command::Redo).unwrap();
    assert!(!workspace.has_chart());
    assert_eq!(workspace.file().session.serialize().unwrap(), after);
    let before_audio = Audio::default();
    let mut after_audio = before_audio;
    after_audio.set_music_rate(1.5).unwrap();
    workspace
        .set_audio_preferences(before_audio, after_audio, None)
        .unwrap();
    workspace.take_audio_preferences_update();
    workspace.execute(Command::Undo).unwrap();
    assert!(!workspace.has_chart());
    assert_eq!(
        workspace.take_audio_preferences_update().unwrap().audio,
        before_audio
    );
    workspace.execute(Command::Redo).unwrap();
    assert!(!workspace.has_chart());
    assert_eq!(
        workspace.take_audio_preferences_update().unwrap().audio,
        after_audio
    );
}
#[test]
fn note_history_does_not_take_focus_from_a_different_open_chart() {
    let (_directory, mut workspace) = setup();
    workspace.execute(Command::Seek(96)).unwrap();
    let before = workspace.file().session.serialize().unwrap();
    workspace.execute(Command::Place(1)).unwrap();
    let after = workspace.file().session.serialize().unwrap();
    workspace.execute(Command::SelectChart(1)).unwrap();
    let focused = workspace.file().session.chart_ids()[1].clone();
    for command in [Command::Undo, Command::Redo] {
        workspace.execute(command).unwrap();
        assert_eq!(workspace.chart(), 1);
        assert_eq!(workspace.row(), 96);
        assert_eq!(
            workspace.file().session.chart_ids()[workspace.chart()],
            focused
        );
    }
    assert_eq!(workspace.file().session.serialize().unwrap(), after);
    workspace.execute(Command::Undo).unwrap();
    assert_eq!(workspace.file().session.serialize().unwrap(), before);
    assert_eq!(workspace.chart(), 1);
}
#[test]
fn structural_deletion_undo_restores_explicit_focus_even_after_last_view_was_closed() {
    let (_directory, mut workspace) = setup();
    let original = workspace.file().session.chart_ids().to_vec();
    let before = workspace.file().session.serialize().unwrap();
    let draft = workspace.deletion_draft().unwrap();
    workspace.delete_chart(&draft).unwrap();
    assert_eq!(workspace.file().session.chart_ids(), &original[1..]);
    workspace.clear_active_chart();
    workspace.execute(Command::Undo).unwrap();
    assert_eq!(workspace.chart(), 0);
    assert_eq!(
        workspace.file().session.chart_ids()[workspace.chart()],
        original[0]
    );
    assert_eq!(workspace.file().session.serialize().unwrap(), before);
    workspace.clear_active_chart();
    workspace.execute(Command::Redo).unwrap();
    assert_eq!(workspace.chart(), 0);
    assert_eq!(
        workspace.file().session.chart_ids()[workspace.chart()],
        original[1]
    );
}
#[test]
fn typed_session_effect_distinguishes_noop_content_preferences_and_identity_structure() {
    let (_directory, workspace) = setup();
    let mut session = Session::new(
        Document::parse(workspace.file().session.serialize().unwrap(), Format::Ssc).unwrap(),
    );
    let original = session.chart_ids().to_vec();
    let noop = session.undo_with_effect().unwrap();
    assert!(!noop.applied);
    assert!(!noop.content_changed);
    assert!(noop.chart_structure.is_none());
    assert_eq!(noop.before_revision, noop.after_revision);
    session.set_text(None, "TITLE", "History").unwrap();
    let value = session.undo_with_effect().unwrap();
    assert!(value.applied);
    assert!(value.content_changed);
    assert!(value.chart_structure.is_none());
    let before_audio = Audio::default();
    let mut after_audio = before_audio;
    after_audio.set_music_rate(1.5).unwrap();
    session
        .set_audio_preferences(before_audio, after_audio, None)
        .unwrap();
    let audio = session.undo_with_effect().unwrap();
    assert!(audio.applied);
    assert!(!audio.content_changed);
    assert!(audio.chart_structure.is_none());
    assert_ne!(audio.before_revision, audio.after_revision);
    let revision = session.revision();
    let added = session.duplicate_chart(0, revision).unwrap();
    let added_id = session.chart_ids()[added].clone();
    let undo = session.undo_with_effect().unwrap();
    assert!(undo.applied);
    let delta = undo.chart_structure.unwrap();
    assert_eq!(delta.before.len(), original.len() + 1);
    assert_eq!(delta.before[added], added_id);
    assert_eq!(delta.after, original);
    let redo = session.redo_with_effect().unwrap();
    let delta = redo.chart_structure.unwrap();
    assert_eq!(delta.before, original);
    assert_eq!(delta.after[added], added_id);
}
