use deadsync_editor::{
    chart_event_copy::{EventType as Type, Plan},
    document::{Document, Format},
    generation::metadata::{ChartData, Definition},
    history::Session,
    notes::Position,
};
use std::collections::BTreeMap;
fn ssc() -> Session {
    Session::new(Document::parse(b"// retained\n#VERSION:0.83;#TITLE:Keep;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=180,4=240;#TIMESIGNATURES:0=2=2,4=3=4;#LABELS:1=source;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=90,2=110;#TIMESIGNATURES:0=4=4,2=5=8;#LABELS:3=old;#STOPS:3=0.5;#NOTES:0100;#NOTEDATA:;#STEPSTYPE:dance-routine;#BPMS:0=100,1=150;#LABELS:2=old;#NOTES:00100000;".as_slice(), Format::Ssc).unwrap())
}
fn pattern(s: &mut Session, chart: usize, row: u64, seed: i32) {
    let mut data = ChartData::existing_or_initial(s.document(), chart).unwrap();
    data.patterns = Some(BTreeMap::from([(
        row,
        Definition {
            random_seed: seed,
            length: 96,
            ..Definition::default()
        },
    )]));
    data.save(s, chart).unwrap();
}
#[test]
fn replace_types_on_multiple_destinations_preserves_unselected_data_and_has_one_undo() {
    let mut s = ssc();
    let before = s.serialize().unwrap();
    let source = s.document().text(Some(0), "BPMS");
    let report = Plan::capture(
        &mut s,
        0,
        [1, 2],
        [Type::Tempo, Type::Signature, Type::Label],
    )
    .unwrap()
    .apply(&mut s)
    .unwrap();
    assert!(report.changed);
    assert_eq!(report.destinations_changed, 2);
    for chart in [1, 2] {
        assert_eq!(
            s.document().text(Some(chart), "BPMS"),
            Some("0=180,4=240".into())
        );
        assert_eq!(
            s.document().text(Some(chart), "TIMESIGNATURES"),
            Some("0=2=2,4=3=4".into())
        );
        assert_eq!(
            s.document().text(Some(chart), "LABELS"),
            Some("1=source".into())
        );
    }
    assert_eq!(s.document().text(Some(0), "BPMS"), source);
    assert_eq!(s.document().text(Some(1), "STOPS"), Some("3=0.5".into()));
    assert!(
        s.grid(1)
            .unwrap()
            .get(Position {
                player: 0,
                lane: 1,
                row: 0
            })
            .is_some()
    );
    let after = s.serialize().unwrap();
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
    s.redo().unwrap();
    assert_eq!(s.serialize().unwrap(), after);
    let reopened = Session::new(Document::parse(after, Format::Ssc).unwrap());
    assert_eq!(reopened.serialize().unwrap(), s.serialize().unwrap());
}
#[test]
fn absent_source_type_clears_destination_while_initial_nodes_and_notes_remain() {
    let mut s = ssc();
    let before = s.serialize().unwrap();
    let report = Plan::capture(&mut s, 0, [1], [Type::Stop])
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert!(report.changed);
    assert_eq!(s.document().text(Some(1), "STOPS"), Some(String::new()));
    assert_eq!(
        s.document().text(Some(1), "BPMS"),
        Some("0=90,2=110".into())
    );
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
}
#[test]
fn patterns_replace_entire_type_and_clear_old_patterns_on_unsupported_destination() {
    let mut s = ssc();
    pattern(&mut s, 0, 48, 42);
    pattern(&mut s, 1, 192, 7);
    pattern(&mut s, 2, 96, 8);
    let before = s.serialize().unwrap();
    let report = Plan::capture(&mut s, 0, [1, 2], [Type::Pattern])
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert_eq!(report.patterns_pasted, 1);
    assert_eq!(report.patterns_skipped, 1);
    let values = ChartData::read(s.document(), 1)
        .unwrap()
        .unwrap()
        .patterns
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[&48].random_seed, 42);
    assert!(
        ChartData::read(s.document(), 2)
            .unwrap()
            .unwrap()
            .patterns
            .unwrap()
            .is_empty()
    );
    assert!(
        s.grid(2)
            .unwrap()
            .get(Position {
                player: 0,
                lane: 2,
                row: 0
            })
            .is_some()
    );
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
}
#[test]
fn plans_reject_stale_foreign_or_invalid_destinations_without_erasing_history() {
    let mut s = ssc();
    let before = s.serialize().unwrap();
    assert!(Plan::capture(&mut s, 0, [1, 99], [Type::Label]).is_err());
    assert!(Plan::capture(&mut s, 0, [0], [Type::Label]).is_err());
    assert!(Plan::capture(&mut s, 0, [1], []).is_err());
    assert_eq!(s.serialize().unwrap(), before);
    s.set_text(None, "TITLE", "Changed").unwrap();
    let plan = Plan::capture(&mut s, 0, [1, 2], [Type::Label]).unwrap();
    s.undo().unwrap();
    let redo = s.redo_label().unwrap().to_owned();
    assert!(plan.apply(&mut s).is_err());
    assert_eq!(s.serialize().unwrap(), before);
    assert_eq!(s.redo_label(), Some(redo.as_str()));
    let plan = Plan::capture(&mut s, 0, [1], [Type::Label]).unwrap();
    let mut foreign = ssc();
    assert!(plan.apply(&mut foreign).is_err());
}
#[test]
fn legacy_ssc_shared_types_are_reported_without_rewriting_source() {
    let bytes = "#VERSION:0.6;#TITLE:Keep;#BPMS:0=120,4=180;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0100;";
    let format = Format::Ssc;
    let mut s = Session::new(Document::parse(bytes.as_bytes(), format).unwrap());
    let before = s.serialize().unwrap();
    let report = Plan::capture(&mut s, 0, [1], [Type::Tempo, Type::Attack])
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert!(!report.changed);
    assert_eq!(report.destinations_changed, 0);
    assert_eq!(report.shared_types_unchanged.len(), 2);
    assert_eq!(s.serialize().unwrap(), before);
    pattern(&mut s, 0, 48, 42);
    pattern(&mut s, 1, 96, 7);
    let before = s.serialize().unwrap();
    let report = Plan::capture(&mut s, 0, [1], [Type::Tempo, Type::Pattern])
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert!(report.changed);
    assert_eq!(report.shared_types_unchanged.len(), 1);
    assert_eq!(
        ChartData::read(s.document(), 1)
            .unwrap()
            .unwrap()
            .patterns
            .unwrap()[&48]
            .random_seed,
        42
    );
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
}

