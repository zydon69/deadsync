use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    playback::Preview,
    song_properties::{self, Draft},
};
use serde_json::Value;
use std::{fs, path::PathBuf};
fn song(format: Format) -> Session {
    let notes = match format {
        Format::Sm => "#NOTES:dance-single::Hard:9::1000\n0000\n0000\n0000;",
        Format::Ssc => "#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0000\n0000\n0000;",
    };
    Session::new(Document::parse(format!("#VERSION:0.83;#TITLE:Source;#MUSIC:song.ogg;#OFFSET:0.375;#BPMS:0=120;#UNKNOWN:preserve;{notes}").into_bytes(), format).unwrap())
}
#[test]
fn sync_schema_and_both_time_conversions_match_original_csharp_methods() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("song.ogg"), b"fixture").unwrap();
    let mut count = 0;
    for entry in fs::read_dir(root.clone()).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("song-sync-") || !name.ends_with(".request.json") {
            continue;
        }
        count += 1;
        let request: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let expected: Value = serde_json::from_slice(
            &fs::read(root.join(name.replace(".request.json", ".expected.json"))).unwrap(),
        )
        .unwrap();
        let mut session = song(Format::Ssc);
        session.set_text(None, "GASongDataVersion", "1").unwrap();
        session
            .set_text(None, "GASongData", request["Content"].as_str().unwrap())
            .unwrap();
        assert_eq!(
            song_properties::sync_offset(session.document()).unwrap(),
            expected["SyncOffset"].as_f64().unwrap()
        );
        let preview = Preview::capture(&session, 0, &dir.path().join("song.ssc")).unwrap();
        for position in expected["Positions"].as_array().unwrap() {
            let time = position["Time"].as_f64().unwrap();
            assert_eq!(
                preview.chart_time_to_music(time).unwrap(),
                position["SongTime"].as_f64().unwrap(),
                "{name} song {time}"
            );
            assert_eq!(
                preview.music_to_chart_time(time).unwrap(),
                position["ChartTime"].as_f64().unwrap(),
                "{name} chart {time}"
            );
        }
    }
    assert_eq!(count, 5);
}
#[test]
fn sync_edits_preserve_other_song_data_and_timing_apply_once_undo_redo_and_reopen() {
    for format in [Format::Sm, Format::Ssc] {
        let mut session = song(format);
        session.set_text(None, "GASongDataVersion", "1").unwrap();
        session.set_text(None, "GASongData", r#"{"SyncOffset":0.009,"DefaultChartType":"dance_double","DefaultChartIndex":2,"Future":{"label":"a;b:c"}}"#).unwrap();
        session.mark_saved();
        let original = session.serialize().unwrap();
        let mut draft = Draft::capture(&session).unwrap();
        draft.set_sync_offset(-0.125).unwrap();
        assert!(draft.apply(&mut session).unwrap());
        assert!(draft.apply(&mut session).is_err());
        let changed = session.serialize().unwrap();
        let data: Value =
            serde_json::from_str(&session.document().text(None, "GASongData").unwrap()).unwrap();
        assert_eq!(data["Future"]["label"], "a;b:c");
        assert_eq!(data["DefaultChartIndex"], 2);
        assert_eq!(session.document().text(None, "OFFSET").unwrap(), "0.375");
        assert_eq!(
            session.document().text(None, "UNKNOWN").unwrap(),
            "preserve"
        );
        assert!(session.undo().unwrap());
        assert_eq!(session.serialize().unwrap(), original);
        assert!(session.redo().unwrap());
        assert_eq!(session.serialize().unwrap(), changed);
        let reopened = Document::parse(changed, format).unwrap();
        assert_eq!(song_properties::sync_offset(&reopened).unwrap(), -0.125);
    }
}
#[test]
fn default_noop_and_stale_foreign_invalid_drafts_do_not_change_history() {
    let mut session = song(Format::Ssc);
    assert_eq!(
        song_properties::sync_offset(session.document()).unwrap(),
        0.009
    );
    session.set_text(None, "TITLE", "Changed").unwrap();
    session.undo().unwrap();
    let mut draft = Draft::capture(&session).unwrap();
    assert!(!draft.apply(&mut session).unwrap());
    assert!(session.redo_label().is_some());
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(draft.set_sync_offset(value).is_err());
    }
    let mut foreign = song(Format::Ssc);
    assert!(draft.apply(&mut foreign).is_err());
    session.redo().unwrap();
    assert!(draft.apply(&mut session).is_err());
}
#[test]
fn future_incomplete_malformed_or_nonfinite_song_data_is_never_overwritten() {
    for (version, data) in [
        ("2", "{}"),
        ("1", "null"),
        ("1", "{broken"),
        ("1", r#"{"SyncOffset":"NaN"}"#),
        ("1", r#"{"SyncOffset":1e999}"#),
    ] {
        let mut session = song(Format::Ssc);
        session
            .set_text(None, "GASongDataVersion", version)
            .unwrap();
        session.set_text(None, "GASongData", data).unwrap();
        let bytes = session.serialize().unwrap();
        assert!(Draft::capture(&session).is_err());
        assert_eq!(session.serialize().unwrap(), bytes);
    }
    let mut session = song(Format::Ssc);
    session.set_text(None, "GASongDataVersion", "1").unwrap();
    assert!(Draft::capture(&session).is_err());
}

#[test]
fn song_music_offset_and_itg_migration_are_atomic_preserve_source_and_drive_preview() {
    for format in [Format::Sm, Format::Ssc] {
        let mut session = song(format);
        let original = session.serialize().unwrap();
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("song.ogg"), b"transport fixture").unwrap();
        let mut draft = Draft::capture(&session).unwrap();
        assert_eq!(draft.music_offset(), 0.375);
        draft.set_sync_offset(0.0).unwrap();
        draft.apply_itg_offset().unwrap();
        assert_eq!(draft.music_offset(), 0.384);
        assert_eq!(draft.sync_offset(), 0.009);
        assert!(draft.apply(&mut session).unwrap());
        let changed = session.serialize().unwrap();
        assert_eq!(
            session.document().text(None, "OFFSET").as_deref(),
            Some("0.384")
        );
        assert_eq!(
            session.document().text(None, "UNKNOWN").as_deref(),
            Some("preserve")
        );
        let preview = Preview::capture(&session, 0, &dir.path().join("song.ssc")).unwrap();
        assert!((preview.chart_time_to_music(1.0).unwrap() - 0.625).abs() < 1e-12);
        assert!(session.undo().unwrap());
        assert_eq!(session.serialize().unwrap(), original);
        assert!(session.redo().unwrap());
        assert_eq!(session.serialize().unwrap(), changed);
        let reopened = Session::new(Document::parse(changed, format).unwrap());
        assert_eq!(Draft::capture(&reopened).unwrap().music_offset(), 0.384);
        let mut second = Draft::capture(&reopened).unwrap();
        second.apply_itg_offset().unwrap();
        assert_eq!(second.music_offset(), 0.393);
    }
}

#[test]
fn song_music_offset_validation_and_noop_preserve_history_and_metadata_bytes() {
    let mut session = song(Format::Ssc);
    let before = session.serialize().unwrap();
    let mut draft = Draft::capture(&session).unwrap();
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(draft.set_music_offset(value).is_err());
    }
    draft
        .set_music_offset(f64::from_bits(0.375f64.to_bits() + 1))
        .unwrap();
    assert!(!draft.has_changes());
    assert!(!draft.apply(&mut session).unwrap());
    assert_eq!(session.serialize().unwrap(), before);
    draft.set_music_offset(-0.25).unwrap();
    assert!(draft.apply(&mut session).unwrap());
    assert!(session.document().text(None, "GASongData").is_none());
    assert!(session.document().text(None, "GASongDataVersion").is_none());
    session.set_text(None, "OFFSET", "invalid").unwrap();
    assert!(Draft::capture(&session).is_err());
}
