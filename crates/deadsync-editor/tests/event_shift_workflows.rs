use deadsync_editor::{
    attacks::{Modifier, inspector::Draft as Attacks},
    document::{Document, Format},
    event_clipboard::{Event, Selection, TimingKind},
    event_shift::Plan,
    generation::metadata::{ChartData, Definition},
    history::Session,
    notes::{Kind, Note, Position, layout_columns},
    timing_edit::{Scope, TempoDraft, signature::Signatures},
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
fn p(player: u32, lane: u16, row: u64) -> Position {
    Position { player, lane, row }
}
fn fixture(case: &Value, format: Format) -> Session {
    let layout = case["Layout"].as_str().unwrap();
    let events = case["Events"].as_array().unwrap();
    let notes = "0".repeat(layout_columns(layout).unwrap() as usize);
    let chart = if format == Format::Sm {
        format!("#NOTES:{layout}::Hard:9::{notes};")
    } else {
        format!("#NOTEDATA:;#STEPSTYPE:{layout};#NOTES:{notes};")
    };
    let prefix = if format == Format::Sm && events.iter().any(|value| value["Kind"] == "Tempo") {
        "#VERSION:0.83;#TITLE:Keep;"
    } else {
        "#VERSION:0.83;#TITLE:Keep;#BPMS:0=120;"
    };
    let mut text = format!("{prefix}{chart}");
    // These fixtures use one chart; in SSC the tags below belong to that chart,
    // while SM's supported properties stay at song scope.
    for (kind, tag) in [
        ("Tempo", "BPMS"),
        ("Signature", "TIMESIGNATURES"),
        ("Label", "LABELS"),
        ("Delay", "DELAYS"),
    ] {
        let values: Vec<_> = events
            .iter()
            .filter(|v| v["Kind"] == kind)
            .map(|v| {
                let beat = v["Row"].as_u64().unwrap() as f64 / 48.0;
                match kind {
                    "Tempo" => format!("{beat}={}", v["Bpm"]),
                    "Signature" => format!("{beat}={}={}", v["N"], v["D"]),
                    "Label" => format!("{beat}=marker"),
                    _ => format!("{beat}=0.25"),
                }
            })
            .collect();
        if !values.is_empty() {
            text += &format!("#{tag}:{};", values.join(","));
        }
    }
    let mut s = Session::new(Document::parse(text.as_bytes(), format).unwrap());
    let definitions: BTreeMap<_, _> = events
        .iter()
        .filter(|v| v["Kind"] == "Pattern")
        .map(|v| {
            (
                v["Row"].as_u64().unwrap(),
                serde_json::from_value::<Definition>(v["Definition"].clone()).unwrap(),
            )
        })
        .collect();
    let mut data = ChartData::existing_or_initial(s.document(), 0).unwrap();
    data.max_player_index = case["Players"].as_u64().unwrap() as u32 - 1;
    if !definitions.is_empty() {
        data.patterns = Some(definitions);
    }
    data.save(&mut s, 0).unwrap();
    let notes: Vec<_> = events
        .iter()
        .filter(|v| v["Kind"] == "Tap" || v["Kind"] == "Hold")
        .map(|v| {
            let end = v.get("End").and_then(Value::as_u64);
            (
                p(
                    v["Player"].as_u64().unwrap_or(0) as u32,
                    v["Lane"].as_u64().unwrap_or(0) as u16,
                    v["Row"].as_u64().unwrap(),
                ),
                Some(Note {
                    kind: if end.is_some() { Kind::Hold } else { Kind::Tap },
                    end,
                    attributes: "".into(),
                    tail_attributes: "".into(),
                }),
            )
        })
        .collect();
    if !notes.is_empty() {
        s.edit_notes(0, "Fixture", notes).unwrap();
    }
    for v in events.iter().filter(|v| v["Kind"] == "Attack") {
        let mut attacks = Attacks::capture(&s, 0, scope(format)).unwrap();
        attacks
            .add(
                v["Row"].as_u64().unwrap(),
                Modifier {
                    name: "Drunk".into(),
                    length: 1.0,
                    ..Modifier::default()
                },
            )
            .unwrap();
        attacks.apply(&mut s, 0).unwrap();
    }
    s
}
fn scope(format: Format) -> Scope {
    if format == Format::Ssc {
        Scope::Chart
    } else {
        Scope::Song
    }
}
fn key(v: &Value) -> Event {
    let row = v["Row"].as_u64().unwrap();
    match v["Kind"].as_str().unwrap() {
        "Tap" | "Hold" => Event::Note(p(
            v["Player"].as_u64().unwrap_or(0) as u32,
            v["Lane"].as_u64().unwrap_or(0) as u16,
            row,
        )),
        "Pattern" => Event::Pattern { row },
        "Attack" => Event::Attack { row },
        kind => Event::Timing {
            row,
            kind: match kind {
                "Tempo" => TimingKind::Tempo,
                "Signature" => TimingKind::Signature,
                "Label" => TimingKind::Label,
                "Delay" => TimingKind::Delay,
                _ => unreachable!(),
            },
        },
    }
}
fn selected(s: &mut Session, keys: &BTreeSet<Event>) -> Selection {
    let mut selection = Selection::capture(s, 0, 0, i32::MAX as u64).unwrap();
    selection.select_all(false);
    for i in 0..selection.events().len() {
        if keys.contains(&selection.events()[i]) {
            selection.set_selected(i, true).unwrap();
        }
    }
    selection
}
fn canonical(mut values: Vec<Value>) -> Vec<Value> {
    for v in &mut values {
        if let Some(bpm) = v.get("Bpm").and_then(Value::as_f64) {
            v["Bpm"] = json!(bpm);
        }
    }
    values.sort_by_key(|v| serde_json::to_string(v).unwrap());
    values
}
fn projection(s: &mut Session, kinds: &BTreeSet<String>, filter: Option<&[Event]>) -> Vec<Value> {
    let selection = Selection::capture(s, 0, 0, i32::MAX as u64).unwrap();
    let format = s.document().format;
    let tempos = TempoDraft::capture(s, 0, scope(format)).unwrap();
    let signatures = Signatures::from_document(s.document(), 0).unwrap();
    let data = ChartData::read(s.document(), 0).unwrap();
    let mut values = Vec::new();
    for &event in selection.events() {
        if filter.is_some_and(|keys| !keys.contains(&event)) {
            continue;
        }
        let row = event.row();
        let (kind, value) = match event {
            Event::Note(position) => {
                let note = s.grid(0).unwrap().get(position).unwrap();
                (
                    if note.end.is_some() { "Hold" } else { "Tap" },
                    json!({"Kind":if note.end.is_some(){"LaneHoldStartNote"}else{"LaneTapNote"},"Row":row,"Lane":position.lane,"Player":position.player,"End":note.end}),
                )
            }
            Event::Timing {
                kind: TimingKind::Tempo,
                ..
            } => (
                "Tempo",
                json!({"Kind":"Tempo","Row":row,"Bpm":tempos.tempos().values()[&row]}),
            ),
            Event::Timing {
                kind: TimingKind::Signature,
                ..
            } => {
                let signature = signatures.values()[&row];
                (
                    "Signature",
                    json!({"Kind":"Signature","Row":row,"N":signature.numerator,"D":signature.denominator}),
                )
            }
            Event::Timing {
                kind: TimingKind::Label,
                ..
            } => (
                "Label",
                json!({"Kind":"Label","Row":row,"Lane":-1,"Player":0,"End":null}),
            ),
            Event::Timing {
                kind: TimingKind::Delay,
                ..
            } => (
                "Delay",
                json!({"Kind":"Stop","Row":row,"Lane":-1,"Player":0,"End":null}),
            ),
            Event::Attack { .. } => (
                "Attack",
                json!({"Kind":"Attack","Row":row,"Lane":-1,"Player":0,"End":null}),
            ),
            Event::Pattern { .. } => (
                "Pattern",
                json!({"Kind":"Pattern","Row":row,"Definition":data.as_ref().unwrap().patterns.as_ref().unwrap()[&row]}),
            ),
            _ => continue,
        };
        if kinds.contains(kind) {
            values.push(value);
        }
    }
    canonical(values)
}
#[test]
fn twelve_original_shift_actions_match_business_results_selection_and_history() {
    let input: Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/event-shift.request.json"
    ))
    .unwrap();
    let expected: Vec<Value> = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/event-shift.expected.json"
    ))
    .unwrap();
    assert_eq!(expected.len(), 12);
    let mut compared = 0;
    for format in [Format::Ssc, Format::Sm] {
        for (case, expected) in input["Cases"].as_array().unwrap().iter().zip(&expected) {
            assert_eq!(case["Name"], expected["Name"]);
            let events = case["Events"].as_array().unwrap();
            if format == Format::Sm && events.iter().any(|v| v["Kind"] == "Label") {
                continue;
            }
            let kinds = events
                .iter()
                .map(|v| v["Kind"].as_str().unwrap().to_owned())
                .collect();
            let keys = events
                .iter()
                .filter(|v| v["Selected"].as_bool().unwrap_or(true))
                .map(key)
                .collect();
            let mut s = fixture(case, format);
            assert_eq!(
                projection(&mut s, &kinds, None),
                canonical(expected["Before"].as_array().unwrap().clone()),
                "{} before {format:?}",
                case["Name"]
            );
            let before = s.serialize().unwrap();
            let choices = selected(&mut s, &keys);
            let report = Plan::capture(&mut s, &choices, case["Rows"].as_i64().unwrap())
                .unwrap()
                .apply(&mut s)
                .unwrap();
            assert_eq!(
                projection(&mut s, &kinds, None),
                canonical(expected["After"].as_array().unwrap().clone()),
                "{} after {format:?}",
                case["Name"]
            );
            assert_eq!(
                projection(&mut s, &kinds, Some(&report.selection_after)),
                canonical(expected["SelectedAfter"].as_array().unwrap().clone()),
                "{} selection {format:?}",
                case["Name"]
            );
            let after = s.serialize().unwrap();
            if report.changed {
                s.undo().unwrap();
                assert_eq!(s.serialize().unwrap(), before);
                s.redo().unwrap();
                assert_eq!(s.serialize().unwrap(), after);
            }
            let mut reopened = Session::new(Document::parse(after, format).unwrap());
            assert_eq!(
                projection(&mut reopened, &kinds, None),
                canonical(expected["Redone"].as_array().unwrap().clone()),
                "{} reopen {format:?}",
                case["Name"]
            );
            compared += 1;
        }
    }
    assert_eq!(compared, 19);
}