fn fixture_session(case: &serde_json::Value) -> Session {
    let charts: Vec<_> = std::iter::once(&case["Source"])
        .chain(case["Destinations"].as_array().unwrap())
        .collect();
    let mut text = String::from("#VERSION:0.83;#TITLE:Oracle;#BPMS:0=120;");
    for chart in &charts {
        let layout = chart["Layout"].as_str().unwrap();
        text += &format!(
            "#NOTEDATA:;#STEPSTYPE:{layout};#NOTES:{};",
            "0".repeat(deadsync_editor::notes::layout_columns(layout).unwrap() as usize)
        );
        for (kind, tag) in [
            ("Tempo", "BPMS"),
            ("Signature", "TIMESIGNATURES"),
            ("Label", "LABELS"),
            ("Delay", "DELAYS"),
        ] {
            let values: Vec<_> = chart["Events"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|value| value["Kind"] == kind)
                .map(|value| {
                    let beat = value["Row"].as_u64().unwrap() as f64 / 48.0;
                    match kind {
                        "Tempo" => format!("{beat}={}", value["Bpm"]),
                        "Signature" => format!("{beat}={}={}", value["N"], value["D"]),
                        "Label" => format!("{beat}=marker"),
                        _ => format!("{beat}=0.25"),
                    }
                })
                .collect();
            if !values.is_empty() {
                text += &format!("#{tag}:{};", values.join(","));
            }
        }
    }
    let mut s = Session::new(Document::parse(text.as_bytes(), Format::Ssc).unwrap());
    for (index, chart) in charts.iter().enumerate() {
        let mut data = ChartData::existing_or_initial(s.document(), index).unwrap();
        data.max_player_index = chart["Players"].as_u64().unwrap() as u32 - 1;
        let definitions: BTreeMap<_, _> = chart["Events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|value| value["Kind"] == "Pattern")
            .map(|value| {
                (
                    value["Row"].as_u64().unwrap(),
                    serde_json::from_value(value["Definition"].clone()).unwrap(),
                )
            })
            .collect();
        if !definitions.is_empty() {
            data.patterns = Some(definitions);
        }
        data.save(&mut s, index).unwrap();
        for value in chart["Events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|value| value["Kind"] == "Attack")
        {
            use deadsync_editor::{
                attacks::{Modifier, inspector::Draft},
                timing_edit::Scope,
            };
            let mut draft = Draft::capture(&s, index, Scope::Chart).unwrap();
            draft
                .add(
                    value["Row"].as_u64().unwrap(),
                    Modifier {
                        name: "Drunk".into(),
                        length: 1.0,
                        ..Modifier::default()
                    },
                )
                .unwrap();
            draft.apply(&mut s, index).unwrap();
        }
    }
    s
}
fn fixture_projection(
    s: &mut Session,
    chart: usize,
    kinds: &std::collections::BTreeSet<String>,
) -> Vec<serde_json::Value> {
    use deadsync_editor::{
        event_clipboard::{Event, Selection, TimingKind},
        timing_edit::{Scope, TempoDraft, signature::Signatures},
    };
    use serde_json::json;
    let selection = Selection::capture(s, chart, 0, i32::MAX as u64).unwrap();
    let tempos = TempoDraft::capture(s, chart, Scope::Chart).unwrap();
    let signatures = Signatures::from_document(s.document(), chart).unwrap();
    let data = ChartData::read(s.document(), chart).unwrap();
    let mut result = Vec::new();
    for event in selection.events() {
        let row = event.row();
        let (kind, value) = match event {
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
                let value = signatures.values()[&row];
                (
                    "Signature",
                    json!({"Kind":"Signature","Row":row,"N":value.numerator,"D":value.denominator}),
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
            result.push(value);
        }
    }
    canonical(result)
}
fn canonical(mut values: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
    for value in &mut values {
        if let Some(bpm) = value.get("Bpm").and_then(serde_json::Value::as_f64) {
            value["Bpm"] = serde_json::json!(bpm);
        }
    }
    values.sort_by_key(|value| serde_json::to_string(value).unwrap());
    values
}
#[test]
fn original_chart_wide_action_matches_representable_ssc_cases_and_undo_redo() {
    use serde_json::Value;
    let input: Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/chart-copy.request.json"
    ))
    .unwrap();
    let expected: Vec<Value> = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/chart-copy.expected.json"
    ))
    .unwrap();
    assert_eq!(expected.len(), 10);
    let mut compared = 0;
    for (case, expected) in input["Cases"].as_array().unwrap().iter().zip(expected) {
        assert_eq!(case["Name"], expected["Name"]);
        if case["Name"] == "first-event-destination-position-preserved" {
            // This action-only fixture bypasses EditorChart's load invariant:
            // real charts acquire row-zero initial nodes before this action.
            // The separate loaded_initial_nodes test executes the source action
            // after those load preconditions, then compares native behavior.
            assert!(expected["After"][0].as_array().unwrap().iter().any(|event| {
                event["Kind"] == "Tempo" && event["Row"] == 48 && event["Bpm"] == 180
            }));
            continue;
        }
        let charts: Vec<_> = std::iter::once(&case["Source"])
            .chain(case["Destinations"].as_array().unwrap())
            .collect();
        let kinds = charts
            .iter()
            .flat_map(|chart| chart["Events"].as_array().unwrap())
            .map(|event| event["Kind"].as_str().unwrap().to_owned())
            .collect();
        let types =
            case["Types"]
                .as_array()
                .unwrap()
                .iter()
                .map(|kind| match kind.as_str().unwrap() {
                    "Tempo" => Type::Tempo,
                    "Signature" => Type::Signature,
                    "Label" => Type::Label,
                    "Pattern" => Type::Pattern,
                    "Delay" => Type::Delay,
                    "Attack" => Type::Attack,
                    _ => unreachable!(),
                });
        let mut s = fixture_session(case);
        for (index, before) in expected["Before"].as_array().unwrap().iter().enumerate() {
            assert_eq!(
                fixture_projection(&mut s, index + 1, &kinds),
                canonical(before.as_array().unwrap().clone()),
                "{} before",
                case["Name"]
            );
        }
        let before = s.serialize().unwrap();
        let report = Plan::capture(&mut s, 0, 1..charts.len(), types)
            .unwrap()
            .apply(&mut s)
            .unwrap();
        for (index, after) in expected["After"].as_array().unwrap().iter().enumerate() {
            assert_eq!(
                fixture_projection(&mut s, index + 1, &kinds),
                canonical(after.as_array().unwrap().clone()),
                "{} after",
                case["Name"]
            );
        }
        let after = s.serialize().unwrap();
        if report.changed {
            s.undo().unwrap();
            assert_eq!(s.serialize().unwrap(), before);
            s.redo().unwrap();
            assert_eq!(s.serialize().unwrap(), after);
        }
        let mut reopened = Session::new(Document::parse(after, Format::Ssc).unwrap());
        for (index, after) in expected["Redone"].as_array().unwrap().iter().enumerate() {
            assert_eq!(
                fixture_projection(&mut reopened, index + 1, &kinds),
                canonical(after.as_array().unwrap().clone()),
                "{} reopen",
                case["Name"]
            );
        }
        compared += 1;
    }
    assert_eq!(compared, 9);
}

