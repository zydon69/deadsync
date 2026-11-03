use deadsync_editor::{
    audio_preferences::File,
    autogen_preferences::{self as settings, Target},
};
#[test]
fn autogen_choices_roundtrip_and_unrelated_unknown_fields_remain() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("prefs.json");
    std::fs::write(&path,br#"{"Version":1,"LastSelectedAutogenChartType":"8","LastSelectedAutogenPerformedChartConfig":"6276c906-ea8f-43b3-9500-0ddeac7bdc22","Future":{"x":8}}"#).unwrap();
    let mut file = File::open(&path).unwrap();
    let mut prefs = file.autogen();
    assert_eq!(prefs.target.layout().unwrap(), "pump-double");
    prefs.performed = settings::STAMINA_DOUBLE.parse().unwrap();
    file.set_autogen(prefs.clone());
    file.save().unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved["LastSelectedAutogenChartType"], "8");
    assert_eq!(saved["Future"], serde_json::json!({"x":8}));
    assert_eq!(File::open(&path).unwrap().autogen(), prefs);
    prefs.target = Target::from_layout("smx-dual").unwrap();
    file.set_autogen(prefs.clone());
    file.save().unwrap();
    assert_eq!(File::open(&path).unwrap().autogen(), prefs);
}
#[test]
fn external_preferences_edit_refuses_publication_and_keeps_new_intent_in_memory() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("prefs.json");
    std::fs::write(&path, br#"{"Version":1}"#).unwrap();
    let mut file = File::open(&path).unwrap();
    let mut prefs = file.autogen();
    prefs.target = Target::from_layout("dance-double").unwrap();
    file.set_autogen(prefs.clone());
    std::fs::write(&path, br#"{"Version":1,"External":true}"#).unwrap();
    assert!(file.save().is_err());
    assert_eq!(file.autogen(), prefs);
    assert_eq!(
        std::fs::read(&path).unwrap(),
        br#"{"Version":1,"External":true}"#
    );
}
#[test]
fn unsupported_source_target_is_preserved_until_a_supported_target_is_explicitly_chosen() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("prefs.json");
    std::fs::write(
        &path,
        br#"{"Version":1,"LastSelectedAutogenChartType":"999"}"#,
    )
    .unwrap();
    let mut file = File::open(&path).unwrap();
    let mut prefs = file.autogen();
    assert!(prefs.target.layout().is_err());
    prefs.performed = settings::STAMINA_SINGLE.parse().unwrap();
    file.set_autogen(prefs);
    file.save().unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved["LastSelectedAutogenChartType"], "999");
}
