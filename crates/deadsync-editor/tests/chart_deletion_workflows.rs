use deadsync_editor::{
    chart_creation,
    chart_deletion::Draft,
    document::{Document, Format},
    generation::metadata::ChartData,
    history::Session,
    notes::{Note, Position},
    workspace::{Command, Workspace},
};
use std::fs;
const SONG: &str = "#VERSION:0.83;#TITLE:Keep;#BPMS:0=120;#VENDOR:root;#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:First;#DIFFICULTY:Hard;#METER:9;#X:one;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:Second;#DIFFICULTY:Easy;#METER:2;#X:two;#NOTES:0100;#NOTEDATA:;#STEPSTYPE:dance-double;#DESCRIPTION:Third;#DIFFICULTY:Medium;#METER:5;#X:three;#NOTES:00001000;";
fn session() -> Session {
    Session::new(Document::parse(SONG.as_bytes(), Format::Ssc).unwrap())
}
#[test]
fn ssc_middle_deletion_keeps_pending_other_charts_unknown_tags_and_exact_history() {
    let mut s = session();
    s.edit_notes(
        2,
        "Pending",
        [(
            Position {
                player: 0,
                lane: 7,
                row: 48,
            },
            Some(Note::tap()),
        )],
    )
    .unwrap();
    s.set_text(Some(2), "CREDIT", "Été 日本語").unwrap();
    s.set_text(Some(1), "DESCRIPTION", "Second éditée").unwrap();
    let expected = s.grid(2).unwrap().clone();
    let before = s.serialize().unwrap();
    let draft = Draft::capture(&s, 1).unwrap();
    assert!(!draft.is_last());
    assert_eq!(draft.description(), "Second éditée");
    draft.apply(&mut s).unwrap();
    assert_eq!(s.document().charts().len(), 2);
    assert_eq!(s.document().text(None, "VENDOR").as_deref(), Some("root"));
    assert_eq!(s.document().text(Some(0), "X").as_deref(), Some("one"));
    assert_eq!(s.document().text(Some(1), "X").as_deref(), Some("three"));
    assert_eq!(s.grid(1).unwrap(), &expected);
    assert_eq!(
        s.document().text(Some(1), "CREDIT").as_deref(),
        Some("Été 日本語")
    );
    let after = s.serialize().unwrap();
    assert!(!String::from_utf8_lossy(&after).contains("#X:two;"));
    assert_eq!(s.undo_label(), Some("Delete chart"));
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
    s.redo().unwrap();
    assert_eq!(s.serialize().unwrap(), after);
}
#[test]
fn sm_metadata_is_removed_and_renumbered_without_changing_values_or_unrelated_tags() {
    let raw=b"#TITLE:CP\xe9;#BPMS:0=120;#GAChartDataVersionCHART0000:1;#GAChartDataCHART0000:{\"Future\":\"zero\"};#GAChartDataVersionCHART0001:1;#GAChartDataCHART0001:{\"Patterns\":{\"48\":{\"Length\":96}},\"Future\":\"one\"};#GAChartDataCHART9999:{\"Orphan\":1};#VENDORCHART0001:keep;#NOTES:dance-single:A:Hard:9::1000;#NOTES2:dance-double:B:Easy:2::00001000;#FOOTER:keep;";
    let mut s = Session::new(Document::parse(raw.as_slice(), Format::Sm).unwrap());
    let source_value = s
        .document()
        .raw_property(None, "GAChartDataCHART0001")
        .unwrap()
        .to_vec();
    Draft::capture(&s, 0).unwrap().apply(&mut s).unwrap();
    assert_eq!(s.document().charts().len(), 1);
    assert_eq!(
        s.document()
            .raw_property(None, "GAChartDataCHART0000")
            .unwrap(),
        source_value
    );
    assert!(
        s.document()
            .raw_property(None, "GAChartDataCHART0001")
            .is_none()
    );
    assert_eq!(
        ChartData::read(s.document(), 0)
            .unwrap()
            .unwrap()
            .patterns
            .unwrap()[&48]
            .length,
        96
    );
    assert_eq!(
        s.document().text(None, "VENDORCHART0001").as_deref(),
        Some("keep")
    );
    assert!(
        s.document()
            .raw_property(None, "GAChartDataCHART9999")
            .is_some()
    );
    assert_eq!(s.document().text(None, "FOOTER").as_deref(), Some("keep"));
    let after = s.serialize().unwrap();
    let reopened = Session::new(Document::parse(after.clone(), Format::Sm).unwrap());
    assert!(ChartData::read(reopened.document(), 0).unwrap().is_some());
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), raw);
    s.redo().unwrap();
    assert_eq!(s.serialize().unwrap(), after);
}
#[test]
fn timing_reference_index_missing_index_and_numeric_enums_are_updated_with_future_fields() {
    for (kind, index, deleted, expected) in [
        ("\"dance_single\"", Some(1), 0, Some(0)),
        ("0", Some(1), 0, Some(0)),
        ("\"DANCE_SINGLE\"", None, 0, Some(0)),
        ("\"dance_single\"", Some(0), 1, Some(0)),
    ] {
        let index = index.map_or(String::new(), |v| format!(",\"DefaultChartIndex\":{v}"));
        let prefix = format!(
            "#GASongDataVersion:1;#GASongData:{{\"DefaultChartType\":{kind}{index},\"SyncOffset\":0.125,\"Future\":{{\"Keep\":true}}}};"
        );
        let mut s = Session::new(
            Document::parse(format!("{prefix}{SONG}").into_bytes(), Format::Ssc).unwrap(),
        );
        let mut draft = Draft::capture(&s, deleted).unwrap();
        // This test isolates index remapping; inject a deterministic order.
        // Cultural fallback ordering is covered by song_timing_chart_workflows.
        let snapshot = s.song_timing_snapshot().unwrap();
        draft
            .prepare_song_timing(&s, &Default::default(), snapshot.source_indices())
            .unwrap();
        draft.apply(&mut s).unwrap();
        let value: serde_json::Value =
            serde_json::from_str(&s.document().text(None, "GASongData").unwrap()).unwrap();
        assert_eq!(value["Future"]["Keep"], true);
        assert_eq!(value["SyncOffset"], 0.125);
        if let Some(index) = expected {
            assert!(!value["DefaultChartType"].is_null());
            if value.get("DefaultChartIndex").is_some() {
                assert_eq!(value["DefaultChartIndex"], index);
            }
        } else {
            assert!(value["DefaultChartType"].is_null());
            assert_eq!(value["DefaultChartIndex"], 0);
        }
    }
}
#[test]
fn stale_foreign_wrong_chart_and_unknown_song_metadata_preserve_document_and_redo() {
    let mut s = session();
    let draft = Draft::capture(&s, 0).unwrap();
    let mut foreign = session();
    assert!(draft.apply(&mut foreign).is_err());
    s.set_text(None, "TITLE", "Edited").unwrap();
    let changed = s.serialize().unwrap();
    assert!(draft.apply(&mut s).is_err());
    assert_eq!(s.serialize().unwrap(), changed);
    s.undo().unwrap();
    assert!(Draft::capture(&s, 99).is_err());
    assert!(s.redo().unwrap());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut w = Workspace::open(&path, 0).unwrap();
    let draft = w.deletion_draft().unwrap();
    w.execute(Command::SelectChart(1)).unwrap();
    assert!(w.delete_chart(&draft).is_err());
    assert_eq!(w.file().session.serialize().unwrap(), SONG.as_bytes());
    let raw = format!("#GASongDataVersion:99;#GASongData:{{\"Future\":1}};{SONG}");
    let mut s = Session::new(Document::parse(raw.as_bytes(), Format::Ssc).unwrap());
    assert!(Draft::capture(&s, 0).unwrap().apply(&mut s).is_err());
    assert_eq!(s.serialize().unwrap(), raw.as_bytes());
    assert!(!s.is_dirty());
}
#[test]
fn last_chart_empty_workspace_undo_new_chart_recovery_and_save_are_safe() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let raw = "#VERSION:0.83;#TITLE:Keep;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:10000000\n&\n00001000;";
    fs::write(&path, raw).unwrap();
    let mut w = Workspace::open(&path, 0).unwrap();
    let draft = w.deletion_draft().unwrap();
    assert!(draft.is_last());
    w.delete_chart(&draft).unwrap();
    assert!(!w.has_chart());
    assert!(w.active_chart().is_none());
    assert!(w.try_grid().is_none());
    assert!(w.execute(Command::Place(0)).is_err());
    assert!(w.execute(Command::NextMeasure).is_err());
    let empty = w.file().session.serialize().unwrap();
    w.execute(Command::SaveRecovery).unwrap();
    let mut recovered = Workspace::open(&path, 0).unwrap();
    recovered.execute(Command::Recover).unwrap();
    assert!(!recovered.has_chart());
    let create = recovered.new_chart_draft().unwrap();
    recovered.create_chart(&create, "dance-routine").unwrap();
    assert_eq!(recovered.grid().players(), 2);
    drop(recovered);
    w.execute(Command::Undo).unwrap();
    assert!(w.has_chart());
    assert_eq!(w.grid().players(), 2);
    assert_eq!(w.file().session.serialize().unwrap(), raw.as_bytes());
    w.execute(Command::Redo).unwrap();
    assert!(!w.has_chart());
    let create = w.new_chart_draft().unwrap();
    w.create_chart(&create, "dance-single").unwrap();
    assert!(w.has_chart());
    assert_eq!(w.grid().iter().count(), 0);
    w.execute(Command::Undo).unwrap();
    assert!(!w.has_chart());
    w.execute(Command::Save).unwrap();
    let mut reopened = Workspace::open(&path, 0).unwrap();
    assert!(!reopened.has_chart());
    assert_eq!(reopened.file().session.serialize().unwrap(), empty);
    assert!(!reopened.has_chart());
    let create = chart_creation::Draft::capture(&reopened.file().session).unwrap();
    reopened.create_chart(&create, "dance-routine").unwrap();
    assert_eq!(reopened.grid().players(), 2);
}
#[test]
fn successor_prefers_matching_type_and_difficulty_then_counterpart_and_restores_focus() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut w = Workspace::open(&path, 0).unwrap();
    let mut draft = w.deletion_draft().unwrap();
    let snapshot = w.file().session.song_timing_snapshot().unwrap();
    draft
        .prepare_song_timing(
            &w.file().session,
            &Default::default(),
            snapshot.source_indices(),
        )
        .unwrap();
    w.delete_chart(&draft).unwrap();
    assert_eq!(w.chart(), 0);
    assert_eq!(
        w.file().session.document().charts()[0].description,
        "Second"
    );
    w.execute(Command::Undo).unwrap();
    assert_eq!(w.chart(), 0);
    w.execute(Command::SelectChart(2)).unwrap();
    let draft = w.deletion_draft().unwrap();
    w.delete_chart(&draft).unwrap();
    assert_eq!(w.chart(), 0);
    w.execute(Command::Undo).unwrap();
    assert_eq!(w.chart(), 2);
    w.execute(Command::Redo).unwrap();
    assert_eq!(w.chart(), 0);
}