#[test]
fn all_thirteen_original_type_choices_copy_payloads_without_copying_notes_or_chart_settings() {
    use deadsync_editor::{
        attacks::{Modifier, inspector::Draft},
        event_clipboard::{Event, Selection},
        timing_edit::Scope,
    };
    let mut s = ssc();
    for (tag, value) in [
        ("STOPS", "2=0.5"),
        ("DELAYS", "3=0.25"),
        ("WARPS", "4=1"),
        ("SCROLLS", "5=-1"),
        ("SPEEDS", "6=2=1=0"),
        ("FAKES", "7=1"),
        ("COMBOS", "8=2=3"),
        ("TICKCOUNTS", "9=8"),
    ] {
        s.set_text(Some(0), tag, value).unwrap();
    }
    pattern(&mut s, 0, 48, 42);
    let mut attacks = Draft::capture(&s, 0, Scope::Chart).unwrap();
    attacks
        .add(
            96,
            Modifier {
                name: "Drunk".into(),
                length: 2.0,
                ..Modifier::default()
            },
        )
        .unwrap();
    attacks.apply(&mut s, 0).unwrap();
    let before = s.serialize().unwrap();
    let report = Plan::capture(&mut s, 0, [1], Type::ALL)
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert!(report.changed);
    assert_eq!(report.patterns_pasted, 1);
    for tag in [
        "BPMS",
        "TIMESIGNATURES",
        "LABELS",
        "STOPS",
        "DELAYS",
        "WARPS",
        "SCROLLS",
        "SPEEDS",
        "FAKES",
        "COMBOS",
        "TICKCOUNTS",
    ] {
        assert_eq!(
            s.document().text(Some(0), tag),
            s.document().text(Some(1), tag),
            "{tag}"
        );
    }
    let events = |s: &mut Session, chart| {
        Selection::capture(s, chart, 0, i32::MAX as u64)
            .unwrap()
            .events()
            .iter()
            .copied()
            .filter(|event| !matches!(event, Event::Note(_)))
            .collect::<Vec<_>>()
    };
    assert_eq!(events(&mut s, 0), events(&mut s, 1));
    assert!(
        s.grid(0)
            .unwrap()
            .get(Position {
                player: 0,
                lane: 0,
                row: 0
            })
            .is_some()
    );
    assert!(
        s.grid(1)
            .unwrap()
            .get(Position {
                player: 0,
                lane: 1,
                row: 0
            })
            .is_some()
    );
    assert!(
        s.grid(1)
            .unwrap()
            .get(Position {
                player: 0,
                lane: 0,
                row: 0
            })
            .is_none()
    );
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
}

