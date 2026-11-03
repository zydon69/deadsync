#[path = "support/source_output_context.rs"]
mod source_output_context;
use deadsync_editor::{
    attacks::{Modifier, inspector::Draft as AttackDraft},
    document::{Document, Format},
    event_clipboard::{Clipboard, Event, Selection, TimingKind},
    generation::metadata::{ChartData, Definition},
    history::Session,
    notes::{Kind, Note, Position, layout_columns},
    storage::FileSession,
    timing_edit::Scope,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};

fn p(player: u32, lane: u16, row: u64) -> Position {
    Position { player, lane, row }
}
fn empty(format: Format, layout: &str) -> Session {
    let notes = "0".repeat(layout_columns(layout).unwrap() as usize);
    let text = if format == Format::Sm {
        format!(
            "// retain\n#TITLE:Keep;#VENDOR:opaque;#BPMS:0=120;#NOTES:{layout}::Hard:9:0,0,0,0,0:{notes};"
        )
    } else {
        format!(
            "// retain\n#VERSION:0.83;#TITLE:Keep;#VENDOR:opaque;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:{layout};#DIFFICULTY:Hard;#METER:9;#CUSTOM:opaque;#NOTES:{notes};"
        )
    };
    Session::new(Document::parse(text.as_bytes(), format).unwrap())
}
fn definition(seed: i32, length: u64) -> Definition {
    Definition {
        pattern_config_guid: "03de57bd-4329-4de6-a722-a171c11fdd16".parse().unwrap(),
        performed_chart_config_guid: "41a7f76e-596a-4fbe-bf6e-6a77a898e4a1".parse().unwrap(),
        random_seed: seed,
        length,
        start_position_inclusive: true,
        end_position_inclusive: false,
        ignore_preceding_distribution: true,
        ..Default::default()
    }
}
fn markers(s: &mut Session, values: impl IntoIterator<Item = (u64, Definition)>) {
    let mut data = ChartData::existing_or_initial(s.document(), 0).unwrap();
    data.patterns = Some(values.into_iter().collect());
    data.save(s, 0).unwrap();
}
fn choose(
    s: &mut Session,
    start: u64,
    end: u64,
    mut wanted: impl FnMut(Event) -> bool,
) -> Selection {
    let mut selection = Selection::capture(s, 0, start, end).unwrap();
    selection.select_all(false);
    for i in 0..selection.events().len() {
        if wanted(selection.events()[i]) {
            selection.set_selected(i, true).unwrap();
        }
    }
    selection
}
fn pattern_clip(s: &mut Session, start: u64, end: u64) -> Clipboard {
    choose(s, start, end, |event| {
        matches!(event, Event::Pattern { .. })
    })
    .copy(s)
    .unwrap()
    .unwrap()
}