#[test]
fn opaque_survivors_remain_preserved_without_a_ghost_active_chart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source = "#VERSION:0.83;#TITLE:Keep;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:vendor-opaque;#VENDOR:data;#NOTES:opaque;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    fs::write(&path, source).unwrap();
    let mut w = Workspace::open(&path, 1).unwrap();
    let draft = w.deletion_draft().unwrap();
    w.delete_chart(&draft).unwrap();
    assert!(!w.has_chart());
    assert_eq!(w.file().session.document().charts().len(), 1);
    assert!(w.execute(Command::SelectChart(0)).is_err());
    assert!(!w.has_chart());
    let bytes = w.file().session.serialize().unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains("#VENDOR:data;#NOTES:opaque;"));
    w.execute(Command::Save).unwrap();
    let mut reopened = Workspace::open(&path, 0).unwrap();
    assert!(!reopened.has_chart());
    let draft = reopened.new_chart_draft().unwrap();
    reopened.create_chart(&draft, "dance-single").unwrap();
    assert_eq!(reopened.chart(), 1);
    assert_eq!(reopened.grid().iter().count(), 0);
    reopened.execute(Command::Undo).unwrap();
    assert!(!reopened.has_chart());
    assert_eq!(reopened.file().session.serialize().unwrap(), bytes);
    w.execute(Command::Undo).unwrap();
    assert_eq!(w.chart(), 1);
    assert_eq!(w.file().session.serialize().unwrap(), source.as_bytes());
}