#[test]
fn failure_in_last_destination_rolls_back_first_destination_and_preserves_redo() {
    let mut s = ssc();
    s.set_text(Some(2), "GAChartDataVersion", "1").unwrap();
    s.set_text(Some(2), "GAChartData", "{").unwrap();
    s.set_text(None, "TITLE", "Redo").unwrap();
    s.undo().unwrap();
    let before = s.serialize().unwrap();
    let redo = s.redo_label().unwrap().to_owned();
    let revision = s.revision();
    let plan = Plan::capture(&mut s, 0, [1, 2], [Type::Label]).unwrap();
    assert!(plan.apply(&mut s).is_err());
    assert_eq!(s.serialize().unwrap(), before);
    assert_eq!(s.redo_label(), Some(redo.as_str()));
    assert_eq!(s.revision(), revision);
    assert_eq!(s.document().text(Some(1), "LABELS"), Some("3=old".into()));
}
#[test]
fn net_identical_replacement_preserves_redo_token_and_dirty_resident_notes() {
    use deadsync_editor::notes::Note;
    let mut s = ssc();
    s.set_text(Some(1), "LABELS", "1=source").unwrap();
    s.edit_notes(
        1,
        "Unsaved destination",
        [(
            Position {
                player: 0,
                lane: 3,
                row: 48,
            },
            Some(Note::tap()),
        )],
    )
    .unwrap();
    s.set_text(None, "TITLE", "Redo").unwrap();
    s.undo().unwrap();
    let before = s.serialize().unwrap();
    let redo = s.redo_label().unwrap().to_owned();
    let revision = s.revision();
    let reusable = Plan::capture(&mut s, 0, [1], [Type::Label]).unwrap();
    let report = Plan::capture(&mut s, 0, [1], [Type::Label])
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert!(!report.changed);
    assert_eq!(report.destinations_changed, 0);
    assert_eq!(s.serialize().unwrap(), before);
    assert_eq!(s.revision(), revision);
    assert_eq!(s.redo_label(), Some(redo.as_str()));
    assert!(
        s.grid(1)
            .unwrap()
            .get(Position {
                player: 0,
                lane: 3,
                row: 48
            })
            .is_some()
    );
    assert!(!reusable.apply(&mut s).unwrap().changed);
    // Repeat with a destination grid not loaded at capture; cache population
    // must not be mistaken for a resident note/participant edit.
    let bytes = s.serialize().unwrap();
    let mut fresh = Session::new(Document::parse(bytes, Format::Ssc).unwrap());
    assert!(fresh.loaded_grid(1).is_none());
    let plan = Plan::capture(&mut fresh, 0, [1], [Type::Label]).unwrap();
    assert!(fresh.loaded_grid(1).is_none());
    assert!(!plan.apply(&mut fresh).unwrap().changed);
}

