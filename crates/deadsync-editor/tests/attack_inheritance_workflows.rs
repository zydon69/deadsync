use deadsync_editor::{
    attacks::{Draft as FileDraft, inspector::Draft, placement::Timeline},
    document::{Document, Format},
    history::Session,
    notes::{Kind, Note, Position},
    timing_edit::Scope,
};
fn source() -> Session {
    let request: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/attack-resident-inheritance-normalization.request.json"
    ))
    .unwrap();
    Session::new(
        Document::parse(
            request["Cases"][0]["Content"].as_str().unwrap().as_bytes(),
            Format::Ssc,
        )
        .unwrap(),
    )
}
fn rows(session: &Session, chart: usize) -> Vec<u64> {
    Timeline::capture(session, chart)
        .unwrap()
        .markers()
        .iter()
        .map(|m| m.row)
        .collect()
}
fn clear(session: &mut Session, chart: usize, raw: bool) -> Result<bool, deadsync_editor::Error> {
    if raw {
        let mut draft = FileDraft::capture(session, chart, Scope::Chart)?;
        while !draft.attacks().events().is_empty() {
            draft.attacks_mut().remove(0)?;
        }
        draft.apply(session, chart)
    } else {
        let mut draft = Draft::capture(session, chart, Scope::Chart)?;
        let indices = (0..draft.attacks().events().len()).collect::<Vec<_>>();
        draft.remove_events(&indices)?;
        draft.apply(session, chart)
    }
}
#[test]
fn inherited_and_local_target_clear_preserve_peers_pending_notes_save_reopen_and_exact_history() {
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/attack-resident-inheritance-normalization.expected.json"
    ))
    .unwrap();
    for raw in [false, true] {
        for target in [0, 2] {
            let mut session = source();
            let position = Position {
                row: 72,
                lane: 2,
                player: 0,
            };
            session
                .edit_notes(
                    1,
                    "Pending peer note",
                    [(
                        position,
                        Some(Note {
                            kind: Kind::Tap,
                            end: None,
                            attributes: "".into(),
                            tail_attributes: "".into(),
                        }),
                    )],
                )
                .unwrap();
            let notes = session.grid(1).unwrap().clone();
            let before = session.serialize().unwrap();
            let original: Vec<_> = (0..3).map(|chart| rows(&session, chart)).collect();
            let local_peer = session
                .document()
                .raw_property(Some(2), "ATTACKS")
                .map(|v| v.to_vec());
            assert!(clear(&mut session, target, raw).unwrap());
            assert!(rows(&session, target).is_empty());
            assert_eq!(session.grid(1).unwrap(), &notes);
            for chart in 0..3 {
                if chart != target {
                    assert_eq!(rows(&session, chart), original[chart]);
                    assert!(
                        session
                            .document()
                            .raw_property(Some(chart), "BPMS")
                            .is_some()
                    );
                }
            }
            if target == 0 {
                for chart in 0..3 {
                    let reader_rows: Vec<_> = expected[1]["Charts"][chart]["Attacks"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v["Row"].as_u64().unwrap())
                        .collect();
                    assert_eq!(rows(&session, chart), reader_rows);
                }
                assert_eq!(
                    session.document().raw_property(Some(2), "ATTACKS"),
                    local_peer.as_deref()
                );
            }
            assert_eq!(
                session.document().raw_property(None, "ATTACKS"),
                Some(b"".as_slice())
            );
            let saved = session.serialize().unwrap();
            let reopened = Session::new(Document::parse(saved.clone(), Format::Ssc).unwrap());
            for chart in 0..3 {
                assert_eq!(rows(&reopened, chart), rows(&session, chart));
            }
            session.undo().unwrap();
            assert_eq!(session.serialize().unwrap(), before);
            session.redo().unwrap();
            assert_eq!(session.serialize().unwrap(), saved);
        }
    }
}
fn hidden() -> Session {
    let mut session = Session::new(Document::parse(b"#VERSION:0.83;#BPMS:0=120;#WARPS:4=4;#ATTACKS:TIME=1:LEN=2:MODS=100% Dizzy;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0100;".as_slice(), Format::Ssc).unwrap());
    let mut draft = Draft::capture(&session, 1, Scope::Song).unwrap();
    let modifier = draft.attacks().events()[0].modifiers[0].clone();
    draft.set_at_row(0, 0, 240, modifier).unwrap();
    draft.apply(&mut session, 1).unwrap();
    assert!(Timeline::capture(&session, 1).unwrap().has_resident_rows());
    session
}
#[test]
fn peer_resident_warp_rows_survive_scope_detachment_recovery_and_atomic_disk_save() {
    use deadsync_editor::storage::FileSession;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("warp.ssc");
    let original = hidden().serialize().unwrap();
    std::fs::write(&path, &original).unwrap();
    let mut file = FileSession::open(&path).unwrap();
    clear(&mut file.session, 0, false).unwrap();
    assert!(rows(&file.session, 0).is_empty());
    assert_eq!(rows(&file.session, 1), [240]);
    assert!(
        file.session
            .document()
            .text(Some(1), "DSAttackRows")
            .is_some()
    );
    assert!(
        file.session
            .document()
            .raw_property(Some(1), "BPMS")
            .is_some()
    );
    file.save_recovery().unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), original);
    let mut recovered = FileSession::open(&path).unwrap();
    recovered.recover().unwrap();
    assert!(rows(&recovered.session, 0).is_empty());
    assert_eq!(rows(&recovered.session, 1), [240]);
    recovered.save().unwrap();
    let reopened = FileSession::open(&path).unwrap();
    assert!(rows(&reopened.session, 0).is_empty());
    assert_eq!(rows(&reopened.session, 1), [240]);
}
#[test]
fn future_local_metadata_cannot_be_overwritten_when_materializing_an_inherited_peer() {
    let mut session = hidden();
    let mut future: serde_json::Value = serde_json::from_str(
        &session
            .document()
            .text(Some(1), "DSAttackRowsSong")
            .unwrap(),
    )
    .unwrap();
    future["version"] = 2.into();
    session
        .set_text(Some(1), "DSAttackRows", &future.to_string())
        .unwrap();
    session.set_text(None, "TITLE", "Redo").unwrap();
    session.undo().unwrap();
    let before = session.serialize().unwrap();
    let redo = session.redo_label().unwrap().to_owned();
    assert!(clear(&mut session, 0, false).is_err());
    assert_eq!(session.serialize().unwrap(), before);
    assert_eq!(session.redo_label(), Some(redo.as_str()));
}
#[test]
fn absent_song_tempo_and_partial_chart_timing_keep_their_original_effective_clocks() {
    let mut session = Session::new(Document::parse(b"#VERSION:0.83;#OFFSET:0.25;#ATTACKS:TIME=1:LEN=1:MODS=100% Dizzy;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=240;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0100;#NOTEDATA:;#STEPSTYPE:dance-single;#OFFSET:0.5;#BPMS:4=180;#NOTES:0010;".as_slice(), Format::Ssc).unwrap());
    let original: Vec<_> = (0..3).map(|c| rows(&session, c)).collect();
    clear(&mut session, 0, false).unwrap();
    assert!(rows(&session, 0).is_empty());
    for chart in [1, 2] {
        assert_eq!(rows(&session, chart), original[chart]);
    }
    assert!(session.document().raw_property(Some(1), "BPMS").is_some());
}
