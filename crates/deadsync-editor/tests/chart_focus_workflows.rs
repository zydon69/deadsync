use deadsync_editor::{
    chart_focus::{Position, Transition},
    playback::Clock,
    workspace::{Command, Workspace},
};
use std::fs;
fn workspace(
    source_timing: &str,
    target_timing: &str,
    target_notes: &str,
) -> (tempfile::TempDir, Workspace) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("focus.ssc");
    fs::write(&path, format!("#VERSION:0.83;#BPMS:0=120;#OFFSET:0;#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0}};#VENDOR:keep;#NOTEDATA:;#STEPSTYPE:dance-single;{source_timing}#NOTES:1000\n0000\n0000\n0000;#NOTEDATA:;#STEPSTYPE:dance-double;{target_timing}#NOTES:{target_notes};")).unwrap();
    (directory, Workspace::open(&path, 0).unwrap())
}
fn ordinary() -> (tempfile::TempDir, Workspace) {
    workspace(
        "",
        "#OFFSET:0.25;#BPMS:0=60;",
        "10000000\n00000000\n00000000\n00000000",
    )
}
fn close(left: f64, right: f64) {
    assert!((left - right).abs() < 1e-10, "{left} != {right}");
}
#[test]
fn explicit_focus_preserves_music_time_across_offsets_and_bpm_without_content_history_edits() {
    let (directory, mut workspace) = ordinary();
    workspace.execute(Command::Seek(96)).unwrap();
    workspace.execute(Command::ToggleSelection).unwrap();
    let source = workspace.file().session.serialize().unwrap();
    let revision = workspace.file().session.revision();
    let ids = workspace.file().session.chart_ids().to_vec();
    let transition = Transition::at_cursor(&workspace, 1).unwrap();
    close(transition.position().music_time(), 1.0);
    close(transition.position().chart_time(), 1.25);
    close(transition.position().row(), 60.0);
    let position = transition.apply(&mut workspace).unwrap();
    assert_eq!(workspace.chart(), 1);
    assert_eq!(workspace.row(), 60);
    assert_eq!(workspace.selection(), None);
    assert!(position.is_current(&workspace));
    close(
        Clock::capture(&workspace.file().session, 1)
            .unwrap()
            .music_time(60)
            .unwrap(),
        1.0,
    );
    assert_eq!(workspace.file().session.revision(), revision);
    assert_eq!(workspace.file().session.chart_ids(), ids);
    assert_eq!(workspace.file().session.serialize().unwrap(), source);
    assert_eq!(
        fs::read(directory.path().join("focus.ssc")).unwrap(),
        source
    );
    assert!(!workspace.file().session.is_dirty());
    assert!(workspace.file().session.undo_label().is_none());
}
#[test]
fn fractional_and_negative_focus_positions_keep_exact_time_separate_from_edit_cursor() {
    let (_directory, mut workspace) = ordinary();
    let position = Transition::at_time(&workspace, 1, 1.01)
        .unwrap()
        .apply(&mut workspace)
        .unwrap();
    close(position.row(), 60.48);
    assert_eq!(workspace.row(), 60);
    close(position.music_time(), 1.01);
    assert!(position.is_current(&workspace));
    // Round-tripping the integral cursor would lose 0.01 seconds.
    assert_ne!(
        Clock::capture(&workspace.file().session, 1)
            .unwrap()
            .music_time(workspace.row())
            .unwrap(),
        position.music_time()
    );
    let negative = Transition::at_time(&workspace, 0, -0.25)
        .unwrap()
        .apply(&mut workspace)
        .unwrap();
    close(negative.row(), -24.0);
    assert_eq!(workspace.row(), 0);
    close(negative.music_time(), -0.25);
    assert!(negative.is_current(&workspace));
}
#[test]
fn time_inside_stop_and_after_warp_is_preserved_without_ambiguous_row_roundtrip() {
    let (_directory, mut workspace) = workspace(
        "#STOPS:1=1;#DELAYS:2=0.125;#WARPS:3=2;",
        "#BPMS:0=120;",
        "10000000\n00000000\n00000000\n00000000",
    );
    let stopped = Transition::at_time(&workspace, 0, 0.75)
        .unwrap()
        .apply(&mut workspace)
        .unwrap();
    close(stopped.row(), 48.0);
    close(stopped.music_time(), 0.75);
    let target = Transition::at_time(&workspace, 1, stopped.music_time())
        .unwrap()
        .apply(&mut workspace)
        .unwrap();
    close(target.row(), 72.0);
    close(target.music_time(), 0.75);
    for time in [0.5, 1.25, 2.5, 2.75] {
        let expected = Clock::capture(&workspace.file().session, 0)
            .unwrap()
            .position(time)
            .unwrap();
        let position = Transition::at_time(&workspace, 0, time)
            .unwrap()
            .apply(&mut workspace)
            .unwrap();
        close(position.row(), expected);
        close(position.music_time(), time);
        let back = Transition::at_time(&workspace, 1, position.music_time())
            .unwrap()
            .apply(&mut workspace)
            .unwrap();
        close(back.music_time(), time);
        close(back.row(), time * 96.0);
    }
}
#[test]
fn invalid_conversion_or_target_grid_never_partially_changes_focus_or_cursor() {
    for (timing, notes) in [("#OFFSET:NaN;", "10000000"), ("", "broken")] {
        let (_directory, mut workspace) = workspace("", timing, notes);
        workspace.execute(Command::Seek(96)).unwrap();
        workspace.execute(Command::ToggleSelection).unwrap();
        let source = workspace.file().session.serialize().unwrap();
        let result = Transition::at_cursor(&workspace, 1)
            .and_then(|transition| transition.apply(&mut workspace));
        assert!(result.is_err());
        assert_eq!(workspace.chart(), 0);
        assert_eq!(workspace.row(), 96);
        assert_eq!(workspace.selection(), Some((96, 96)));
        assert_eq!(workspace.file().session.serialize().unwrap(), source);
    }
    let (_directory, workspace) = ordinary();
    for time in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(Transition::at_time(&workspace, 1, time).is_err());
    }
    assert!(Transition::at_cursor(&workspace, usize::MAX).is_err());
}
#[test]
fn stale_cursor_content_focus_and_session_reject_deferred_focus_transitions() {
    for action in [Command::Seek(1), Command::Place(1), Command::SelectChart(1)] {
        let (_directory, mut workspace) = ordinary();
        let transition = Transition::at_time(&workspace, 1, 0.75).unwrap();
        workspace.execute(action).unwrap();
        let chart = workspace.chart();
        let row = workspace.row();
        let source = workspace.file().session.serialize().unwrap();
        assert!(transition.apply(&mut workspace).is_err());
        assert_eq!(workspace.chart(), chart);
        assert_eq!(workspace.row(), row);
        assert_eq!(workspace.file().session.serialize().unwrap(), source);
    }
    let (_first, workspace) = ordinary();
    let (_second, mut reopened) = ordinary();
    assert!(
        Transition::at_cursor(&workspace, 1)
            .unwrap()
            .apply(&mut reopened)
            .is_err()
    );
}
#[test]
fn legacy_select_chart_keeps_its_row_contract_and_position_anchor_expires_after_cursor_edit() {
    let (_directory, mut workspace) = ordinary();
    workspace.execute(Command::Seek(96)).unwrap();
    workspace.execute(Command::SelectChart(1)).unwrap();
    assert_eq!(workspace.row(), 96);
    let position = Position::at_time(&workspace.file().session, 1, 1.01).unwrap();
    workspace
        .execute(Command::Seek(position.edit_row()))
        .unwrap();
    assert!(position.is_current(&workspace));
    workspace
        .execute(Command::Seek(position.edit_row() + 1))
        .unwrap();
    assert!(!position.is_current(&workspace));
}