#[test]
fn detached_definition_preserves_every_field_extras_destination_settings_and_saved_history() {
    for format in [Format::Sm, Format::Ssc] {
        let mut source = empty(format, "dance-single");
        let mut copied = definition(-77, 144);
        copied.extra.insert(
            "Future".into(),
            json!({"nested": [1, "é", {"enabled": true}]}),
        );
        markers(&mut source, [(48, copied.clone()), (96, definition(9, 24))]);
        let clipboard = pattern_clip(&mut source, 48, 48);
        markers(&mut source, [(48, definition(999, 1))]); // source edits cannot change the detached value
        let source_after = source.serialize().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(format!("song.{}", format.extension()));
        let mut target = empty(format, "dance-double");
        let mut data = ChartData::default();
        data.music_offset = 0.375;
        data.should_use_chart_music_offset = true;
        data.extra.insert("FutureChart".into(), json!([1, 2, 3]));
        data.patterns = Some(BTreeMap::from([
            (144, definition(8, 192)),
            (192, definition(7, 24)),
        ]));
        data.save(&mut target, 0).unwrap();
        target
            .edit_notes(0, "Keep note", [(p(0, 0, 192), Some(Note::tap()))])
            .unwrap();
        let before = target.serialize().unwrap();
        let before_grid = target.grid(0).unwrap().clone();
        let report = clipboard.paste_with_report(&mut target, 0, 192, 0).unwrap();
        assert!(report.changed);
        assert_eq!(report.patterns_pasted, 1);
        assert_eq!(report.patterns_skipped, 0);
        assert_eq!(target.grid(0).unwrap(), &before_grid);
        let result = ChartData::read(target.document(), 0).unwrap().unwrap();
        assert_eq!(result.patterns.as_ref().unwrap()[&192], copied);
        assert_eq!(result.patterns.as_ref().unwrap()[&144], definition(8, 192));
        assert_eq!(result.extra, data.extra);
        assert_eq!(result.expressed_chart_config, data.expressed_chart_config);
        assert_eq!(result.music_offset, data.music_offset);
        assert_eq!(result.max_player_index, data.max_player_index);
        assert_eq!(source.serialize().unwrap(), source_after);
        let after = target.serialize().unwrap();
        target.undo().unwrap();
        assert_eq!(target.serialize().unwrap(), before);
        target.redo().unwrap();
        assert_eq!(target.serialize().unwrap(), after);
        std::fs::write(&path, &after).unwrap();
        let mut reopened = FileSession::open(&path).unwrap();
        reopened.save().unwrap();
        assert_eq!(reopened.session.serialize().unwrap(), after);
        assert_eq!(
            ChartData::read(reopened.session.document(), 0)
                .unwrap()
                .unwrap()
                .patterns
                .unwrap()[&192],
            copied
        );
        let published = std::fs::read(&path).unwrap();
        source_output_context::assert_context(
            &after,
            &published,
            format,
            source_output_context::Context {
                sync_offset: 0.009,
                chart_type: "dance_double",
                chart_index: 0,
            },
        );
        let mut actual_reopen = FileSession::open(&path).unwrap();
        assert_eq!(actual_reopen.session.serialize().unwrap(), published);
        assert_eq!(actual_reopen.session.grid(0).unwrap(), &before_grid);
        assert_eq!(
            ChartData::read(actual_reopen.session.document(), 0)
                .unwrap()
                .unwrap()
                .patterns
                .unwrap()[&192],
            copied
        );
    }
}

#[test]
fn cutting_and_deleting_marker_heads_never_clear_their_generated_notes() {
    for format in [Format::Sm, Format::Ssc] {
        let mut source = empty(format, "dance-single");
        markers(
            &mut source,
            [(0, definition(42, 96)), (48, definition(5, 48))],
        );
        source
            .edit_notes(
                0,
                "Generated notes",
                [
                    (p(0, 0, 0), Some(Note::tap())),
                    (p(0, 1, 48), Some(Note::tap())),
                ],
            )
            .unwrap();
        let before = source.serialize().unwrap();
        let grid = source.grid(0).unwrap().clone();
        let selected = choose(&mut source, 0, 0, |event| {
            matches!(event, Event::Pattern { .. })
        });
        let clip = selected.cut(&mut source).unwrap().unwrap();
        assert_eq!(source.grid(0).unwrap(), &grid);
        assert!(
            !ChartData::read(source.document(), 0)
                .unwrap()
                .unwrap()
                .patterns
                .unwrap()
                .contains_key(&0)
        );
        assert!(selected.copy(&mut source).is_err()); // selection token is stale after cutting
        source.undo().unwrap();
        assert_eq!(source.serialize().unwrap(), before);
        let delete = choose(&mut source, 48, 48, |event| {
            matches!(event, Event::Pattern { .. })
        });
        assert!(delete.delete(&mut source).unwrap());
        assert_eq!(source.grid(0).unwrap(), &grid);
        assert!(clip.paste(&mut source, 0, 192, 0).unwrap());
        assert_eq!(
            ChartData::read(source.document(), 0)
                .unwrap()
                .unwrap()
                .patterns
                .unwrap()[&192],
            definition(42, 96)
        );
        let inside = Selection::capture(&mut source, 0, 193, 200).unwrap();
        assert!(
            !inside
                .events()
                .iter()
                .any(|event| matches!(event, Event::Pattern { .. }))
        ); // no clipping/invented head
    }
}

