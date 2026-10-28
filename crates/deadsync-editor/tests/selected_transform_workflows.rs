use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    note_operation::Scope,
    notes::{Kind, Note, Position},
    selected_transform::{Impact, ImpactKind, Plan, Refusal},
    transform::{self, Transform},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::AtomicBool,
};
fn p(row: u64, lane: u16, player: u32) -> Position {
    Position { row, lane, player }
}
fn note(kind: Kind, end: Option<u64>) -> Note {
    Note {
        kind,
        end,
        attributes: "".into(),
        tail_attributes: "".into(),
    }
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
fn operation(case: &serde_json::Value) -> Transform {
    match case["Operation"].as_str().unwrap() {
        "Mirror" => Transform::Mirror,
        "Flip" => Transform::Flip,
        "MirrorAndFlip" => Transform::MirrorAndFlip,
        "Left" => Transform::ShiftLanes {
            right: false,
            wrap: false,
        },
        "Right" => Transform::ShiftLanes {
            right: true,
            wrap: false,
        },
        "LeftWrap" => Transform::ShiftLanes {
            right: false,
            wrap: true,
        },
        "RightWrap" => Transform::ShiftLanes {
            right: true,
            wrap: true,
        },
        "Rows" => Transform::ShiftRows(case["Rows"].as_i64().unwrap()),
        "Assign" => Transform::AssignPlayer(case["PlayerA"].as_u64().unwrap() as u32),
        "SwapPlayers" => Transform::SwapPlayers(
            case["PlayerA"].as_u64().unwrap() as u32,
            case["PlayerB"].as_u64().unwrap() as u32,
        ),
        _ => panic!("operation"),
    }
}
fn source(layout: &str, format: Format) -> Session {
    let n = "0".repeat(deadsync_editor::notes::layout_columns(layout).unwrap() as usize);
    let text = match format {
        Format::Sm => {
            format!("#TITLE:Transform;#VENDOR:keep;#BPMS:0=120;#NOTES:{layout}::Hard:9::{n};")
        }
        Format::Ssc => format!(
            "#VERSION:0.83;#TITLE:Transform;#VENDOR:keep;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:{layout};#NOTES:{n};"
        ),
    };
    Session::new(Document::parse(text.into_bytes(), format).unwrap())
}
#[test]
fn original_22_actions_match_or_refuse_three_proven_lossy_owner_cases_in_both_formats() {
    let req: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/selected-transform.request.json"
    ))
    .unwrap();
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/selected-transform.expected.json"
    ))
    .unwrap();
    let (mut matched, mut refused) = (0, 0);
    for (case, oracle) in req["Cases"]
        .as_array()
        .unwrap()
        .iter()
        .zip(expected["Cases"].as_array().unwrap())
    {
        for format in [Format::Sm, Format::Ssc] {
            let layout = case["Layout"].as_str().unwrap();
            let mut s = source(layout, format);
            let players = case["Players"].as_u64().unwrap() as u32;
            if deadsync_editor::notes::supports_variable_players(layout) {
                s.set_participants(0, players).unwrap();
            }
            let (mut edits, mut selected) = (BTreeMap::new(), BTreeSet::new());
            for e in case["Events"].as_array().unwrap() {
                if e["Kind"] == "Tempo" {
                    continue;
                }
                let pos = p(
                    e["Row"].as_u64().unwrap(),
                    e["Lane"].as_u64().unwrap() as u16,
                    e["Player"].as_u64().unwrap() as u32,
                );
                edits.insert(
                    pos,
                    Some(note(kind(e["Kind"].as_str().unwrap()), e["End"].as_u64())),
                );
                if e["Selected"] != false {
                    selected.insert(pos);
                }
            }
            s.edit_notes(0, "fixture", edits).unwrap();
            let before = s.serialize().unwrap();
            let revision = s.revision();
            let result = Plan::prepare(&mut s, 0, Scope::Heads(selected), operation(case));
            if case["Name"]
                .as_str()
                .unwrap()
                .starts_with("owner-assignment-")
            {
                assert_eq!(result.err().unwrap().kind(), Refusal::OwnerCollision);
                assert_eq!(s.serialize().unwrap(), before);
                assert_eq!(s.revision(), revision);
                refused += 1;
                continue;
            }
            let plan = result.unwrap();
            assert_eq!(s.serialize().unwrap(), before);
            let ack = plan.impacts().clone();
            let changed = plan.apply(&mut s, 0, &ack).unwrap();
            let expected_notes: BTreeMap<_, _> = oracle["After"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|e| e["Kind"] != "Tempo")
                .map(|e| {
                    (
                        p(
                            e["Row"].as_u64().unwrap(),
                            e["Lane"].as_u64().unwrap() as u16,
                            e["Player"].as_u64().unwrap() as u32,
                        ),
                        (kind(e["Kind"].as_str().unwrap()), e["End"].as_u64()),
                    )
                })
                .collect();
            let actual: BTreeMap<_, _> = s
                .grid(0)
                .unwrap()
                .iter()
                .map(|(p, n)| (p, (n.kind, n.end)))
                .collect();
            assert_eq!(actual, expected_notes, "{}", case["Name"]);
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
            matched += 1;
        }
    }
    assert_eq!((matched, refused), (38, 6));
}
#[test]
fn unselected_hold_trim_requires_exact_individual_acknowledgement_and_restores_every_note() {
    let mut s = source("dance-single", Format::Ssc);
    s.edit_notes(
        0,
        "fixture",
        [
            (p(48, 0, 0), Some(Note::tap())),
            (p(0, 3, 0), Some(note(Kind::Hold, Some(96)))),
            (p(48, 1, 0), Some(Note::tap())),
        ],
    )
    .unwrap();
    let before = s.serialize().unwrap();
    let plan = Plan::prepare(
        &mut s,
        0,
        Scope::Heads(BTreeSet::from([p(48, 0, 0)])),
        Transform::Mirror,
    )
    .unwrap();
    let impact = Impact {
        position: p(0, 3, 0),
        kind: ImpactKind::OtherChanged,
    };
    assert_eq!(plan.impacts(), &BTreeSet::from([impact]));
    assert_eq!(plan.report().other_affected, 1);
    assert_eq!(
        plan.apply(&mut s, 0, &BTreeSet::new())
            .err()
            .unwrap()
            .kind(),
        Refusal::MissingReview
    );
    assert_eq!(s.serialize().unwrap(), before);
    plan.apply(&mut s, 0, &BTreeSet::from([impact])).unwrap();
    assert_eq!(s.grid(0).unwrap().get(p(0, 3, 0)).unwrap().end, Some(36));
    assert!(s.grid(0).unwrap().get(p(48, 1, 0)).is_some());
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
}
#[test]
fn boundary_deletion_is_reviewed_and_invalid_extra_acknowledgement_is_refused() {
    let mut s = source("dance-single", Format::Ssc);
    s.edit_notes(
        0,
        "fixture",
        [
            (p(48, 0, 0), Some(Note::tap())),
            (p(48, 2, 0), Some(Note::tap())),
        ],
    )
    .unwrap();
    let before = s.serialize().unwrap();
    let plan = Plan::prepare(
        &mut s,
        0,
        Scope::Heads(BTreeSet::from([p(48, 0, 0)])),
        Transform::ShiftLanes {
            right: false,
            wrap: false,
        },
    )
    .unwrap();
    assert_eq!(plan.report().deleted, 1);
    let mut wrong = plan.impacts().clone();
    wrong.insert(Impact {
        position: p(48, 2, 0),
        kind: ImpactKind::OtherDeleted,
    });
    assert_eq!(
        plan.apply(&mut s, 0, &wrong).err().unwrap().kind(),
        Refusal::MissingReview
    );
    assert_eq!(s.serialize().unwrap(), before);
    plan.apply(&mut s, 0, plan.impacts()).unwrap();
    assert!(s.grid(0).unwrap().get(p(48, 0, 0)).is_none());
    assert!(s.grid(0).unwrap().get(p(48, 2, 0)).is_some());
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
}
#[test]
fn stale_foreign_cancellation_and_limits_leave_content_and_redo_unchanged() {
    let mut s = source("dance-single", Format::Ssc);
    s.edit_notes(0, "fixture", [(p(48, 0, 0), Some(Note::tap()))])
        .unwrap();
    let before = s.serialize().unwrap();
    let scope = Scope::All { player: None };
    assert!(
        Plan::prepare_bounded(
            &mut s,
            0,
            scope.clone(),
            Transform::Mirror,
            0,
            &AtomicBool::new(false)
        )
        .is_err()
    );
    assert!(
        Plan::prepare_bounded(
            &mut s,
            0,
            scope.clone(),
            Transform::Mirror,
            10,
            &AtomicBool::new(true)
        )
        .is_err()
    );
    let plan = Plan::prepare(&mut s, 0, scope, Transform::Mirror).unwrap();
    s.set_text(None, "TITLE", "new edit").unwrap();
    s.undo().unwrap();
    let redo = s.redo_label().unwrap().to_string();
    assert_eq!(
        plan.apply(&mut s, 0, plan.impacts()).err().unwrap().kind(),
        Refusal::Stale
    );
    assert_eq!(s.serialize().unwrap(), before);
    assert_eq!(s.redo_label(), Some(redo.as_str()));
    let mut foreign = source("dance-single", Format::Ssc);
    assert_eq!(
        plan.apply(&mut foreign, 0, plan.impacts())
            .err()
            .unwrap()
            .kind(),
        Refusal::Stale
    );
}
#[test]
fn original_shift_snap_off_is_one_beat_while_insertion_off_remains_one_row() {
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/selected-transform.expected.json"
    ))
    .unwrap();
    for (index, case) in expected["ShiftSteps"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        assert_eq!(
            deadsync_editor::workspace::SNAP_ROWS[index],
            case["SnapRows"].as_u64().unwrap()
        );
        assert_eq!(
            deadsync_editor::workspace::snap_shift_rows(index),
            case["ShiftRows"].as_u64().unwrap()
        );
    }
    assert_eq!(deadsync_editor::workspace::snap_edit_rows(9), 1);
    assert_eq!(deadsync_editor::workspace::snap_edit_rows(0), 48);
}
#[test]
fn exact_public_engine_and_legacy_range_use_the_same_spatial_and_owner_rules() {
    for transform in [
        Transform::Mirror,
        Transform::ShiftLanes {
            right: true,
            wrap: true,
        },
        Transform::ShiftRows(48),
        Transform::AssignPlayer(1),
        Transform::SwapPlayers(0, 1),
    ] {
        let mut s = source("dance-routine", Format::Ssc);
        s.edit_notes(
            0,
            "fixture",
            [
                (p(48, 0, 0), Some(Note::tap())),
                (p(96, 1, 1), Some(note(Kind::Roll, Some(144)))),
            ],
        )
        .unwrap();
        let bytes = s.serialize().unwrap();
        let mut other = Session::new(Document::parse(bytes, Format::Ssc).unwrap());
        let heads = s.grid(0).unwrap().iter().map(|(p, _)| p).collect();
        let a = transform::apply_positions(&mut s, 0, &heads, transform);
        let b = transform::apply(&mut other, 0, 0, i32::MAX as u64, None, transform);
        assert_eq!(a.is_ok(), b.is_ok());
        assert_eq!(s.serialize().unwrap(), other.serialize().unwrap());
    }
}
