use deadsync_editor::{
    audio_preferences::{
        File,
        source_import::{Disposition, Draft},
    },
    document::{Document, Format},
    history::Session,
    key_binding_preferences::source_profile::Platform,
};
#[test]
fn raw_source_width_and_dense_are_imported_and_persisted_without_slider_bounds() {
    let current = File::memory().unwrap();
    let mut session = Session::new(Document::parse(b"#TITLE:raw;".as_slice(), Format::Sm).unwrap());
    for raw in [-40.0, -0.1, 0.0, 1.01, 12.0, 512.0] {
        let source = serde_json::json!({"PreferencesWaveForm":{"WaveFormMaxXPercentagePerChannel":raw,"DenseScale":raw}});
        let draft = Draft::prepare(
            &mut session,
            None,
            &current,
            &serde_json::to_vec(&source).unwrap(),
            Platform::Mac,
        )
        .unwrap();
        let p = draft.target().waveform();
        assert_eq!(p.channel_width(), raw);
        assert_eq!(p.dense_scale(), raw);
        for key in ["WaveFormMaxXPercentagePerChannel", "DenseScale"] {
            let row = draft
                .report()
                .items
                .iter()
                .find(|row| row.path == format!("PreferencesWaveForm.{key}"))
                .unwrap();
            assert_eq!(row.disposition, Disposition::Applied);
            assert_eq!(
                row.effective.as_ref().and_then(serde_json::Value::as_f64),
                Some(raw)
            );
        }
        let mut published = File::memory().unwrap();
        published.set_waveform(p);
        assert_eq!(published.waveform(), p);
    }
}
