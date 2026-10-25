use deadsync_editor::{
    analysis::{self, task::Task},
    document::{Document, Format},
    history::Session,
    notes::{Note, Position},
};
use std::sync::atomic::AtomicBool;
const SONG: &str = "#VERSION:0.83;#BPMS:0=120,4=240;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001,\n1100\n0000\n0000\n0000;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=90;#NOTES:1000;";
fn source() -> Session {
    Session::new(Document::parse(SONG.as_bytes(), Format::Ssc).unwrap())
}
#[test]
fn snapshot_report_matches_rssp_stats_density_clock_and_hash_without_writes() {
    let mut session = source();
    session
        .edit_notes(
            0,
            "Add",
            [(
                Position {
                    player: 0,
                    lane: 3,
                    row: 48,
                },
                Some(Note::tap()),
            )],
        )
        .unwrap();
    let before = session.serialize().unwrap();
    let expected = analysis::chart(&session, 0, &rssp::AnalysisOptions::default()).unwrap();
    let report = Task::capture(&session, 0)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    assert!(report.is_current(&session, 0));
    assert!(!report.is_current(&session, 1));
    let summary = report.summary();
    assert_eq!(summary.stats, expected.stats);
    assert_eq!(summary.short_hash, expected.short_hash);
    assert_eq!(summary.measure_densities, expected.measure_densities);
    assert_eq!(summary.measure_nps_vec, expected.measure_nps_vec);
    assert_eq!(summary.timing_segments.bpms, expected.timing_segments.bpms);
    assert_eq!(summary.tech_counts, expected.tech_counts);
    assert_eq!(report.row_for_bin(0).unwrap(), 0);
    assert_eq!(report.row_for_bin(1).unwrap(), 192);
    assert!(report.row_for_bin(summary.measure_densities.len()).is_err());
    assert_eq!(session.serialize().unwrap(), before);
    assert_eq!(session.undo_label(), Some("Add"));
}
#[test]
fn capture_is_immutable_and_reports_reject_other_sessions_or_revisions() {
    let mut session = source();
    let task = Task::capture(&session, 0).unwrap();
    session
        .edit_notes(
            0,
            "Add",
            [(
                Position {
                    player: 0,
                    lane: 3,
                    row: 48,
                },
                Some(Note::tap()),
            )],
        )
        .unwrap();
    let report = task.run(&AtomicBool::new(false)).unwrap();
    assert!(!report.is_current(&session, 0));
    assert!(!report.is_current(&source(), 0));
    assert_eq!(report.summary().stats.total_arrows, 6);
    let fresh = Task::capture(&session, 1)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    assert!(fresh.is_current(&session, 1));
    assert_eq!(fresh.summary().stats.total_arrows, 1);
    assert_eq!(fresh.summary().timing_segments.bpms, [(0.0, 90.0)]);
    session.undo().unwrap();
    assert!(!fresh.is_current(&session, 1));
}
#[test]
fn invalid_charts_unsupported_layout_and_cancel_fail_without_changing_source() {
    let session = source();
    assert!(Task::capture(&session, 2).is_err());
    let before = session.serialize().unwrap();
    assert!(
        Task::capture(&session, 0)
            .unwrap()
            .run(&AtomicBool::new(true))
            .is_err()
    );
    assert_eq!(session.serialize().unwrap(), before);
    let session = Session::new(
        Document::parse(
            SONG.replace("dance-single", "dance-couple").as_bytes(),
            Format::Ssc,
        )
        .unwrap(),
    );
    assert!(
        Task::capture(&session, 0)
            .unwrap()
            .run(&AtomicBool::new(false))
            .is_err()
    );
    assert!(!session.is_dirty());
}

#[test]
fn inline_measure_separators_match_canonical_rssp_without_rewriting_source() {
    let canonical = SONG.replace("0001,", "0001\n,\n");
    let expected = rssp::analyze(
        canonical.as_bytes(),
        "ssc",
        &rssp::AnalysisOptions::default(),
    )
    .unwrap();
    let session = source();
    let before = session.serialize().unwrap();
    let report = Task::capture(&session, 0)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    assert_eq!(
        report.summary().measure_densities,
        expected.charts[0].measure_densities
    );
    assert_eq!(
        report.summary().measure_nps_vec,
        expected.charts[0].measure_nps_vec
    );
    assert_eq!(report.summary().short_hash, expected.charts[0].short_hash);
    assert_eq!(report.summary().stats, expected.charts[0].stats);
    assert_eq!(report.row_for_bin(1).unwrap(), 192);
    assert_eq!(session.serialize().unwrap(), before);
    assert!(!session.is_dirty());
}
