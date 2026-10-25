use deadsync_editor::{
    Error,
    document::{Document, Format},
    generation::{
        config::PatternConfig,
        metadata::{ChartData, Definition, Guid, PATTERN_BALANCED, Registry, SIXTEENTHS},
        pattern_job::Task,
        profiles::{Config, Kind, Loaded, Profile, ProfileFile},
    },
    history::Session,
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor")
}
fn fixture(name: &str) -> Value {
    serde_json::from_slice(&fs::read(root().join(format!("profile-{name}.request.json"))).unwrap())
        .unwrap()
}
fn kind(value: &Value) -> Kind {
    match value["Kind"].as_str().unwrap() {
        "Pattern" => Kind::Pattern,
        "Performed" => Kind::Performed,
        _ => Kind::Expressed,
    }
}
fn compare(a: &Value, b: &Value) {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => assert_eq!(a.as_f64(), b.as_f64()),
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len());
            for (k, v) in a {
                compare(v, b.get(k).unwrap_or_else(|| panic!("Missing {k}")));
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b) {
                compare(a, b);
            }
        }
        _ => assert_eq!(a, b),
    }
}
#[test]
fn wrapper_setter_order_and_effective_configs_match_original_editor_bodies() {
    let root = root();
    let mut count = 0;
    for entry in fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("profile-") || !name.ends_with(".request.json") {
            continue;
        }
        let input: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let expected: Value = serde_json::from_slice(
            &fs::read(root.join(name.replace(".request.json", ".expected.json"))).unwrap(),
        )
        .unwrap();
        let result = Profile::parse(kind(&input), input["Content"].as_str().unwrap().as_bytes());
        let valid = expected["Success"] == true && expected["Valid"] == true;
        assert_eq!(result.is_ok(), valid, "{name}: {:?}", result.as_ref().err());
        if let Ok(profile) = result {
            assert_eq!(
                profile.guid().to_string(),
                expected["Guid"].as_str().unwrap()
            );
            assert_eq!(profile.name(), expected["Name"].as_str());
            assert_eq!(profile.description(), expected["Description"].as_str());
            compare(&profile.config().json().unwrap(), &expected["Config"]);
            let reopened = Profile::parse(kind(&input), &profile.serialize().unwrap()).unwrap();
            assert_eq!(
                reopened.config(),
                profile.config(),
                "{name}: export must preserve setter order"
            );
        }
        count += 1;
    }
    assert_eq!(count, 20);
}
#[test]
fn imported_unknown_fields_and_wrapper_order_survive_rename_and_guarded_save() {
    let input = fixture("unknown");
    let mut profile =
        Profile::parse(Kind::Pattern, input["Content"].as_str().unwrap().as_bytes()).unwrap();
    let config = profile.config().clone();
    let guid = profile.guid().clone();
    profile.rename("Renamed / motif é").unwrap();
    let json: Value = serde_json::from_slice(&profile.serialize().unwrap()).unwrap();
    assert_eq!(json["Unknown"]["keep"], true);
    assert_eq!(json["Config"]["Future"], 123);
    assert_eq!(profile.config(), &config);
    assert_eq!(profile.guid(), &guid);
    let dir = tempfile::tempdir().unwrap();
    let mut file = profile.export_new(dir.path()).unwrap();
    let mut other = ProfileFile::open(file.path()).unwrap();
    let mut edited = file.profile().clone();
    edited.rename("First writer").unwrap();
    file.save(&edited).unwrap();
    let mut stale = other.profile().clone();
    stale.rename("Second writer").unwrap();
    assert!(matches!(other.save(&stale), Err(Error::ExternalChange)));
    assert_eq!(stale.name(), Some("Second writer"));
    assert_eq!(other.profile().name(), Some("Renamed / motif é"));
    assert_eq!(
        ProfileFile::open(file.path()).unwrap().profile().name(),
        Some("First writer")
    );
    let bytes = fs::read(file.path()).unwrap();
    assert!(edited.export_new(dir.path()).is_err());
    assert_eq!(fs::read(file.path()).unwrap(), bytes);
    fs::write(file.path(), b"external edit").unwrap();
    assert!(matches!(file.save(&edited), Err(Error::ExternalChange)));
    assert_eq!(fs::read(file.path()).unwrap(), b"external edit");
}
#[test]
fn all_subdivisions_can_be_exported_loaded_resolved_and_used_by_a_saved_marker() {
    let dir = tempfile::tempdir().unwrap();
    let mut ids = Vec::new();
    for subdivision in [1, 2, 3, 4, 6, 8, 12, 16, 48] {
        let profile = Profile::new_pattern(
            &format!("Subdivision {subdivision}"),
            PatternConfig {
                beat_sub_division: subdivision,
                ..PatternConfig::sixteenths()
            },
        )
        .unwrap();
        assert_ne!(profile.guid(), &Guid::default());
        assert!(!ids.iter().any(|(id, _)| id == profile.guid()));
        let file = profile.export_new(dir.path()).unwrap();
        assert!(
            file.path()
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("pc-")
        );
        ids.push((profile.guid().clone(), subdivision));
    }
    let loaded = Loaded::directory(dir.path()).unwrap();
    assert!(loaded.issues.is_empty());
    assert_eq!(loaded.profiles.len(), 9);
    assert_eq!(loaded.registry.patterns.len(), 11);
    for (guid, subdivision) in ids {
        let song=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#METER:9;#NOTES:1000\n0100\n0010\n0001;";
        let mut session =
            Session::new(Document::parse(Arc::<[u8]>::from(song.as_slice()), Format::Ssc).unwrap());
        ChartData {
            patterns: Some(BTreeMap::from([(
                48,
                Definition {
                    pattern_config_guid: guid,
                    performed_chart_config_guid: PATTERN_BALANCED.parse().unwrap(),
                    length: 48,
                    random_seed: 42,
                    start_position_inclusive: true,
                    end_position_inclusive: true,
                    ..Definition::default()
                },
            )])),
            ..ChartData::default()
        }
        .save(&mut session, 0)
        .unwrap();
        let snapshot = session.serialize().unwrap();
        let preview = Task::capture_saved(&session, 0, 48, &loaded.registry)
            .unwrap()
            .run(&AtomicBool::new(false), |_, _| {})
            .unwrap();
        assert_eq!(preview.generated().steps.len(), subdivision as usize + 1);
        assert_eq!(session.serialize().unwrap(), snapshot);
    }
}
#[test]
fn directory_load_preserves_bad_files_and_builtins_and_reports_duplicate_identities() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixture("pattern");
    let mut first =
        Profile::parse(Kind::Pattern, input["Content"].as_str().unwrap().as_bytes()).unwrap();
    first.rename("First").unwrap();
    fs::write(dir.path().join("pc-a.json"), first.serialize().unwrap()).unwrap();
    let input = fixture("pattern-after");
    let mut second =
        Profile::parse(Kind::Pattern, input["Content"].as_str().unwrap().as_bytes()).unwrap();
    second.rename("Second").unwrap();
    fs::write(dir.path().join("pc-z.json"), second.serialize().unwrap()).unwrap();
    fs::write(dir.path().join("pc-bad.json"), b"invalid").unwrap();
    let builtin = input["Content"]
        .as_str()
        .unwrap()
        .replace("12345678-9abc-4def-8123-456789abcdef", SIXTEENTHS);
    fs::write(dir.path().join("pc-default.json"), builtin.as_bytes()).unwrap();
    fs::write(dir.path().join("unrelated.txt"), b"keep").unwrap();
    let before = fs::read(dir.path().join("pc-bad.json")).unwrap();
    let loaded = Loaded::directory(dir.path()).unwrap();
    assert_eq!(loaded.issues.len(), 3);
    assert_eq!(loaded.profiles.len(), 1);
    assert_eq!(
        loaded
            .registry
            .patterns
            .get(second.guid())
            .unwrap()
            .beat_sub_division,
        2
    );
    assert_eq!(
        loaded
            .registry
            .patterns
            .get(&SIXTEENTHS.parse().unwrap())
            .unwrap()
            .beat_sub_division,
        4
    );
    assert_eq!(fs::read(dir.path().join("pc-bad.json")).unwrap(), before);
    assert_eq!(
        fs::read(dir.path().join("pc-default.json")).unwrap(),
        builtin.as_bytes()
    );
    assert_eq!(fs::read(dir.path().join("unrelated.txt")).unwrap(), b"keep");
    assert!(
        Profile::parse(Kind::Pattern, b"{}")
            .unwrap()
            .register(&mut Registry::builtin())
            .is_ok()
    ); // reference library defaults are a valid empty-weight config
}
#[test]
fn missing_ids_receive_stable_exported_identity_and_wrong_kind_files_are_rejected() {
    let mut value = fixture("pattern");
    let mut body: Value = serde_json::from_str(value["Content"].as_str().unwrap()).unwrap();
    body.as_object_mut().unwrap().remove("Guid");
    value["Content"] = Value::from(body.to_string());
    let profile =
        Profile::parse(Kind::Pattern, value["Content"].as_str().unwrap().as_bytes()).unwrap();
    let reloaded = Profile::parse(Kind::Pattern, &profile.serialize().unwrap()).unwrap();
    assert_eq!(profile.guid(), reloaded.guid());
    assert!(Kind::from_path(std::path::Path::new("pc-config.JSON")).is_none());
    assert!(Kind::from_path(std::path::Path::new("file.json")).is_none());
    let other = Profile::new_pattern("Other", PatternConfig::sixteenths()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut file = profile.export_new(dir.path()).unwrap();
    assert!(file.save(&other).is_err());
    assert!(matches!(profile.config(), Config::Pattern(_)));
}

#[test]
fn absent_directories_are_lazy_and_cancelled_loads_do_not_read_or_publish_profiles() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("not-created");
    let loaded = Loaded::directory(&path).unwrap();
    assert_eq!(loaded.registry.patterns.len(), 2);
    assert!(loaded.issues.is_empty());
    assert!(!path.exists());
    assert!(Loaded::directory_checked(&path, &AtomicBool::new(true)).is_err());
    assert!(!path.exists());
}
#[test]
fn all_profile_kinds_can_share_a_guid_and_resolve_in_their_own_namespaces() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["pattern", "performed-after", "expressed"] {
        let input = fixture(name);
        Profile::parse(kind(&input), input["Content"].as_str().unwrap().as_bytes())
            .unwrap()
            .export_new(dir.path())
            .unwrap();
    }
    let loaded = Loaded::directory(dir.path()).unwrap();
    assert_eq!(loaded.profiles.len(), 3);
    assert!(loaded.issues.is_empty());
    let guid: Guid = "12345678-9abc-4def-8123-456789abcdef".parse().unwrap();
    assert!(loaded.registry.patterns.contains_key(&guid));
    assert!(loaded.registry.performed.contains_key(&guid));
    assert!(loaded.registry.expressed.contains_key(&guid));
    let metadata = ChartData {
        expressed_chart_config: guid.clone(),
        patterns: Some(BTreeMap::from([(
            48,
            Definition {
                pattern_config_guid: guid.clone(),
                performed_chart_config_guid: guid.clone(),
                length: 48,
                random_seed: 42,
                start_position_inclusive: true,
                end_position_inclusive: true,
                ..Definition::default()
            },
        )])),
        ..ChartData::default()
    };
    let options = metadata.options(48, &loaded.registry).unwrap();
    assert_eq!(
        &options.pattern,
        loaded.registry.patterns.get(&guid).unwrap()
    );
    assert_eq!(
        &options.performed,
        loaded.registry.performed.get(&guid).unwrap()
    );
    assert_eq!(
        &options.expressed,
        loaded.registry.expressed.get(&guid).unwrap()
    );
}
#[test]
fn directory_jobs_bind_the_song_snapshot_and_support_cancellation() {
    use deadsync_editor::{
        generation::pattern_job::SavedTask,
        notes::{Note, Position},
    };
    let dir = tempfile::tempdir().unwrap();
    let profile = Profile::new_pattern("Custom", PatternConfig::eighths()).unwrap();
    profile.export_new(dir.path()).unwrap();
    let mut source=Session::new(Document::parse(Arc::<[u8]>::from(b"#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#METER:9;#NOTES:1000\n0100\n0010\n0001;".as_slice()),Format::Ssc).unwrap());
    ChartData {
        patterns: Some(BTreeMap::from([(
            48,
            Definition {
                pattern_config_guid: profile.guid().clone(),
                performed_chart_config_guid: PATTERN_BALANCED.parse().unwrap(),
                length: 48,
                random_seed: 42,
                start_position_inclusive: true,
                end_position_inclusive: true,
                ..Definition::default()
            },
        )])),
        ..ChartData::default()
    }
    .save(&mut source, 0)
    .unwrap();
    let task = SavedTask::capture(&source, 0, 48, dir.path()).unwrap();
    source
        .edit_notes(
            0,
            "Edit",
            [(
                Position {
                    row: 180,
                    lane: 0,
                    player: 0,
                },
                Some(Note::tap()),
            )],
        )
        .unwrap();
    let result = task.run(&AtomicBool::new(false), |_, _| {}).unwrap();
    assert!(!result.is_current(&source));
    assert!(result.apply(&mut source, 0).is_err());
    let before = source.serialize().unwrap();
    let mut job = SavedTask::capture(&source, 0, 48, dir.path())
        .unwrap()
        .spawn()
        .unwrap();
    job.cancel();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        if let Some(result) = job.poll() {
            assert!(result.is_err());
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert_eq!(source.serialize().unwrap(), before);
}

#[test]
fn unknown_numeric_literals_are_preserved_without_rounding_or_overflow() {
    let input = fixture("unknown-big-numbers");
    let mut profile =
        Profile::parse(Kind::Pattern, input["Content"].as_str().unwrap().as_bytes()).unwrap();
    profile.rename("Keep unknown numeric values").unwrap();
    let bytes = profile.serialize().unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    assert!(text.contains("123456789012345678901234567890"));
    assert!(text.contains("1e999"));
    let reopened = Profile::parse(Kind::Pattern, &bytes).unwrap();
    assert_eq!(reopened.config(), profile.config());
}
