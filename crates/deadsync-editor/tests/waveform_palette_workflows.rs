use deadsync_editor::{
    audio_preferences::File,
    waveform_palette::{Color, Palette, SparseMode},
};
use std::fs;
#[test]
fn original_sparse_modes_scale_rgb_but_preserve_dense_alpha_and_custom_colour() {
    let mut p = Palette::default();
    assert_eq!(p.dense().rgba(), [0.0, 0.389, 0.183, 0.8]);
    assert_eq!(p.background().rgba(), [0.0; 4]);
    assert_eq!(p.darken(), 0.8);
    let dense = Color::new([1.0, 0.5, 0.25, 0.4]).unwrap();
    let custom = Color::new([0.1, 0.2, 0.3, 0.9]).unwrap();
    p.set_dense(dense);
    p.set_sparse(custom);
    p.set_darken(0.25).unwrap();
    assert_eq!(p.resolved_sparse().rgba(), [0.25, 0.125, 0.0625, 0.4]);
    p.set_mode(SparseMode::SameAsDenseColor);
    assert_eq!(p.resolved_sparse(), dense);
    p.set_mode(SparseMode::UniqueColor);
    assert_eq!(p.resolved_sparse(), custom);
    p.set_mode(SparseMode::DarkerDenseColor);
    p.set_darken(0.0).unwrap();
    assert_eq!(p.resolved_sparse().rgba(), [0.0, 0.0, 0.0, 0.4]);
    assert_eq!(p.sparse(), custom);
}
#[test]
fn palette_values_are_validated_and_file_roundtrip_preserves_unknown_nested_fields() {
    let mut p = Palette::default();
    let original = p;
    for n in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.1, 1.01] {
        assert!(Color::new([n, 0.0, 0.0, 1.0]).is_err());
        assert!(p.set_darken(n).is_err());
        assert_eq!(p, original);
    }
    assert!(p.dense().with_component(4, 0.5).is_err());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    let source = r#"{"Version":1,"Waveform":{"WaveFormDenseColor":{"X":0,"Y":0.5,"Z":1,"W":0.4,"Future":7},"WaveFormSparseColor":[1,0,0,0.3],"WaveFormSparseColorOption":"UniqueColor","FutureWave":true}}"#;
    fs::write(&path, source).unwrap();
    let mut file = File::open(&path).unwrap();
    assert!(!file.save().unwrap());
    assert_eq!(fs::read_to_string(&path).unwrap(), source);
    let expected = file.waveform().palette();
    let mut audio = file.audio();
    audio.set_fade_in(0.25).unwrap();
    file.set_audio(audio);
    file.save().unwrap();
    assert_eq!(File::open(&path).unwrap().waveform().palette(), expected);
    let json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(json["Waveform"]["WaveFormDenseColor"]["Future"], 7);
    assert_eq!(json["Waveform"]["FutureWave"], true);
}
#[test]
fn invalid_palette_documents_remain_read_only_including_out_of_range_before_float_rounding() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    for fields in [
        r#""WaveFormDenseColor":null"#,
        r#""WaveFormDenseColor":[0,0,0]"#,
        r#""WaveFormDenseColor":[0,0,0,1.000000001]"#,
        r#""WaveFormDenseColor":{"X":0,"Y":0,"Z":0}"#,
        r#""WaveFormSparseColorOption":"Future""#,
        r#""WaveFormSparseColorScale":1.000000001"#,
        r#""WaveFormSparseColorScale":"bad""#,
    ] {
        let source = format!("{{\"Version\":1,\"Waveform\":{{{fields}}}}}");
        fs::write(&path, &source).unwrap();
        assert!(File::open(&path).is_err(), "{source}");
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
    }
}
