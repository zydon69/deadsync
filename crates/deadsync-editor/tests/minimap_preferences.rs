use deadsync_editor::{audio_preferences::File, minimap_viewport::Axis};
#[test]
fn original_defaults_ranges_enum_and_unknown_fields_survive_save() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    std::fs::write(&path, r#"{"Version":1,"MiniMap":{"Unknown":42,"MiniMapSpacingModeForVariable":"future"},"Other":{"x":1}}"#).unwrap();
    let mut file = File::open(&path).unwrap();
    let mut settings = file.minimap();
    assert_eq!(settings.variable_axis(), Axis::Time);
    assert_eq!(settings.visible_range(Axis::Time), 240.0);
    assert_eq!(settings.visible_range(Axis::Row), 24576.0);
    assert!(!settings.quantize());
    settings.set_variable_axis(Axis::Row);
    settings.set_visible_range(Axis::Time, 120.0).unwrap();
    settings.set_visible_range(Axis::Row, 384.0).unwrap();
    settings.set_quantize(true);
    file.set_minimap(settings);
    file.save().unwrap();
    assert_eq!(File::open(&path).unwrap().minimap(), settings);
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert!(json["MiniMap"]["MiniMapVisibleTimeRange"].is_u64());
    assert!(json["MiniMap"]["MiniMapVisibleRowRange"].is_u64());
    assert_eq!(json["MiniMap"]["Unknown"], 42);
    assert_eq!(json["Other"]["x"], 1);
}
#[test]
fn invalid_ranges_and_external_changes_never_overwrite_preferences() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    let mut file = File::open(&path).unwrap();
    let mut settings = file.minimap();
    settings.set_visible_range(Axis::Row, 0.0).unwrap();
    assert_eq!(settings.visible_range(Axis::Row), 0.0);
    for value in [-1.0, 0.5, f64::NAN, f64::INFINITY, u32::MAX as f64 + 1.0] {
        assert!(settings.set_visible_range(Axis::Row, value).is_err());
    }
    settings.set_quantize(true);
    file.set_minimap(settings);
    let external = b"{\"Version\":1,\"Other\":true}";
    std::fs::write(&path, external).unwrap();
    assert!(file.save().is_err());
    assert_eq!(std::fs::read(&path).unwrap(), external);
}
