use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    notes::{Kind, Position, layout_columns},
    pad::{LAYOUTS, Pad},
    storage::FileSession,
    transform::{self, Transform},
    workspace::{Command, Workspace},
};
use std::{fs, sync::Arc};

const SSC: &[u8] =
    b"#TITLE:Steps;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:\n2000\n0000\n3000\n0000\n;";

#[test]
fn every_reference_layout_has_valid_relationships_and_round_trip_symmetries() {
    for layout in LAYOUTS {
        let pad = Pad::builtin(layout).unwrap();
        assert_eq!(
            pad.arrows.len(),
            usize::from(layout_columns(layout).unwrap()),
            "{layout}"
        );
        for (lane, mirrored) in pad.mirror.iter().enumerate() {
            if let Some(target) = mirrored {
                assert_eq!(
                    pad.mirror[usize::from(*target)],
                    Some(lane as u16),
                    "{layout}"
                );
            }
        }
        for (lane, flipped) in pad.flip.iter().enumerate() {
            if let Some(target) = flipped {
                assert_eq!(
                    pad.flip[usize::from(*target)],
                    Some(lane as u16),
                    "{layout}"
                );
            }
        }
    }
    // GrooveAuthor mirrors left/right and flips front/back. A lane reversal
    // would incorrectly exchange down/up on a dance-single pad.
    assert_eq!(
        Pad::builtin("dance-single").unwrap().mirror,
        [Some(3), Some(1), Some(2), Some(0)]
    );
    assert_eq!(
        Pad::builtin("dance-single").unwrap().flip,
        [Some(0), Some(2), Some(1), Some(3)]
    );
}

#[test]
fn transform_workflow_saves_and_reopens_sustains_and_has_one_undo_per_gesture() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("chart.ssc");
    fs::write(&path, SSC).unwrap();
    let mut editor = Workspace::open(&path, 0).unwrap();
    editor.execute(Command::ToggleSelection).unwrap();
    editor.execute(Command::Seek(96)).unwrap();
    editor
        .execute(Command::Transform(Transform::Mirror))
        .unwrap();
    assert_eq!(
        editor
            .grid()
            .get(Position {
                player: 0,
                lane: 3,
                row: 0
            })
            .unwrap()
            .end,
        Some(96)
    );
    editor.execute(Command::Save).unwrap();
    let reopened = Workspace::open(&path, 0).unwrap();
    assert_eq!(reopened.grid(), editor.grid());
    editor.execute(Command::Undo).unwrap();
    // Undo restores the exact authoring document. Saving separately materializes
    // source-compatible opening context, as EditorSong.SerializeCustomSongData does.
    assert_eq!(editor.file().session.serialize().unwrap(), SSC);
    editor.execute(Command::Save).unwrap();
    assert_eq!(editor.file().session.serialize().unwrap(), SSC);
    let output = fs::read(&path).unwrap();
    let saved = Document::parse(output, Format::Ssc).unwrap();
    assert_eq!(
        saved.value(saved.charts()[0].notes.clone()),
        b"\n2000\n0000\n3000\n0000\n"
    );
    let metadata: serde_json::Value =
        serde_json::from_str(&saved.text(None, "GASongData").unwrap()).unwrap();
    assert_eq!(
        metadata,
        serde_json::json!({
            "SyncOffset": 0.009, "DefaultChartType": "dance_single", "DefaultChartIndex": 0
        })
    );
    assert_eq!(saved.text(None, "GASongDataVersion").as_deref(), Some("1"));
    let reopened = Workspace::open(&path, 0).unwrap();
    assert_eq!(reopened.grid(), editor.grid());
}

#[test]
fn hold_creation_copy_cut_paste_conversion_and_redo_share_the_ui_command_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("chart.ssc");
    fs::write(&path, SSC).unwrap();
    let mut editor = Workspace::open(&path, 0).unwrap();
    editor.execute(Command::Seek(192)).unwrap();
    editor.execute(Command::ToggleSelection).unwrap();
    editor.execute(Command::Seek(288)).unwrap();
    editor.execute(Command::SelectTool(Kind::Roll)).unwrap();
    editor.execute(Command::Place(2)).unwrap();
    assert_eq!(
        editor
            .grid()
            .get(Position {
                player: 0,
                lane: 2,
                row: 192
            })
            .unwrap()
            .end,
        Some(288)
    );
    editor.execute(Command::Cut).unwrap();
    assert!(
        editor
            .grid()
            .get(Position {
                player: 0,
                lane: 2,
                row: 192
            })
            .is_none()
    );
    editor.execute(Command::ToggleSelection).unwrap();
    editor.execute(Command::Seek(384)).unwrap();
    editor.execute(Command::Paste).unwrap();
    assert_eq!(
        editor
            .grid()
            .get(Position {
                player: 0,
                lane: 2,
                row: 384
            })
            .unwrap()
            .end,
        Some(480)
    );
    editor
        .execute(Command::Transform(Transform::Convert {
            from: Kind::Roll,
            to: Kind::Hold,
            sustain_rows: 48,
        }))
        .unwrap();
    assert_eq!(
        editor
            .grid()
            .get(Position {
                player: 0,
                lane: 2,
                row: 384
            })
            .unwrap()
            .kind,
        Kind::Hold
    );
    editor.execute(Command::Undo).unwrap();
    assert_eq!(
        editor
            .grid()
            .get(Position {
                player: 0,
                lane: 2,
                row: 384
            })
            .unwrap()
            .kind,
        Kind::Roll
    );
    editor.execute(Command::Redo).unwrap();
    editor.execute(Command::Save).unwrap();
    assert_eq!(Workspace::open(&path, 0).unwrap().grid(), editor.grid());
}

#[test]
fn row_shift_replaces_collisions_and_drops_negative_heads_with_exact_undo() {
    let source = b"#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:\n1000\n1000\n0000\n0000\n;";
    let mut session =
        Session::new(Document::parse(Arc::<[u8]>::from(source.as_slice()), Format::Ssc).unwrap());
    assert!(transform::apply(&mut session, 0, 0, 0, None, Transform::ShiftRows(48)).unwrap());
    assert!(
        session
            .grid(0)
            .unwrap()
            .get(Position {
                row: 0,
                lane: 0,
                player: 0
            })
            .is_none()
    );
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), source);
    assert!(transform::apply(&mut session, 0, 0, 0, None, Transform::ShiftRows(-1)).unwrap());
    session.undo().unwrap();
    assert!(!session.is_dirty());
    assert_eq!(session.serialize().unwrap(), source);
}

#[test]
fn two_editors_cannot_overwrite_each_others_successful_save() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("chart.ssc");
    fs::write(&path, SSC).unwrap();
    let mut first = FileSession::open(&path).unwrap();
    let mut second = FileSession::open(&path).unwrap();
    first.session.set_text(None, "TITLE", "First").unwrap();
    second.session.set_text(None, "TITLE", "Second").unwrap();
    first.save().unwrap();
    assert!(second.save().is_err());
    assert!(second.session.is_dirty());
    assert_eq!(
        FileSession::open(&path)
            .unwrap()
            .session
            .document()
            .text(None, "TITLE")
            .as_deref(),
        Some("First")
    );
}
