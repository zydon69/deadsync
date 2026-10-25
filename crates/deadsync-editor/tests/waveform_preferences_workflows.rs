use deadsync_editor::{Error, audio_preferences::File, waveform_preferences::Preferences};
use std::fs;
#[test]
fn original_defaults_ranges_and_missing_file_noop_are_explicit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    let mut file = File::open(&path).unwrap();
    let defaults = Preferences::default();
    assert!(defaults.show() && defaults.enabled());
    assert_eq!(defaults.channel_width(), 0.9);
    assert_eq!(defaults.dense_scale(), 6.0);
    assert_eq!(file.waveform(), defaults);
    assert!(!file.save().unwrap());
    assert!(!path.exists());
    for number in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.1, 11.0] {
        let mut value = defaults;
        assert!(value.set_channel_width(number).is_err());
        assert!(value.set_dense_scale(number).is_err());
        assert_eq!(value, defaults);
    }
    let mut value = defaults;
    assert!(value.set_channel_width(1.01).is_err());
    assert!(value.set_dense_scale(10.01).is_err());
    value.set_channel_width(0.0).unwrap();
    value.set_dense_scale(10.0).unwrap();
    assert_eq!(value.channel_width(), 0.0);
    assert_eq!(value.dense_scale(), 10.0);
}
#[test]
fn waveform_and_audio_save_together_preserving_unknown_fields_and_source_on_noop() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    let original = r#"{ "Version":1, "Audio":{"PreviewFadeInTime":0.2,"FutureAudio":7},"Waveform":{"FutureWave":[1,2],"DenseScale":4},"Other":true }"#;
    fs::write(&path, original).unwrap();
    let mut file = File::open(&path).unwrap();
    assert!(!file.save().unwrap());
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
    let mut wave = file.waveform();
    wave.set_show(false);
    wave.set_enabled(false);
    wave.set_channel_width(0.65).unwrap();
    wave.set_dense_scale(3.25).unwrap();
    file.set_waveform(wave);
    assert!(file.save().unwrap());
    let mut reopened = File::open(&path).unwrap();
    assert_eq!(reopened.waveform(), wave);
    assert_eq!(reopened.audio().fade_in(), 0.2);
    let mut audio = reopened.audio();
    audio.set_fade_out(2.0).unwrap();
    reopened.set_audio(audio);
    reopened.save().unwrap();
    assert_eq!(File::open(&path).unwrap().waveform(), wave);
    let json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(json["Waveform"]["FutureWave"], serde_json::json!([1, 2]));
    assert_eq!(json["Audio"]["FutureAudio"], 7);
    assert_eq!(json["Other"], true);
}
#[test]
fn waveform_save_refuses_conflicts_and_cancelled_draft_is_noop() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    let mut first = File::open(&path).unwrap();
    let mut stale = File::open(&path).unwrap();
    let original = first.waveform();
    let mut wave = original;
    wave.set_show(false);
    first.set_waveform(wave);
    first.set_waveform(original);
    assert!(!first.has_changes());
    assert!(!first.save().unwrap());
    assert!(!path.exists());
    first.set_waveform(wave);
    first.save().unwrap();
    let saved = fs::read(&path).unwrap();
    wave.set_dense_scale(3.0).unwrap();
    stale.set_waveform(wave);
    assert!(matches!(stale.save(), Err(Error::ExternalChange)));
    assert!(stale.has_changes());
    assert_eq!(fs::read(&path).unwrap(), saved);
}
#[test]
fn invalid_waveform_fields_never_rewrite_preference_source() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    for fields in [
        "null",
        "[]",
        r#"{"ShowWaveForm":1}"#,
        r#"{"EnableWaveForm":"yes"}"#,
        r#"{"WaveFormMaxXPercentagePerChannel":1.1}"#,
        r#"{"DenseScale":-1}"#,
        r#"{"DenseScale":1e400}"#,
    ] {
        let source = format!("{{\"Version\":1,\"Waveform\":{fields}}}");
        fs::write(&path, &source).unwrap();
        assert!(File::open(&path).is_err(), "{source}");
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
    }
}
