use deadsync_editor::performance::monitor::{Monitor, Timing};
use serde_json::Value;

#[test]
fn original_preferences_defaults_conversion_and_restoration_match() {
    use deadsync_editor::performance::preferences::{Preferences, Scale};
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/performance-preferences.expected.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let mut input = case["Input"].as_object().unwrap().clone();
        // Explicit source conversion precedes the native Version1 reader.
        input.insert(
            "FrameMaxTime".into(),
            Scale::read_source(input.get("FrameMaxTime"))
                .unwrap()
                .source_name()
                .into(),
        );
        let preferences = Preferences::read(&input).unwrap();
        let mut fields = serde_json::Map::new();
        preferences.write(&mut fields).unwrap();
        assert_eq!(
            Value::Object(fields),
            case["Initial"],
            "input {}",
            case["Input"]
        );
        assert_eq!(
            !preferences.restoration_needed(),
            case["UsingDefaults"].as_bool().unwrap()
        );
        let changed = Preferences::read(case["Changed"].as_object().unwrap()).unwrap();
        let mut fields = serde_json::Map::new();
        changed.restored().write(&mut fields).unwrap();
        assert_eq!(Value::Object(fields), case["Restored"]);
    }
}

#[test]
fn performance_preferences_preserve_unknown_fields_and_reject_nonfinite_edits() {
    use deadsync_editor::performance::preferences::Preferences;
    let mut fields = serde_json::json!({"Future":17})
        .as_object()
        .unwrap()
        .clone();
    let mut preferences = Preferences::default();
    preferences.explicit_seconds = f64::NAN;
    let original = fields.clone();
    assert!(preferences.write(&mut fields).is_err());
    assert_eq!(fields, original);
    preferences.explicit_seconds = -0.5;
    preferences.max_frames = -3;
    preferences.write(&mut fields).unwrap();
    assert_eq!(fields["Future"], 17);
    assert_eq!(Preferences::read(&fields).unwrap(), preferences);
}

#[test]
fn performance_file_round_trip_preserves_other_groups_and_checks_external_changes() {
    use deadsync_editor::{audio_preferences::File, performance::preferences::Scale};
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("preferences.json");
    std::fs::write(&path, br#"{"Version":1,"Performance":{"Future":17,"FrameMaxTime":"99","ShowPerformanceWindow":true},"Vendor":{"raw":"kept"}}"#).unwrap();
    let mut file = File::open(&path).unwrap();
    assert_eq!(file.performance().scale, Scale::Unknown(99));
    let mut preferences = file.performance();
    preferences.paused = true;
    preferences.max_frames = 128;
    file.set_performance(preferences).unwrap();
    file.save().unwrap();
    assert_eq!(File::open(&path).unwrap().performance(), preferences);
    let mut disk: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(disk["Performance"]["Future"], 17);
    assert_eq!(disk["Vendor"]["raw"], "kept");
    disk["Vendor"]["raw"] = "external".into();
    let external = serde_json::to_vec(&disk).unwrap();
    std::fs::write(&path, &external).unwrap();
    preferences.paused = false;
    file.set_performance(preferences).unwrap();
    assert!(matches!(
        file.save(),
        Err(deadsync_editor::Error::ExternalChange)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), external);
}

#[test]
fn original_performance_restore_history_matches_and_keeps_chartless_song_clean() {
    use deadsync_editor::{
        audio_preferences::File,
        document::{Document, Format},
        history::Session,
        performance::preferences::Preferences,
    };
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/performance-preferences.expected.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("preferences.json");
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({"Version":1,
            "Performance":case["Changed"],"Vendor":17}))
            .unwrap(),
        )
        .unwrap();
        let file = File::open(&path).unwrap();
        let mut session = Session::new(
            Document::parse(b"#VERSION:0.83;#TITLE:Untouched;".as_slice(), Format::Ssc).unwrap(),
        );
        let song = session.serialize().unwrap();
        assert!(
            session
                .restore_performance_preferences(file.performance(), Some(&file))
                .unwrap()
        );
        assert_eq!(
            session
                .take_performance_preferences_update()
                .unwrap()
                .preferences,
            Preferences::read(case["Restored"].as_object().unwrap()).unwrap()
        );
        assert_eq!(session.undo_label(), Some("Restore Performance defaults"));
        assert!(session.can_undo_application_change());
        assert!(session.undo().unwrap());
        assert_eq!(
            session
                .take_performance_preferences_update()
                .unwrap()
                .preferences,
            Preferences::read(case["Undone"].as_object().unwrap()).unwrap()
        );
        assert!(session.redo().unwrap());
        assert_eq!(
            session
                .take_performance_preferences_update()
                .unwrap()
                .preferences,
            Preferences::read(case["Redone"].as_object().unwrap()).unwrap()
        );
        assert!(
            !session
                .restore_performance_preferences(
                    file.performance().restored(),
                    Some(&File::open(&path).unwrap())
                )
                .unwrap()
        );
        assert_eq!(session.serialize().unwrap(), song);
        assert!(!session.is_dirty());
        let disk: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(disk["Vendor"], 17);
    }
}

