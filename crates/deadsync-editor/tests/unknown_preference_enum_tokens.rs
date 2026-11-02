use deadsync_editor::audio_preferences::File;
use serde_json::{Value, json};

#[test]
fn visual_edits_preserve_existing_native_unknown_enum_token_representations() {
    for token in [json!(99), json!("99"), json!("+99")] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preferences.json");
        let initial = json!({
            "Version":1,
            "Vendor":{"keep":true},
            "Waveform":{"WaveFormSparseColorOption":token,"WaveFormDrawLocation":token},
            "MiniMap":{"MiniMapSpacingModeForVariable":token}
        });
        std::fs::write(&path, serde_json::to_vec(&initial).unwrap()).unwrap();
        let mut file = File::open(&path).unwrap();
        let mut waveform = file.waveform();
        waveform.set_channel_width(0.123456).unwrap();
        file.set_waveform(waveform);
        let mut minimap = file.minimap();
        minimap.set_quantize(!minimap.quantize());
        file.set_minimap(minimap);
        file.save().unwrap();
        let saved: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        for (group, field) in [
            ("Waveform", "WaveFormSparseColorOption"),
            ("Waveform", "WaveFormDrawLocation"),
            ("MiniMap", "MiniMapSpacingModeForVariable"),
        ] {
            assert_eq!(saved[group][field], token);
        }
        assert_eq!(saved["Vendor"], initial["Vendor"]);
        let reopened = File::open(&path).unwrap();
        assert_eq!(reopened.waveform(), waveform);
        assert_eq!(reopened.minimap(), minimap);
    }
}
