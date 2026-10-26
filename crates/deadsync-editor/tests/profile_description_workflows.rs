use deadsync_editor::generation::profiles::{Kind, Profile, ProfileFile};
#[test]
fn descriptions_preserve_missing_null_duplicates_unknown_numbers_and_effective_config() {
    for source in [
        r#"{"Guid":"12345678-9abc-4def-8123-456789abcdef","Config":{},"Future":1e999}"#,
        r#"{"Guid":"12345678-9abc-4def-8123-456789abcdef","Description":null,"Config":{},"Future":1e999}"#,
    ] {
        let mut profile = Profile::parse(Kind::Expressed, source.as_bytes()).unwrap();
        let before = profile.serialize().unwrap();
        let config = profile.config().clone();
        let id = profile.guid().clone();
        assert!(!profile.set_description(None).unwrap());
        assert_eq!(profile.serialize().unwrap(), before);
        assert!(profile.set_description(Some("Été 日本語: \\ ; #")).unwrap());
        assert_eq!(profile.description(), Some("Été 日本語: \\ ; #"));
        assert_eq!(profile.config(), &config);
        assert_eq!(profile.guid(), &id);
        assert!(
            std::str::from_utf8(&profile.serialize().unwrap())
                .unwrap()
                .contains("1e999")
        );
        let bytes = profile.serialize().unwrap();
        assert!(
            !profile
                .set_description(profile.description().map(str::to_owned).as_deref())
                .unwrap()
        );
        assert_eq!(profile.serialize().unwrap(), bytes);
        assert!(profile.set_description(Some("")).unwrap());
        assert_eq!(profile.description(), Some(""));
        assert!(profile.set_description(None).unwrap());
        assert_eq!(
            Profile::parse(Kind::Expressed, &profile.serialize().unwrap())
                .unwrap()
                .description(),
            None
        );
    }
    let mut profile=Profile::parse(Kind::Expressed,br#"{"Guid":"12345678-9abc-4def-8123-456789abcdef","Description":"Earlier","Config":{},"Description":"Effective"}"#).unwrap();
    assert!(profile.set_description(None).unwrap());
    let bytes = profile.serialize().unwrap();
    assert!(std::str::from_utf8(&bytes).unwrap().contains("Earlier"));
    assert_eq!(
        Profile::parse(Kind::Expressed, &bytes)
            .unwrap()
            .description(),
        None
    );
}
#[test]
fn descriptions_save_and_duplicate_for_all_profile_families_without_changing_source_identity() {
    use deadsync_editor::generation::{
        config::{ExpressedConfig, PatternConfig, PerformedConfig},
        profiles::Config,
    };
    let configs = [
        Config::Pattern(PatternConfig::sixteenths()),
        Config::Performed(PerformedConfig::balanced()),
        Config::Expressed(ExpressedConfig::default()),
    ];
    for config in configs {
        let id = "12345678-9abc-4def-8123-456789abcdef";
        let source = serde_json::to_vec(
            &serde_json::json!({"Guid":id,"Name":"Original","Config":config.json().unwrap()}),
        )
        .unwrap();
        let mut profile = Profile::parse(config.kind(), &source).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let mut file = profile.export_new(dir.path()).unwrap();
        profile.set_description(Some("Line one\nLine two")).unwrap();
        file.save(&profile).unwrap();
        let reopened = ProfileFile::open(file.path()).unwrap();
        assert_eq!(reopened.profile().guid(), profile.guid());
        assert_eq!(reopened.profile().description(), Some("Line one\nLine two"));
        assert_eq!(reopened.profile().config(), &config);
        let copy = profile.duplicate("Copy").unwrap();
        assert_ne!(copy.guid(), profile.guid());
        assert_eq!(copy.description(), profile.description());
        assert_eq!(profile.name(), Some("Original"));
    }
}
