use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    note_swap::{self, Pair},
    notes::{Kind, Note, Position},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
fn position(row: u64, lane: u16, player: u32) -> Position {
    Position { row, lane, player }
}
fn session(format: Format) -> Session {
    let source = match format {
        Format::Sm => {
            "#TITLE:Swap;#VENDOR:keep;#BPMS:0=120;#NOTES:dance-routine::Hard:9::00000000;"
        }
        Format::Ssc => {
            "#VERSION:0.83;#TITLE:Swap;#VENDOR:keep;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:00000000;"
        }
    };
    Session::new(Document::parse(Arc::<[u8]>::from(source.as_bytes()), format).unwrap())
}
fn kind(value: &str) -> Kind {
    match value {
        "Tap" => Kind::Tap,
        "Mine" => Kind::Mine,
        "Fake" => Kind::Fake,
        "Lift" => Kind::Lift,
        "Hold" => Kind::Hold,
        "Roll" => Kind::Roll,
        _ => panic!("kind"),
    }
}
fn pair(value: &str) -> Pair {
    match value {
        "SwapTapsAndMines" => Pair::TapsMines,
        "SwapTapsAndFakes" => Pair::TapsFakes,
        "SwapTapsAndLifts" => Pair::TapsLifts,
        "SwapMinesAndFakes" => Pair::MinesFakes,
        "SwapMinesAndLifts" => Pair::MinesLifts,
        "SwapHoldsAndRolls" => Pair::HoldsRolls,
        "SwapHoldsAndTaps" => Pair::HoldsTaps,
        "SwapHoldsAndMines" => Pair::HoldsMines,
        "SwapRollsAndTaps" => Pair::RollsTaps,
        "SwapRollsAndMines" => Pair::RollsMines,
        _ => panic!("pair"),
    }
}
#[test]
fn original_forty_actions_match_or_refuse_the_twenty_proven_chart_dropping_readbacks() {
    let request: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/grooveauthor/note-swap.request.json")).unwrap();
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/note-swap.expected.json"
    ))
    .unwrap();
    let mut count = 0;
    let mut refused = 0;
    for (case, oracle) in request["Cases"]
        .as_array()
        .unwrap()
        .iter()
        .zip(expected["Cases"].as_array().unwrap())
    {
        assert_eq!(case["Name"], oracle["Name"]);
        for format in [Format::Sm, Format::Ssc] {
            let mut s = session(format);
            let mut notes = BTreeMap::new();
            let mut selected = BTreeSet::new();
            for event in case["Events"].as_array().unwrap() {
                if event["Kind"] == "Tempo" {
                    continue;
                }
                let p = position(
                    event["Row"].as_u64().unwrap(),
                    event["Lane"].as_u64().unwrap() as u16,
                    event["Player"].as_u64().unwrap() as u32,
                );
                notes.insert(
                    p,
                    Some(Note {
                        kind: kind(event["Kind"].as_str().unwrap()),
                        end: event["End"].as_u64(),
                        attributes: "".into(),
                        tail_attributes: "".into(),
                    }),
                );
                if event["Selected"].as_bool().unwrap_or(true) {
                    selected.insert(p);
                }
            }
            s.edit_notes(0, "fixture", notes).unwrap();
            let before = s.serialize().unwrap();
            let revision = s.revision();
            let unsafe_result = oracle["After"].as_array().unwrap().iter().any(|e| {
                matches!(e["Kind"].as_str(), Some("Hold" | "Roll"))
                    && e["End"].as_u64() == e["Row"].as_u64()
            });
            let result =
                note_swap::apply(&mut s, 0, &selected, pair(case["Method"].as_str().unwrap()));
            if unsafe_result {
                assert!(
                    result
                        .unwrap_err()
                        .to_string()
                        .contains("explicit sustain duration")
                );
                assert_eq!(s.serialize().unwrap(), before);
                assert_eq!(s.revision(), revision);
                refused += 1;
                continue;
            }
            let changed = result.unwrap();
            let expected_notes: BTreeMap<_, _> = oracle["After"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|event| event["Kind"] != "Tempo")
                .map(|event| {
                    (
                        position(
                            event["Row"].as_u64().unwrap(),
                            event["Lane"].as_u64().unwrap() as u16,
                            event["Player"].as_u64().unwrap() as u32,
                        ),
                        (kind(event["Kind"].as_str().unwrap()), event["End"].as_u64()),
                    )
                })
                .collect();
            let actual_notes: BTreeMap<_, _> = s
                .grid(0)
                .unwrap()
                .iter()
                .map(|(position, note)| (position, (note.kind, note.end)))
                .collect();
            assert_eq!(actual_notes, expected_notes, "{}", case["Name"]);
            let after = s.serialize().unwrap();
            let mut reopened =
                Session::new(Document::parse(Arc::<[u8]>::from(after.clone()), format).unwrap());
            assert_eq!(reopened.grid(0).unwrap(), s.grid(0).unwrap());
            if changed {
                s.undo().unwrap();
                assert_eq!(s.serialize().unwrap(), before);
                s.redo().unwrap();
                assert_eq!(s.serialize().unwrap(), after);
            } else {
                assert_eq!(before, after);
                assert_eq!(revision, s.revision());
            }
            count += 1;
        }
    }
    assert_eq!((count, refused), (60, 20));
}
#[test]
fn mixed_simple_and_sustain_refusal_preserves_redo_and_never_publishes_half_swap() {
    for format in [Format::Sm, Format::Ssc] {
        let mut s = session(format);
        let tap = position(0, 0, 0);
        let hold = position(48, 1, 1);
        s.edit_notes(
            0,
            "fixture",
            BTreeMap::from([
                (tap, Some(Note::tap())),
                (
                    hold,
                    Some(Note {
                        kind: Kind::Hold,
                        end: Some(96),
                        attributes: "".into(),
                        tail_attributes: "".into(),
                    }),
                ),
            ]),
        )
        .unwrap();
        s.edit_notes(
            0,
            "redo",
            BTreeMap::from([(position(144, 2, 0), Some(Note::tap()))]),
        )
        .unwrap();
        let redo = s.serialize().unwrap();
        s.undo().unwrap();
        let before = s.serialize().unwrap();
        let revision = s.revision();
        assert!(
            note_swap::apply(&mut s, 0, &BTreeSet::from([tap, hold]), Pair::HoldsTaps).is_err()
        );
        assert_eq!(s.serialize().unwrap(), before);
        assert_eq!(s.revision(), revision);
        s.redo().unwrap();
        assert_eq!(s.serialize().unwrap(), redo);
    }
}
