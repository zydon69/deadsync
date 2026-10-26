use deadsync_editor::generation::{
    config::PerformedConfig,
    profile_controls::Editor,
    profiles::{Config, Kind, Profile, ProfileFile, TravelSpeed},
};
fn profile() -> Profile {
    Profile::parse(Kind::Performed,&serde_json::to_vec(&serde_json::json!({"Guid":"12345678-9abc-4def-8123-456789abcdef","Name":"Original","Config":PerformedConfig::balanced()})).unwrap()).unwrap()
}
#[test]
fn all_subdivisions_save_reopen_wrappers_and_seconds_without_accumulating_config_members() {
    let dir = tempfile::tempdir().unwrap();
    let mut profile = profile();
    let mut file = profile.export_new(dir.path()).unwrap();
    let mut config_count = None;
    for subdivision_index in 0..9 {
        let speed = TravelSpeed {
            subdivision_index,
            min_bpm: 125,
            max_bpm: 170,
        };
        assert!(profile.set_travel_speed(speed).unwrap());
        let Config::Performed(config) = profile.config() else {
            panic!()
        };
        let subdivision = TravelSpeed::SUBDIVISIONS[subdivision_index] as f64;
        assert_eq!(
            config.step_tightening.speed_min_time_seconds.to_bits(),
            (60.0 / (subdivision * 170.0) * 2.0).to_bits()
        );
        assert_eq!(
            config.step_tightening.speed_max_time_seconds.to_bits(),
            (60.0 / (subdivision * 125.0) * 2.0).to_bits()
        );
        let bytes = profile.serialize().unwrap();
        let count = String::from_utf8(bytes.clone())
            .unwrap()
            .matches("\"Config\"")
            .count();
        if let Some(expected) = config_count {
            assert_eq!(count, expected);
        }
        config_count = Some(count);
        assert!(!profile.set_travel_speed(speed).unwrap());
        assert_eq!(profile.serialize().unwrap(), bytes);
        file.save(&profile).unwrap();
        let reopened = ProfileFile::open(file.path()).unwrap();
        assert_eq!(reopened.profile().travel_speed(), Some(speed));
        assert_eq!(reopened.profile().config(), profile.config());
    }
}
#[test]
fn applying_wrapper_values_after_config_override_matches_original_after_fixture() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    let request: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("profile-performed-before.request.json")).unwrap(),
    )
    .unwrap();
    let mut profile = Profile::parse(
        Kind::Performed,
        request["Content"].as_str().unwrap().as_bytes(),
    )
    .unwrap();
    let expected: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("profile-performed-after.expected.json")).unwrap(),
    )
    .unwrap();
    let speed = TravelSpeed {
        subdivision_index: 2,
        min_bpm: 110,
        max_bpm: 190,
    };
    assert_eq!(profile.travel_speed(), Some(speed));
    assert!(profile.set_travel_speed(speed).unwrap());
    let expected: PerformedConfig = serde_json::from_value(expected["Config"].clone()).unwrap();
    assert_eq!(profile.config(), &Config::Performed(expected));
}
#[test]
fn invalid_speed_edits_are_atomic_and_direct_seconds_edits_can_follow_wrapper_edits() {
    let mut profile = profile();
    let before = profile.serialize().unwrap();
    for speed in [
        TravelSpeed {
            subdivision_index: 9,
            min_bpm: 125,
            max_bpm: 170,
        },
        TravelSpeed {
            subdivision_index: 3,
            min_bpm: 0,
            max_bpm: 170,
        },
        TravelSpeed {
            subdivision_index: 3,
            min_bpm: 171,
            max_bpm: 170,
        },
        TravelSpeed {
            subdivision_index: 3,
            min_bpm: 125,
            max_bpm: 1001,
        },
    ] {
        assert!(profile.set_travel_speed(speed).is_err());
        assert_eq!(profile.serialize().unwrap(), before);
    }
    let speed = TravelSpeed {
        subdivision_index: 3,
        min_bpm: 140,
        max_bpm: 190,
    };
    profile.set_travel_speed(speed).unwrap();
    let mut config = profile.config().clone();
    if let Config::Performed(c) = &mut config {
        c.step_tightening.speed_min_time_seconds = 0.1;
    }
    profile.set_config(config.clone()).unwrap();
    let reopened = Profile::parse(Kind::Performed, &profile.serialize().unwrap()).unwrap();
    assert_eq!(reopened.travel_speed(), Some(speed));
    assert_eq!(reopened.config(), &config);
    let mut editor = Editor::new(&config).unwrap();
    editor.apply_travel_speed(speed).unwrap();
    let Config::Performed(c) = editor.finish().unwrap() else {
        panic!()
    };
    assert_eq!(
        (
            c.step_tightening.speed_min_time_seconds,
            c.step_tightening.speed_max_time_seconds
        ),
        speed.intervals().unwrap()
    );
}
#[test]
fn presets_expose_original_editor_bpm_defaults_without_changing_generation_config() {
    use deadsync_editor::{
        document::{Document, Format},
        generation::profile_catalog::{BuiltinLabel, Task},
        history::Session,
    };
    use std::sync::{Arc, atomic::AtomicBool};
    let session = Session::new(
        Document::parse(
            Arc::<[u8]>::from(
                b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;"
                    .as_slice(),
            ),
            Format::Ssc,
        )
        .unwrap(),
    );
    let catalog = Task::capture(&session, 0, None)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    for entry in catalog.entries(Kind::Performed) {
        let profile = catalog
            .profile(&session, 0, Kind::Performed, &entry.guid)
            .unwrap();
        let speed = profile.travel_speed().unwrap();
        assert_eq!(speed.subdivision_index, 3);
        assert_eq!(speed.max_bpm, 170);
        assert_eq!(
            speed.min_bpm,
            if entry.builtin == Some(BuiltinLabel::StaminaDouble) {
                99
            } else {
                125
            }
        );
        let Config::Performed(c) = profile.config() else {
            panic!()
        };
        assert_eq!(
            (
                c.step_tightening.speed_min_time_seconds,
                c.step_tightening.speed_max_time_seconds
            ),
            speed.intervals().unwrap()
        );
    }
}
