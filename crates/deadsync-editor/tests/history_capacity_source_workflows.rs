use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    notes::{Note, Position},
    storage::FileSession,
};
const SONG: &str = "#VERSION:0.83;#TITLE:0;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\r\n0000\r\n0000\r\n0000;";
fn session() -> Session {
    Session::new(Document::parse(SONG.as_bytes(), Format::Ssc).unwrap())
}
#[test]
fn resize_past_and_future_retention_matches_actual_fumen_stack_25_cases() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/history-capacity.expected.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let mut s = session();
        s.set_history_capacity(4).unwrap();
        for i in 1..=6 {
            s.set_text(None, "TITLE", &i.to_string()).unwrap();
        }
        for _ in 0..case["Undo"].as_u64().unwrap() {
            assert!(s.undo().unwrap());
        }
        s.set_history_capacity(case["Size"].as_u64().unwrap() as usize)
            .unwrap();
        let mut past = Vec::new();
        while s.undo_label().is_some() {
            let value = s
                .document()
                .text(None, "TITLE")
                .unwrap()
                .parse::<u64>()
                .unwrap();
            assert!(s.undo().unwrap());
            past.push(value);
        }
        let mut future = Vec::new();
        while s.redo().unwrap() {
            future.push(
                s.document()
                    .text(None, "TITLE")
                    .unwrap()
                    .parse::<u64>()
                    .unwrap(),
            );
        }
        assert_eq!(serde_json::json!(past), case["Past"]);
        assert_eq!(serde_json::json!(future), case["Future"]);
    }
}
#[test]
fn trimming_note_history_cannot_discard_live_notes_when_serializing_or_saving() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("song.ssc");
    std::fs::write(&path, SONG).unwrap();
    let mut file = FileSession::open(&path).unwrap();
    file.session.set_history_capacity(1).unwrap();
    let note = Position {
        player: 0,
        lane: 1,
        row: 48,
    };
    file.session
        .edit_notes(0, "Note", [(note, Some(Note::tap()))])
        .unwrap();
    file.session.set_text(None, "TITLE", "Latest").unwrap();
    assert!(file.session.is_dirty());
    assert!(file.session.undo().unwrap());
    assert!(!file.session.undo().unwrap());
    assert!(file.session.grid(0).unwrap().get(note).is_some());
    file.save().unwrap();
    assert!(!file.session.is_dirty());
    let mut reloaded = FileSession::open(&path).unwrap();
    assert!(reloaded.session.grid(0).unwrap().get(note).is_some());
    assert_eq!(
        reloaded.session.document().text(None, "TITLE").as_deref(),
        Some("0")
    );
}
#[test]
fn untouched_loaded_notes_stay_exact_and_structural_undo_restores_touched_grid_identity() {
    let mut s = session();
    s.grid(0).unwrap();
    assert_eq!(s.serialize().unwrap(), SONG.as_bytes());
    let note = Position {
        player: 0,
        lane: 1,
        row: 48,
    };
    s.edit_notes(0, "Note", [(note, Some(Note::tap()))])
        .unwrap();
    let before = s.serialize().unwrap();
    let draft = deadsync_editor::chart_deletion::Draft::capture(&s, 0).unwrap();
    draft.apply(&mut s).unwrap();
    assert_eq!(s.document().charts().len(), 0);
    assert!(s.undo().unwrap());
    assert_eq!(s.serialize().unwrap(), before);
    assert!(s.grid(0).unwrap().get(note).is_some());
}
