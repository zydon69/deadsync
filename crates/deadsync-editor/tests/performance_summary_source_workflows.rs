use deadsync_editor::performance::{
    monitor::{Monitor, Timing},
    preferences::{Preferences, Scale},
    summary::Summary,
};
use serde_json::Value;

fn same_float(actual: f32, expected: &Value) {
    if expected.as_str() == Some("NaN") {
        assert!(actual.is_nan());
    } else {
        assert_eq!(
            actual.to_bits(),
            (expected.as_f64().unwrap() as f32).to_bits()
        );
    }
}

#[test]
fn whole_original_ui_summary_matches_sixty_recorded_draws() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/performance-summary.expected.json")).unwrap();
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 60);
    for case in fixture["cases"].as_array().unwrap() {
        let input = &case["input"];
        let mut monitor = Monitor::default();
        let mut tick = 100;
        monitor.begin_frame(tick);
        for (index, duration) in input["durations"].as_array().unwrap().iter().enumerate() {
            for (timing_index, timing) in Timing::ALL.into_iter().skip(1).enumerate() {
                monitor.add(
                    timing,
                    (index as i64 + 1) * (timing_index as i64 + 1) * 100_000,
                );
            }
            tick += duration.as_i64().unwrap();
            monitor.begin_frame(tick);
        }
        let preferences = Preferences {
            max_frames: input["count"].as_i64().unwrap() as i32,
            scale: Scale::from_raw(input["mode"].as_i64().unwrap() as i32),
            explicit_seconds: 0.0125,
            ..Preferences::default()
        };
        let summary =
            Summary::capture(&monitor, preferences, case["frequency"].as_u64().unwrap()).unwrap();
        let plots = case["plots"].as_array().unwrap();
        assert_eq!(plots.len(), Timing::ALL.len());
        for (index, plot) in plots.iter().enumerate() {
            assert_eq!(
                summary.frames.len(),
                plot["count"].as_u64().unwrap() as usize
            );
            same_float(summary.plot_scale, &plot["max"]);
            same_float(summary.current[index], &case["current"][index]);
            same_float(summary.averages[index], &case["averages"][index]);
            same_float(summary.maxima[index], &case["maxima"][index]);
            for (actual, expected) in summary.frames.iter().zip(plot["data"].as_array().unwrap()) {
                same_float(actual[index], expected);
            }
        }
        let frame_average = format!("{:.6} ms", summary.frame_average * 1000.0);
        assert_eq!(frame_average, case["labels"]["Avg Frame Time"], "{input}");
    }
}

#[test]
fn empty_recorder_and_missing_frequency_have_no_summary() {
    let mut monitor = Monitor::default();
    assert!(Summary::capture(&monitor, Preferences::default(), 1_000_000_000).is_none());
    monitor.begin_frame(0);
    monitor.begin_frame(10);
    assert!(Summary::capture(&monitor, Preferences::default(), 0).is_none());
}
