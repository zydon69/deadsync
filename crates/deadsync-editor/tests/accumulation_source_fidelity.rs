use deadsync_editor::{
    Error,
    audio_preferences::{
        File,
        source_import::{Disposition, Draft},
    },
    density::{Event, Series},
    density_preferences::Accumulation,
    document::{Document, Format},
    history::Session,
    key_binding_preferences::source_profile::Platform,
};
use serde_json::{Value, json};
fn session() -> Session {
    Session::new(Document::parse(b"#TITLE:Unchanged;".as_slice(), Format::Sm).unwrap())
}
fn build(rows: &Value, file: &File) -> Series {
    Series::build(
        rows.as_array().unwrap().iter().map(|row| Event {
            row: row.as_u64().unwrap(),
            step: true,
            fake: false,
        }),
        (0..8).map(|i| f64::from(i) * 2.0).collect(),
        16.0,
        file.streams().density_settings(),
        100,
    )
    .unwrap()
}
fn breakdown(series: &Series) -> &str {
    series.breakdown.as_deref().unwrap_or("No Streams")
}
#[test]
fn original_rebuild_transcripts_use_rows_for_unknown_and_native_edits_recompute_consistently() {
    let oracle: Value =
        serde_json::from_str(include_str!("fixtures/accumulation-source.json")).unwrap();
    for case in oracle["cases"].as_array().unwrap() {
        let mut session = session();
        let file = File::memory().unwrap();
        let draft = Draft::prepare(
            &mut session,
            None,
            &file,
            case["raw"].as_str().unwrap().as_bytes(),
            Platform::Mac,
        )
        .unwrap();
        let file = draft.target();
        assert_eq!(
            file.streams().accumulation.name(),
            case["value"]["PreferencesStream"]["AccumulationType"]
        );
        assert_eq!(
            file.density().accumulation.name(),
            case["value"]["PreferencesDensityGraph"]["AccumulationType"]
        );
        let unknown = matches!(file.streams().accumulation, Accumulation::Unknown(_));
        if unknown {
            assert!(
                draft
                    .report()
                    .items
                    .iter()
                    .any(|item| item.path == "PreferencesStream.AccumulationType"
                        && item.disposition == Disposition::Adapted)
            );
        }
        for stage in case["stages"].as_array().unwrap() {
            let before = build(&stage["initial_rows"], file);
            let after = build(&stage["rows_after"], file);
            assert_eq!(breakdown(&before), stage["before"].as_str().unwrap());
            assert_eq!(breakdown(&after), stage["rebuilt"].as_str().unwrap());
            if !unknown {
                assert_eq!(breakdown(&after), stage["incremental"].as_str().unwrap());
            }
            // Unknown native edits deliberately use the reference Row rebuild, fixing its
            // stale incremental streams. The original incremental witness is kept intact.
            if unknown {
                assert_eq!(
                    after.samples(Accumulation::Row),
                    after.samples(file.density().accumulation)
                );
            }
        }
    }
}
#[test]
fn raw_accumulation_survives_restore_save_undo_redo_and_reopen() {
    for raw in [99, -99] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prefs.json");
        std::fs::write(&path,serde_json::to_vec(&json!({"Version":1,"Stream":{"AccumulationType":raw.to_string()},"DensityGraph":{"AccumulationType":raw.to_string()},"Vendor":{"keep":99}})).unwrap()).unwrap();
        let mut file = File::open(&path).unwrap();
        let mut session = session();
        let before = file.streams();
        assert_eq!(before.accumulation, Accumulation::Unknown(raw));
        let after = deadsync_editor::stream_preferences::Preferences::default();
        session
            .set_stream_preferences(before, after, Some(&file))
            .unwrap();
        file = session
            .take_stream_preferences_update()
            .unwrap()
            .file
            .unwrap();
        session.capture_source_preferences_context(&file);
        session.undo().unwrap();
        file = session
            .take_stream_preferences_update()
            .unwrap()
            .file
            .unwrap();
        assert_eq!(
            File::open(&path).unwrap().streams().accumulation,
            Accumulation::Unknown(raw)
        );
        session.capture_source_preferences_context(&file);
        session.redo().unwrap();
        assert_eq!(File::open(&path).unwrap().streams(), after);
        let saved: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved["Vendor"]["keep"], 99);
        assert_eq!(
            File::open(&path).unwrap().density().accumulation,
            Accumulation::Unknown(raw)
        );
    }
}
#[test]
fn unsafe_note_type_is_an_atomic_consumer_refusal_with_raw_input_untouched() {
    for raw in ["99", "-99"] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prefs.json");
        let bytes = br#"{"Version":1,"Vendor":{"keep":true}}"#;
        std::fs::write(&path, bytes).unwrap();
        let file = File::open(&path).unwrap();
        let mut session = session();
        let song = session.serialize().unwrap();
        let input = serde_json::to_vec(&json!({"PreferencesStream":{"NoteType":raw}})).unwrap();
        let original = input.clone();
        let result = Draft::prepare(&mut session, None, &file, &input, Platform::Mac);
        assert!(
            matches!(result,Err(Error::Unsupported(message)) if message==format!("Unsafe source stream NoteType: {raw}"))
        );
        assert_eq!(input, original);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(session.serialize().unwrap(), song);
        assert!(session.take_source_preferences_update().is_none());
        assert!(file.source_archive().is_none());
    }
    for raw in [
        json!(99),
        json!(2147483648_i64),
        json!("2147483648"),
        json!("invalid"),
    ] {
        let mut session = session();
        let file = File::memory().unwrap();
        let input = serde_json::to_vec(&json!({"PreferencesStream":{"NoteType":raw}})).unwrap();
        let draft = Draft::prepare(&mut session, None, &file, &input, Platform::Mac).unwrap();
        assert_eq!(
            draft.target().streams().note_type,
            deadsync_editor::stream_preferences::NoteType::Quarter
        );
    }
    let mut session = session();
    let file = File::memory().unwrap();
    assert!(matches!(
        Draft::prepare(
            &mut session,
            None,
            &file,
            br#"{"PreferencesStream":{"MinimumLengthToConsiderStream":2147483648}}"#,
            Platform::Mac
        ),
        Err(Error::InvalidFormat(_))
    ));
}

