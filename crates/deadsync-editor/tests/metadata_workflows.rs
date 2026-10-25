use deadsync_editor::{
    document::{Document, Format},
    generation::{
        metadata::{ChartData, DYNAMIC, Definition, Guid, PATTERN_BALANCED, Registry, SIXTEENTHS},
        pattern_job::Task,
    },
    history::Session,
    notes::{Note, Position},
    storage::FileSession,
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};
fn marker() -> Definition {
    Definition {
        pattern_config_guid: SIXTEENTHS.parse().unwrap(),
        performed_chart_config_guid: PATTERN_BALANCED.parse().unwrap(),
        length: 48,
        random_seed: 42,
        start_position_inclusive: true,
        end_position_inclusive: true,
        ..Definition::default()
    }
}
fn data() -> ChartData {
    ChartData {
        patterns: Some(BTreeMap::from([(48, marker())])),
        ..ChartData::default()
    }
}
fn bytes(format: Format) -> Vec<u8> {
    let mut bytes = b"#TITLE:Caf\xe9;#VENDOR:keep;#BPMS:0=120;".to_vec();
    for _ in 0..2 {
        bytes.extend_from_slice(match format {
            Format::Ssc => b"#NOTEDATA:;#STEPSTYPE:dance-single;#METER:9;#NOTES:",
            Format::Sm => b"#NOTES:dance-single:source:Hard:9:0,0,0,0,0:",
        });
        bytes.extend_from_slice(b"1000\n0100\n0010\n0001;");
    }
    bytes
}
fn values_equal(actual: &Value, expected: &Value) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => assert_eq!(a.as_f64(), b.as_f64()),
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len());
            for (key, v) in a {
                values_equal(v, b.get(key).unwrap_or_else(|| panic!("Missing {key}")));
            }
        }
        _ => assert_eq!(actual, expected),
    }
}
#[test]
fn chart_metadata_matches_original_csharp_serialized_types() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    let mut count = 0;
    for entry in fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("metadata-") || !name.ends_with(".request.json") {
            continue;
        }
        let input: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let expected: Value = serde_json::from_slice(
            &fs::read(root.join(name.replace(".request.json", ".expected.json"))).unwrap(),
        )
        .unwrap();
        let result = ChartData::from_json(input["Content"].as_str().unwrap());
        assert_eq!(
            result.is_ok(),
            expected["Success"].as_bool().unwrap(),
            "{name}"
        );
        if let Ok(data) = result {
            values_equal(&serde_json::to_value(data).unwrap(), &expected["Data"]);
        }
        count += 1;
    }
    assert_eq!(count, 7);
}
#[test]
fn marker_save_is_one_gesture_preserves_pending_notes_and_unknown_data_in_sm_and_ssc() {
    for format in [Format::Sm, Format::Ssc] {
        let original = bytes(format);
        let mut source =
            Session::new(Document::parse(Arc::<[u8]>::from(original.clone()), format).unwrap());
        let position = Position {
            player: 0,
            row: 12,
            lane: 3,
        };
        source
            .edit_notes(0, "Before", [(position, Some(Note::tap()))])
            .unwrap();
        let before = source.serialize().unwrap();
        let mut data = data();
        data.extra.insert(
            "FutureSetting".into(),
            serde_json::json!({"path":"a\\b;#tag:// test","unicode":"été"}),
        );
        data.patterns
            .as_mut()
            .unwrap()
            .get_mut(&48)
            .unwrap()
            .extra
            .insert("FutureMarker".into(), serde_json::json!([1, 2, 3]));
        assert!(data.save(&mut source, 1).unwrap());
        assert_eq!(source.undo_label(), Some("Change pattern markers"));
        let after = source.serialize().unwrap();
        assert!(after.starts_with(b"#TITLE:Caf\xe9;#VENDOR:keep;"));
        assert!(ChartData::read(source.document(), 0).unwrap().is_none());
        let loaded = ChartData::read(source.document(), 1).unwrap().unwrap();
        assert_eq!(loaded.extra["FutureSetting"], data.extra["FutureSetting"]);
        assert_eq!(
            loaded.patterns.as_ref().unwrap()[&48].extra["FutureMarker"],
            serde_json::json!([1, 2, 3])
        );
        assert!(!loaded.save(&mut source, 1).unwrap());
        source.undo().unwrap();
        assert_eq!(source.serialize().unwrap(), before);
        source.undo().unwrap();
        assert_eq!(source.serialize().unwrap(), original);
        source.redo().unwrap();
        source.redo().unwrap();
        assert_eq!(source.serialize().unwrap(), after);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(format!("song.{}", format.extension()));
        fs::write(&path, &original).unwrap();
        let mut file = FileSession::open(&path).unwrap();
        file.session = source;
        file.save().unwrap();
        let reopened = FileSession::open(&path).unwrap();
        let loaded = ChartData::read(reopened.session.document(), 1)
            .unwrap()
            .unwrap();
        assert_eq!(loaded.patterns.as_ref().unwrap()[&48].random_seed, 42);
        assert_eq!(loaded.extra["FutureSetting"], data.extra["FutureSetting"]);
        if format == Format::Sm {
            assert!(String::from_utf8_lossy(&after).contains("#GAChartData"));
            assert!(String::from_utf8_lossy(&after).contains("CHART0001"));
        }
    }
}
#[test]
fn profile_resolution_is_explicit_and_marker_options_can_drive_a_preview() {
    let registry = Registry::builtin();
    assert_eq!(registry.patterns.len(), 2);
    assert_eq!(registry.performed.len(), 7);
    assert_eq!(registry.expressed.len(), 3);
    let source =
        Session::new(Document::parse(Arc::<[u8]>::from(bytes(Format::Ssc)), Format::Ssc).unwrap());
    let data = data();
    let options = data.options(48, &registry).unwrap();
    assert_eq!(options.seed, 42);
    assert_eq!(options.span.steps(12).unwrap(), (48, 96, 5));
    let preview = Task::capture(&source, 0, options)
        .unwrap()
        .run(&AtomicBool::new(false), |_, _| {})
        .unwrap();
    assert_eq!(preview.generated().steps.len(), 5);
    let mut unresolved = data.clone();
    unresolved
        .patterns
        .as_mut()
        .unwrap()
        .get_mut(&48)
        .unwrap()
        .pattern_config_guid = Guid::default();
    assert!(
        unresolved
            .options(48, &registry)
            .unwrap_err()
            .to_string()
            .contains("Missing pattern")
    );
    unresolved = data.clone();
    unresolved.expressed_chart_config = Guid::default();
    assert!(unresolved.options(48, &registry).is_err());
    assert!(data.options(0, &registry).is_err());
}
#[test]
fn unsupported_versions_malformed_data_and_atomic_failures_keep_history_and_bytes() {
    let mut source =
        Session::new(Document::parse(Arc::<[u8]>::from(bytes(Format::Ssc)), Format::Ssc).unwrap());
    source
        .edit_notes(
            0,
            "Edit",
            [(
                Position {
                    player: 0,
                    row: 12,
                    lane: 1,
                },
                Some(Note::tap()),
            )],
        )
        .unwrap();
    source.undo().unwrap();
    let before = source.serialize().unwrap();
    assert!(
        source
            .set_properties_atomic(
                Some(0),
                &[("TITLE", "temporary"), ("NOTES", "unsafe")],
                "Invalid"
            )
            .is_err()
    );
    assert_eq!(source.serialize().unwrap(), before);
    assert!(
        source
            .set_properties_atomic(Some(0), &[("One", "first"), ("ONE", "second")], "Duplicate")
            .is_err()
    );
    assert_eq!(source.serialize().unwrap(), before);
    assert!(source.redo().unwrap());
    source.set_text(Some(0), "GAChartDataVersion", "2").unwrap();
    source.set_text(Some(0), "GAChartData", "{}").unwrap();
    let before = source.serialize().unwrap();
    let revision = source.revision();
    assert!(data().save(&mut source, 0).is_err());
    assert_eq!(source.serialize().unwrap(), before);
    assert_eq!(source.revision(), revision);
    for json in [
        r#"{"Patterns":{"-1":{"Length":1}}}"#,
        r#"{"Patterns":{"2147483647":{"Length":1}}}"#,
        r#"{"Patterns":{"0":{"Length":-1}}}"#,
        r#"{"Patterns":{"0":{"Length":2147483648}}}"#,
    ] {
        assert!(ChartData::from_json(json).is_err(), "{json}");
    }
    assert!(ChartData::from_json(r#"{"MusicOffset":1e400}"#).is_err());
    assert!(ChartData::read(source.document(), 5).is_err());
    source
        .set_text(Some(0), "GAChartDataVersion", " +01 ")
        .unwrap();
    assert!(ChartData::read(source.document(), 0).unwrap().is_some());
}
#[test]
fn guid_parsing_and_marker_unknown_fields_survive_explicit_edits() {
    let guid: Guid = DYNAMIC.to_uppercase().parse().unwrap();
    assert_eq!(guid.to_string(), DYNAMIC);
    for text in ["", "not-a-guid", "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaax"] {
        assert!(text.parse::<Guid>().is_err());
    }
    let json = r#"{"Unknown":{"nested":true},"Patterns":{"0":{"Length":48,"Extension":"keep"}}}"#;
    let mut data = ChartData::from_json(json).unwrap();
    data.patterns
        .as_mut()
        .unwrap()
        .get_mut(&0)
        .unwrap()
        .random_seed = 7;
    let value = serde_json::to_value(data).unwrap();
    assert_eq!(value["Unknown"]["nested"], true);
    assert_eq!(value["Patterns"]["0"]["Extension"], "keep");
}

#[test]
fn unknown_metadata_fields_cannot_shadow_the_original_schema() {
    let mut data = data();
    data.extra
        .insert("MusicOffset".into(), serde_json::json!(5));
    assert!(data.validate().is_err());
    data.extra.clear();
    data.patterns
        .as_mut()
        .unwrap()
        .get_mut(&48)
        .unwrap()
        .extra
        .insert("Length".into(), serde_json::json!(1));
    assert!(data.validate().is_err());
}

#[test]
fn custom_tag_case_is_preserved_through_updates_history_and_chart_cloning() {
    let original = bytes(Format::Ssc);
    let mut source =
        Session::new(Document::parse(Arc::<[u8]>::from(original.clone()), Format::Ssc).unwrap());
    source.set_text(Some(0), "CamelCaseExtra", "first").unwrap();
    source
        .set_text(Some(0), "CAMELCASEEXTRA", "second")
        .unwrap();
    assert!(
        String::from_utf8_lossy(&source.serialize().unwrap()).contains("#CamelCaseExtra:second;")
    );
    source.undo().unwrap();
    assert!(
        String::from_utf8_lossy(&source.serialize().unwrap()).contains("#CamelCaseExtra:first;")
    );
    source.undo().unwrap();
    assert_eq!(source.serialize().unwrap(), original);
    source.redo().unwrap();
    source.redo().unwrap();
    let grid = source.grid(0).unwrap().clone();
    let revision = source.revision();
    source
        .append_chart(0, "dance-single", &grid, revision, "Clone")
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&source.serialize().unwrap())
            .matches("#CamelCaseExtra:second;")
            .count(),
        2
    );
}
