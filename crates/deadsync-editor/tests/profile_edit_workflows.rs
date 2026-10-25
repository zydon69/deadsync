use deadsync_editor::generation::{
    config::ExpressedConfig,
    profiles::{Config, Kind, Profile, ProfileFile},
};
use serde_json::Value;
use std::fs;

#[test]
fn edits_survive_original_wrapper_orders_and_repeated_edits_do_not_accumulate_config() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    let mut tested = 0;
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("profile-") || !name.ends_with(".request.json") {
            continue;
        }
        let request: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let kind = match request["Kind"].as_str().unwrap() {
            "Pattern" => Kind::Pattern,
            "Performed" => Kind::Performed,
            _ => Kind::Expressed,
        };
        let Ok(mut profile) = Profile::parse(kind, request["Content"].as_str().unwrap().as_bytes())
        else {
            continue;
        };
        let before = profile.serialize().unwrap();
        assert!(!profile.set_config(profile.config().clone()).unwrap());
        assert_eq!(profile.serialize().unwrap(), before);
        let mut edited = profile.config().clone();
        match &mut edited {
            Config::Pattern(c) => {
                c.beat_sub_division = if c.beat_sub_division == 4 { 8 } else { 4 };
                c.starting_foot_specified = 1 - c.starting_foot_specified;
            }
            Config::Performed(c) => {
                c.step_tightening.speed_min_time_seconds = 0.15;
                c.step_tightening.speed_max_time_seconds = 0.3;
            }
            Config::Expressed(c) => c.min_level_for_brackets += 1,
        }
        assert!(profile.set_config(edited.clone()).unwrap(), "{name}");
        let bytes = profile.serialize().unwrap();
        let reopened = Profile::parse(kind, &bytes).unwrap();
        assert_eq!(reopened.config(), &edited, "{name}");
        assert_eq!(profile.guid(), reopened.guid());
        let count = String::from_utf8(bytes)
            .unwrap()
            .matches("\"Config\"")
            .count();
        let mut next = edited.clone();
        match &mut next {
            Config::Pattern(c) => c.same_arrow_step_weight += 1,
            Config::Performed(c) => c.step_tightening.speed_max_time_seconds = 0.4,
            Config::Expressed(c) => c.min_level_for_brackets += 1,
        }
        assert!(profile.set_config(next).unwrap());
        assert_eq!(
            String::from_utf8(profile.serialize().unwrap())
                .unwrap()
                .matches("\"Config\"")
                .count(),
            count
        );
        tested += 1;
    }
    assert!(tested >= 15, "Only {tested} original fixtures tested");
}

#[test]
fn unknown_large_literals_are_preserved_and_invalid_edits_are_atomic() {
    let source=br#"{"Guid":"12345678-9abc-4def-8123-456789abcdef","Future":1e999,"Config":{"MinLevelForBrackets":3,"Unknown":{"Number":123456789012345678901234567890}}}"#;
    let mut profile = Profile::parse(Kind::Expressed, source).unwrap();
    let mut config = profile.config().clone();
    if let Config::Expressed(c) = &mut config {
        c.min_level_for_brackets = 7;
    }
    profile.set_config(config).unwrap();
    let bytes = profile.serialize().unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    assert!(text.contains("1e999"));
    assert!(text.contains("123456789012345678901234567890"));
    let mut invalid = profile.config().clone();
    if let Config::Expressed(c) = &mut invalid {
        c.balanced_bpm_for_aggressive = f64::NAN;
    }
    assert!(profile.set_config(invalid).is_err());
    assert_eq!(profile.serialize().unwrap(), bytes);
    assert!(
        profile
            .set_config(Config::Pattern(Default::default()))
            .is_err()
    );
    assert_eq!(profile.serialize().unwrap(), bytes);
}

#[test]
fn duplicate_has_new_identity_and_guarded_save_persists_effective_edits() {
    let source =
        br#"{"Guid":"12345678-9abc-4def-8123-456789abcdef","Name":"Original","Config":{}}"#;
    let original = Profile::parse(Kind::Expressed, source).unwrap();
    let mut copy = original.duplicate("Custom").unwrap();
    assert_ne!(copy.guid(), original.guid());
    assert_eq!(copy.name(), Some("Custom"));
    assert_eq!(copy.config(), original.config());
    let dir = tempfile::tempdir().unwrap();
    let mut file = copy.export_new(dir.path()).unwrap();
    copy.set_config(Config::Expressed(ExpressedConfig {
        min_level_for_brackets: 8,
        ..Default::default()
    }))
    .unwrap();
    file.save(&copy).unwrap();
    assert_eq!(
        ProfileFile::open(file.path()).unwrap().profile().config(),
        copy.config()
    );
    assert_eq!(original.name(), Some("Original"));
}