#[test]
fn negative_rows_delete_whole_events_while_zero_shift_and_immutable_nodes_preserve_history() {
    let cases: Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/event-shift.request.json"
    ))
    .unwrap();
    let case = cases["Cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["Name"] == "negative-rows-delete-not-clamp")
        .unwrap();
    let mut s = fixture(case, Format::Ssc);
    let keys = case["Events"].as_array().unwrap().iter().map(key).collect();
    let choices = selected(&mut s, &keys);
    let before = s.serialize().unwrap();
    let report = Plan::capture(&mut s, &choices, -48)
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert_eq!((report.moved, report.deleted, report.immutable), (2, 2, 0));
    assert!(s.grid(0).unwrap().get(p(0, 0, 0)).is_none());
    assert_eq!(
        ChartData::read(s.document(), 0)
            .unwrap()
            .unwrap()
            .patterns
            .unwrap()
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        [0]
    );
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
    let case = cases["Cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["Name"] == "zero-shift-preserves-events")
        .unwrap();
    let mut s = fixture(case, Format::Ssc);
    s.set_text(None, "TITLE", "Redo").unwrap();
    s.undo().unwrap();
    let before = s.serialize().unwrap();
    let revision = s.revision();
    let redo = s.redo_label().unwrap().to_owned();
    let keys = case["Events"].as_array().unwrap().iter().map(key).collect();
    let choices = selected(&mut s, &keys);
    let reusable = Plan::capture(&mut s, &choices, 0).unwrap();
    let report = Plan::capture(&mut s, &choices, 0)
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert!(!report.changed);
    assert_eq!((report.moved, report.deleted, report.immutable), (0, 0, 2));
    assert_eq!(report.selection_after.len(), 3);
    assert_eq!(s.serialize().unwrap(), before);
    assert_eq!(s.revision(), revision);
    assert_eq!(s.redo_label(), Some(redo.as_str()));
    assert!(!reusable.apply(&mut s).unwrap().changed);
    let only_initial = BTreeSet::from([
        Event::Timing {
            row: 0,
            kind: TimingKind::Tempo,
        },
        Event::Timing {
            row: 0,
            kind: TimingKind::Signature,
        },
    ]);
    let choices = selected(&mut s, &only_initial);
    let report = Plan::capture(&mut s, &choices, 96)
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert!(!report.changed);
    assert_eq!(report.immutable, 2);
    assert!(report.selection_after.is_empty());
    assert_eq!(s.serialize().unwrap(), before);
}
#[test]
fn a_single_stepf2_owner_sustain_keeps_owner_head_tail_attributes_and_other_players() {
    for format in [Format::Sm, Format::Ssc] {
        let notes = "{X|s|1|%}[7]{y|v|1|0}Z1000000\n0{3|h|0|_}[8]00000000\n0000000000\n0000000000";
        let chart = if format == Format::Sm {
            format!("#NOTES:pump-routine::Hard:9::{notes};")
        } else {
            format!("#NOTEDATA:;#STEPSTYPE:pump-routine;#NOTES:{notes};")
        };
        let bytes = format!("#VERSION:0.83;#TITLE:Keep;#VENDOR:untouched;#BPMS:0=120;{chart}");
        let mut s = Session::new(Document::parse(bytes.as_bytes(), format).unwrap());
        let before = s.serialize().unwrap();
        let note = s.grid(0).unwrap().get(p(1, 1, 0)).unwrap().clone();
        assert_eq!(s.grid(0).unwrap().players(), 4);
        let choices = selected(&mut s, &BTreeSet::from([Event::Note(p(1, 1, 0))]));
        let report = Plan::capture(&mut s, &choices, 96)
            .unwrap()
            .apply(&mut s)
            .unwrap();
        assert_eq!(report.moved, 1);
        let actual = s.grid(0).unwrap().get(p(1, 1, 96)).unwrap();
        assert_eq!(actual.attributes, note.attributes);
        assert_eq!(actual.tail_attributes, note.tail_attributes);
        assert_eq!(actual.end, Some(144));
        assert_eq!(s.grid(0).unwrap().players(), 4);
        for (player, lane) in [(0, 0), (2, 2), (3, 3)] {
            assert!(s.grid(0).unwrap().get(p(player, lane, 0)).is_some());
        }
        assert_eq!(s.document().text(None, "VENDOR"), Some("untouched".into()));
        let grid = s.grid(0).unwrap().clone();
        let after = s.serialize().unwrap();
        let mut reopened = Session::new(Document::parse(after.clone(), format).unwrap());
        assert_eq!(reopened.grid(0).unwrap(), &grid);
        s.undo().unwrap();
        assert_eq!(s.serialize().unwrap(), before);
        s.redo().unwrap();
        assert_eq!(s.serialize().unwrap(), after);
    }
}
#[test]
fn detached_pattern_definition_and_unknown_metadata_survive_row_shift_and_reopen() {
    let input: Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/event-shift.request.json"
    ))
    .unwrap();
    let case = input["Cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["Name"] == "overlapping-pattern-regions-remain")
        .unwrap();
    for format in [Format::Sm, Format::Ssc] {
        let mut s = fixture(case, format);
        let mut data = ChartData::read(s.document(), 0).unwrap().unwrap();
        data.extra
            .insert("FutureChart".into(), json!({"nested":[1,"two"]}));
        data.patterns
            .as_mut()
            .unwrap()
            .get_mut(&48)
            .unwrap()
            .extra
            .insert("FutureDefinition".into(), json!([true,{"keep":3}]));
        data.save(&mut s, 0).unwrap();
        let before = s.serialize().unwrap();
        let original = data.clone();
        let choices = selected(&mut s, &BTreeSet::from([Event::Pattern { row: 48 }]));
        Plan::capture(&mut s, &choices, 48)
            .unwrap()
            .apply(&mut s)
            .unwrap();
        let mut expected = original;
        let definition = expected.patterns.as_mut().unwrap().remove(&48).unwrap();
        expected.patterns.as_mut().unwrap().insert(96, definition);
        assert_eq!(
            serde_json::to_value(ChartData::read(s.document(), 0).unwrap().unwrap()).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
        let after = s.serialize().unwrap();
        let reopened = Session::new(Document::parse(after, format).unwrap());
        assert_eq!(
            serde_json::to_value(ChartData::read(reopened.document(), 0).unwrap().unwrap())
                .unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
        s.undo().unwrap();
        assert_eq!(s.serialize().unwrap(), before);
    }
}
#[test]
fn stale_foreign_selections_and_shift_plans_never_publish_or_erase_redo() {
    let input: Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/event-shift.request.json"
    ))
    .unwrap();
    let case = &input["Cases"][0];
    let mut s = fixture(case, Format::Ssc);
    let keys = case["Events"].as_array().unwrap().iter().map(key).collect();
    let choices = selected(&mut s, &keys);
    let mut foreign = fixture(case, Format::Ssc);
    assert!(Plan::capture(&mut foreign, &choices, 48).is_err());
    let plan = Plan::capture(&mut s, &choices, 48).unwrap();
    assert!(plan.apply(&mut foreign).is_err());
    s.set_text(None, "TITLE", "Changed").unwrap();
    assert!(Plan::capture(&mut s, &choices, 48).is_err());
    let choices = selected(&mut s, &keys);
    let plan = Plan::capture(&mut s, &choices, 48).unwrap();
    s.undo().unwrap();
    let before = s.serialize().unwrap();
    let redo = s.redo_label().unwrap().to_owned();
    assert!(plan.apply(&mut s).is_err());
    assert_eq!(s.serialize().unwrap(), before);
    assert_eq!(s.redo_label(), Some(redo.as_str()));
}
#[test]
fn checked_head_and_region_end_overflow_leave_document_and_redo_unchanged() {
    let input: Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/event-shift.request.json"
    ))
    .unwrap();
    let case = &input["Cases"][0];
    let mut s = fixture(case, Format::Ssc);
    s.set_text(None, "TITLE", "Redo").unwrap();
    s.undo().unwrap();
    let before = s.serialize().unwrap();
    let redo = s.redo_label().unwrap().to_owned();
    let choices = selected(&mut s, &BTreeSet::from([Event::Note(p(0, 1, 192))]));
    assert!(Plan::capture(&mut s, &choices, i64::MAX).is_err());
    assert_eq!(s.serialize().unwrap(), before);
    assert_eq!(s.redo_label(), Some(redo.as_str()));
    let mut data = ChartData::read(s.document(), 0).unwrap().unwrap();
    let mut definition = data.patterns.as_ref().unwrap()[&144].clone();
    definition.length = i32::MAX as u64;
    data.patterns = Some(BTreeMap::from([(0, definition)]));
    data.save(&mut s, 0).unwrap();
    s.set_text(None, "TITLE", "New redo").unwrap();
    s.undo().unwrap();
    let before = s.serialize().unwrap();
    let redo = s.redo_label().unwrap().to_owned();
    let choices = selected(&mut s, &BTreeSet::from([Event::Pattern { row: 0 }]));
    assert!(Plan::capture(&mut s, &choices, 1).is_err());
    assert_eq!(s.serialize().unwrap(), before);
    assert_eq!(s.redo_label(), Some(redo.as_str()));
}
