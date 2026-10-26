use deadsync_editor::generation::{
    metadata::{DYNAMIC, Registry, SIXTEENTHS},
    profiles::{Config, Kind, Profile, ProfileFile, TravelSpeed},
};
#[test]
fn editor_defaults_match_existing_reference_presets_not_library_deserialization_defaults() {
    let registry = Registry::builtin();
    assert_eq!(
        Config::editor_defaults(Kind::Pattern),
        Config::Pattern(registry.patterns[&SIXTEENTHS.parse().unwrap()].clone())
    );
    assert_eq!(
        Config::editor_defaults(Kind::Expressed),
        Config::Expressed(registry.expressed[&DYNAMIC.parse().unwrap()].clone())
    );
    let Config::Expressed(config) = Config::editor_defaults(Kind::Expressed) else {
        panic!()
    };
    assert_eq!(config.min_level_for_brackets, 7);
    assert!(config.aggressive_when_uncoverable);
    assert_eq!(
        (
            config.balanced_bpm_for_aggressive,
            config.balanced_bpm_for_no_brackets
        ),
        (3.0, 1.0)
    );
}
#[test]
fn reset_all_families_preserves_identity_text_unknown_fields_and_is_idempotent() {
    for kind in [Kind::Pattern, Kind::Performed, Kind::Expressed] {
        let initial = match kind {
            Kind::Pattern => Config::Pattern(Default::default()),
            Kind::Expressed => Config::Expressed(Default::default()),
            Kind::Performed => Config::Performed(
                deadsync_editor::generation::config::PerformedConfig::preset(
                    deadsync_editor::generation::config::PerformedPreset::StaminaSingle,
                ),
            ),
        }
        .json()
        .unwrap()
        .to_string();
        let config = format!(
            "{{\"Vendor\":123456789012345678901234567890,{}",
            &initial[1..]
        );
        let source = format!(
            r#"{{"Guid":"12345678-9abc-4def-8123-456789abcdef","Name":"Keep name","Description":"Keep description","Future":1e999,"Config":{config}}}"#
        );
        let mut profile = Profile::parse(kind, source.as_bytes()).unwrap();
        let id = profile.guid().clone();
        assert!(profile.restore_defaults().unwrap());
        assert_eq!(profile.config(), &Config::editor_defaults(kind));
        assert_eq!(profile.guid(), &id);
        assert_eq!(profile.name(), Some("Keep name"));
        assert_eq!(profile.description(), Some("Keep description"));
        if kind == Kind::Performed {
            assert_eq!(profile.travel_speed(), Some(TravelSpeed::editor_defaults()));
        }
        let bytes = profile.serialize().unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(text.contains("1e999"));
        assert!(text.contains("123456789012345678901234567890"));
        assert!(!profile.restore_defaults().unwrap());
        assert_eq!(profile.serialize().unwrap(), bytes);
        let dir = tempfile::tempdir().unwrap();
        let file = profile.export_new(dir.path()).unwrap();
        let reopened = ProfileFile::open(file.path()).unwrap();
        assert_eq!(reopened.profile().config(), profile.config());
        assert_eq!(reopened.profile().guid(), &id);
    }
}
#[test]
fn reset_repairs_pattern_wrapper_even_when_later_config_already_has_default_subdivision() {
    let config = Config::editor_defaults(Kind::Pattern).json().unwrap();
    let source = format!(
        r#"{{"Guid":"12345678-9abc-4def-8123-456789abcdef","PatternType":"eighthNotes","Config":{config}}}"#
    );
    let mut profile = Profile::parse(Kind::Pattern, source.as_bytes()).unwrap();
    assert_eq!(profile.config(), &Config::editor_defaults(Kind::Pattern));
    assert!(profile.restore_defaults().unwrap());
    let value: serde_json::Value = serde_json::from_slice(&profile.serialize().unwrap()).unwrap();
    assert_eq!(value["PatternType"], "sixteenthNotes");
    assert!(!profile.restore_defaults().unwrap());
}
