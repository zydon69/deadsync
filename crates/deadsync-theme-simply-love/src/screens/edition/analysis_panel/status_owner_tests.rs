//! Both consumers share Panel::Analysis, but only the foreground owner writes status.
use super::*;
use std::time::{Duration, Instant};
#[test]
fn real_rssp_completion_keeps_source_statistics_status_and_retains_background_cache_en_fr() {
    for language in ["en", "fr"] {
        for layout in ["dance-single", "dance-routine"] {
            crate::i18n::init_for_tests();
            crate::i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("analysis.ssc");
            let notes = if layout == "dance-single" {
                "1000\n0100\n0010\n0001"
            } else {
                "10000000\n01000000\n00100000\n00010000"
            };
            std::fs::write(&path,format!("#VERSION:0.83;#TITLE:Status;#ARTIST:Fixture;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:{layout};#DIFFICULTY:Hard;#METER:9;#DESCRIPTION:Report;#NOTES:{notes};")).unwrap();
            let mut state = State::open(&path, 0).unwrap();
            let source = state.workspace.file().session.serialize().unwrap();
            state.open_analysis();
            assert!(
                state.analysis_job.is_some(),
                "real RSSP Task must be pending before switching owner: {}",
                state.status
            );
            state.analysis_key(KeyCode::F2);
            assert!(state.chart_details.active);
            assert!(matches!(state.panel, Some(Panel::Analysis)));
            // Complete the actual source worker first. Do not poll the RSSP receipt until
            // the foreground report has installed its final help/status.
            let deadline = Instant::now() + Duration::from_secs(10);
            while state.is_chart_details_loading_for_test() {
                state.poll_chart_details();
                assert!(Instant::now() < deadline, "source worker: {}", state.status);
                std::thread::yield_now();
            }
            let foreground = state.status.clone();
            assert_eq!(foreground, i18n::tr("Edition", "ChartDetailsHelp"));
            while state.analysis_job.is_some() {
                state.poll_analysis();
                assert_eq!(
                    state.status, foreground,
                    "background RSSP must not overwrite foreground Source Statistics status ({layout})"
                );
                assert!(Instant::now() < deadline, "RSSP worker timeout");
                std::thread::yield_now();
            }
            assert_eq!(state.status, foreground);
            if layout == "dance-single" {
                assert!(
                    state.analysis_report().is_some(),
                    "real supported RSSP result stays cached in background"
                );
            } else {
                assert!(
                    state.analysis_report().is_none(),
                    "unsupported RSSP failure is not a fake successful report"
                );
            }
            assert!(!state.is_chart_details_loading_for_test());
            assert_eq!(state.workspace.file().session.serialize().unwrap(), source);
            assert!(state.workspace.file().session.undo_label().is_none());
        }
    }
}
