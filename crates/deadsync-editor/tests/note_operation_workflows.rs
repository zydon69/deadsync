use deadsync_editor::{
    document::{Document, Format},
    event_clipboard::{
        Selection,
        presets::{Mode, Preset},
    },
    history::Session,
    note_operation::{Operation, Plan, Refusal, Scope},
    note_swap::Pair,
    notes::{Kind, Note, Position},
};
use std::collections::{BTreeMap, BTreeSet};
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
fn operation(value: &str) -> Operation {
    let (from, to) = match value {
        "ConvertTapsToMines" => (Kind::Tap, Kind::Mine),
        "ConvertTapsToFakes" => (Kind::Tap, Kind::Fake),
        "ConvertTapsToLifts" => (Kind::Tap, Kind::Lift),
        "ConvertMinesToTaps" => (Kind::Mine, Kind::Tap),
        "ConvertMinesToFakes" => (Kind::Mine, Kind::Fake),
        "ConvertMinesToLifts" => (Kind::Mine, Kind::Lift),
        "ConvertFakesToTaps" => (Kind::Fake, Kind::Tap),
        "ConvertLiftsToTaps" => (Kind::Lift, Kind::Tap),
        "ConvertHoldsToRolls" => (Kind::Hold, Kind::Roll),
        "ConvertHoldsToTaps" => (Kind::Hold, Kind::Tap),
        "ConvertHoldsToMines" => (Kind::Hold, Kind::Mine),
        "ConvertRollsToHolds" => (Kind::Roll, Kind::Hold),
        "ConvertRollsToTaps" => (Kind::Roll, Kind::Tap),
        "ConvertRollsToMines" => (Kind::Roll, Kind::Mine),
        _ => panic!("method"),
    };
    Operation::Convert {
        from,
        to,
        sustain_rows: 0,
    }
}
fn session(format: Format) -> Session {
    let source = match format {
        Format::Sm => "#TITLE:Ops;#VENDOR:keep;#BPMS:0=120;#NOTES:dance-routine::Hard:9::00000000;",
        Format::Ssc => {
            "#VERSION:0.83;#TITLE:Ops;#VENDOR:keep;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:00000000;"
        }
    };
    Session::new(Document::parse(source.as_bytes(), format).unwrap())
}
#[test]
fn fourteen_original_conversions_in_56_cases_match_112_safe_sm_ssc_roundtrips() {
    let request: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/note-convert.request.json"
    ))
    .unwrap();
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/note-convert.expected.json"
    ))
    .unwrap();
    let mut count = 0;
    for (case, oracle) in request["Cases"]
        .as_array()
        .unwrap()
        .iter()
        .zip(expected["Cases"].as_array().unwrap())
    {
        for format in [Format::Sm, Format::Ssc] {
            let mut s = session(format);
            let mut edits = BTreeMap::new();
            let mut heads = BTreeSet::new();
            for e in case["Events"].as_array().unwrap() {
                if e["Kind"] == "Tempo" {
                    continue;
                }
                let p = Position {
                    row: e["Row"].as_u64().unwrap(),
                    lane: e["Lane"].as_u64().unwrap() as u16,
                    player: e["Player"].as_u64().unwrap() as u32,
                };
                edits.insert(
                    p,
                    Some(Note {
                        kind: kind(e["Kind"].as_str().unwrap()),
                        end: e["End"].as_u64(),
                        attributes: "".into(),
                        tail_attributes: "".into(),
                    }),
                );
                if e["Selected"] != false {
                    heads.insert(p);
                }
            }
            s.edit_notes(0, "fixture", edits).unwrap();
            let before = s.serialize().unwrap();
            let plan = Plan::prepare(
                &mut s,
                0,
                Scope::Heads(heads),
                operation(case["Method"].as_str().unwrap()),
            )
            .unwrap();
            assert_eq!(s.serialize().unwrap(), before);
            let changed = plan.apply(&mut s, 0).unwrap();
            for e in oracle["After"].as_array().unwrap() {
                if e["Kind"] == "Tempo" {
                    continue;
                }
                let p = Position {
                    row: e["Row"].as_u64().unwrap(),
                    lane: e["Lane"].as_u64().unwrap() as u16,
                    player: e["Player"].as_u64().unwrap() as u32,
                };
                let n = s.grid(0).unwrap().get(p).unwrap();
                assert_eq!(
                    n.kind,
                    kind(e["Kind"].as_str().unwrap()),
                    "{}",
                    case["Name"]
                );
                assert_eq!(n.end, e["End"].as_u64());
            }
            let after = s.serialize().unwrap();
            let mut reopened = Session::new(Document::parse(after.clone(), format).unwrap());
            assert_eq!(reopened.grid(0).unwrap(), s.grid(0).unwrap());
            if changed {
                s.undo().unwrap();
                assert_eq!(s.serialize().unwrap(), before);
                s.redo().unwrap();
                assert_eq!(s.serialize().unwrap(), after);
            } else {
                assert_eq!(after, before);
            }
            count += 1;
        }
    }
    assert_eq!(count, 112);
}
#[test]
fn all_and_exact_checked_heads_do_not_confuse_player_or_same_row_notes() {
    let mut s = session(Format::Ssc);
    let p = |lane, player| Position {
        row: 48,
        lane,
        player,
    };
    s.edit_notes(
        0,
        "fixture",
        BTreeMap::from([
            (p(0, 0), Some(Note::tap())),
            (p(1, 0), Some(Note::tap())),
            (p(0, 1), Some(Note::tap())),
        ]),
    )
    .unwrap();
    let mut selection = Selection::capture(&mut s, 0, 0, 96).unwrap();
    selection
        .apply_preset(Preset::Taps, Some(1), Mode::Replace)
        .unwrap();
    let op = Operation::Convert {
        from: Kind::Tap,
        to: Kind::Mine,
        sustain_rows: 0,
    };
    let plan = selection.prepare_note_operation(&mut s, op).unwrap();
    assert_eq!(plan.report().changed, 1);
    plan.apply(&mut s, 0).unwrap();
    assert_eq!(s.grid(0).unwrap().get(p(0, 1)).unwrap().kind, Kind::Mine);
    assert_eq!(s.grid(0).unwrap().get(p(0, 0)).unwrap().kind, Kind::Tap);
    s.undo().unwrap();
    let plan = Plan::prepare(&mut s, 0, Scope::All { player: Some(0) }, op).unwrap();
    assert_eq!(plan.report().changed, 2);
    plan.apply(&mut s, 0).unwrap();
    assert_eq!(s.grid(0).unwrap().get(p(0, 1)).unwrap().kind, Kind::Tap);
}
#[test]
fn reviewed_swaps_refuse_zero_sustains_atomically_and_stale_history_preserves_redo() {
    let mut s = session(Format::Ssc);
    let p = Position {
        row: 0,
        lane: 0,
        player: 0,
    };
    s.edit_notes(0, "fixture", [(p, Some(Note::tap()))])
        .unwrap();
    let before = s.serialize().unwrap();
    assert_eq!(
        Plan::prepare(
            &mut s,
            0,
            Scope::All { player: None },
            Operation::Swap(Pair::HoldsTaps)
        )
        .err()
        .unwrap()
        .kind(),
        Refusal::ZeroDurationSustain
    );
    assert_eq!(s.serialize().unwrap(), before);
    let plan = Plan::prepare(
        &mut s,
        0,
        Scope::All { player: None },
        Operation::Swap(Pair::TapsMines),
    )
    .unwrap();
    s.set_text(None, "TITLE", "intervening").unwrap();
    s.undo().unwrap();
    let redo = s.redo_label().unwrap().to_string();
    assert_eq!(plan.apply(&mut s, 0).err().unwrap().kind(), Refusal::Stale);
    assert_eq!(s.serialize().unwrap(), before);
    assert_eq!(s.redo_label(), Some(redo.as_str()));
    let mut foreign = session(Format::Ssc);
    assert_eq!(
        plan.apply(&mut foreign, 0).err().unwrap().kind(),
        Refusal::Stale
    );
}
#[test]
fn preparation_limits_and_cancellation_preserve_content_and_history() {
    use std::sync::atomic::AtomicBool;
    let mut s = session(Format::Ssc);
    s.edit_notes(
        0,
        "fixture",
        [(
            Position {
                row: 0,
                lane: 0,
                player: 0,
            },
            Some(Note::tap()),
        )],
    )
    .unwrap();
    let before = s.serialize().unwrap();
    let revision = s.revision();
    let op = Operation::Swap(Pair::TapsMines);
    assert!(
        Plan::prepare_bounded(
            &mut s,
            0,
            Scope::All { player: None },
            op,
            0,
            &AtomicBool::new(false)
        )
        .is_err()
    );
    assert!(
        Plan::prepare_bounded(
            &mut s,
            0,
            Scope::All { player: None },
            op,
            10,
            &AtomicBool::new(true)
        )
        .is_err()
    );
    assert_eq!(s.serialize().unwrap(), before);
    assert_eq!(s.revision(), revision);
}