#[test]
fn sm_copy_replaces_resident_timing_and_attacks_per_chart_without_activating_raw_unsupported_tags()
{
    use deadsync_editor::{
        attacks::{self, placement::Timeline},
        timing_edit::{Scope, TempoDraft},
    };
    let raw=b"#TITLE:Keep;#BPMS:0=120,4=180;#WARPS:1=999;#LABELS:1=Ignored;#ATTACKS:TIME=1:LEN=1:MODS=100% Drunk;#NOTES:dance-single::Hard:9::1000,0100;#NOTES:dance-single::Easy:2::0010,0001;#NOTES:dance-single::Medium:5::0100,0010;";
    let mut s = Session::new(Document::parse(raw.as_slice(), Format::Sm).unwrap());
    let mut tempo = TempoDraft::capture(&s, 1, Scope::Chart).unwrap();
    tempo.set(0, 240.).unwrap();
    tempo.apply(&mut s, 1).unwrap();
    let mut attack = attacks::Draft::capture(&s, 1, Scope::Chart).unwrap();
    let mut modifier = attack.attacks().events()[0].modifiers[0].clone();
    modifier.name = "Reverse".into();
    attack.attacks_mut().set(0, 0, 2., modifier).unwrap();
    attack.apply(&mut s, 1).unwrap();
    let source_tempos = TempoDraft::capture(&s, 0, Scope::Chart)
        .unwrap()
        .tempos()
        .clone();
    let source_attacks = attacks::Draft::capture(&s, 0, Scope::Chart)
        .unwrap()
        .attacks()
        .clone();
    let source_rows: Vec<_> = Timeline::capture(&s, 0)
        .unwrap()
        .markers()
        .iter()
        .map(|m| m.row)
        .collect();
    let before = s.serialize().unwrap();
    let target_notes = s.grid(1).unwrap().clone();
    let report = Plan::capture(&mut s, 0, [1], [Type::Tempo, Type::Attack])
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert!(report.changed);
    assert_eq!(report.destinations_changed, 1);
    assert!(report.shared_types_unchanged.is_empty());
    assert_eq!(
        TempoDraft::capture(&s, 1, Scope::Chart).unwrap().tempos(),
        &source_tempos
    );
    assert_eq!(
        attacks::Draft::capture(&s, 1, Scope::Chart)
            .unwrap()
            .attacks(),
        &source_attacks
    );
    assert_eq!(
        Timeline::capture(&s, 1)
            .unwrap()
            .markers()
            .iter()
            .map(|m| m.row)
            .collect::<Vec<_>>(),
        source_rows
    );
    assert_eq!(
        TempoDraft::capture(&s, 0, Scope::Chart).unwrap().tempos(),
        &source_tempos
    );
    assert_eq!(
        attacks::Draft::capture(&s, 0, Scope::Chart)
            .unwrap()
            .attacks(),
        &source_attacks
    );
    assert_eq!(
        TempoDraft::capture(&s, 2, Scope::Chart).unwrap().tempos(),
        &source_tempos
    );
    assert_eq!(
        attacks::Draft::capture(&s, 2, Scope::Chart)
            .unwrap()
            .attacks(),
        &source_attacks
    );
    assert_eq!(s.grid(1).unwrap(), &target_notes);
    assert_eq!(
        s.document().raw_property(None, "BPMS"),
        Some(b"0=120,4=180".as_slice())
    );
    assert_eq!(
        s.document().raw_property(None, "ATTACKS"),
        Some(b"TIME=1:LEN=1:MODS=100% Drunk".as_slice())
    );
    assert_eq!(
        s.document().raw_property(None, "WARPS"),
        Some(b"1=999".as_slice())
    );
    assert_eq!(
        s.document().raw_property(None, "LABELS"),
        Some(b"1=Ignored".as_slice())
    );
    let regions =
        deadsync_editor::timing_edit::region::Draft::capture(&s, 1, Scope::Chart).unwrap();
    assert!(
        regions
            .regions(deadsync_editor::timing_edit::region::Kind::Warp)
            .values()
            .is_empty()
    );
    assert!(
        deadsync_editor::timing_edit::label::Draft::capture(&s, 1, Scope::Chart)
            .unwrap()
            .labels()
            .values()
            .is_empty()
    );
    let after = s.serialize().unwrap();
    let reopened = Session::new(Document::parse(after.clone(), Format::Sm).unwrap());
    assert_eq!(
        attacks::Draft::capture(&reopened, 1, Scope::Chart)
            .unwrap()
            .attacks(),
        &source_attacks
    );
    assert_eq!(
        TempoDraft::capture(&reopened, 1, Scope::Chart)
            .unwrap()
            .tempos(),
        &source_tempos
    );
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
    s.redo().unwrap();
    assert_eq!(s.serialize().unwrap(), after);
}

