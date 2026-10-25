use deadsync_editor::{
    document::Format,
    generation::{editor_timing::EditorTimeline, timing::GenerationTiming},
    notes::Grid,
    rate_mapping::RateMapping,
};
use std::sync::atomic::AtomicBool;
#[test]
fn rate_mapping_preserves_stop_warp_and_negative_lead_in_boundaries() {
    let normal = RateMapping::new(0.0, 0.0, 0.0, 0.0, 1.0 / 96.0).unwrap();
    assert_eq!(normal.time_at(-48.0).unwrap(), -0.5);
    assert_eq!(normal.position_at(-0.5).unwrap(), -48.0);
    let stop = RateMapping::new(96.0, 1.0, 0.0, 0.5, 1.0 / 96.0).unwrap();
    assert_eq!(stop.time_at(96.0).unwrap(), 1.5);
    assert_eq!(stop.position_at(1.25).unwrap(), 96.0);
    assert_eq!(stop.time_at(120.0).unwrap(), 1.75);
    let negative = RateMapping::new(96.0, 1.0, 0.0, -0.5, 1.0 / 96.0).unwrap();
    assert_eq!(negative.time_at(120.0).unwrap(), 1.0);
    assert_eq!(negative.position_at(1.0).unwrap(), 144.0);
    let warp = RateMapping::new(96.0, 1.0, 48.0, 0.0, 1.0 / 96.0).unwrap();
    assert_eq!(warp.time_at(120.0).unwrap(), 1.0);
    assert_eq!(warp.position_at(1.0).unwrap(), 144.0);
    let combined = RateMapping::new(96.0, 1.0, 48.0, -0.5, 1.0 / 96.0).unwrap();
    assert_eq!(combined.time_at(192.0).unwrap(), 1.0);
    assert_eq!(combined.position_at(1.0).unwrap(), 192.0);
}
#[test]
fn timeline_exports_ordered_validated_rate_mappings_with_actual_pause_and_warp_state() {
    let source=b"#VERSION:0.83;#BPMS:0=120,8=150;#STOPS:2=0.5,6=-0.75;#DELAYS:4=0.25;#WARPS:5=1;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    let timing = GenerationTiming::from_simfile(source, Format::Ssc, 0).unwrap();
    let timeline = EditorTimeline::build(
        &timing,
        &Grid::new(4, 1).unwrap(),
        None,
        &AtomicBool::new(false),
    )
    .unwrap();
    let rates: Vec<_> = timeline.rate_mappings().collect();
    assert!(!rates.is_empty());
    let document = deadsync_editor::document::Document::parse(
        std::sync::Arc::<[u8]>::from(source.as_slice()),
        Format::Ssc,
    )
    .unwrap();
    let session = deadsync_editor::history::Session::new(document);
    let clock = deadsync_editor::playback::Clock::capture(&session, 0).unwrap();
    assert_eq!(clock.rate_mappings().collect::<Vec<_>>(), rates);
    assert_eq!(
        clock.spacing_sections().collect::<Vec<_>>(),
        timeline.spacing_sections().collect::<Vec<_>>()
    );

    assert!(
        rates
            .windows(2)
            .all(|pair| (pair[0].1.row(), pair[0].0) <= (pair[1].1.row(), pair[1].0))
    );
    assert!(rates.iter().any(|(_, m)| m.stop() > 0.0));
    assert!(rates.iter().any(|(_, m)| m.stop() < 0.0));
    assert!(rates.iter().any(|(_, m)| m.warp() > 0.0));
    for row in (0..960).step_by(12) {
        let mapping = rates
            .iter()
            .rev()
            .find(|(_, m)| m.row() < row as f64)
            .unwrap_or(&rates[0])
            .1;
        assert_eq!(
            mapping.time_at(row as f64).unwrap(),
            timeline.position_time(row).unwrap()
        );
    }
    for (_, mapping) in rates {
        assert!(mapping.seconds_per_row() > 0.0 && mapping.rows_per_second() > 0.0);
        assert!(mapping.time().is_finite());
    }
}
#[test]
fn invalid_rate_mappings_and_nonfinite_queries_are_explicit() {
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, 0.0] {
        assert!(RateMapping::new(0.0, 0.0, 0.0, 0.0, invalid).is_err());
    }
    assert!(RateMapping::new(-1.0, 0.0, 0.0, 0.0, 1.0).is_err());
    assert!(RateMapping::new(0.0, 0.0, -1.0, 0.0, 1.0).is_err());
    assert!(RateMapping::new(0.0, f64::NAN, 0.0, 0.0, 1.0).is_err());
    assert!(RateMapping::new(0.0, 0.0, 0.0, 0.0, f64::from_bits(1)).is_err());
    let mapping = RateMapping::new(0.0, 0.0, 0.0, 0.0, 1.0).unwrap();
    for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(mapping.time_at(v).is_err());
        assert!(mapping.position_at(v).is_err());
    }
}
