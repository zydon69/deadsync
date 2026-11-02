use deadsync_editor::{
    document::{Document, Format},
    generation::metadata::{ChartData, Definition},
    history::Session,
    notes::{Note, Position},
    workspace::{Command, Workspace},
};
use std::{collections::BTreeMap, fs};
const SSC: &str = "#VERSION:0.83;#TITLE:Keep;#BPMS:0=120;#VENDOR:root;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#BPMS:0=150;#OFFSET:0.125;#LABELS:0=Intro;#XTRA:chart\\;value;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Easy;#METER:2;#NOTES:0100;";
#[test]
fn duplicate_includes_pending_notes_tags_timing_patterns_and_one_structural_history_step() {
    let mut session = Session::new(Document::parse(SSC.as_bytes(), Format::Ssc).unwrap());
    session
        .edit_notes(
            0,
            "Add note",
            [(
                Position {
                    player: 0,
                    lane: 2,
                    row: 48,
                },
                Some(Note::tap()),
            )],
        )
        .unwrap();
    session
        .set_text(Some(0), "CREDIT", "Auteur é 日本語")
        .unwrap();
    let data = ChartData {
        patterns: Some(BTreeMap::from([(
            48,
            Definition {
                length: 96,
                ..Default::default()
            },
        )])),
        ..Default::default()
    };
    data.save(&mut session, 0).unwrap();
    let before = session.serialize().unwrap();
    let revision = session.revision();
    let clone = session.duplicate_chart(0, revision).unwrap();
    assert_eq!(clone, 2);
    assert_eq!(session.document().charts().len(), 3);
    assert!(session.serialize().unwrap().starts_with(&before));
    let expected = session.grid(0).unwrap().clone();
    assert_eq!(session.grid(2).unwrap(), &expected);
    for tag in ["CREDIT", "BPMS", "OFFSET", "LABELS", "XTRA", "GAChartData"] {
        assert_eq!(
            session.document().text(Some(0), tag),
            session.document().text(Some(2), tag),
            "{tag}"
        );
    }
    assert_eq!(session.undo_label(), Some("Duplicate chart"));
    let after = session.serialize().unwrap();
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), before);
    session.redo().unwrap();
    assert_eq!(session.serialize().unwrap(), after);
    assert_eq!(session.grid(2).unwrap(), &expected);
}
#[test]
fn sm_duplicates_notes2_and_indexed_raw_metadata_without_rewriting_original_bytes() {
    let raw=b"#TITLE:CP\xe9;#BPMS:0=120;#GAChartDataVersionCHART0000:1;#GAChartDataCHART0000:{\"Patterns\":{\"48\":{\"Length\":96}},\"Future\":\"keep\"};#NOTES2:dance-single:Desc:Hard:9:0,0,0,0,0:1000;#AFTER:global;";
    let mut session = Session::new(Document::parse(raw.as_slice(), Format::Sm).unwrap());
    let index = session.duplicate_chart(0, 0).unwrap();
    assert_eq!(index, 1);
    assert_eq!(
        session
            .document()
            .raw_property(None, "GAChartDataCHART0000"),
        session
            .document()
            .raw_property(None, "GAChartDataCHART0001")
    );
    assert_eq!(
        ChartData::read(session.document(), 1)
            .unwrap()
            .unwrap()
            .patterns
            .unwrap()[&48]
            .length,
        96
    );
    assert_eq!(
        session.grid(0).unwrap().clone(),
        session.grid(1).unwrap().clone()
    );
    let after = session.serialize().unwrap();
    let header = b"#NOTES2:dance-single:Desc:Hard:9:0,0,0,0,0:1000;";
    assert_eq!(
        after
            .windows(header.len())
            .filter(|bytes| *bytes == header)
            .count(),
        2
    );
    assert!(after.starts_with(b"#TITLE:CP\xe9;#BPMS:0=120;"));
    assert_eq!(
        session.document().text(None, "AFTER").as_deref(),
        Some("global")
    );
    let reopened = Session::new(Document::parse(after.clone(), Format::Sm).unwrap());
    assert_eq!(
        ChartData::read(reopened.document(), 1)
            .unwrap()
            .unwrap()
            .patterns
            .unwrap()[&48]
            .length,
        96
    );
    assert_eq!(
        reopened
            .document()
            .raw_property(None, "GAChartDataCHART0000"),
        reopened
            .document()
            .raw_property(None, "GAChartDataCHART0001")
    );
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), raw);
    session.redo().unwrap();
    assert_eq!(session.serialize().unwrap(), after);
}
#[test]
fn invalid_stale_metadata_collision_and_structural_guards_preserve_redo_and_source() {
    let mut session = Session::new(Document::parse(SSC.as_bytes(), Format::Ssc).unwrap());
    session
        .edit_notes(
            0,
            "Add",
            [(
                Position {
                    player: 0,
                    lane: 2,
                    row: 48,
                },
                Some(Note::tap()),
            )],
        )
        .unwrap();
    session.undo().unwrap();
    let before = session.serialize().unwrap();
    assert!(session.duplicate_chart(9, session.revision()).is_err());
    assert!(session.duplicate_chart(0, 99).is_err());
    assert_eq!(session.serialize().unwrap(), before);
    assert!(session.redo().unwrap());
    let before = session.serialize().unwrap();
    let revision = session.revision();
    assert!(
        session
            .set_text(Some(0), "STEPSTYPE", "dance-double")
            .is_err()
    );
    assert_eq!(session.serialize().unwrap(), before);
    assert_eq!(session.revision(), revision);
    let raw =
        b"#BPMS:0=120;#GAChartDataCHART0001:{\"Future\":1};#NOTES:dance-single::Hard:9::1000;";
    let mut sm = Session::new(Document::parse(raw.as_slice(), Format::Sm).unwrap());
    assert!(sm.duplicate_chart(0, 0).is_err());
    assert_eq!(sm.serialize().unwrap(), raw);
    assert!(!sm.is_dirty());
}
#[test]
fn workspace_clone_edit_undo_redo_save_reopen_restores_active_chart_and_participants() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let song = "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:10000000\n&\n00001000;";
    fs::write(&path, song).unwrap();
    let mut workspace = Workspace::open(&path, 0).unwrap();
    workspace.execute(Command::SelectPlayer(1)).unwrap();
    workspace.execute(Command::Seek(48)).unwrap();
    workspace.execute(Command::Place(5)).unwrap();
    let before = workspace.file().session.serialize().unwrap();
    workspace.execute(Command::DuplicateChart).unwrap();
    assert_eq!(workspace.chart(), 1);
    // FinishAddingChart inherits the active player, clamped to this layout.
    assert_eq!(workspace.player(), 1);
    assert_eq!(workspace.grid().players(), 2);
    assert_eq!(workspace.row(), 48);
    workspace.execute(Command::Place(2)).unwrap();
    let added = Position { player: 1, lane: 2, row: 48 };
    assert_eq!(workspace.grid().get(added), Some(&Note::tap()));
    assert!(workspace.grid().get(Position { player: 0, ..added }).is_none());
    let after = workspace.file().session.serialize().unwrap();
    workspace.execute(Command::Undo).unwrap();
    workspace.execute(Command::Undo).unwrap();
    assert_eq!(workspace.chart(), 0);
    assert_eq!(workspace.player(), 1);
    assert_eq!(workspace.file().session.serialize().unwrap(), before);
    workspace.execute(Command::Redo).unwrap();
    assert_eq!(workspace.chart(), 1);
    assert_eq!(workspace.player(), 1);
    assert!(workspace.grid().get(added).is_none());
    workspace.execute(Command::Redo).unwrap();
    assert_eq!(workspace.grid().get(added), Some(&Note::tap()));
    assert_eq!(workspace.file().session.serialize().unwrap(), after);
    workspace.execute(Command::Save).unwrap();
    let reopened = Workspace::open(&path, 1).unwrap();
    assert_eq!(reopened.grid(), workspace.grid());
    assert!(!reopened.file().session.is_dirty());
}
