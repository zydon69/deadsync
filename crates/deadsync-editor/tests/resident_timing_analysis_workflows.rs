use deadsync_editor::{
    analysis,
    document::{Document, Format},
    history::Session,
    timing_edit::{Scope, TempoDraft, region},
};
use std::sync::atomic::AtomicBool;
const NOTES: &str = "1000\n0000\n0000\n0000\n,\n0100\n0000\n0000\n0000\n,\n0010\n0000\n0000\n0000";
fn source() -> String {
    format!(
        "#TITLE:Analysis;#MUSIC:main.ogg;#BPMS:0=120;#WARPS:1=100;#CREDIT:BR Global;#NOTES:dance-single:FS Credit:smaniac:9:0.1,0.2,0.3,0.4,0.5,3,0,0,0,0,0,0,0,0:{NOTES};#NOTES:dance-single:Peer:Easy:2:0,0,0,0,0:{NOTES};"
    )
}
fn compare_metrics(actual: &rssp::ChartSummary, expected: &rssp::ChartSummary) {
    assert_eq!(actual.duration_seconds, expected.duration_seconds);
    assert_eq!(actual.max_nps, expected.max_nps);
    assert_eq!(actual.median_nps, expected.median_nps);
    assert_eq!(actual.measure_nps_vec, expected.measure_nps_vec);
    assert_eq!(actual.measure_densities, expected.measure_densities);
    assert_eq!(actual.row_to_beat, expected.row_to_beat);
    assert_eq!(actual.bpm_neutral_hash, expected.bpm_neutral_hash);
    assert_eq!(actual.chart_bpms, expected.chart_bpms);
    assert_eq!(actual.chart_warps, expected.chart_warps);
}
#[test]
fn actual_rssp_analysis_and_worker_use_local_owner_preserving_sm_metadata_and_session_bytes() {
    let original = source();
    let options = rssp::AnalysisOptions::default();
    let mut session = Session::new(Document::parse(original.as_bytes(), Format::Sm).unwrap());
    let untouched = analysis::chart(&session, 0, &options).unwrap();
    let mut draft = region::Draft::capture(&session, 0, Scope::Chart).unwrap();
    draft.set(region::Kind::Warp, 48, 48).unwrap();
    draft.apply(&mut session, 0).unwrap();
    let bytes = session.serialize().unwrap();
    let revision = session.revision();
    let actual = analysis::chart(&session, 0, &options).unwrap();
    let expected_bytes = format!(
        "#VERSION:0.83;#MUSIC:main.ogg;#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:FS Credit;#DIFFICULTY:Challenge;#METER:9;#BPMS:0=120;#WARPS:1=1;#NOTES:{NOTES};"
    );
    let expected =
        analysis::chart_bytes(expected_bytes.as_bytes(), Format::Ssc, 0, &options).unwrap();
    compare_metrics(&actual, &expected);
    assert_ne!(actual.duration_seconds, untouched.duration_seconds);
    assert_eq!(actual.step_artist_str, untouched.step_artist_str);
    assert_eq!(actual.description_str, untouched.description_str);
    assert_eq!(actual.difficulty_str, untouched.difficulty_str);
    assert_eq!(actual.tech_notation_str, untouched.tech_notation_str);
    assert_eq!(actual.cached_radar_values, untouched.cached_radar_values);
    assert!(actual.cached_radar_values.is_some());
    assert_eq!(actual.chart_name_str, untouched.chart_name_str);
    assert_eq!(actual.music_path, untouched.music_path);
    assert!(actual.chart_has_own_timing);
    let worker = analysis::task::Task::capture(&session, 0)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    assert!(worker.is_current(&session, 0));
    compare_metrics(worker.summary(), &actual);
    assert_eq!(session.serialize().unwrap(), bytes);
    assert_eq!(session.revision(), revision);
    assert_eq!(session.document().format, Format::Sm);
    let peer = analysis::chart(&session, 1, &options).unwrap();
    let original_peer =
        analysis::chart_bytes(original.as_bytes(), Format::Sm, 1, &options).unwrap();
    assert_eq!(peer.duration_seconds, original_peer.duration_seconds);
    assert_eq!(peer.measure_nps_vec, original_peer.measure_nps_vec);
}
#[test]
fn song_owner_is_effective_without_inventing_local_ownership_and_undo_returns_to_original_analysis()
{
    let original = source();
    let options = rssp::AnalysisOptions::default();
    let mut session = Session::new(Document::parse(original.as_bytes(), Format::Sm).unwrap());
    let initial = analysis::chart(&session, 1, &options).unwrap();
    let mut tempo = TempoDraft::capture(&session, 0, Scope::Song).unwrap();
    tempo.set(0, 240.).unwrap();
    tempo.apply(&mut session, 0).unwrap();
    let result = analysis::chart(&session, 1, &options).unwrap();
    assert!(!result.chart_has_own_timing);
    assert_eq!(result.chart_bpms, None);
    assert_ne!(result.duration_seconds, initial.duration_seconds);
    assert_ne!(result.max_nps, initial.max_nps);
    assert_eq!(result.description_str, initial.description_str);
    assert_eq!(result.step_artist_str, initial.step_artist_str);
    session.undo().unwrap();
    let restored = analysis::chart(&session, 1, &options).unwrap();
    assert_eq!(restored.duration_seconds, initial.duration_seconds);
    assert_eq!(restored.max_nps, initial.max_nps);
}