#[test]
fn unsupported_multiplayer_destinations_skip_only_markers_and_report_mixed_results() {
    for layout in [
        "dance-routine",
        "pump-routine",
        "smx-team",
        "dance-couple",
        "pump-couple",
    ] {
        let canonical = match layout {
            "dance-routine" | "dance-couple" => "dance-double",
            "pump-routine" | "pump-couple" => "pump-double",
            _ => "smx-full",
        };
        let mut source = empty(Format::Ssc, canonical);
        markers(&mut source, [(48, definition(42, 96))]);
        source
            .edit_notes(0, "Copy note", [(p(0, 0, 0), Some(Note::tap()))])
            .unwrap();
        let clip = choose(&mut source, 0, 48, |event| {
            matches!(event, Event::Note(_) | Event::Pattern { .. })
        })
        .copy(&mut source)
        .unwrap()
        .unwrap();
        let mut target = empty(Format::Ssc, layout);
        if deadsync_editor::notes::supports_variable_players(layout) {
            ChartData::default().save(&mut target, 0).unwrap();
            assert_eq!(target.grid(0).unwrap().players(), 1);
        }
        let before = target.serialize().unwrap();
        let report = clip.paste_with_report(&mut target, 0, 192, 0).unwrap();
        assert!(report.changed);
        assert_eq!(report.patterns_skipped, 1);
        assert_eq!(report.patterns_pasted, 0);
        assert!(target.grid(0).unwrap().get(p(0, 0, 192)).is_some());
        assert!(
            ChartData::read(target.document(), 0)
                .unwrap()
                .is_none_or(|data| data.patterns.is_none())
        );
        target.undo().unwrap();
        assert_eq!(target.serialize().unwrap(), before);
        let only = pattern_clip(&mut source, 48, 48);
        let revision = target.revision();
        let report = only.paste_with_report(&mut target, 0, 192, 0).unwrap();
        assert!(!report.changed);
        assert_eq!(report.patterns_skipped, 1);
        assert_eq!(target.revision(), revision);
        assert_eq!(target.serialize().unwrap(), before);
    }
}

#[test]
fn pattern_overflow_and_invalid_destination_metadata_roll_back_without_losing_redo() {
    let mut source = empty(Format::Ssc, "dance-single");
    markers(&mut source, [(0, definition(42, 96))]);
    let only = pattern_clip(&mut source, 0, 0);
    let mut target = empty(Format::Ssc, "dance-single");
    target.set_text(None, "TITLE", "Undo me").unwrap();
    target.undo().unwrap();
    let before = target.serialize().unwrap();
    let redo = target.redo_label().map(str::to_owned);
    let revision = target.revision();
    assert!(only.paste(&mut target, 0, i32::MAX as u64 - 12, 0).is_err());
    assert_eq!(target.serialize().unwrap(), before);
    assert_eq!(target.redo_label(), redo.as_deref());
    assert_eq!(target.revision(), revision);
    source
        .edit_notes(0, "Copy note", [(p(0, 0, 0), Some(Note::tap()))])
        .unwrap();
    let mixed = choose(&mut source, 0, 0, |event| {
        matches!(event, Event::Note(_) | Event::Pattern { .. })
    })
    .copy(&mut source)
    .unwrap()
    .unwrap();
    target
        .set_text(Some(0), "GAChartDataVersion", "999")
        .unwrap();
    let before = target.serialize().unwrap();
    let revision = target.revision();
    assert!(mixed.paste(&mut target, 0, 192, 0).is_err());
    assert_eq!(target.serialize().unwrap(), before);
    assert_eq!(target.revision(), revision);
    assert!(target.grid(0).unwrap().get(p(0, 0, 192)).is_none());
}

#[test]
fn stable_selection_order_preserves_original_phases_without_nontransitive_comparison() {
    let mut source = empty(Format::Ssc, "dance-single");
    markers(&mut source, [(0, definition(42, 96))]);
    source
        .edit_notes(
            0,
            "Same row",
            [
                (p(0, 1, 0), Some(Note::tap())),
                (
                    p(0, 0, 0),
                    Some(Note {
                        kind: Kind::Hold,
                        end: Some(96),
                        attributes: "".into(),
                        tail_attributes: "".into(),
                    }),
                ),
            ],
        )
        .unwrap();
    source.set_text(Some(0), "DELAYS", "0=0.25").unwrap();
    add_attack(&mut source, Format::Ssc, 0);
    let selection = Selection::capture(&mut source, 0, 0, 0).unwrap();
    let kinds: Vec<_> = selection
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::Attack { .. } => Some("attack"),
            Event::Timing {
                kind: TimingKind::Delay,
                ..
            } => Some("delay"),
            Event::Pattern { .. } => Some("pattern"),
            Event::Note(position) => Some(if position.lane == 1 { "tap" } else { "hold" }),
            _ => None,
        })
        .collect();
    assert_eq!(kinds, ["attack", "delay", "tap", "pattern", "hold"]);
    let second = Selection::capture(&mut source, 0, 0, 0).unwrap();
    assert_eq!(selection.events(), second.events());
}

