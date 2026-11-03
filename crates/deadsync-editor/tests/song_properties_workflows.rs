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

#[test]
fn sample_region_changes_are_one_reversible_song_transaction_in_sm_and_ssc() {
    for format in [Format::Sm, Format::Ssc] {
        let mut session = song(format);
        let original = session.serialize().unwrap();
        let mut draft = Draft::capture(&session).unwrap();
        assert_eq!(draft.sample_start(), 0.0);
        assert_eq!(draft.sample_length(), 0.0);
        draft.set_sample_start(12.3456).unwrap();
        draft.set_sample_length(15.25).unwrap();
        draft.set_music_offset(-0.125).unwrap();
        assert!(draft.apply(&mut session).unwrap());
        assert_eq!(
            session.document().text(None, "SAMPLESTART").as_deref(),
            Some("12.3456")
        );
        assert_eq!(
            session.document().text(None, "SAMPLELENGTH").as_deref(),
            Some("15.25")
        );
        assert_eq!(
            session.document().text(None, "UNKNOWN").as_deref(),
            Some("preserve")
        );
        assert!(session.document().text(None, "GASongData").is_none());
        let changed = session.serialize().unwrap();
        session.undo().unwrap();
        assert_eq!(session.serialize().unwrap(), original);
        session.redo().unwrap();
        assert_eq!(session.serialize().unwrap(), changed);
        let reopened = Session::new(Document::parse(changed, format).unwrap());
        let read = Draft::capture(&reopened).unwrap();
        assert_eq!(read.sample_start(), 12.3456);
        assert_eq!(read.sample_length(), 15.25);
    }
}

#[test]
fn sample_region_noops_and_invalid_values_do_not_create_history_or_erase_source() {
    let mut session = song(Format::Ssc);
    session.set_text(None, "SAMPLESTART", "-1.250000").unwrap();
    session.set_text(None, "SAMPLELENGTH", "15.000000").unwrap();
    let original = session.serialize().unwrap();
    let mut draft = Draft::capture(&session).unwrap();
    draft.set_sample_start(-1.25).unwrap();
    draft
        .set_sample_length(f64::from_bits(15.0f64.to_bits() + 1))
        .unwrap();
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(draft.set_sample_start(value).is_err());
        assert!(draft.set_sample_length(value).is_err());
    }
    assert!(!draft.has_changes());
    assert!(!draft.apply(&mut session).unwrap());
    assert_eq!(session.serialize().unwrap(), original);
    session.set_text(None, "SAMPLELENGTH", "invalid").unwrap();
    assert!(Draft::capture(&session).is_err());
}

#[test]
fn sample_position_actions_use_draft_clock_without_audio_files_or_document_mutation() {
    let session = song(Format::Ssc);
    let original = session.serialize().unwrap();
    let mut draft = Draft::capture(&session).unwrap();
    draft.set_music_offset(0.25).unwrap();
    draft.set_sync_offset(0.0).unwrap();
    let clock = draft.clock(&session, 0).unwrap();
    let seconds = clock.music_time(96).unwrap(); // beat 2 = one second at 120 BPM
    assert_eq!(seconds, 0.75);
    draft.set_sample_start_from_time(seconds).unwrap();
    draft
        .set_sample_end_from_time(clock.music_time(192).unwrap())
        .unwrap();
    assert_eq!(draft.sample_start(), 0.75);
    assert_eq!(draft.sample_length(), 1.0);
    assert_eq!(draft.sample_end().unwrap(), 1.75);
    assert!((clock.position(draft.sample_start()).unwrap() - 96.0).abs() < 1e-10);
    assert!((clock.position(draft.sample_end().unwrap()).unwrap() - 192.0).abs() < 1e-10);
    assert_eq!(session.serialize().unwrap(), original);
    draft.set_sample_end_from_time(-3.0).unwrap();
    assert_eq!(draft.sample_length(), 0.0);
    draft.set_sample_start_from_time(-3.0).unwrap();
    assert_eq!(draft.sample_start(), 0.0);
    assert!(draft.set_sample_start_from_time(f64::NAN).is_err());
    assert!(draft.set_sample_end_from_time(f64::INFINITY).is_err());
    draft.set_sample_start(f64::MAX).unwrap();
    draft.set_sample_length(f64::MAX).unwrap();
    assert!(draft.sample_end().is_err());
    let foreign = song(Format::Ssc);
    assert!(draft.clock(&foreign, 0).is_err());
}

