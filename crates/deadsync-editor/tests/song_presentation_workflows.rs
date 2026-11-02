use deadsync_editor::{
    chart_focus::Position,
    document::{Document, Format},
    history::Session,
    song_properties::{Draft, Marker},
    song_text::Field,
    workspace::Workspace,
};
fn source(timing: &str) -> Vec<u8> {
    format!("#VERSION:0.83;#BPMS:0=120;#OFFSET:0;#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0}};#SAMPLESTART:1.25;#SAMPLELENGTH:5;#LASTSECONDHINT:2;{timing}#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;").into_bytes()
}
fn session(timing: &str) -> Session {
    Session::new(Document::parse(source(timing), Format::Ssc).unwrap())
}
fn exact(a: &Position, b: &Position) {
    assert_eq!(a.row().to_bits(), b.row().to_bits());
    assert_eq!(a.music_time().to_bits(), b.music_time().to_bits());
    assert_eq!(a.chart_time().to_bits(), b.chart_time().to_bits());
    assert_eq!(a.edit_row(), b.edit_row());
    assert_eq!(a.chart(), b.chart());
}
#[test]
fn metadata_receipt_preserves_distinct_equal_time_warp_positions_without_inverse_or_seek() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("warp.ssc");
    std::fs::write(&path, source("#WARPS:1=2;")).unwrap();
    let mut workspace = Workspace::open(&path, 0).unwrap();
    let a = Position::at_fractional_row(&workspace.file().session, 0, 60.5).unwrap();
    let b = Position::at_fractional_row(&workspace.file().session, 0, 90.75).unwrap();
    assert_ne!(a.row(), b.row());
    assert_eq!(a.music_time(), b.music_time());
    workspace.seek_preserving_selection(a.edit_row()).unwrap();
    let mut draft = workspace.song_properties().unwrap();
    draft
        .move_marker_to_time(Marker::Preview, a.music_time())
        .unwrap();
    let applied = workspace
        .apply_song_properties_with_presentation_receipt(&draft)
        .unwrap();
    assert!(applied.changed());
    let receipt = applied.presentation_receipt().unwrap();
    assert!(receipt.preserves_audio());
    let rebound_a = a
        .rebind_song_properties(&workspace.file().session, receipt)
        .unwrap();
    let rebound_b = b
        .rebind_song_properties(&workspace.file().session, receipt)
        .unwrap();
    exact(&a, &rebound_a);
    exact(&b, &rebound_b);
    assert!(rebound_a.is_current(&workspace));
    assert_eq!(workspace.row(), a.edit_row());
    assert!(
        a.rebind_song_properties(&session("#WARPS:1=2;"), receipt)
            .is_err()
    );
}
#[test]
fn stop_interior_time_and_row_are_rebound_independently() {
    let mut session = session("#STOPS:1=1;");
    let focus = Position::at_time(&session, 0, 0.75).unwrap();
    assert_eq!(focus.row(), 48.);
    assert_eq!(focus.music_time(), 0.75);
    assert_ne!(
        Position::at_fractional_row(&session, 0, 48.)
            .unwrap()
            .music_time(),
        0.75
    );
    let mut draft = Draft::capture(&session).unwrap();
    draft.move_marker_to_time(Marker::EndHint, 0.75).unwrap();
    let applied = draft.apply_with_presentation_receipt(&mut session).unwrap();
    let receipt = applied.presentation_receipt().unwrap();
    let rebound = focus.rebind_song_properties(&session, receipt).unwrap();
    exact(&focus, &rebound);
    session
        .set_text(None, "TITLE", "different revision")
        .unwrap();
    assert!(focus.rebind_song_properties(&session, receipt).is_err());
}
#[test]
fn only_real_clock_preserving_commits_mint_receipts_and_audio_change_is_explicit() {
    for clock_field in [false, true] {
        let mut session = session("");
        let mut draft = Draft::capture(&session).unwrap();
        if clock_field {
            draft.set_music_offset(0.25).unwrap();
        } else {
            draft.set_sync_offset(0.25).unwrap();
        }
        let applied = draft.apply_with_presentation_receipt(&mut session).unwrap();
        assert!(applied.changed());
        assert!(applied.presentation_receipt().is_none());
    }
    let mut session = session("");
    let draft = Draft::capture(&session).unwrap();
    let revision = session.revision();
    let applied = draft.apply_with_presentation_receipt(&mut session).unwrap();
    assert!(!applied.changed());
    assert!(applied.presentation_receipt().is_none());
    assert_eq!(session.revision(), revision);
    let focus = Position::at_fractional_row(&session, 0, 12.48).unwrap();
    let mut draft = Draft::capture(&session).unwrap();
    draft.set_text(Field::Music, "changed.ogg").unwrap();
    let applied = draft.apply_with_presentation_receipt(&mut session).unwrap();
    let receipt = applied.presentation_receipt().unwrap();
    assert!(!receipt.preserves_audio());
    exact(
        &focus,
        &focus.rebind_song_properties(&session, receipt).unwrap(),
    );
}
