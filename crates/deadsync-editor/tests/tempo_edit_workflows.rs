use deadsync_editor::{
    analysis,
    document::{Document, Format},
    history::Session,
    notes::{Note, Position},
    storage::FileSession,
    timing_edit::{
        Scope, TIMING_TAGS, TempoDraft,
        tempo::{MIN_BPM, Tempos},
    },
    workspace::{Command, Workspace},
};
use serde_json::Value;
use std::{fs, path::PathBuf, sync::Arc};
fn assert_values(tempos: &Tempos, expected: &Value, name: &str) {
    let expected = expected.as_array().unwrap();
    assert_eq!(tempos.values().len(), expected.len(), "{name}");
    for ((row, bpm), expected) in tempos.values().iter().zip(expected) {
        assert_eq!(*row, expected["Row"].as_u64().unwrap(), "{name} row");
        assert_eq!(
            bpm.to_bits(),
            expected["Bpm"].as_f64().unwrap().to_bits(),
            "{name} BPM at {row}"
        );
    }
}
#[test]
fn tempo_loading_and_edits_match_pinned_fumen_and_original_setter() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    let mut count = 0;
    for file in fs::read_dir(&root).unwrap() {
        let path = file.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("tempo-edit-") || !name.ends_with(".request.json") {
            continue;
        }
        count += 1;
        let request: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let expected: Value = serde_json::from_slice(
            &fs::read(root.join(name.replace(".request.json", ".expected.json"))).unwrap(),
        )
        .unwrap();
        let text = request["Segments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pair| format!("{}={}", pair[0], pair[1]))
            .collect::<Vec<_>>()
            .join(",");
        let mut tempos = Tempos::parse(&text, request["Fallback"].as_f64().unwrap()).unwrap();
        assert_values(&tempos, &expected["Initial"], &name);
        for (operation, expected) in request["Operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(expected["Stages"].as_array().unwrap())
        {
            let row = operation[1].as_u64().unwrap();
            let result = match operation[0].as_str().unwrap() {
                "set" => tempos.set(row, operation[2].as_f64().unwrap()),
                "move" => tempos.move_node(row, operation[2].as_u64().unwrap()),
                "remove" => tempos.remove(row),
                _ => panic!("Unknown tempo operation"),
            };
            if expected["Error"].is_null() {
                assert_eq!(
                    result.unwrap(),
                    expected["Changed"].as_bool().unwrap(),
                    "{name} changed"
                );
            } else {
                assert!(result.is_err(), "{name} invalid operation");
            }
            assert_values(
                &tempos,
                &expected["Values"],
                &format!("{name} after {operation}"),
            );
            assert_eq!(
                Tempos::parse(&tempos.serialize(), 120.0).unwrap().values(),
                tempos.values(),
                "{name} serialized map"
            );
        }
    }
    assert_eq!(count, 15);
}
const SSC:&[u8]=b"// preserve source\n#VERSION:0.83;#TITLE:Timing;#VENDOR:keep;#OFFSET:0.25;#BPMS:0=120,4=180;#STOPS:2=0.25;#DELAYS:3=0.125;#WARPS:8=1;#TIMESIGNATURES:0=4=4,16=3=4;#TICKCOUNTS:0=4;#COMBOS:0=1=1;#SPEEDS:0=1=0=0,6=2=1=0;#SCROLLS:0=1,4=0.5;#FAKES:12=1;#LABELS:1=caf\xe9\\;keep;\n#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#VENDOR_CHART:first;#NOTES:1000\n0100\n0010\n0001;\n#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=150;#OFFSET:-0.125;#STOPS:1=0.1;#NOTES:1000\n0100\n0010\n0001;\n#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001;";
fn session(bytes: &[u8], format: Format) -> Session {
    Session::new(Document::parse(Arc::<[u8]>::from(bytes), format).unwrap())
}
fn signature(source: &Session, chart: usize) -> String {
    format!(
        "{:?}",
        analysis::chart(source, chart, &rssp::AnalysisOptions::default())
            .unwrap()
            .timing_segments
    )
}
#[test]
fn first_chart_bpm_edit_detaches_all_inherited_timing_bytes_in_one_undoable_gesture() {
    let mut source = session(SSC, Format::Ssc);
    let mut draft = TempoDraft::capture(&source, 0, Scope::Chart).unwrap();
    assert!(draft.inherited());
    assert_eq!(draft.affected_charts(), [0]);
    let before = source.serialize().unwrap();
    let other = [signature(&source, 1), signature(&source, 2)];
    let before_target = analysis::chart(&source, 0, &rssp::AnalysisOptions::default()).unwrap();
    draft.set(0, 160.0).unwrap();
    draft.set(96, 200.0).unwrap();
    assert!(draft.apply(&mut source, 0).unwrap());
    assert_eq!(source.undo_label(), Some("Edit BPM timing"));
    for tag in TIMING_TAGS {
        if tag != "BPMS" {
            assert_eq!(
                source.document().raw_property(Some(0), tag),
                source.document().raw_property(None, tag),
                "Copied {tag}"
            );
        }
    }
    assert_eq!(
        source.document().raw_property(None, "BPMS"),
        Some(b"0=120,4=180".as_slice())
    );
    assert_eq!(
        source.document().text(Some(0), "BPMS").as_deref(),
        Some("0=160,2=200,4=180")
    );
    let after_target = analysis::chart(&source, 0, &rssp::AnalysisOptions::default()).unwrap();
    assert_eq!(
        before_target.timing_segments.stops,
        after_target.timing_segments.stops
    );
    assert_eq!(
        before_target.timing_segments.delays,
        after_target.timing_segments.delays
    );
    assert_eq!(
        before_target.timing_segments.warps,
        after_target.timing_segments.warps
    );
    assert_eq!(
        before_target.timing_segments.speeds,
        after_target.timing_segments.speeds
    );
    assert_eq!(
        before_target.timing_segments.scrolls,
        after_target.timing_segments.scrolls
    );
    assert_eq!(
        before_target.timing_segments.fakes,
        after_target.timing_segments.fakes
    );
    assert_eq!([signature(&source, 1), signature(&source, 2)], other);
    let after = source.serialize().unwrap();
    let mut reopened = session(&after, Format::Ssc);
    for chart in 0..3 {
        assert_eq!(source.grid(chart).unwrap(), reopened.grid(chart).unwrap());
    }
    source.undo().unwrap();
    assert_eq!(source.serialize().unwrap(), before);
    source.redo().unwrap();
    assert_eq!(source.serialize().unwrap(), after);
}
#[test]
fn chart_with_own_timing_does_not_pull_song_stops_or_offset_and_song_scope_affects_only_inheritors()
{
    let mut source = session(SSC, Format::Ssc);
    let global = source
        .document()
        .raw_property(None, "BPMS")
        .unwrap()
        .to_vec();
    let mut draft = TempoDraft::capture(&source, 1, Scope::Chart).unwrap();
    assert!(!draft.inherited());
    draft.set(0, 175.0).unwrap();
    draft.apply(&mut source, 1).unwrap();
    assert_eq!(
        source.document().raw_property(None, "BPMS").unwrap(),
        global
    );
    assert_eq!(
        source.document().text(Some(1), "OFFSET").as_deref(),
        Some("-0.125")
    );
    assert_eq!(
        source.document().text(Some(1), "STOPS").as_deref(),
        Some("1=0.1")
    );
    assert!(source.document().raw_property(Some(1), "DELAYS").is_none());
    let own = signature(&source, 1);
    let mut song = TempoDraft::capture(&source, 0, Scope::Song).unwrap();
    assert_eq!(song.affected_charts(), [0, 2]);
    song.set(0, 200.0).unwrap();
    song.apply(&mut source, 0).unwrap();
    assert_eq!(signature(&source, 1), own);
    assert_eq!(
        TempoDraft::capture(&source, 2, Scope::Chart)
            .unwrap()
            .tempos()
            .at(0),
        200.0
    );
}
#[test]
fn sm_shared_timing_saves_and_reopens_with_pending_notes_and_one_property_history_action() {
    let bytes=b"#TITLE:SM;#BPMS:0=120;#STOPS:1=0.2;#NOTES:dance-single::Hard:9::1000\n0100\n0010\n0001;#NOTES:dance-single::Easy:2::1000;";
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.sm");
    fs::write(&path, bytes).unwrap();
    let mut file = FileSession::open(&path).unwrap();
    assert!(TempoDraft::capture(&file.session, 0, Scope::Chart).unwrap().inherited());
    file.session
        .edit_notes(
            1,
            "Pending note",
            [(
                Position {
                    player: 0,
                    row: 48,
                    lane: 3,
                },
                Some(Note::tap()),
            )],
        )
        .unwrap();
    let before = file.session.serialize().unwrap();
    let mut workspace = Workspace::from_file(file, 0).unwrap();
    let mut draft = workspace.tempo_draft(Scope::Song).unwrap();
    assert_eq!(draft.affected_charts(), [0, 1]);
    draft.set(0, 187.3).unwrap();
    draft.set(144, 200.0).unwrap();
    draft.move_node(144, 192).unwrap();
    workspace.execute(Command::SelectChart(1)).unwrap();
    assert!(workspace.apply_tempo(&draft).is_err());
    workspace.execute(Command::SelectChart(0)).unwrap();
    workspace.apply_tempo(&draft).unwrap();
    let after = workspace.file().session.serialize().unwrap();
    workspace.execute(Command::Save).unwrap();
    let reopened = FileSession::open(&path).unwrap();
    assert_eq!(
        TempoDraft::capture(&reopened.session,0,Scope::Song).unwrap().tempos().serialize(),
        "0=187.3,4=200"
    );
    assert_eq!(
        reopened.session.document().text(None, "STOPS").as_deref(),
        Some("1=0.2")
    );
    assert_eq!(reopened.session.document().text(None,"BPMS").as_deref(),Some("0=120"),"native owner does not overwrite original SM timing tags");
    workspace.execute(Command::Undo).unwrap();
    assert_eq!(workspace.file().session.serialize().unwrap(), before);
    workspace.execute(Command::Redo).unwrap();
    assert_eq!(workspace.file().session.serialize().unwrap(), after);
}
#[test]
fn no_op_foreign_stale_wrong_chart_and_invalid_values_do_not_change_source_or_redo() {
    let mut source = session(SSC, Format::Ssc);
    source.set_text(None, "TITLE", "Redo").unwrap();
    source.undo().unwrap();
    let before = source.serialize().unwrap();
    let label = source.redo_label().unwrap().to_owned();
    let mut draft = TempoDraft::capture(&source, 0, Scope::Chart).unwrap();
    assert!(!draft.apply(&mut source, 0).unwrap());
    assert_eq!(source.redo_label(), Some(label.as_str()));
    for value in [0.0, -1.0, f64::NAN, f64::INFINITY, MIN_BPM * 0.5, 1e308] {
        assert!(draft.set(0, value).is_err());
        assert!(!draft.has_changes());
    }
    assert!(draft.remove(0).is_err());
    assert!(draft.move_node(0, 96).is_err());
    assert!(draft.move_node(192, 0).is_err());
    assert!(draft.set(i32::MAX as u64 + 1, 120.0).is_err());
    assert_eq!(source.serialize().unwrap(), before);
    draft.set(0, 160.0).unwrap();
    let mut foreign = session(SSC, Format::Ssc);
    assert!(draft.apply(&mut foreign, 0).is_err());
    assert!(draft.apply(&mut source, 1).is_err());
    source.set_text(None, "TITLE", "Changed").unwrap();
    let changed = source.serialize().unwrap();
    assert!(draft.apply(&mut source, 0).is_err());
    assert_eq!(source.serialize().unwrap(), changed);
}
#[test]
fn legacy_and_offset_only_scopes_are_explicit_and_fallback_uses_preceding_chart_histogram() {
    let legacy = SSC
        .windows(b"0.83".len())
        .position(|bytes| bytes == b"0.83")
        .unwrap();
    let mut bytes = SSC.to_vec();
    bytes[legacy..legacy + 4].copy_from_slice(b"0.60");
    let source = session(&bytes, Format::Ssc);
    assert!(TempoDraft::capture(&source, 0, Scope::Chart).is_err());
    assert!(TempoDraft::capture(&source, 0, Scope::Song).is_err());
    assert!(deadsync_editor::timing_edit::migration::has_legacy_chart_timing(source.document()));
    assert_eq!(source.serialize().unwrap(), bytes);
    let source=session(b"#VERSION:0.83;#BPMS:0=187.3;#STOPS:1=1;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=150;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#OFFSET:0.25;#NOTES:1000;",Format::Ssc);
    let draft = TempoDraft::capture(&source, 1, Scope::Chart).unwrap();
    assert!(!draft.inherited());
    assert_eq!(draft.tempos().at(0), 150.0);
    let source = session(
        b"#VERSION:0.83;#DISPLAYBPM:143.5:190;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
        Format::Ssc,
    );
    assert_eq!(
        TempoDraft::capture(&source, 0, Scope::Chart)
            .unwrap()
            .tempos()
            .at(0),
        143.5
    );
    let source=session(b"#VERSION:0.83;#BPMS:0=187.3;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=150;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=180;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#OFFSET:0.25;#NOTES:1000;",Format::Ssc);
    assert_eq!(
        TempoDraft::capture(&source, 2, Scope::Chart)
            .unwrap()
            .tempos()
            .at(0),
        180.0
    );
    let source=session(b"#VERSION:0.83;#BPMS:0=187.3 // comment\n;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:4=180;#NOTES:1000;",Format::Ssc);
    assert_eq!(
        TempoDraft::capture(&source, 0, Scope::Chart)
            .unwrap()
            .tempos()
            .at(0),
        187.3
    );
}

#[test]
fn missing_initial_tempo_uses_chart_type_group_order_for_tied_counts() {
    let source = session(
        b"#VERSION:0.83;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=150;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-double;#BPMS:0=180;#NOTES:10000000;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=210;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#OFFSET:0;#NOTES:1000;",
        Format::Ssc,
    );
    // Original enumeration is single(150,210), double(180), so the last
    // newly inserted histogram key is 180 despite file order ending at 210.
    assert_eq!(
        TempoDraft::capture(&source, 3, Scope::Chart)
            .unwrap()
            .tempos()
            .at(0),
        180.0
    );
}

#[test]
fn atomic_property_validation_still_rejects_structural_or_duplicate_fields_without_erasing_redo() {
    let mut source = session(SSC, Format::Ssc);
    source.set_text(None, "TITLE", "Redo this").unwrap();
    source.undo().unwrap();
    let before = source.serialize().unwrap();
    let redo = source.redo_label().unwrap().to_owned();
    for properties in [
        vec![("TITLE", "Changed"), ("NOTES", "1000")],
        vec![("TITLE", "Changed"), ("Title", "Again")],
    ] {
        assert!(
            source
                .set_properties_atomic(None, &properties, "Invalid property gesture")
                .is_err()
        );
        assert_eq!(source.serialize().unwrap(), before);
        assert_eq!(source.redo_label(), Some(redo.as_str()));
    }
    assert!(source.redo().unwrap());
}

fn check_file_reader(source: &Session, expected: &Value, name: &str) {
    use deadsync_editor::generation::timing::GenerationTiming;
    let bytes = source.serialize().unwrap();
    assert_eq!(
        source.document().charts().len(),
        expected.as_array().unwrap().len()
    );
    for (index, chart) in expected.as_array().unwrap().iter().enumerate() {
        let scope = if source.document().format == Format::Ssc {
            Scope::Chart
        } else {
            Scope::Song
        };
        let draft = TempoDraft::capture(source, index, scope).unwrap();
        assert_values(draft.tempos(), &chart["Bpms"], name);
        let offset = source
            .document()
            .text(Some(index), "OFFSET")
            .or_else(|| source.document().text(None, "OFFSET"))
            .unwrap_or_else(|| "0".into())
            .parse::<f64>()
            .unwrap();
        assert_eq!(
            offset,
            chart["Offset"].as_f64().unwrap(),
            "{name} chart {index} offset"
        );
        let rows = chart["Notes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|note| note["Row"].as_u64().unwrap())
            .collect::<Vec<_>>();
        let times = GenerationTiming::from_simfile(&bytes, source.document().format, index)
            .unwrap()
            .times(rows)
            .unwrap();
        for (time, note) in times.iter().zip(chart["Notes"].as_array().unwrap()) {
            assert_eq!(
                *time,
                note["Time"].as_f64().unwrap(),
                "{name} chart {index} row {}",
                note["Row"]
            );
        }
    }
}
#[test]
fn scoped_tempo_edits_match_actual_original_reader_before_and_after_export() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    let mut count = 0;
    for entry in fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("tempo-file-") || !name.ends_with(".request.json") {
            continue;
        }
        count += 1;
        let request: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let expected: Value = serde_json::from_slice(
            &fs::read(root.join(name.replace(".request.json", ".expected.json"))).unwrap(),
        )
        .unwrap();
        let format = if request["Format"] == "ssc" {
            Format::Ssc
        } else {
            Format::Sm
        };
        let chart = request["Chart"].as_u64().unwrap() as usize;
        let scope = if request["Scope"] == "Chart" {
            Scope::Chart
        } else {
            Scope::Song
        };
        let mut source = session(request["Content"].as_str().unwrap().as_bytes(), format);
        check_file_reader(&source, &expected["Before"], &name);
        let before = source.serialize().unwrap();
        if request["Scope"] == "Offset" {
            let mut draft =
                deadsync_editor::chart_properties::Draft::capture(&source, chart).unwrap();
            draft
                .set_music_offset(request["Operations"][0][2].as_f64().unwrap())
                .unwrap();
            draft.set_use_chart_offset(true);
            draft.apply(&mut source, chart).unwrap();
        } else {
            let mut draft = TempoDraft::capture(&source, chart, scope).unwrap();
            for operation in request["Operations"].as_array().unwrap() {
                assert_eq!(operation[0], "set");
                draft
                    .set(
                        operation[1].as_u64().unwrap(),
                        operation[2].as_f64().unwrap(),
                    )
                    .unwrap();
            }
            draft.apply(&mut source, chart).unwrap();
        }
        check_file_reader(&source, &expected["After"], &name);
        let after = source.serialize().unwrap();
        let reopened = session(&after, format);
        check_file_reader(&reopened, &expected["After"], &name);
        source.undo().unwrap();
        assert_eq!(source.serialize().unwrap(), before);
        check_file_reader(&source, &expected["Before"], &name);
    }
    assert_eq!(count, 5);
}

