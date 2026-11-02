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
    for number in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, f64::MAX] {
        let mut value = defaults;
        assert!(value.set_channel_width(number).is_err());
        assert!(value.set_dense_scale(number).is_err());
        assert_eq!(value, defaults);
    }
    let mut value = defaults;
    value.set_channel_width(1.01).unwrap();
    assert_eq!(value.channel_width(), 1.01);
    assert_eq!(value.effective_channel_width(), 1.0);
    value.set_dense_scale(10.01).unwrap();
    assert_eq!(value.dense_scale(), 10.01);
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
        r#"{"WaveFormMaxXPercentagePerChannel":1e50}"#,
        r#"{"DenseScale":-1e50}"#,
        r#"{"DenseScale":1e400}"#,
    ] {
        let source = format!("{{\"Version\":1,\"Waveform\":{fields}}}");
        fs::write(&path, &source).unwrap();
        assert!(File::open(&path).is_err(), "{source}");
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
    }
}

#[test]
fn finite_source_values_outside_slider_ranges_remain_raw_and_noop_preserves_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    let source = r#"{ "Version":1,"Waveform":{"WaveFormMaxXPercentagePerChannel":1.1,"DenseScale":-1},"Vendor":{"keep":true} }"#;
    fs::write(&path, source).unwrap();
    let mut file = File::open(&path).unwrap();
    assert_eq!(file.waveform().channel_width(), 1.1);
    assert_eq!(file.waveform().dense_scale(), -1.0);
    assert!(!file.save().unwrap());
    assert_eq!(fs::read_to_string(&path).unwrap(), source);
}

#[test]
fn scroll_policy_defaults_round_trips_and_preserves_other_preferences() {
    use deadsync_editor::waveform_spacing::ScrollMode;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("policy.json");
    let original = r#"{ "Version":1,"Other":{"keep":true},"Audio":{"Future":5},"Waveform":{"FuturePolicy":[1,2],"SparseColor":{"Future":9}} }"#;
    fs::write(&path, original).unwrap();
    let mut file = File::open(&path).unwrap();
    assert_eq!(file.waveform().scroll_mode(), ScrollMode::MostCommonTempo);
    assert!(!file.save().unwrap());
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
    for mode in [
        ScrollMode::CurrentTempo,
        ScrollMode::CurrentTempoAndRate,
        ScrollMode::MostCommonTempo,
    ] {
        let mut settings = file.waveform();
        settings.set_scroll_mode(mode);
        file.set_waveform(settings);
        assert!(file.save().unwrap());
        assert_eq!(File::open(&path).unwrap().waveform().scroll_mode(), mode);
        let json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(json["Waveform"]["RowBasedWaveFormScrollMode"], mode.name());
        assert_eq!(json["Waveform"]["FuturePolicy"], serde_json::json!([1, 2]));
        assert_eq!(json["Other"]["keep"], true);
        assert_eq!(json["Audio"]["Future"], 5);
        assert_eq!(json["Waveform"]["SparseColor"]["Future"], 9);
    }
}
#[test]
fn scroll_policy_reads_original_numeric_values_and_rejects_unknown_values_without_rewrite() {
    use deadsync_editor::waveform_spacing::ScrollMode;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("policy.json");
    for (value, mode) in [
        (0, ScrollMode::CurrentTempo),
        (1, ScrollMode::CurrentTempoAndRate),
        (2, ScrollMode::MostCommonTempo),
    ] {
        let text =
            format!("{{\"Version\":1,\"Waveform\":{{\"RowBasedWaveFormScrollMode\":{value}}}}}");
        fs::write(&path, &text).unwrap();
        let mut file = File::open(&path).unwrap();
        assert_eq!(file.waveform().scroll_mode(), mode);
        assert!(!file.save().unwrap());
        assert_eq!(fs::read_to_string(&path).unwrap(), text);
    }
    for bad in [
        serde_json::json!("FutureMode"),
        serde_json::json!(3),
        serde_json::json!(-1),
        serde_json::json!(true),
        serde_json::json!(null),
        serde_json::json!([]),
    ] {
        let text = serde_json::json!({"Version":1,"Waveform":{"RowBasedWaveFormScrollMode":bad}})
            .to_string();
        fs::write(&path, &text).unwrap();
        assert!(File::open(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), text);
    }
}