#[test]
fn accepted_source_import_restore_and_history_keep_unknown_memory_owner_and_archive() {
    let mut session = session();
    let file = File::memory().unwrap();
    let raw=br#"{"PreferencesStream":{"AccumulationType":"99"},"PreferencesDensityGraph":{"AccumulationType":"-99"}}"#;
    let draft = Draft::prepare(&mut session, None, &file, raw, Platform::Mac).unwrap();
    session
        .import_source_preferences(draft, &file, None, true)
        .unwrap();
    let mut file = session
        .take_source_preferences_update()
        .unwrap()
        .snapshot()
        .file()
        .clone();
    let before = file.streams();
    let after = deadsync_editor::stream_preferences::Preferences::default();
    session
        .set_stream_preferences(before, after, Some(&file))
        .unwrap();
    file = session
        .take_stream_preferences_update()
        .unwrap()
        .file
        .unwrap();
    session.capture_source_preferences_context(&file);
    session.undo().unwrap();
    file = session
        .take_stream_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert_eq!(file.streams().accumulation, Accumulation::Unknown(99));
    assert_eq!(file.density().accumulation, Accumulation::Unknown(-99));
    assert!(file.source_archive().is_some());
    assert!(!file.has_changes());
    session.capture_source_preferences_context(&file);
    session.redo().unwrap();
    file = session
        .take_stream_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert_eq!(file.streams(), after);
    assert!(file.path().as_os_str().is_empty());
}
#[test]
fn unsafe_consumer_witness_keeps_actual_exception_distinct_from_loader_acceptance() {
    let oracle: Value =
        serde_json::from_str(include_str!("fixtures/stream-unsafe-note-type.json")).unwrap();
    for row in oracle.as_array().unwrap() {
        let raw = row["raw"].as_i64().unwrap();
        if (0..9).contains(&raw) {
            assert!(row["error"].is_null());
        } else {
            assert_eq!(row["error"], "IndexOutOfRangeException");
        }
    }
}

#[test]
fn unsafe_refusal_preserves_previous_import_archive_owner_and_undo_action() {
    let mut session = session();
    let file = File::memory().unwrap();
    let initial = br#"{"Vendor":{"opaque":[1,2]},"PreferencesStream":{"AccumulationType":"99"}}"#;
    let draft = Draft::prepare(&mut session, None, &file, initial, Platform::Mac).unwrap();
    session
        .import_source_preferences(draft, &file, None, true)
        .unwrap();
    let file = session
        .take_source_preferences_update()
        .unwrap()
        .snapshot()
        .file()
        .clone();
    let archive = file.source_archive().unwrap().clone();
    let revision = session.revision();
    let before = file.streams();
    let song = session.serialize().unwrap();
    assert!(
        Draft::prepare(
            &mut session,
            None,
            &file,
            br#"{"PreferencesStream":{"NoteType":"99"}}"#,
            Platform::Mac
        )
        .is_err()
    );
    assert_eq!(file.source_archive(), Some(&archive));
    assert_eq!(file.streams(), before);
    assert_eq!(session.revision(), revision);
    assert_eq!(session.serialize().unwrap(), song);
    assert!(session.take_source_preferences_update().is_none());
    session.capture_source_preferences_context(&file);
    session.undo().unwrap();
    assert!(
        session
            .take_source_preferences_update()
            .unwrap()
            .snapshot()
            .file()
            .source_archive()
            .is_none()
    );
}