#[test]
fn sample_region_positions_keep_fractional_boundaries_and_follow_tempo_changes() {
    let mut session = song(Format::Ssc);
    session.set_text(None, "BPMS", "0=120,2=60").unwrap();
    let original = session.serialize().unwrap();
    let mut draft = Draft::capture(&session).unwrap();
    draft.set_music_offset(0.0).unwrap();
    draft.set_sync_offset(0.0).unwrap();
    draft.set_sample_start(0.125).unwrap();
    draft.set_sample_length(1.625).unwrap();
    assert_eq!(draft.sample_positions(&session, 0).unwrap(), [12.0, 132.0]);
    draft.set_sample_start(0.126).unwrap();
    let positions = draft.sample_positions(&session, 0).unwrap();
    assert!((positions[0] - 12.096).abs() < 1e-10);
    assert!((positions[1] - 132.048).abs() < 1e-10);
    assert_eq!(session.serialize().unwrap(), original);
    session.set_text(None, "TITLE", "New revision").unwrap();
    assert!(draft.sample_positions(&session, 0).is_err());
}

#[test]
fn sample_audio_uses_raw_file_seconds_and_dedicated_preview_precedence() {
    let session = song(Format::Ssc);
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("song.ogg"), b"transport fixture").unwrap();
    let path = dir.path().join("song.ssc");
    let mut draft = Draft::capture(&session).unwrap();
    assert!(draft.sample_audio(&session, 0, &path).is_err());
    draft.set_sample_start(12.5).unwrap();
    draft.set_sample_length(15.0).unwrap();
    draft.set_music_offset(0.75).unwrap();
    draft.set_sync_offset(0.125).unwrap();
    let sample = draft.sample_audio(&session, 0, &path).unwrap();
    assert_eq!(sample.start, 12.5);
    assert_eq!(sample.length, Some(15.0));
    assert_eq!(sample.path, dir.path().join("song.ogg"));
    let mut session = session;
    session.set_text(None, "PREVIEW", "preview.wav").unwrap();
    fs::write(dir.path().join("preview.wav"), b"dedicated fixture").unwrap();
    let draft = Draft::capture(&session).unwrap();
    let sample = draft.sample_audio(&session, 0, &path).unwrap();
    assert_eq!(sample.start, 0.0);
    assert_eq!(sample.length, None);
    assert_eq!(sample.path, dir.path().join("preview.wav"));
    fs::remove_file(dir.path().join("preview.wav")).unwrap();
    assert!(draft.sample_audio(&session, 0, &path).is_err());
}

#[test]
fn staged_sample_clock_keeps_chart_offset_override_and_switches_back_to_song_inheritance() {
    let mut session = song(Format::Ssc);
    let mut chart = deadsync_editor::chart_properties::Draft::capture(&session, 0).unwrap();
    chart.set_music_offset(0.5).unwrap();
    chart.set_use_chart_offset(true);
    chart.apply(&mut session, 0).unwrap();
    let source = session.serialize().unwrap();
    let mut draft = Draft::capture(&session).unwrap();
    draft.set_music_offset(3.0).unwrap();
    draft.set_sync_offset(0.009).unwrap();
    draft.set_sample_start(1.0).unwrap();
    draft.set_sample_length(0.5).unwrap();
    let positions = draft.sample_positions(&session, 0).unwrap();
    assert!((positions[0] - (1.0 + 0.5 - 0.009) * 96.0).abs() < 1e-10);
    assert!((positions[1] - (1.5 + 0.5 - 0.009) * 96.0).abs() < 1e-10);
    assert_eq!(session.serialize().unwrap(), source);
    let mut chart = deadsync_editor::chart_properties::Draft::capture(&session, 0).unwrap();
    chart.set_use_chart_offset(false);
    chart.apply(&mut session, 0).unwrap();
    assert!(draft.sample_positions(&session, 0).is_err());
    let mut draft = Draft::capture(&session).unwrap();
    draft.set_music_offset(3.0).unwrap();
    draft.set_sync_offset(0.009).unwrap();
    draft.set_sample_start(1.0).unwrap();
    draft.set_sample_length(0.5).unwrap();
    let positions = draft.sample_positions(&session, 0).unwrap();
    assert!((positions[0] - (1.0 + 3.0 - 0.009) * 96.0).abs() < 1e-10);
    assert!((positions[1] - (1.5 + 3.0 - 0.009) * 96.0).abs() < 1e-10);
}

