use deadsync_editor::{
    Error,
    audio_preferences::File,
    zoom::{MIN_ZOOM, Manager},
    zoom_preferences::Preferences,
};
use std::fs;
#[test]
fn original_defaults_and_ranges_are_validated_before_mutation() {
    let defaults = Preferences::default();
    assert_eq!(
        (
            defaults.duration(),
            defaults.multiplier(),
            defaults.size_cap(),
            defaults.limited()
        ),
        (0.1, 1.2, 1.0, false)
    );
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, 5.0] {
        let mut p = defaults;
        assert!(p.set_duration(value).is_err());
        assert!(p.set_multiplier(value).is_err());
        assert!(p.set_size_cap(value).is_err());
        assert_eq!(p, defaults);
    }
    let mut p = defaults;
    assert!(p.set_size_cap(0.0).is_err());
    assert!(p.set_duration(1.01).is_err());
    assert!(p.set_multiplier(0.99).is_err());
    p.set_duration(0.0).unwrap();
    p.set_multiplier(4.0).unwrap();
    p.set_size_cap(MIN_ZOOM).unwrap();
    p.set_limited(true);
    let mut manager = Manager::default();
    p.apply(&mut manager).unwrap();
    assert_eq!(manager.spacing_zoom(), MIN_ZOOM);
    assert_eq!(manager.size_zoom(), MIN_ZOOM);
    assert_eq!(manager.interpolation_duration(), 0.0);
    assert_eq!(manager.zoom_multiplier(), 4.0);
}
#[test]
fn zoom_preferences_roundtrip_preserves_other_groups_unknown_fields_and_exact_noop_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    let original = r#"{ "Version":1,"Zoom":{"FutureZoom":[1,2]},"Audio":{"PreviewFadeInTime":0.25},"Waveform":{"DenseScale":3},"FutureRoot":true }"#;
    fs::write(&path, original).unwrap();
    let mut file = File::open(&path).unwrap();
    assert!(!file.save().unwrap());
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
    let mut settings = file.zoom();
    settings.set_duration(0.25).unwrap();
    settings.set_multiplier(1.7).unwrap();
    settings.set_size_cap(0.75).unwrap();
    settings.set_limited(true);
    file.set_zoom(settings);
    assert!(file.save().unwrap());
    let mut reopened = File::open(&path).unwrap();
    assert_eq!(reopened.zoom(), settings);
    let mut audio = reopened.audio();
    audio.set_fade_out(2.0).unwrap();
    reopened.set_audio(audio);
    reopened.save().unwrap();
    assert_eq!(File::open(&path).unwrap().zoom(), settings);
    let json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(json["Zoom"]["FutureZoom"], serde_json::json!([1, 2]));
    assert_eq!(json["FutureRoot"], true);
    assert_eq!(json["Waveform"]["DenseScale"], 3.0);
    assert_eq!(json["Audio"]["PreviewFadeInTime"], 0.25);
}
#[test]
fn malformed_and_future_zoom_settings_never_rewrite_the_source() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    for zoom in [
        "null",
        "[]",
        "true",
        r#"{"SizeCap":0}"#,
        r#"{"ZoomMultiplier":4.1}"#,
        r#"{"ScrollInterpolationDuration":-0.1}"#,
        r#"{"LimitZoomToSize":1}"#,
        r#"{"SizeCap":"1"}"#,
    ] {
        let original = format!(r#"{{"Version":1,"Zoom":{zoom}}}"#);
        fs::write(&path, &original).unwrap();
        assert!(File::open(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
    }
    fs::write(&path, r#"{"Version":2,"Zoom":{}}"#).unwrap();
    assert!(File::open(&path).is_err());
}
#[test]
fn missing_files_and_external_modifications_keep_zoom_writes_explicit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    let mut file = File::open(&path).unwrap();
    assert!(!file.save().unwrap());
    assert!(!path.exists());
    let mut settings = file.zoom();
    settings.set_duration(0.2).unwrap();
    file.set_zoom(settings);
    let external = r#"{"Version":1,"Zoom":{"ZoomMultiplier":2}}"#;
    fs::write(&path, external).unwrap();
    assert!(matches!(file.save(), Err(Error::ExternalChange)));
    assert!(file.has_changes());
    assert_eq!(fs::read_to_string(&path).unwrap(), external);
}
