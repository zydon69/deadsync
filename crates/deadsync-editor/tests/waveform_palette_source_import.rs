use deadsync_editor::{
    audio_preferences::{
        File,
        source_import::{Disposition, Draft},
    },
    document::{Document, Format},
    history::Session,
    key_binding_preferences::source_profile::Platform,
    waveform_palette::SparseMode,
    waveform_preferences::DrawLocation,
};
#[test]
fn pinned_real_models_public_colors_and_sparse_fallback_are_consumed_not_archived() {
    let cases: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/waveform_palette_source.expected.json"
    ))
    .unwrap();
    let current = File::memory().unwrap();
    let mut session =
        Session::new(Document::parse(b"#TITLE:palette;".as_slice(), Format::Sm).unwrap());
    for case in cases.as_array().unwrap() {
        let prepared = Draft::prepare(
            &mut session,
            None,
            &current,
            case["raw"].as_str().unwrap().as_bytes(),
            Platform::Mac,
        );
        if !case["accepted"].as_bool().unwrap() {
            assert!(prepared.is_err(), "{}", case["name"]);
            continue;
        }
        let draft = prepared.unwrap();
        let p = draft.target().waveform();
        let expected = &case["waveform"];
        let palette = p.palette();
        for (actual, key) in [
            (palette.dense().rgba(), "WaveFormDenseColor"),
            (palette.sparse().rgba(), "WaveFormSparseColor"),
            (palette.background().rgba(), "WaveFormBackgroundColor"),
        ] {
            for (index, component) in ["X", "Y", "Z", "W"].into_iter().enumerate() {
                assert_eq!(
                    actual[index],
                    expected[key][component].as_f64().unwrap() as f32,
                    "{} {key}.{component}",
                    case["name"]
                );
            }
        }
        assert_eq!(
            palette.darken(),
            expected["WaveFormSparseColorScale"].as_f64().unwrap() as f32
        );
        if case["name"].as_str().unwrap().starts_with("palette-raw-") {
            assert_eq!(palette.mode(), SparseMode::Unknown(99));
            assert_eq!(palette.resolved_sparse(), palette.sparse());
            assert_eq!(p.location(), DrawLocation::Unknown(99));
            assert!(!p.location().draws_chart(true, true, 0.0, 0.0));
            for key in ["WaveFormSparseColorOption", "WaveFormDrawLocation"] {
                assert!(
                    !draft
                        .report()
                        .items
                        .iter()
                        .any(|row| row.path == format!("PreferencesWaveForm.{key}")
                            && row.disposition == Disposition::Retained)
                );
                assert!(
                    draft
                        .report()
                        .items
                        .iter()
                        .any(|row| row.path == format!("PreferencesWaveForm.{key}")
                            && row.disposition == Disposition::Applied)
                );
            }
        }
        let mut file = File::memory().unwrap();
        file.set_waveform(p);
        assert_eq!(file.waveform(), p);
    }
}