#[test]
fn loaded_initial_nodes_and_copy_match_original_action_with_real_load_preconditions() {
    let request: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/chart-copy-loaded-initial.request.json"
    )).unwrap();
    let expected: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/chart-copy-loaded-initial.expected.json"
    )).unwrap();
    let case = &request["Cases"][0];
    let reference = &expected[0];
    assert_eq!(case["Name"], reference["Name"]);
    let kinds = ["Tempo".to_owned(), "Signature".to_owned()].into_iter().collect();
    let mut session = fixture_session(case);
    let source = fixture_projection(&mut session, 0, &kinds);
    assert_eq!(source, canonical(reference["SourceBefore"].as_array().unwrap().clone()));
    assert_eq!(fixture_projection(&mut session, 1, &kinds), canonical(reference["Before"][0].as_array().unwrap().clone()));
    let before = session.serialize().unwrap();
    let notes: Vec<_> = (0..2).map(|chart| session.grid(chart).unwrap().clone()).collect();
    Plan::capture(&mut session, 0, [1], [Type::Tempo, Type::Signature]).unwrap().apply(&mut session).unwrap();
    let after = session.serialize().unwrap();
    let expected_after = canonical(reference["After"][0].as_array().unwrap().clone());
    assert_eq!(fixture_projection(&mut session, 1, &kinds), expected_after);
    assert_eq!(fixture_projection(&mut session, 0, &kinds), source);
    for (chart, grid) in notes.iter().enumerate() { assert_eq!(session.grid(chart).unwrap(), grid); }
    let mut reopened = Session::new(Document::parse(after.clone(), Format::Ssc).unwrap());
    assert_eq!(fixture_projection(&mut reopened, 1, &kinds), expected_after);
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), before);
    session.redo().unwrap();
    assert_eq!(session.serialize().unwrap(), after);
}