#[test]
fn dedicated_preview_draft_drives_audition_and_reversible_native_save_in_both_formats() {
    use deadsync_editor::song_text::Field;
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("preview.wav"), b"fixture").unwrap();
    let mut session = song(Format::Ssc);
    let original = session.serialize().unwrap();
    let mut draft = Draft::capture(&session).unwrap();
    draft.set_text(Field::PreviewMusic, "preview.wav").unwrap();
    let audio = draft
        .sample_audio(&session, 0, &dir.path().join("song.ssc"))
        .unwrap();
    assert_eq!(audio.path, dir.path().join("preview.wav"));
    assert_eq!(audio.length, None);
    assert_eq!(session.serialize().unwrap(), original);
    draft.apply(&mut session).unwrap();
    let changed = session.serialize().unwrap();
    assert_eq!(
        session.document().text(None, "PREVIEW").as_deref(),
        Some("preview.wav")
    );
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), original);
    session.redo().unwrap();
    assert_eq!(session.serialize().unwrap(), changed);
    let mut sm = song(Format::Sm);
    let original = sm.serialize().unwrap();
    let mut draft = Draft::capture(&sm).unwrap();
    assert!(draft.supports_text(Field::PreviewMusic));
    draft.set_text(Field::PreviewMusic, "preview.wav").unwrap();
    assert!(draft.apply(&mut sm).unwrap());
    assert_eq!(
        Draft::capture(&sm).unwrap().text(Field::PreviewMusic),
        "preview.wav"
    );
    sm.undo().unwrap();
    assert_eq!(sm.serialize().unwrap(), original);
}

#[test]
fn last_second_hint_is_reversible_in_both_formats_and_preserves_unedited_legacy_values() {
    let mut session = song(Format::Ssc);
    session
        .set_text(None, "LASTSECONDHINT", "-2.000000")
        .unwrap();
    let original = session.serialize().unwrap();
    let mut draft = Draft::capture(&session).unwrap();
    assert!(draft.supports_last_second_hint());
    assert_eq!(draft.last_second_hint(), -2.0);
    assert!(!draft.apply(&mut session).unwrap());
    for value in [f64::NAN, f64::INFINITY] {
        assert!(draft.set_last_second_hint(value).is_err());
    }
    draft.set_last_second_hint(12.5).unwrap();
    assert!(draft.apply(&mut session).unwrap());
    let changed = session.serialize().unwrap();
    assert_eq!(
        session.document().text(None, "LASTSECONDHINT").as_deref(),
        Some("12.5")
    );
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), original);
    session.redo().unwrap();
    assert_eq!(session.serialize().unwrap(), changed);
    let reopened = Session::new(Document::parse(changed, Format::Ssc).unwrap());
    assert_eq!(Draft::capture(&reopened).unwrap().last_second_hint(), 12.5);
    let mut session = song(Format::Sm);
    let original = session.serialize().unwrap();
    let mut draft = Draft::capture(&session).unwrap();
    assert!(draft.supports_last_second_hint());
    draft.set_last_second_hint(1.0).unwrap();
    assert!(draft.apply(&mut session).unwrap());
    assert_eq!(Draft::capture(&session).unwrap().last_second_hint(), 1.0);
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), original);
}
