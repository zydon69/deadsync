use deadsync_editor::{
    document::{Document, Format},
    generation::{editor_timing::EditorTimeline, timing::GenerationTiming},
    history::Session,
    timing_edit::{
        Scope, TIMING_TAGS,
        region::{Draft, Kind, Regions},
    },
    workspace::{Command, Workspace},
};
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor")
}
fn session(bytes: &[u8], format: Format) -> Session {
    Session::new(Document::parse(Arc::<[u8]>::from(bytes), format).unwrap())
}
fn kind(value: &Value) -> Kind {
    match value.as_str().unwrap() {
        "Warp" => Kind::Warp,
        "Fake" => Kind::Fake,
        _ => panic!("region kind"),
    }
}
fn values(regions: &Regions, expected: &Value, name: &str) {
    let expected = expected.as_array().unwrap();
    assert_eq!(regions.values().len(), expected.len(), "{name}");
    for ((row, length), node) in regions.values().iter().zip(expected) {
        assert_eq!(*row, node["Row"].as_u64().unwrap(), "{name}");
        assert_eq!(*length, node["Length"].as_u64().unwrap(), "{name} at {row}");
    }
}
#[test]
fn region_importers_minima_setters_and_movement_match_original_reference() {
    let mut count = 0;
    for entry in fs::read_dir(root()).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("region-edit-") || !name.ends_with(".request.json") {
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
        let mut regions = Regions::parse(kind(&request["Kind"]), &text).unwrap();
        values(&regions, &expected["Initial"], &name);
        for (op, expected) in request["Operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(expected["Stages"].as_array().unwrap())
        {
            let row = op[1].as_u64().unwrap();
            let result = match op[0].as_str().unwrap() {
                "set" => regions.set(row, op[2].as_u64().unwrap()),
                "remove" => regions.remove(row),
                "move" => regions.move_node(row, op[2].as_u64().unwrap()),
                _ => panic!("operation"),
            };
            if expected["Error"].is_null() {
                assert_eq!(
                    result.unwrap(),
                    expected["Changed"].as_bool().unwrap(),
                    "{name}: {op}"
                );
            } else {
                assert!(result.is_err(), "{name}: {op}");
            }
            values(&regions, &expected["Values"], &format!("{name}: {op}"));
            assert_eq!(
                Regions::parse(regions.kind(), &regions.serialize()).unwrap(),
                regions,
                "{name} round trip"
            );
        }
    }
    assert_eq!(count, 24);
}
const SSC:&[u8]=b"// keep\n#VERSION:0.83;#TITLE:Regions;#BPMS:0=120,4=180;#STOPS:2=0.25;#DELAYS:3=0.125;#OFFSET:0.25;#WARPS:2=1;#FAKES:0=6,1=1 // spelling\n;#TIMESIGNATURES:0=4=4;#TICKCOUNTS:0=4;#COMBOS:0=1=1;#SPEEDS:0=1=0=0;#SCROLLS:0=1;#LABELS:1=caf\xe9\\;keep;#NOTEDATA:;#STEPSTYPE:dance-single;#VENDOR:keep;#NOTES:1000\n0100\n0010\n0001;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=150;#OFFSET:0;#WARPS:1=0.5;#NOTES:0001;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0010;";
#[test]
fn local_region_edit_copies_raw_timing_and_preserves_untouched_fake_map_in_one_undo() {
    let mut source = session(SSC, Format::Ssc);
    let grid = source.grid(0).unwrap().clone();
    let mut draft = Draft::capture(&source, 0, Scope::Chart).unwrap();
    assert!(draft.inherited());
    assert_eq!(draft.affected_charts(), [0]);
    draft.set(Kind::Warp, 48, 24).unwrap();
    assert_eq!(source.serialize().unwrap(), SSC);
    draft.apply(&mut source, 0).unwrap();
    assert_eq!(
        source.document().text(Some(0), "WARPS").as_deref(),
        Some("1=0.5,2=1")
    );
    for tag in TIMING_TAGS {
        if tag != "WARPS" {
            assert_eq!(
                source.document().raw_property(Some(0), tag),
                source.document().raw_property(None, tag),
                "{tag}"
            );
        }
    }
    assert_eq!(source.grid(0).unwrap(), &grid);
    assert!(source.document().raw_property(Some(2), "WARPS").is_none());
    let after = source.serialize().unwrap();
    source.undo().unwrap();
    assert_eq!(source.serialize().unwrap(), SSC);
    source.redo().unwrap();
    assert_eq!(source.serialize().unwrap(), after);
}
#[test]
fn overlapping_regions_coexist_but_same_type_start_collisions_and_overflow_are_atomic() {
    let mut warps = Regions::parse(Kind::Warp, "0=8,1=2,4=8").unwrap();
    assert_eq!(warps.values().len(), 3);
    assert!(warps.move_node(0, 48).is_err());
    assert_eq!(warps.values().get(&0), Some(&384));
    assert!(warps.move_node(0, 24).unwrap());
    assert!(warps.remove(24).unwrap());
    let before = warps.clone();
    for (row, length) in [
        (0, 0),
        (1, i32::MAX as u64),
        (i32::MAX as u64, 1),
        (u64::MAX, 1),
        (1, u64::MAX),
    ] {
        assert!(warps.set(row, length).is_err());
        assert_eq!(warps, before);
    }
    assert!(warps.move_node(48, i32::MAX as u64).is_err());
    assert_eq!(warps, before);
    assert!(warps.move_node(999, 0).is_err());
    assert!(Regions::parse(Kind::Fake, "NaN=1").is_err());
    assert!(Regions::parse(Kind::Fake, "0=inf").is_err());
    let mut source = session(SSC, Format::Ssc);
    let mut draft = Draft::capture(&source, 0, Scope::Chart).unwrap();
    draft.set(Kind::Warp, 0, 48).unwrap();
    draft.set(Kind::Fake, 0, 96).unwrap();
    draft.move_node(Kind::Fake, 0, 24).unwrap();
    draft.apply(&mut source, 0).unwrap();
    assert_eq!(
        source.document().text(Some(0), "WARPS").as_deref(),
        Some("0=1,2=1")
    );
    assert_eq!(
        source.document().text(Some(0), "FAKES").as_deref(),
        Some("0.5=2,1=1")
    );
    source.undo().unwrap();
    assert_eq!(source.serialize().unwrap(), SSC);
}
#[test]
fn shared_regions_affect_inheritors_while_own_timing_keeps_missing_fields_missing() {
    let mut source = session(SSC, Format::Ssc);
    let mut draft = Draft::capture(&source, 0, Scope::Song).unwrap();
    assert_eq!(draft.affected_charts(), [0, 2]);
    draft.set(Kind::Warp, 0, 48).unwrap();
    draft.apply(&mut source, 0).unwrap();
    assert!(source.document().raw_property(Some(0), "WARPS").is_none());
    assert_eq!(
        source.document().text(Some(1), "WARPS").as_deref(),
        Some("1=0.5")
    );
    source.undo().unwrap();
    let mut own = Draft::capture(&source, 1, Scope::Chart).unwrap();
    assert!(!own.inherited());
    assert!(own.regions(Kind::Fake).values().is_empty());
    own.set(Kind::Fake, 0, 48).unwrap();
    own.apply(&mut source, 1).unwrap();
    assert!(source.document().raw_property(Some(1), "STOPS").is_none());
    assert_eq!(
        source.document().text(Some(1), "WARPS").as_deref(),
        Some("1=0.5")
    );
}
#[test]
fn no_op_invalid_discarded_foreign_stale_and_wrong_chart_drafts_preserve_history() {
    let mut source = session(SSC, Format::Ssc);
    source.set_text(None, "TITLE", "Redo").unwrap();
    source.undo().unwrap();
    let redo = source.redo_label().unwrap().to_owned();
    let mut draft = Draft::capture(&source, 0, Scope::Chart).unwrap();
    assert!(!draft.set(Kind::Warp, 96, 48).unwrap());
    assert!(draft.set(Kind::Fake, 0, 0).is_err());
    assert!(!draft.has_changes());
    assert!(!draft.apply(&mut source, 0).unwrap());
    assert_eq!(source.redo_label(), Some(redo.as_str()));
    assert_eq!(source.serialize().unwrap(), SSC);
    draft.set(Kind::Warp, 48, 48).unwrap();
    assert!(draft.apply(&mut source, 1).is_err());
    let mut foreign = session(SSC, Format::Ssc);
    assert!(draft.apply(&mut foreign, 0).is_err());
    assert_eq!(foreign.serialize().unwrap(), SSC);
    drop(draft);
    assert_eq!(source.serialize().unwrap(), SSC);
    assert_eq!(source.redo_label(), Some(redo.as_str()));
    let mut draft = Draft::capture(&source, 0, Scope::Chart).unwrap();
    draft.set(Kind::Warp, 48, 48).unwrap();
    source.set_text(None, "TITLE", "Revision").unwrap();
    let before = source.serialize().unwrap();
    assert!(draft.apply(&mut source, 0).is_err());
    assert_eq!(source.serialize().unwrap(), before);
}
#[test]
fn empty_local_sets_remain_empty_after_save_with_pending_notes_and_both_undos() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SSC).unwrap();
    let mut workspace = Workspace::open(&path, 0).unwrap();
    workspace.execute(Command::Place(2)).unwrap();
    let notes = workspace.grid().clone();
    let mut draft = workspace.region_draft(Scope::Chart).unwrap();
    draft.remove(Kind::Warp, 96).unwrap();
    draft.remove(Kind::Fake, 0).unwrap();
    draft.remove(Kind::Fake, 48).unwrap();
    workspace.apply_regions(&draft).unwrap();
    assert_eq!(fs::read(&path).unwrap(), SSC);
    workspace.execute(Command::Save).unwrap();
    let reopened = Workspace::open(&path, 0).unwrap();
    assert_eq!(reopened.grid(), &notes);
    for kind in [Kind::Warp, Kind::Fake] {
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
                .region_draft(Scope::Chart)
                .unwrap()
                .regions(kind)
                .values()
                .is_empty()
        );
    }
    workspace.execute(Command::Undo).unwrap();
    assert_eq!(workspace.grid(), &notes);
    workspace.execute(Command::Undo).unwrap();
    assert_eq!(workspace.file().session.serialize().unwrap(), SSC);
}
fn check_reader(source: &mut Session, expected: &Value, name: &str) {
    let bytes = source.serialize().unwrap();
    assert_eq!(
        source.document().charts().len(),
        expected.as_array().unwrap().len()
    );
    for (index, chart) in expected.as_array().unwrap().iter().enumerate() {
        let draft = Draft::capture(source, index, Scope::Chart).unwrap();
        values(draft.regions(Kind::Warp), &chart["Warps"], name);
        values(draft.regions(Kind::Fake), &chart["Fakes"], name);
        let timing = GenerationTiming::from_simfile(&bytes, Format::Ssc, index).unwrap();
        let rows = chart["Notes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|note| note["Row"].as_u64().unwrap())
            .collect::<Vec<_>>();
        let times = timing.times(rows).unwrap();
        for (time, note) in times.iter().zip(chart["Notes"].as_array().unwrap()) {
            assert_eq!(
                *time,
                note["Time"].as_f64().unwrap(),
                "{name} file chart {index} row {}",
                note["Row"]
            );
        }
        let grid = source.grid(index).unwrap();
        let timeline = EditorTimeline::build(&timing, grid, None, &AtomicBool::new(false)).unwrap();
        let editor = &chart["Editor"];
        assert_eq!(
            timeline.heads().len(),
            editor["Heads"].as_array().unwrap().len()
        );
        for (head, expected) in timeline
            .heads()
            .iter()
            .zip(editor["Heads"].as_array().unwrap())
        {
            assert_eq!(
                head.position.row,
                expected["Row"].as_u64().unwrap(),
                "{name}"
            );
            assert_eq!(
                u64::from(head.position.lane),
                expected["Lane"].as_u64().unwrap(),
                "{name}"
            );
            assert_eq!(
                head.time,
                expected["Time"].as_f64().unwrap(),
                "{name} editor chart {index} row {}",
                head.position.row
            );
            assert_eq!(
                head.fake,
                expected["Fake"].as_bool().unwrap(),
                "{name} fake chart {index} row {}",
                head.position.row
            );
        }
        assert_eq!(
            timeline.step_count() as u64,
            editor["StepCount"].as_u64().unwrap(),
            "{name}"
        );
        assert_eq!(
            timeline.chart_end().unwrap(),
            editor["End"].as_f64().unwrap(),
            "{name}"
        );
        for (i, time) in editor["Times"].as_array().unwrap().iter().enumerate() {
            assert_eq!(
                timeline.position_time((i * 12) as u64).unwrap(),
                time.as_f64().unwrap(),
                "{name} query chart {index} row {}",
                i * 12
            );
        }
    }
}
#[test]
fn real_file_reader_and_original_editor_fake_flags_counts_and_clocks_match_edits_and_reopen() {
    let mut count = 0;
    for entry in fs::read_dir(root()).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("region-file-") || !name.ends_with(".request.json") {
            continue;
        }
        count += 1;
        let request: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let expected: Value = serde_json::from_slice(
            &fs::read(root().join(name.replace(".request.json", ".expected.json"))).unwrap(),
        )
        .unwrap();
        let bytes = request["Content"].as_str().unwrap().as_bytes();
        let mut source = session(bytes, Format::Ssc);
        check_reader(&mut source, &expected["Before"], &name);
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
                "set" => draft.set(kind, row, op[3].as_u64().unwrap()),
                "remove" => draft.remove(kind, row),
                "move" => draft.move_node(kind, row, op[3].as_u64().unwrap()),
                _ => panic!("operation"),
            }
            .unwrap();
        }
        draft.apply(&mut source, index).unwrap();
        check_reader(&mut source, &expected["After"], &name);
        let after = source.serialize().unwrap();
        check_reader(&mut session(&after, Format::Ssc), &expected["After"], &name);
        source.undo().unwrap();
        assert_eq!(source.serialize().unwrap(), bytes);
    }
    assert_eq!(count, 4);
}
#[test]
fn legacy_ssc_uses_shared_regions_and_sm_export_requires_explicit_migration() {
    let mut source = session(
        b"#VERSION:0.6;#BPMS:0=120;#WARPS:1=1;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
        Format::Ssc,
    );
    assert!(Draft::capture(&source, 0, Scope::Chart).is_err());
    let mut draft = Draft::capture(&source, 0, Scope::Song).unwrap();
    draft.set(Kind::Fake, 0, 48).unwrap();
    draft.apply(&mut source, 0).unwrap();
    assert_eq!(
        source.document().text(None, "FAKES").as_deref(),
        Some("0=1")
    );
    let sm = session(
        b"#BPMS:0=120;#NOTES:dance-single::Hard:9::1000;",
        Format::Sm,
    );
    assert!(Draft::capture(&sm, 0, Scope::Chart).is_err());
    assert!(Draft::capture(&sm, 0, Scope::Song).is_err());
    assert!(Draft::capture(&sm, 999, Scope::Song).is_err());
}