fn populate(format: Format, layout: &str, players: u32, values: &[Value]) -> Session {
    let mut session = empty(format, layout);
    if deadsync_editor::notes::supports_variable_players(layout) {
        session.set_participants(0, players).unwrap();
    }
    let notes: Vec<_> = values
        .iter()
        .filter_map(|value| {
            let kind = value["Kind"].as_str().unwrap();
            if !matches!(kind, "Tap" | "Hold") {
                return None;
            }
            let position = p(
                value["Player"].as_u64().unwrap_or(0) as u32,
                value["Lane"].as_u64().unwrap_or(0) as u16,
                value["Row"].as_u64().unwrap(),
            );
            Some((
                position,
                Some(Note {
                    kind: if kind == "Hold" {
                        Kind::Hold
                    } else {
                        Kind::Tap
                    },
                    end: value.get("End").and_then(Value::as_u64),
                    attributes: "".into(),
                    tail_attributes: "".into(),
                }),
            ))
        })
        .collect();
    if !notes.is_empty() {
        session.edit_notes(0, "Fixture notes", notes).unwrap();
    }
    let definitions: Vec<_> = values
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
        markers(&mut session, definitions);
    }
    let scope = (format == Format::Ssc).then_some(0);
    for value in values {
        let beat = value["Row"].as_u64().unwrap() as f64 / 48.0;
        match value["Kind"].as_str().unwrap() {
            "Label" => {
                session
                    .set_text(scope, "LABELS", &format!("{beat}=marker"))
                    .unwrap();
            }
            "Delay" => {
                session
                    .set_text(scope, "DELAYS", &format!("{beat}=0.25"))
                    .unwrap();
            }
            "Attack" => add_attack(&mut session, format, value["Row"].as_u64().unwrap()),
            _ => {}
        }
    }
    session
}
fn add_attack(session: &mut Session, format: Format, row: u64) {
    let scope = if format == Format::Ssc {
        Scope::Chart
    } else {
        Scope::Song
    };
    let mut attacks = AttackDraft::capture(session, 0, scope).unwrap();
    attacks
        .add(
            row,
            Modifier {
                name: "Drunk".into(),
                length: 1.0,
                ..Modifier::default()
            },
        )
        .unwrap();
    attacks.apply(session, 0).unwrap();
}
fn projection(session: &mut Session) -> Vec<Value> {
    let selection = Selection::capture(session, 0, 0, i32::MAX as u64).unwrap();
    let data = ChartData::read(session.document(), 0).unwrap();
    selection.events().iter().filter_map(|event| match event {
        Event::Pattern {row} => Some(json!({"Kind":"Pattern","Row":row,"Definition":data.as_ref().unwrap().patterns.as_ref().unwrap()[row]})),
        Event::Note(position) => { let note = session.grid(0).unwrap().get(*position).unwrap(); Some(json!({"Kind":if note.end.is_some(){"LaneHoldStartNote"}else{"LaneTapNote"},"Row":position.row,"Lane":position.lane,"Player":position.player,"End":note.end})) },
        Event::Timing {row,kind: TimingKind::Label} => Some(json!({"Kind":"Label","Row":row,"Lane":-1,"Player":0,"End":null})),
        Event::Timing {row,kind: TimingKind::Delay} => Some(json!({"Kind":"Stop","Row":row,"Lane":-1,"Player":0,"End":null})),
        Event::Attack {row} => Some(json!({"Kind":"Attack","Row":row,"Lane":-1,"Player":0,"End":null})), _ => None,
    }).collect()
}
fn canonical(mut values: Vec<Value>) -> Vec<Value> {
    values.sort_by_key(|value| serde_json::to_string(value).unwrap());
    values
}

