use deadsync_editor::{
    chart_focus::Position,
    document::{Document, Format},
    history::Session,
    song_properties::{Draft, live::Plan},
    song_text::Field,
};
fn session() -> Session {
    Session::new(Document::parse(b"#VERSION:0.83;#MUSIC:main.wav;#OFFSET:0.125;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;".as_slice(),Format::Ssc).unwrap())
}
#[test]
fn prospective_song_offsets_preserve_fractional_row_then_typed_undo_redo() {
    let mut s = session();
    let original = s.serialize().unwrap();
    let row = 137.25;
    let old = Position::at_fractional_row(&s, 0, row).unwrap();
    let revision = s.revision();
    let mut draft = Draft::capture(&s).unwrap();
    draft.set_sync_offset(0.375).unwrap();
    draft.set_music_offset(0.25).unwrap();
    let plan = Plan::prepare(&draft, &s, Some((0, row))).unwrap();
    assert_eq!(s.serialize().unwrap(), original);
    assert_eq!(s.revision(), revision);
    assert!(!s.history_changes_song_properties_live(false));
    let application = plan.apply(&mut s).unwrap();
    assert!(application.changed());
    let position = application.into_position().unwrap();
    assert!(position.is_bound_to(&s, 0));
    assert_eq!(position.row().to_bits(), row.to_bits());
    assert_eq!(position.music_time(), old.music_time() + 0.25);
    assert!(s.history_changes_song_properties_live(false));
    let after = s.serialize().unwrap();
    for (redo, bytes, time) in [
        (false, &original, old.music_time()),
        (true, &after, position.music_time()),
    ] {
        let plan = Plan::prepare_history(&s, Some((0, row)), redo).unwrap();
        let position = plan.apply(&mut s).unwrap().into_position().unwrap();
        assert_eq!(s.serialize().unwrap(), *bytes);
        assert_eq!(position.row().to_bits(), row.to_bits());
        assert_eq!(position.music_time(), time);
        assert!(position.is_bound_to(&s, 0));
    }
}
#[test]
fn invalid_prospective_clock_and_stale_review_are_atomic_including_redo() {
    let mut s = session();
    s.set_text(None, "TITLE", "Prior").unwrap();
    s.undo().unwrap();
    let original = s.serialize().unwrap();
    let revision = s.revision();
    let redo = s.redo_label().map(str::to_owned);
    let mut draft = Draft::capture(&s).unwrap();
    draft.set_sync_offset(f64::MAX).unwrap();
    draft.set_music_offset(-f64::MAX).unwrap();
    assert!(Plan::prepare(&draft, &s, Some((0, 96.25))).is_err());
    assert_eq!(s.serialize().unwrap(), original);
    assert_eq!(s.revision(), revision);
    assert_eq!(s.redo_label(), redo.as_deref());
    let mut draft = Draft::capture(&s).unwrap();
    draft.set_text(Field::Music, "other.wav").unwrap();
    let plan = Plan::prepare(&draft, &s, Some((0, 96.25))).unwrap();
    s.redo().unwrap();
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), original);
    assert!(plan.apply(&mut s).is_err());
    assert_eq!(s.serialize().unwrap(), original);
}
#[test]
fn metadata_and_preview_keep_existing_receipts_while_chartless_music_history_is_typed() {
    let mut s = session();
    let mut draft = Draft::capture(&s).unwrap();
    draft.set_text(Field::Title, "Title").unwrap();
    draft.set_text(Field::PreviewMusic, "preview.wav").unwrap();
    assert!(!draft.requires_live_handoff());
    let application = draft.apply_with_presentation_receipt(&mut s).unwrap();
    assert!(
        application
            .presentation_receipt()
            .unwrap()
            .preserves_audio()
    );
    assert!(!s.history_changes_song_properties_live(false));
    let mut draft = Draft::capture(&s).unwrap();
    draft.set_text(Field::Music, "next.wav").unwrap();
    let plan = Plan::prepare(&draft, &s, None).unwrap();
    assert!(plan.apply(&mut s).unwrap().into_position().is_none());
    assert!(s.history_changes_song_properties_live(false));
    assert!(
        Plan::prepare_history(&s, None, false)
            .unwrap()
            .apply(&mut s)
            .unwrap()
            .into_position()
            .is_none()
    );
    assert_eq!(
        s.document().text(None, "MUSIC").as_deref(),
        Some("main.wav")
    );
}
#[test]
fn chart_offset_override_is_effective_and_global_offset_still_has_typed_history() {
    let mut s = session();
    s.set_text(Some(0), "OFFSET", "0.5").unwrap();
    let before = Position::at_fractional_row(&s, 0, 96.5).unwrap();
    let mut draft = Draft::capture(&s).unwrap();
    draft.set_music_offset(0.875).unwrap();
    let position = Plan::prepare(&draft, &s, Some((0, 96.5)))
        .unwrap()
        .apply(&mut s)
        .unwrap()
        .into_position()
        .unwrap();
    assert_eq!(
        position.music_time().to_bits(),
        before.music_time().to_bits()
    );
    assert_eq!(position.row().to_bits(), before.row().to_bits());
    assert!(s.history_changes_song_properties_live(false));
}
#[test]
fn typed_song_music_history_relocates_both_directions_and_recovery_keeps_clock() {
    use deadsync_editor::storage::FileSession;
    let root = tempfile::tempdir().unwrap();
    let base = root.path().canonicalize().unwrap();
    let old = base.join("old");
    let new = base.join("new");
    std::fs::create_dir(&old).unwrap();
    std::fs::create_dir(&new).unwrap();
    for name in ["main.wav", "next.wav"] {
        std::fs::write(old.join(name), b"descriptor").unwrap();
    }
    let path = old.join("song.ssc");
    std::fs::write(&path, session().serialize().unwrap()).unwrap();
    let mut file = FileSession::open(&path).unwrap();
    let mut draft = Draft::capture(&file.session).unwrap();
    draft.set_text(Field::Music, "next.wav").unwrap();
    draft.set_sync_offset(0.375).unwrap();
    Plan::prepare(&draft, &file.session, Some((0, 96.25)))
        .unwrap()
        .apply(&mut file.session)
        .unwrap();
    let destination = new.join("copy.ssc");
    let review = file.prepare_save_as(&destination).unwrap();
    file.save_as(review).unwrap();
    assert!(file.session.history_changes_song_properties_live(false));
    for (redo, name, sync) in [(false, "main.wav", 0.), (true, "next.wav", 0.375)] {
        let position = Plan::prepare_history(&file.session, Some((0, 96.25)), redo)
            .unwrap()
            .apply(&mut file.session)
            .unwrap()
            .into_position()
            .unwrap();
        assert_eq!(position.row(), 96.25);
        assert_eq!(Draft::capture(&file.session).unwrap().sync_offset(), sync);
        let relative = file.session.document().text(None, "MUSIC").unwrap();
        assert_eq!(
            new.join(relative).canonicalize().unwrap(),
            old.join(name).canonicalize().unwrap()
        );
    }
    let bytes = file.session.serialize().unwrap();
    file.save_recovery().unwrap();
    drop(file);
    let mut reopened = FileSession::open(&destination).unwrap();
    reopened.recover().unwrap();
    assert_eq!(reopened.session.serialize().unwrap(), bytes);
    assert_eq!(
        Draft::capture(&reopened.session).unwrap().sync_offset(),
        0.375
    );
    assert_eq!(
        Position::at_fractional_row(&reopened.session, 0, 96.25)
            .unwrap()
            .row(),
        96.25
    );
}