#[test]
fn adding_chart_music_offset_also_preserves_inherited_bpm_and_all_other_timing() {
    let mut source = session(SSC, Format::Ssc);
    let before = source.serialize().unwrap();
    let before_timing = signature(&source, 0);
    let others = [signature(&source, 1), signature(&source, 2)];
    let mut draft = deadsync_editor::chart_properties::Draft::capture(&source, 0).unwrap();
    draft.set_music_offset(0.5).unwrap();
    draft.set_use_chart_offset(true);
    assert!(draft.apply(&mut source, 0).unwrap());
    assert_eq!(
        source.document().text(Some(0), "OFFSET").as_deref(),
        Some("0.5")
    );
    for tag in TIMING_TAGS {
        if tag != "OFFSET" {
            assert_eq!(
                source.document().raw_property(Some(0), tag),
                source.document().raw_property(None, tag),
                "Offset insertion must preserve {tag}"
            );
        }
    }
    assert_eq!(signature(&source, 0), before_timing);
    assert_eq!([signature(&source, 1), signature(&source, 2)], others);
    let after = source.serialize().unwrap();
    source.undo().unwrap();
    assert_eq!(source.serialize().unwrap(), before);
    source.redo().unwrap();
    assert_eq!(source.serialize().unwrap(), after);
    assert_eq!(signature(&session(&after, Format::Ssc), 0), before_timing);
}
