use deadsync_editor::{
    document::{Document, Format},
    generation::timing::GenerationTiming,
    history::Session,
    notes::{Note, Position},
    timing_edit::{
        Scope, TIMING_TAGS,
        pause::{Draft, Kind, Pauses},
    },
    workspace::{Command, Workspace},
};
use serde_json::Value;
use std::{fs, path::PathBuf, sync::Arc};
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor")
}
fn source(bytes: &[u8], format: Format) -> Session {
    Session::new(Document::parse(Arc::<[u8]>::from(bytes), format).unwrap())
}
fn kind(value: &Value) -> Kind {
    match value.as_str().unwrap() {
        "Stop" => Kind::Stop,
        "Delay" => Kind::Delay,
        _ => panic!("pause type"),
    }
}
fn values(pauses: &Pauses, expected: &Value, name: &str) {
    let expected = expected.as_array().unwrap();
    assert_eq!(pauses.values().len(), expected.len(), "{name}");
    for ((row, seconds), node) in pauses.values().iter().zip(expected) {
        assert_eq!(*row, node["Row"].as_u64().unwrap(), "{name}");
        assert_eq!(
            seconds.to_bits(),
            node["Seconds"].as_f64().unwrap().to_bits(),
            "{name} at {row}"
        );
    }
}
#[test]
fn pauses_match_original_importers_setters_and_movement_guards() {
    let mut count = 0;
    for file in fs::read_dir(root()).unwrap() {
        let path = file.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("pause-edit-") || !name.ends_with(".request.json") {
            continue;
        }
        count += 1;
        let request: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let expected: Value = serde_json::from_slice(
            &fs::read(root().join(name.replace(".request.json", ".expected.json"))).unwrap(),
        )
        .unwrap();
        let text = request["Segments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pair| format!("{}={}", pair[0], pair[1]))
            .collect::<Vec<_>>()
            .join(",");
        let mut pauses = Pauses::parse(kind(&request["Kind"]), &text).unwrap();
        values(&pauses, &expected["Initial"], &name);
        for (operation, expected) in request["Operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(expected["Stages"].as_array().unwrap())
        {
            let row = operation[1].as_u64().unwrap();
            let result = match operation[0].as_str().unwrap() {
                "set" => pauses.set(row, operation[2].as_f64().unwrap()),
                "remove" => pauses.remove(row),
                "move" => pauses.move_node(row, operation[2].as_u64().unwrap()),
                _ => panic!("operation"),
            };
            if expected["Error"].is_null() {
                assert_eq!(
                    result.unwrap(),
                    expected["Changed"].as_bool().unwrap(),
                    "{name}: {operation}"
                );
            } else {
                assert!(result.is_err(), "{name}: {operation}");
            }
            values(
                &pauses,
                &expected["Values"],
                &format!("{name}: {operation}"),
            );
            assert_eq!(
                Pauses::parse(pauses.kind(), &pauses.serialize()).unwrap(),
                pauses,
                "{name} round trip"
            );
        }
    }
    assert_eq!(count, 22);
}
const SSC:&[u8]=b"// unchanged\n#VERSION:0.83;#TITLE:Pause;#BPMS:0=120,4=180;#STOPS:2=0.25;#DELAYS:3=0.125 // keep this spelling\n;#OFFSET:0.25;#WARPS:8=1;#TIMESIGNATURES:0=4=4;#TICKCOUNTS:0=4;#COMBOS:0=1=1;#SPEEDS:0=1=0=0;#SCROLLS:0=1;#FAKES:12=1;#LABELS:1=caf\xe9\\;keep;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=150;#OFFSET:0;#STOPS:1=0.5;#NOTES:0001;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0010;";
#[test]
fn first_local_pause_preserves_all_raw_inherited_fields_and_unchanged_delay_in_one_gesture() {
    let mut source = source(SSC, Format::Ssc);
    let original_notes = source.grid(0).unwrap().clone();
    let mut draft = Draft::capture(&source, 0, Scope::Chart).unwrap();
    assert!(draft.inherited());
    assert_eq!(draft.affected_charts(), [0]);
    draft.set(Kind::Stop, 48, -0.5).unwrap();
    assert_eq!(source.serialize().unwrap(), SSC);
    assert!(draft.apply(&mut source, 0).unwrap());
    assert_eq!(
        source.document().text(Some(0), "STOPS").as_deref(),
        Some("1=-0.5,2=0.25")
    );
    for tag in TIMING_TAGS {
        if tag != "STOPS" {
            assert_eq!(
                source.document().raw_property(Some(0), tag),
                source.document().raw_property(None, tag),
                "{tag}"
            );
        }
    }
    assert_eq!(source.grid(0).unwrap(), &original_notes);
    assert!(source.document().raw_property(Some(2), "STOPS").is_none());
    let after = source.serialize().unwrap();
    assert!(source.undo().unwrap());
    assert_eq!(source.serialize().unwrap(), SSC);
    assert!(source.redo().unwrap());
    assert_eq!(source.serialize().unwrap(), after);
}
#[test]
fn both_types_can_coincide_and_their_initial_nodes_can_move_or_be_deleted() {
    let mut source = source(SSC, Format::Ssc);
    let mut draft = Draft::capture(&source, 0, Scope::Chart).unwrap();
    draft.set(Kind::Stop, 0, -0.5).unwrap();
    draft.set(Kind::Delay, 0, 0.25).unwrap();
    draft.move_node(Kind::Stop, 0, 48).unwrap();
    draft.move_node(Kind::Delay, 0, 48).unwrap();
    assert!(draft.move_node(Kind::Stop, 48, 96).is_err());
    assert_eq!(draft.pauses(Kind::Stop).values().get(&48), Some(&-0.5));
    draft.remove(Kind::Stop, 96).unwrap();
    draft.remove(Kind::Delay, 144).unwrap();
    draft.apply(&mut source, 0).unwrap();
    assert_eq!(
        source.document().text(Some(0), "STOPS").as_deref(),
        Some("1=-0.5")
    );
    assert_eq!(
        source.document().text(Some(0), "DELAYS").as_deref(),
        Some("1=0.25")
    );
    source.undo().unwrap();
    assert_eq!(source.serialize().unwrap(), SSC);
}
#[test]
fn owned_timing_does_not_pull_song_fields_and_shared_scope_only_changes_inheritors() {
    let mut source = source(SSC, Format::Ssc);
    let mut own = Draft::capture(&source, 1, Scope::Chart).unwrap();
    assert!(!own.inherited());
    assert!(own.pauses(Kind::Delay).values().is_empty());
    own.set(Kind::Delay, 48, 0.25).unwrap();
    own.apply(&mut source, 1).unwrap();
    assert!(source.document().raw_property(Some(1), "WARPS").is_none());
    assert_eq!(
        source.document().text(Some(1), "STOPS").as_deref(),
        Some("1=0.5")
    );
    source.undo().unwrap();
    let mut shared = Draft::capture(&source, 0, Scope::Song).unwrap();
    assert_eq!(shared.affected_charts(), [0, 2]);
    shared.set(Kind::Delay, 48, 0.5).unwrap();
    shared.apply(&mut source, 0).unwrap();
    assert!(source.document().raw_property(Some(0), "DELAYS").is_none());
    assert!(source.document().raw_property(Some(1), "DELAYS").is_none());
    assert_eq!(
        source.document().text(None, "DELAYS").as_deref(),
        Some("1=0.5,3=0.125")
    );
}
#[test]
fn invalid_foreign_stale_wrong_chart_and_no_op_drafts_preserve_data_and_redo() {
    let mut source = source(SSC, Format::Ssc);
    source
        .edit_notes(
            0,
            "Note for redo",
            [(
                Position {
                    row: 48,
                    lane: 2,
                    player: 0,
                },
                Some(Note::tap()),
            )],
        )
        .unwrap();
    source.undo().unwrap();
    let redo = source.redo_label().unwrap().to_owned();
    let mut draft = Draft::capture(&source, 0, Scope::Chart).unwrap();
    for (kind, value) in [
        (Kind::Stop, 0.0),
        (Kind::Stop, f64::INFINITY),
        (Kind::Delay, -0.5),
        (Kind::Delay, f64::NAN),
        (Kind::Delay, 1e308),
    ] {
        assert!(draft.set(kind, 48, value).is_err());
    }
    assert!(draft.set(Kind::Stop, i32::MAX as u64 + 1, 1.0).is_err());
    assert!(draft.move_node(Kind::Stop, 999, 0).is_err());
    assert!(!draft.has_changes());
    assert!(!draft.apply(&mut source, 0).unwrap());
    assert_eq!(source.redo_label(), Some(redo.as_str()));
    assert_eq!(source.serialize().unwrap(), SSC);
    draft.set(Kind::Delay, 48, 0.25).unwrap();
    assert!(draft.apply(&mut source, 1).is_err());
    let mut foreign = source_from_same_bytes();
    assert!(draft.apply(&mut foreign, 0).is_err());
    assert_eq!(foreign.serialize().unwrap(), SSC);
    source.set_text(None, "TITLE", "new revision").unwrap();
    let before = source.serialize().unwrap();
    assert!(draft.apply(&mut source, 0).is_err());
    assert_eq!(source.serialize().unwrap(), before);
}
fn source_from_same_bytes() -> Session {
    source(SSC, Format::Ssc)
}
#[test]
fn workspace_saves_empty_pause_sets_without_falling_back_to_song_values() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SSC).unwrap();
    let mut workspace = Workspace::open(&path, 0).unwrap();
    workspace.execute(Command::Place(2)).unwrap();
    let notes = workspace.grid().clone();
    let mut draft = workspace.pause_draft(Scope::Chart).unwrap();
    draft.remove(Kind::Stop, 96).unwrap();
    draft.remove(Kind::Delay, 144).unwrap();
    workspace.apply_pauses(&draft).unwrap();
    assert_eq!(fs::read(&path).unwrap(), SSC);
    workspace.execute(Command::Save).unwrap();
    let reopened = Workspace::open(&path, 0).unwrap();
    assert_eq!(reopened.grid(), &notes);
    for kind in [Kind::Stop, Kind::Delay] {
        assert_eq!(
            reopened
                .file()
                .session
                .document()
                .text(Some(0), kind.tag())
                .as_deref(),
            Some("")
        );
        assert!(
            reopened
                .pause_draft(Scope::Chart)
                .unwrap()
                .pauses(kind)
                .values()
                .is_empty()
        );
    }
    workspace.execute(Command::Undo).unwrap();
    assert_eq!(workspace.grid(), &notes);
    workspace.execute(Command::Undo).unwrap();
    assert_eq!(workspace.file().session.serialize().unwrap(), SSC);
}
fn check_reader(source: &Session, expected: &Value, name: &str) {
    let bytes = source.serialize().unwrap();
    assert_eq!(
        source.document().charts().len(),
        expected.as_array().unwrap().len()
    );
    for (index, chart) in expected.as_array().unwrap().iter().enumerate() {
        let scope = if source.document().format == Format::Sm {
            Scope::Song
        } else {
            Scope::Chart
        };
        let draft = Draft::capture(source, index, scope).unwrap();
        for kind in [Kind::Stop, Kind::Delay] {
            let expected = Value::Array(
                chart["Pauses"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|pause| pause["Delay"].as_bool().unwrap() == (kind == Kind::Delay))
                    .cloned()
                    .collect(),
            );
            values(draft.pauses(kind), &expected, name);
        }
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
fn edits_match_real_fumen_reader_pause_order_and_note_times_before_after_save() {
    let mut count = 0;
    for file in fs::read_dir(root()).unwrap() {
        let path = file.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("pause-file-") || !name.ends_with(".request.json") {
            continue;
        }
        count += 1;
        let request: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let expected: Value = serde_json::from_slice(
            &fs::read(root().join(name.replace(".request.json", ".expected.json"))).unwrap(),
        )
        .unwrap();
        let format = if request["Format"] == "sm" {
            Format::Sm
        } else {
            Format::Ssc
        };
        let original = request["Content"].as_str().unwrap().as_bytes();
        let mut source = source(original, format);
        check_reader(&source, &expected["Before"], &name);
        let index = request["Chart"].as_u64().unwrap() as usize;
        let scope = if request["Scope"] == "Song" {
            Scope::Song
        } else {
            Scope::Chart
        };
        let mut draft = Draft::capture(&source, index, scope).unwrap();
        for op in request["Operations"].as_array().unwrap() {
            let kind = kind(&op[1]);
            let row = op[2].as_u64().unwrap();
            match op[0].as_str().unwrap() {
                "set" => draft.set(kind, row, op[3].as_f64().unwrap()),
                "move" => draft.move_node(kind, row, op[3].as_u64().unwrap()),
                "remove" => draft.remove(kind, row),
                _ => panic!("operation"),
            }
            .unwrap();
        }
        draft.apply(&mut source, index).unwrap();
        check_reader(&source, &expected["After"], &name);
        let bytes = source.serialize().unwrap();
        let reopened = Session::new(Document::parse(Arc::<[u8]>::from(bytes), format).unwrap());
        check_reader(&reopened, &expected["After"], &name);
        source.undo().unwrap();
        assert_eq!(source.serialize().unwrap(), original);
    }
    assert_eq!(count, 5);
}
#[test]
fn legacy_and_sm_require_shared_timing_and_missing_or_malformed_data_is_explicit() {
    for (bytes,format) in [(b"#BPMS:0=120;#STOPS:1=0.5;#NOTES:dance-single::Hard:9::1000;".as_slice(),Format::Sm),(b"#VERSION:0.6;#BPMS:0=120;#STOPS:1=0.5;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",Format::Ssc)]{
        let mut source=source(bytes,format);assert!(Draft::capture(&source,0,Scope::Chart).is_err());let mut draft=Draft::capture(&source,0,Scope::Song).unwrap();draft.set(Kind::Stop,0,-0.25).unwrap();draft.apply(&mut source,0).unwrap();assert_eq!(source.document().text(None,"STOPS").as_deref(),Some("0=-0.25,1=0.5"));
    }
    assert!(Draft::capture(&source(SSC, Format::Ssc), 999, Scope::Song).is_err());
    assert!(Pauses::parse(Kind::Stop, "1=broken").is_err());
    assert!(Pauses::parse(Kind::Delay, "NaN=1").is_err());
    let mut empty = Pauses::parse(Kind::Delay, "").unwrap();
    assert!(empty.set(0, 0.0).unwrap());
    assert!(!empty.set(0, -0.0).unwrap());
    assert!(empty.remove(0).unwrap());
}
