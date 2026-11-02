use deadsync_editor::{
    audio_preferences::File,
    document::{Document, Format},
    history::Session,
};
use serde_json::{Value, json};

#[test]
fn original_rate_shortcuts_include_unchecked_integer_boundaries_and_global_history() {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/audio-rate-source.json")).unwrap();
    for case in cases {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preferences.json");
        std::fs::write(
            &path,
            serde_json::to_vec(&json!({
                "Version":1, "Audio":{"MusicRate":case["raw"]}, "Vendor":{"keep":7}
            }))
            .unwrap(),
        )
        .unwrap();
        let mut file = File::open(&path).unwrap();
        let before = file.audio();
        assert_eq!(
            before.music_rate().to_bits(),
            case["before"].as_f64().unwrap().to_bits(),
            "{case}"
        );
        let mut after = before;
        after.step_music_rate(case["increase"].as_bool().unwrap());
        assert_eq!(
            after.music_rate().to_bits(),
            case["after"].as_f64().unwrap().to_bits(),
            "{case}"
        );
        let mut session =
            Session::new(Document::parse(b"#TITLE:Rate;".as_slice(), Format::Sm).unwrap());
        let original = session.serialize().unwrap();
        let changed = session
            .set_audio_preferences(before, after, Some(&file))
            .unwrap();
        assert_eq!(changed, case["changed"].as_bool().unwrap());
        if changed {
            file = session
                .take_audio_preferences_update()
                .unwrap()
                .file
                .unwrap();
            assert_eq!(
                File::open(&path).unwrap().audio().music_rate(),
                after.music_rate()
            );
            session.capture_source_preferences_context(&file);
            assert!(session.undo().unwrap());
            file = session
                .take_audio_preferences_update()
                .unwrap()
                .file
                .unwrap();
            assert_eq!(
                file.audio().music_rate().to_bits(),
                case["undone"].as_f64().unwrap().to_bits()
            );
            session.capture_source_preferences_context(&file);
            assert!(session.redo().unwrap());
            file = session
                .take_audio_preferences_update()
                .unwrap()
                .file
                .unwrap();
            assert_eq!(
                file.audio().music_rate().to_bits(),
                case["redone"].as_f64().unwrap().to_bits()
            );
        } else {
            assert!(session.take_audio_preferences_update().is_none());
            assert!(!file.has_changes());
        }
        assert_eq!(session.serialize().unwrap(), original);
        assert!(!session.is_dirty());
        assert_eq!(
            serde_json::from_slice::<Value>(&std::fs::read(&path).unwrap()).unwrap()["Vendor"],
            json!({"keep":7})
        );
    }
}
