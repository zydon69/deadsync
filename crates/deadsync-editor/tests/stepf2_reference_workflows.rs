use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    notes::{Grid, Kind},
    stepf2,
};
use serde_json::{Value, json};
use std::path::PathBuf;

fn signature(grid: &Grid) -> Vec<Value> {
    let mut notes: Vec<_> = grid
        .iter()
        .map(|(position, note)| {
            let kind = match note.kind {
                Kind::Tap => "tap",
                Kind::Hold => "hold",
                Kind::Roll => "roll",
                Kind::Mine => "mine",
                Kind::Lift => "lift",
                Kind::Fake => "fake",
                Kind::Keysound => "keysound",
                Kind::Attack => "attack",
            };
            (
                position,
                json!([position.row, position.lane, position.player, kind, note.end]),
            )
        })
        .collect();
    notes.sort_by_key(|(position, _)| (position.row, position.lane, position.player));
    notes.into_iter().map(|(_, value)| value).collect()
}

#[test]
fn co_op_ownership_note_kinds_and_hold_ends_match_actual_pinned_fumen_readers() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    let request: Value =
        serde_json::from_slice(&std::fs::read(root.join("stepf2-notes.request.json")).unwrap())
            .unwrap();
    let expected: Value =
        serde_json::from_slice(&std::fs::read(root.join("stepf2-notes.expected.json")).unwrap())
            .unwrap();
    let cases = request["Cases"].as_array().unwrap();
    assert_eq!(cases.len(), 24);
    assert_eq!(expected.as_array().unwrap().len(), cases.len());
    for (input, expected) in cases.iter().zip(expected.as_array().unwrap()) {
        assert_eq!(input["Name"], expected["Name"]);
        let format = if input["Format"] == "sm" {
            Format::Sm
        } else {
            Format::Ssc
        };
        let source = input["Content"].as_str().unwrap();
        let mut session = Session::new(Document::parse(source.as_bytes(), format).unwrap());
        let descriptor = session.document().charts()[0].clone();
        let reference = &expected["Read"][0];
        let coop = stepf2::is_coop(
            session.document().value(descriptor.notes),
            &descriptor.steps_type,
        );
        let reference_notes: Vec<_> = reference["Notes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|note| {
                json!([
                    note["Row"],
                    note["Lane"],
                    note["Player"],
                    note["Kind"],
                    note["End"]
                ])
            })
            .collect();
        let grid = session.grid(0).unwrap();
        assert_eq!(signature(grid), reference_notes, "{}", input["Name"]);
        if coop || descriptor.steps_type == "pump-double" {
            assert_eq!(
                u64::from(grid.players()),
                reference["Players"].as_u64().unwrap(),
                "{}",
                input["Name"]
            );
        }
        if coop {
            assert_eq!(reference["Layout"], "pump-routine");
            assert_eq!(
                signature(&stepf2::parse(&grid.serialize().unwrap()).unwrap()),
                reference_notes
            );
        }
        assert!(expected["Saved"].as_bool().unwrap());
        let writer_notes: Vec<_> = expected["Reloaded"][0]["Notes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|note| {
                json!([
                    note["Row"],
                    note["Lane"],
                    note["Player"],
                    note["Kind"],
                    note["End"]
                ])
            })
            .collect();
        assert_eq!(
            writer_notes, reference_notes,
            "Fumen writer note projection {}",
            input["Name"]
        );
        // Loading for editing alone preserves the actual source, including its
        // layout spelling, whitespace, and flags the C# writer would discard.
        assert_eq!(session.serialize().unwrap(), source.as_bytes());
    }
}

#[test]
fn oracle_exposes_source_flag_loss_in_fumen_writer_without_claiming_ui_parity() {
    let expected: Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/stepf2-notes.expected.json"
    ))
    .unwrap();
    let compound: Vec<_> = expected
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["Name"].as_str().unwrap().contains("compound-fakes"))
        .collect();
    assert_eq!(compound.len(), 4);
    for case in compound {
        let hold = &case["Read"][0]["Notes"][1];
        let saved_hold = &case["Reloaded"][0]["Notes"][1];
        assert_eq!(hold["Kind"], "hold");
        assert_eq!(hold["FakeFlag"], true);
        assert_eq!(hold["Vanish"], true);
        assert_eq!(saved_hold["Kind"], "hold");
        assert_eq!(saved_hold["FakeFlag"], false);
        assert_eq!(saved_hold["Vanish"], false);
    }
}
