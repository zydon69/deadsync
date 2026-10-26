use deadsync_editor::generation::profiles::{Config, Kind, Profile, ProfileFile};
use std::fs;
#[test]
fn explicit_export_preserves_identity_configuration_and_unknown_fields_for_every_family() {
    for kind in [Kind::Pattern, Kind::Performed, Kind::Expressed] {
        let dir = tempfile::tempdir().unwrap();
        let json = format!(
            r#"{{"Guid":"12345678-9abc-4def-8123-456789abcdef","Name":"Été","Future":1e999,"Config":{}}}"#,
            Config::editor_defaults(kind).json().unwrap()
        );
        let mut profile = Profile::parse(kind, json.as_bytes()).unwrap();
        profile.set_config(Config::editor_defaults(kind)).unwrap();
        let path = dir.path().join(format!("{}shared.json", kind.prefix()));
        let file = profile.export_to(&path).unwrap();
        assert_eq!(file.profile().guid(), profile.guid());
        let loaded = ProfileFile::open(&path).unwrap();
        assert_eq!(loaded.profile().config(), profile.config());
        assert_eq!(loaded.profile().name(), Some("Été"));
        assert!(fs::read_to_string(&path).unwrap().contains("1e999"));
        let before = fs::read(&path).unwrap();
        assert!(profile.export_to(&path).is_err());
        assert_eq!(fs::read(path).unwrap(), before);
    }
}
#[test]
fn wrong_kind_filename_and_missing_directory_publish_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let profile = Profile::parse(
        Kind::Expressed,
        br#"{"Guid":"12345678-9abc-4def-8123-456789abcdef","Config":{}}"#,
    )
    .unwrap();
    for name in [
        "pc-wrong.json",
        "ecc-wrong.txt",
        "wrong.json",
        "missing/ecc-profile.json",
    ] {
        let path = dir.path().join(name);
        assert!(profile.export_to(&path).is_err());
        assert!(!path.exists());
    }
}
