#[path = "support/source_output_context.rs"]
mod source_output_context;
use deadsync_editor::{
    document::{Document, Format},
    generation::{
        clear_patterns::Task,
        config::PatternConfig,
        marker_edit::Draft,
        metadata::{ChartData, Definition, SIXTEENTHS},
        pattern_job::Span,
        profiles::Profile,
    },
    history::Session,
    notes::{Grid, Kind, Note, Position},
    storage::FileSession,
    workspace::Workspace,
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};
fn source(grid: &Grid, format: Format) -> Session {
    let prefix = match format {
        Format::Sm => {
            "#TITLE:Keep;#VENDOR:opaque;#BPMS:0=120;#NOTES:dance-single:source:Hard:9:0,0,0,0,0:"
        }
        Format::Ssc => {
            "#TITLE:Keep;#VENDOR:opaque;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:"
        }
    };
    let mut bytes = prefix.as_bytes().to_vec();
    bytes.extend(grid.serialize().unwrap());
    bytes.push(b';');
    Session::new(Document::parse(Arc::<[u8]>::from(bytes), format).unwrap())
}
fn marker(length: u64) -> Definition {
    Definition {
        length,
        pattern_config_guid: SIXTEENTHS.parse().unwrap(),
        start_position_inclusive: true,
        end_position_inclusive: true,
        extra: BTreeMap::from([("Future".into(), serde_json::json!("keep"))]),
        ..Definition::default()
    }
}
fn save_markers(source: &mut Session, markers: BTreeMap<u64, Definition>) {
    ChartData {
        patterns: Some(markers),
        extra: BTreeMap::from([("FutureChart".into(), serde_json::json!(5))]),
        ..ChartData::default()
    }
    .save(source, 0)
    .unwrap();
}
fn grid() -> Grid {
    let mut grid = Grid::new(4, 2).unwrap();
    for player in 0..2 {
        for (row, lane, kind, end) in [
            (0, 0, Kind::Hold, Some(144)),
            (47, 1, Kind::Roll, Some(144)),
            (48, 2, Kind::Mine, None),
            (60, 3, Kind::Keysound, None),
            (72, 2, Kind::Fake, None),
            (84, 3, Kind::Lift, None),
            (96, 2, Kind::Tap, None),
            (192, 0, Kind::Tap, None),
        ] {
            grid.set(
                Position { player, row, lane },
                Some(Note {
                    kind,
                    end,
                    ..Note::tap()
                }),
            )
            .unwrap();
        }
    }
    grid
}
#[test]
fn clearing_saved_markers_matches_unchanged_original_deletion_and_bound_methods() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    let mut count = 0;
    for entry in fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("clear-pattern-") || !name.ends_with(".request.json") {
            continue;
        }
        let input: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let expected: Value = serde_json::from_slice(
            &fs::read(root.join(name.replace(".request.json", ".expected.json"))).unwrap(),
        )
        .unwrap();
        let expected = &expected["Cleared"];
        let mut grid = Grid::new(4, 1).unwrap();
        for note in input["Notes"].as_array().unwrap() {
            let kind = match note[2].as_str().unwrap() {
                "1" => Kind::Tap,
                "2" => Kind::Hold,
                "4" => Kind::Roll,
                "M" => Kind::Mine,
                "K" => Kind::Keysound,
                "F" => Kind::Fake,
                "L" => Kind::Lift,
                _ => panic!(),
            };
            grid.set(
                Position {
                    player: 0,
                    row: note[0].as_u64().unwrap(),
                    lane: note[1].as_u64().unwrap() as u16,
                },
                Some(Note {
                    kind,
                    end: note.get(3).and_then(Value::as_u64),
                    ..Note::tap()
                }),
            )
            .unwrap();
        }
        let dir = tempfile::tempdir().unwrap();
        let mut source = source(&grid, Format::Ssc);
        let mut markers = BTreeMap::new();
        for item in input["ClearPatterns"].as_array().unwrap() {
            let spacing = item[2].as_u64().unwrap();
            let profile = Profile::new_pattern(
                "Oracle",
                PatternConfig {
                    beat_sub_division: (48 / spacing) as u32,
                    ..PatternConfig::sixteenths()
                },
            )
            .unwrap();
            profile.export_new(dir.path()).unwrap();
            markers.insert(
                item[0].as_u64().unwrap(),
                Definition {
                    length: item[1].as_u64().unwrap(),
                    pattern_config_guid: profile.guid().clone(),
                    start_position_inclusive: item[3].as_bool().unwrap(),
                    end_position_inclusive: item[4].as_bool().unwrap(),
                    ..Definition::default()
                },
            );
        }
        let mut sorted = input["ClearPatterns"].as_array().unwrap().clone();
        sorted.sort_by_key(|v| v[0].as_u64().unwrap());
        for (item, bounds) in sorted.iter().zip(expected["Bounds"].as_array().unwrap()) {
            let start = item[0].as_u64().unwrap();
            let actual = Span {
                start,
                end: start + item[1].as_u64().unwrap(),
                include_start: item[3].as_bool().unwrap(),
                include_end: item[4].as_bool().unwrap(),
            }
            .optional_steps(item[2].as_u64().unwrap())
            .unwrap();
            assert_eq!(
                actual.map(|v| v.2).unwrap_or(0),
                bounds["Count"].as_u64().unwrap(),
                "{name}"
            );
            if let Some((first, last, _)) = actual {
                assert_eq!(first, bounds["First"].as_u64().unwrap());
                assert_eq!(last, bounds["Last"].as_u64().unwrap());
            }
        }
        let rows = markers.keys().rev().copied().collect::<Vec<_>>();
        save_markers(&mut source, markers);
        let before = source.serialize().unwrap();
        let preview = Task::capture(&source, 0, rows, Some(dir.path()))
            .unwrap()
            .run(&AtomicBool::new(false), |_, _| {})
            .unwrap();
        assert_eq!(source.serialize().unwrap(), before);
        assert_eq!(
            preview.report().removed as u64,
            expected["Removed"].as_u64().unwrap(),
            "{name}"
        );
        assert_eq!(
            preview.report().shortened_holds as u64,
            expected["Shortened"].as_u64().unwrap(),
            "{name}"
        );
        let mut actual = preview
            .grid()
            .iter()
            .map(|(p, n)| (p.row, p.lane, format!("{:?}", n.kind), n.end))
            .collect::<Vec<_>>();
        let mut reference = expected["Notes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| {
                (
                    n["Row"].as_u64().unwrap(),
                    n["Lane"].as_u64().unwrap() as u16,
                    match n["Symbol"].as_str().unwrap() {
                        "1" => "Tap",
                        "2" => "Hold",
                        "4" => "Roll",
                        "M" => "Mine",
                        "K" => "Keysound",
                        "F" => "Fake",
                        "L" => "Lift",
                        _ => panic!(),
                    }
                    .to_string(),
                    n["End"].as_u64(),
                )
            })
            .collect::<Vec<_>>();
        actual.sort();
        reference.sort();
        assert_eq!(actual, reference, "{name}");
        count += 1;
    }
    assert_eq!(count, 12);
}
#[test]
fn clear_shortens_crossing_holds_removes_all_participants_and_is_one_saveable_gesture() {
    for format in [Format::Sm, Format::Ssc] {
        let mut source = source(&grid(), format);
        save_markers(&mut source, BTreeMap::from([(48, marker(48))]));
        let before = source.serialize().unwrap();
        let metadata = ChartData::read(source.document(), 0).unwrap().unwrap();
        // Missing performed/expressed configs do not matter for deletion.
        let preview = Task::capture(&source, 0, [48], None)
            .unwrap()
            .run(&AtomicBool::new(false), |_, _| {})
            .unwrap();
        assert_eq!(preview.report().removed, 12);
        assert_eq!(preview.report().shortened_holds, 2);
        for player in 0..2 {
            assert_eq!(
                preview
                    .grid()
                    .get(Position {
                        player,
                        lane: 0,
                        row: 0
                    })
                    .unwrap()
                    .end,
                Some(36)
            );
            assert!(
                preview
                    .grid()
                    .get(Position {
                        player,
                        lane: 0,
                        row: 192
                    })
                    .is_some()
            );
        }
        assert!(preview.apply(&mut source, 0).unwrap());
        let after = source.serialize().unwrap();
        assert_eq!(
            serde_json::to_value(ChartData::read(source.document(), 0).unwrap().unwrap()).unwrap(),
            serde_json::to_value(metadata).unwrap()
        );
        source.undo().unwrap();
        assert_eq!(source.serialize().unwrap(), before);
        source.redo().unwrap();
        assert_eq!(source.serialize().unwrap(), after);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(if format == Format::Sm {
            "song.sm"
        } else {
            "song.ssc"
        });
        fs::write(&path, &before).unwrap();
        let mut file = FileSession::open(&path).unwrap();
        let task = Task::capture(&file.session, 0, [48], None).unwrap();
        task.run(&AtomicBool::new(false), |_, _| {})
            .unwrap()
            .apply(&mut file.session, 0)
            .unwrap();
        assert_eq!(file.session.serialize().unwrap(), after);
        file.save().unwrap();
        assert_eq!(file.session.serialize().unwrap(), after, "Save must not rewrite resident Undo bytes");
        let published = fs::read(&path).unwrap();
        source_output_context::assert_context_only(&after, &published, format);
        file.session.undo().unwrap();
        assert_eq!(file.session.serialize().unwrap(), before);
        file.session.redo().unwrap();
        assert_eq!(file.session.serialize().unwrap(), after);
        let mut reopened = FileSession::open(&path).unwrap();
        assert_eq!(reopened.session.serialize().unwrap(), published);
        assert_eq!(reopened.session.document().charts().len(), source.document().charts().len());
        for index in 0..source.document().charts().len() {
            assert_eq!(reopened.session.grid(index).unwrap(), source.grid(index).unwrap());
            assert_eq!(
                serde_json::to_value(ChartData::read(reopened.session.document(), index).unwrap()).unwrap(),
                serde_json::to_value(ChartData::read(source.document(), index).unwrap()).unwrap(),
            );
        }
    }
}
#[test]
fn empty_regions_and_already_cleared_regions_are_noops_preserving_redo() {
    let mut source = source(&grid(), Format::Ssc);
    let mut empty = marker(0);
    empty.start_position_inclusive = false;
    save_markers(&mut source, BTreeMap::from([(48, empty)]));
    let before = source.serialize().unwrap();
    source
        .edit_notes(
            0,
            "Other",
            [(
                Position {
                    player: 0,
                    row: 300,
                    lane: 1,
                },
                Some(Note::tap()),
            )],
        )
        .unwrap();
    let after = source.serialize().unwrap();
    source.undo().unwrap();
    let task = Task::capture(&source, 0, [48, 48], None).unwrap();
    let preview = task.run(&AtomicBool::new(false), |_, _| {}).unwrap();
    assert_eq!(preview.report().removed, 0);
    assert_eq!(preview.report().shortened_holds, 0);
    assert!(!preview.apply(&mut source, 0).unwrap());
    assert_eq!(source.serialize().unwrap(), before);
    source.redo().unwrap();
    assert_eq!(source.serialize().unwrap(), after);
    let mut source = self::source(&grid(), Format::Ssc);
    save_markers(&mut source, BTreeMap::from([(48, marker(48))]));
    Task::capture(&source, 0, [48], None)
        .unwrap()
        .run(&AtomicBool::new(false), |_, _| {})
        .unwrap()
        .apply(&mut source, 0)
        .unwrap();
    let before = source.serialize().unwrap();
    let preview = Task::capture(&source, 0, [48], None)
        .unwrap()
        .run(&AtomicBool::new(false), |_, _| {})
        .unwrap();
    assert_eq!(preview.report().removed, 0);
    assert_eq!(preview.report().shortened_holds, 0);
    assert!(!preview.apply(&mut source, 0).unwrap());
    assert_eq!(source.serialize().unwrap(), before);
    assert!(
        Span {
            start: 1,
            end: 1,
            include_start: false,
            include_end: true
        }
        .optional_steps(u64::MAX)
        .is_err()
    );
}
#[test]
fn cancelled_foreign_stale_wrong_chart_and_missing_profile_actions_are_read_only() {
    let mut source = source(&grid(), Format::Ssc);
    save_markers(&mut source, BTreeMap::from([(48, marker(48))]));
    let before = source.serialize().unwrap();
    let task = Task::capture(&source, 0, [48], None).unwrap();
    assert!(task.run(&AtomicBool::new(true), |_, _| {}).is_err());
    let preview = Task::capture(&source, 0, [48], None)
        .unwrap()
        .run(&AtomicBool::new(false), |_, _| {})
        .unwrap();
    let mut foreign =
        Session::new(Document::parse(Arc::<[u8]>::from(before.clone()), Format::Ssc).unwrap());
    assert!(preview.apply(&mut foreign, 0).is_err());
    assert!(preview.apply(&mut source, 1).is_err());
    source
        .edit_notes(
            0,
            "Other",
            [(
                Position {
                    player: 0,
                    row: 300,
                    lane: 1,
                },
                Some(Note::tap()),
            )],
        )
        .unwrap();
    assert!(preview.apply(&mut source, 0).is_err());
    source.undo().unwrap();
    assert_eq!(source.serialize().unwrap(), before);
    assert!(
        Task::capture(&source, 0, [999], None)
            .unwrap()
            .run(&AtomicBool::new(false), |_, _| {})
            .is_err()
    );
    let mut data = ChartData::read(source.document(), 0).unwrap().unwrap();
    data.patterns
        .as_mut()
        .unwrap()
        .get_mut(&48)
        .unwrap()
        .pattern_config_guid = Default::default();
    data.save(&mut source, 0).unwrap();
    let before = source.serialize().unwrap();
    assert!(
        Task::capture(&source, 0, [48], None)
            .unwrap()
            .run(&AtomicBool::new(false), |_, _| {})
            .is_err()
    );
    assert_eq!(source.serialize().unwrap(), before);
}
#[test]
fn drafts_require_applied_settings_and_background_cancellation_never_changes_source() {
    let mut source = source(&grid(), Format::Ssc);
    save_markers(&mut source, BTreeMap::from([(48, marker(48))]));
    let mut draft = Draft::at_cursor(&source, 0, 48).unwrap();
    draft.start = 60;
    assert!(draft.clear_task(&source, 0, None).is_err());
    draft.start = 48;
    let before = source.serialize().unwrap();
    let cancelled = AtomicBool::new(false);
    assert!(
        Task::capture(&source, 0, [48], None)
            .unwrap()
            .run(&cancelled, |_, _| cancelled
                .store(true, std::sync::atomic::Ordering::Relaxed))
            .is_err()
    );
    assert_eq!(source.serialize().unwrap(), before);
    let mut job = draft.clear_task(&source, 0, None).unwrap().spawn().unwrap();
    job.cancel();
    let limit = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        if let Some(result) = job.poll() {
            assert!(result.is_err());
            break;
        }
        assert!(std::time::Instant::now() < limit);
        std::thread::yield_now();
    }
    assert!(job.poll().is_none());
    assert_eq!(source.serialize().unwrap(), before);
}
#[test]
fn navigation_is_strict_wraps_and_preserves_chart_history() {
    let mut source = source(&grid(), Format::Ssc);
    save_markers(
        &mut source,
        BTreeMap::from([(48, marker(100)), (96, marker(0)), (288, marker(48))]),
    );
    let before = source.serialize().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, &before).unwrap();
    let mut workspace = Workspace::open(&path, 0).unwrap();
    for (cursor, forward, expected) in [
        (0, true, 48),
        (48, true, 96),
        (96, false, 48),
        (48, false, 288),
        (288, true, 48),
        (72, false, 48),
        (72, true, 96),
        (400, false, 288),
        (400, true, 48),
    ] {
        workspace
            .execute(deadsync_editor::workspace::Command::Seek(cursor))
            .unwrap();
        assert!(workspace.navigate_saved_pattern(forward).unwrap());
        assert_eq!(workspace.row(), expected);
        assert_eq!(workspace.file().session.serialize().unwrap(), before);
    }
    let empty = ChartData::default();
    assert_eq!(empty.adjacent_marker(0, true), None);
    assert_eq!(empty.adjacent_marker(0, false), None);
    let single = ChartData {
        patterns: Some(BTreeMap::from([(48, marker(0))])),
        ..ChartData::default()
    };
    assert_eq!(single.adjacent_marker(48, true), Some(48));
    assert_eq!(single.adjacent_marker(48, false), Some(48));
}