#[test]
fn performance_history_merges_unowned_visibility_and_refuses_owned_external_changes() {
    use deadsync_editor::{
        audio_preferences::File,
        document::{Document, Format},
        history::Session,
    };
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("preferences.json");
    std::fs::write(&path, br#"{"Version":1,"Performance":{"ShowPerformanceWindow":true,"MaxFramesToDraw":128,"Future":17}}"#).unwrap();
    let file = File::open(&path).unwrap();
    let mut session =
        Session::new(Document::parse(b"#TITLE:Clean;".as_slice(), Format::Ssc).unwrap());
    session
        .restore_performance_preferences(file.performance(), Some(&file))
        .unwrap();
    session.take_performance_preferences_update();
    let mut external = File::open(&path).unwrap();
    let mut preferences = external.performance();
    preferences.visible = false;
    external.set_performance(preferences).unwrap();
    external.save().unwrap();
    session.undo().unwrap();
    let merged = session
        .take_performance_preferences_update()
        .unwrap()
        .preferences;
    assert!(!merged.visible);
    assert_eq!(merged.max_frames, 128);
    let mut external = File::open(&path).unwrap();
    let mut preferences = external.performance();
    preferences.max_frames = 256;
    external.set_performance(preferences).unwrap();
    external.save().unwrap();
    let disk = std::fs::read(&path).unwrap();
    assert!(matches!(
        session.redo(),
        Err(deadsync_editor::Error::ExternalChange)
    ));
    assert_eq!(session.redo_label(), Some("Restore Performance defaults"));
    assert_eq!(std::fs::read(&path).unwrap(), disk);
    assert!(session.take_performance_preferences_update().is_none());
}

#[test]
fn original_recorder_snapshots_match_ring_pause_resume_and_accumulation() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/performance-monitor.expected.json")).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    let results = fixture["results"].as_array().unwrap();
    for (case, expected) in cases.iter().zip(results) {
        assert_eq!(case["name"], expected["name"]);
        let mut monitor = Monitor::with_capacity(case["capacity"].as_u64().unwrap() as usize);
        assert_eq!(monitor.frames().count(), 0);
        for (operation, snapshot) in case["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(expected["snapshots"].as_array().unwrap())
        {
            match operation[0].as_str().unwrap() {
                "begin" => monitor.begin_frame(operation[1].as_i64().unwrap()),
                "add" => monitor.add(Timing::EditorCpu, operation[1].as_i64().unwrap()),
                "enabled" => monitor.set_enabled(operation[1].as_bool().unwrap()),
                _ => unreachable!(),
            }
            let actual: Vec<[i64; 2]> = monitor
                .frames()
                .map(|frame| [frame.ticks(Timing::Frame), frame.ticks(Timing::EditorCpu)])
                .collect();
            assert_eq!(
                serde_json::to_value(actual).unwrap(),
                *snapshot,
                "{} after {operation}",
                case["name"]
            );
        }
    }
}

#[test]
fn timed_operations_accumulate_and_paused_recording_is_inert() {
    let mut monitor = Monitor::with_capacity(3);
    monitor.start(Timing::Update, 10);
    monitor.end(Timing::Update, 20);
    monitor.begin_frame(100);
    monitor.start(Timing::Update, 101);
    monitor.end(Timing::Update, 108);
    monitor.start(Timing::Update, 110);
    monitor.end(Timing::Update, 114);
    monitor.set_enabled(false);
    monitor.start(Timing::Update, 200);
    monitor.end(Timing::Update, 999);
    monitor.begin_frame(120);
    let frame = monitor.frames().next().unwrap();
    assert_eq!(frame.ticks(Timing::Update), 11);
    assert_eq!(frame.ticks(Timing::Frame), 20);
    assert_eq!(monitor.frames().count(), 1);
}