#[test]
fn fifteen_original_action_and_force_add_cases_match_pattern_business_results_in_sm_and_ssc() {
    let input: Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/pattern-clipboard.request.json"
    ))
    .unwrap();
    let expected: Vec<Value> = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/pattern-clipboard.expected.json"
    ))
    .unwrap();
    assert_eq!(input["Cases"].as_array().unwrap().len(), 15);
    assert_eq!(expected.len(), 15);
    for format in [Format::Sm, Format::Ssc] {
        for (case, expected) in input["Cases"].as_array().unwrap().iter().zip(&expected) {
            assert_eq!(case["Name"], expected["Name"]);
            assert_eq!(expected["ClonesMatchedBefore"], true);
            assert_eq!(expected["ClonesIndependentAfter"], true);
            // The original action is format-independent memory behavior. Its Label
            // fixture can only round-trip through SSC: original Fumen SMWriter
            // emits no LABELS, and Rust deliberately retains that export contract.
            if format == Format::Sm && case["Name"] == "pattern-replaces-same-head" {
                assert!(
                    case["Destination"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|e| e["Kind"] == "Label")
                );
                assert!(
                    expected["Before"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|e| e["Kind"] == "Label")
                );
                continue;
            }
            let layout = case["Layout"].as_str().unwrap();
            let copied = case["Copied"].as_array().unwrap();
            let owners = copied
                .iter()
                .filter_map(|value| value["Player"].as_u64())
                .max()
                .unwrap_or(0) as u32
                + 1;
            let widest = copied
                .iter()
                .filter_map(|value| value["Lane"].as_u64())
                .max()
                .unwrap_or(0)
                + 1;
            let source_layout = if owners > 1 {
                "dance-routine"
            } else if widest > 4 && layout == "dance-single" {
                "dance-double"
            } else {
                match layout {
                    "dance-routine" | "dance-couple" => "dance-double",
                    "pump-routine" => "pump-double",
                    "smx-team" => "smx-full",
                    _ => layout,
                }
            };
            let mut source = populate(format, source_layout, owners, copied);
            let desired: Vec<_> = copied
                .iter()
                .map(|value| match value["Kind"].as_str().unwrap() {
                    "Pattern" => Event::Pattern {
                        row: value["Row"].as_u64().unwrap(),
                    },
                    "Tap" | "Hold" => Event::Note(p(
                        value["Player"].as_u64().unwrap_or(0) as u32,
                        value["Lane"].as_u64().unwrap_or(0) as u16,
                        value["Row"].as_u64().unwrap(),
                    )),
                    "Attack" => Event::Attack {
                        row: value["Row"].as_u64().unwrap(),
                    },
                    "Delay" => Event::Timing {
                        row: value["Row"].as_u64().unwrap(),
                        kind: TimingKind::Delay,
                    },
                    _ => unreachable!(),
                })
                .collect();
            let selected = choose(&mut source, 0, 256, |event| desired.contains(&event));
            assert_eq!(
                selected.selected_count(),
                desired.len(),
                "{} {format:?}",
                case["Name"]
            );
            let clipboard = selected.copy(&mut source).unwrap().unwrap();
            let mut target = populate(
                format,
                layout,
                case["Players"].as_u64().unwrap() as u32,
                case["Destination"].as_array().unwrap(),
            );
            assert_eq!(
                canonical(projection(&mut target)),
                canonical(expected["Before"].as_array().unwrap().clone()),
                "{} before",
                case["Name"]
            );
            let before = target.serialize().unwrap();
            let report = clipboard
                .paste_with_report(
                    &mut target,
                    0,
                    case["PasteRow"].as_u64().unwrap(),
                    case["ActivePlayer"].as_u64().unwrap() as u32,
                )
                .unwrap();
            assert_eq!(
                report.notes_skipped,
                usize::from(case["Name"] == "cross-layout-skips-only-invalid-lanes"),
                "{} skipped lanes",
                case["Name"]
            );
            assert_eq!(
                canonical(projection(&mut target)),
                canonical(expected["After"].as_array().unwrap().clone()),
                "{} after {format:?}",
                case["Name"]
            );
            let after = target.serialize().unwrap();
            if report.changed {
                target.undo().unwrap();
                assert_eq!(target.serialize().unwrap(), before);
                assert_eq!(
                    canonical(projection(&mut target)),
                    canonical(expected["Undone"].as_array().unwrap().clone())
                );
                target.redo().unwrap();
                assert_eq!(target.serialize().unwrap(), after);
            }
            let mut reopened =
                Session::new(Document::parse(Arc::<[u8]>::from(after), format).unwrap());
            assert_eq!(
                canonical(projection(&mut reopened)),
                canonical(expected["Redone"].as_array().unwrap().clone()),
                "{} reload",
                case["Name"]
            );
        }
    }
}