#[test]
fn final_view_can_close_and_reopen_atomically_without_deleting_the_song() {
    let (_directory, mut workspace) = ordinary();
    let source = workspace.file().session.serialize().unwrap();
    workspace.clear_active_chart();
    assert!(!workspace.has_chart());
    assert_eq!(workspace.file().session.document().charts().len(), 2);
    let stale = Transition::open_at_time(&workspace, 1, 1.01).unwrap();
    workspace.execute(Command::SelectChart(0)).unwrap();
    assert!(stale.apply(&mut workspace).is_err());
    workspace.clear_active_chart();
    let position = Transition::open_at_time(&workspace, 1, 1.01)
        .unwrap()
        .apply(&mut workspace)
        .unwrap();
    assert_eq!(workspace.chart(), 1);
    assert_eq!(workspace.row(), 60);
    close(position.music_time(), 1.01);
    assert_eq!(workspace.file().session.serialize().unwrap(), source);
    assert!(!workspace.file().session.is_dirty());
}
#[test]
fn song_level_binding_remains_specific_to_content_and_session_without_a_focused_chart() {
    use deadsync_editor::{
        chart_order::SessionBinding,
        document::{Document, Format},
        history::Session,
    };
    let source = b"#VERSION:0.83;#BPMS:0=120;".to_vec();
    let mut session = Session::new(Document::parse(source.clone(), Format::Ssc).unwrap());
    let binding = SessionBinding::capture(&session);
    assert!(binding.is_current(&session));
    let other = Session::new(Document::parse(source, Format::Ssc).unwrap());
    assert!(!binding.is_current(&other));
    session.set_text(None, "TITLE", "Changed").unwrap();
    assert!(!binding.is_current(&session));
}

#[test]
fn empty_undo_redo_does_not_reopen_a_closed_view_but_real_preference_history_still_executes() {
    let (_directory, mut workspace) = ordinary();
    let source = workspace.file().session.serialize().unwrap();
    workspace.clear_active_chart();
    workspace.execute(Command::Undo).unwrap();
    workspace.execute(Command::Redo).unwrap();
    assert!(!workspace.has_chart());
    assert_eq!(workspace.file().session.serialize().unwrap(), source);
    let before = deadsync_editor::audio_preferences::Audio::default();
    let mut after = before;
    after.set_music_rate(1.5).unwrap();
    let content_revision = workspace.file().session.content_revision();
    assert!(
        workspace
            .set_audio_preferences(before, after, None)
            .unwrap()
    );
    assert_eq!(
        workspace.file().session.content_revision(),
        content_revision
    );
    workspace.take_audio_preferences_update();
    workspace.execute(Command::Undo).unwrap();
    assert_eq!(
        workspace.take_audio_preferences_update().unwrap().audio,
        before
    );
    assert_eq!(
        workspace.file().session.content_revision(),
        content_revision
    );
    workspace.execute(Command::Redo).unwrap();
    assert_eq!(
        workspace.take_audio_preferences_update().unwrap().audio,
        after
    );
    assert!(!workspace.file().session.is_dirty());
}