#[test]
fn nested_vendor_fields_survive_and_dynamic_dictionary_keys_can_be_removed() {
    use deadsync_editor::generation::config::{PerformedConfig, PerformedPreset};
    let config = PerformedConfig::preset(PerformedPreset::Balanced);
    let body = serde_json::to_string(&config).unwrap().replace(
        "\"StepTightening\":{",
        "\"StepTightening\":{\"Future\":1e999,",
    );
    assert!(body.contains("1e999"));
    let source = format!(r#"{{"Guid":"12345678-9abc-4def-8123-456789abcdef","Config":{body}}}"#);
    let mut profile = Profile::parse(Kind::Performed, source.as_bytes()).unwrap();
    let mut edited = profile.config().clone();
    if let Config::Performed(c) = &mut edited {
        assert!(c.arrow_weights.remove("dance-single").is_some());
        c.step_tightening.speed_max_time_seconds = 0.5;
    }
    profile.set_config(edited.clone()).unwrap();
    let bytes = profile.serialize().unwrap();
    assert!(std::str::from_utf8(&bytes).unwrap().contains("1e999"));
    assert_eq!(
        Profile::parse(Kind::Performed, &bytes).unwrap().config(),
        &edited
    );
    let Config::Performed(c) = profile.config() else {
        panic!()
    };
    assert!(!c.arrow_weights.contains_key("dance-single"));
}

#[test]
fn builtin_presets_can_only_be_exported_after_duplication() {
    use deadsync_editor::generation::{
        config::{PerformedConfig, PerformedPreset},
        metadata::PATTERN_BALANCED,
    };
    let source=serde_json::to_vec(&serde_json::json!({"Guid":PATTERN_BALANCED,"Name":"Preset","Config":PerformedConfig::preset(PerformedPreset::PatternBalanced)})).unwrap();
    let original = Profile::parse(Kind::Performed, &source).unwrap();
    let dir = tempfile::tempdir().unwrap();
    assert!(original.export_new(dir.path()).is_err());
    let copy = original.duplicate("My preset").unwrap();
    let file = copy.export_new(dir.path()).unwrap();
    assert_ne!(copy.guid(), original.guid());
    assert_eq!(
        ProfileFile::open(file.path()).unwrap().profile().config(),
        original.config()
    );
}

#[test]
fn saved_configuration_edit_changes_generation_of_existing_marker() {
    use deadsync_editor::{
        document::{Document, Format},
        generation::{
            config::PatternConfig,
            metadata::{ChartData, Definition, PATTERN_BALANCED},
            pattern_job::Task,
            profiles::Loaded,
        },
        history::Session,
    };
    use std::{
        collections::BTreeMap,
        sync::{Arc, atomic::AtomicBool},
    };
    let dir = tempfile::tempdir().unwrap();
    let mut profile = Profile::new_pattern("Editable", PatternConfig::sixteenths()).unwrap();
    let mut file = profile.export_new(dir.path()).unwrap();
    let song=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#METER:9;#NOTES:1000\n0100\n0010\n0001;";
    let mut session =
        Session::new(Document::parse(Arc::<[u8]>::from(song.as_slice()), Format::Ssc).unwrap());
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
                ..Default::default()
            },
        )])),
        ..Default::default()
    }
    .save(&mut session, 0)
    .unwrap();
    let snapshot = session.serialize().unwrap();
    for subdivision in [4, 8] {
        let mut edited = profile.config().clone();
        if let Config::Pattern(c) = &mut edited {
            c.beat_sub_division = subdivision;
        }
        profile.set_config(edited).unwrap();
        file.save(&profile).unwrap();
        let loaded = Loaded::directory(dir.path()).unwrap();
        assert!(loaded.issues.is_empty());
        let preview = Task::capture_saved(&session, 0, 48, &loaded.registry)
            .unwrap()
            .run(&AtomicBool::new(false), |_, _| {})
            .unwrap();
        assert_eq!(preview.generated().steps.len(), subdivision as usize + 1);
        assert_eq!(session.serialize().unwrap(), snapshot);
    }
}
