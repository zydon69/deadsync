use deadsync_editor::{
    audio_preferences::File,
    density::{Event, Series, StreamSettings},
    density_preferences::{Accumulation, Preferences},
    document::{Document, Format},
    history::Session,
};
use serde_json::{Value, json};
fn fixtures() -> Value {
    serde_json::from_str(include_str!("fixtures/density.expected.json")).unwrap()
}
#[test]
fn original_preference_defaults_postload_partial_vectors_and_restore_history_match() {
    let f = fixtures();
    for case in f["Preferences"].as_array().unwrap() {
        let input = Preferences::read(case["Input"].as_object().unwrap()).unwrap();
        let normalized = Preferences::read(case["Normalized"].as_object().unwrap()).unwrap();
        assert_eq!(input, normalized);
        let changed = Preferences::read(case["Changed"].as_object().unwrap()).unwrap();
        let undone = Preferences::read(case["Undone"].as_object().unwrap()).unwrap();
        assert_eq!(changed, undone);
        assert_eq!(
            Preferences::read(case["Restored"].as_object().unwrap()).unwrap(),
            Preferences::default()
        );
    }
    assert_eq!(
        Preferences::read(json!({"DensityGraphLowColor":{"X":1}}).as_object().unwrap())
            .unwrap()
            .low,
        [1.0, 0.0, 0.0, 0.0]
    );
}
#[test]
fn exact_source_counts_stream_gaps_and_nps_match_with_documented_safe_overflow() {
    let expected = fixtures();
    let requests: Value =
        serde_json::from_str(include_str!("fixtures/density.expected.request.json")).unwrap();
    for (request, case) in requests
        .as_array()
        .unwrap()
        .iter()
        .zip(expected["Cases"].as_array().unwrap())
    {
        assert_eq!(request["Name"], case["Name"]);
        let name = request["Name"].as_str().unwrap();
        let stream = StreamSettings::read(request["Stream"].as_object().unwrap()).unwrap();
        // Timing is a separate existing engine; these starts were produced by
        // the unchanged source RecomputeMeasureTiming where Rates are supplied.
        let times = case["Result"]["Measures"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["StartTime"].as_f64().unwrap())
            .collect();
        let events = request["Events"].as_array().unwrap().iter().map(|e| Event {
            row: e["Row"].as_u64().unwrap(),
            step: e["Step"].as_bool().unwrap(),
            fake: e["Fake"].as_bool().unwrap(),
        });
        let s = Series::build(
            events,
            times,
            request["FinalTime"].as_f64().unwrap(),
            stream,
            2_000_000,
        )
        .unwrap();
        let result = &case["Result"];
        assert_eq!(
            s.breakdown.as_deref().unwrap_or("No Streams"),
            result["Breakdown"].as_str().unwrap(),
            "{name}"
        );
        for (m, e) in s
            .measures
            .iter()
            .zip(result["Measures"].as_array().unwrap())
        {
            assert_eq!(m.start_time, e["StartTime"].as_f64().unwrap());
            assert_eq!(m.rows, e["RowsWithSteps"].as_u64().unwrap() as u32);
            if name != "byte-overflow-source" {
                assert_eq!(m.steps, e["Steps"].as_u64().unwrap() as u32);
            } else {
                assert_eq!(m.steps % 256, e["Steps"].as_u64().unwrap() as u32);
            }
        }
        assert_eq!(s.streams.len(), result["Streams"].as_array().unwrap().len());
        for (s, e) in s.streams.iter().zip(result["Streams"].as_array().unwrap()) {
            assert_eq!(s.start, e["Start"].as_u64().unwrap() as usize);
            assert_eq!(s.last, e["Last"].as_u64().unwrap() as usize);
        }
        for (mode, e) in [Accumulation::Step, Accumulation::Row]
            .into_iter()
            .zip(result["Peaks"].as_array().unwrap())
        {
            let samples = s.samples(mode);
            assert!(
                samples
                    .iter()
                    .all(|v| v.rate.is_finite() && v.percent.is_finite())
            );
            if name != "byte-overflow-source" {
                assert!((s.peak(mode) - e["Peak"].as_f64().unwrap()).abs() < 1e-12);
                for (v, e) in samples.iter().zip(e["Samples"].as_array().unwrap()) {
                    assert!((v.rate - e["StepsPerSecond"].as_f64().unwrap()).abs() < 1e-12);
                    if let Some(percent) = e["Percent"].as_f64() {
                        assert!((v.percent - percent).abs() < 1e-12);
                    } else {
                        assert_eq!(v.percent, 0.0);
                    }
                }
            }
        }
        assert_eq!(
            s.source_byte_overflows,
            usize::from(name == "byte-overflow-source")
        );
    }
}
#[test]
fn preference_maps_preserve_unknown_nested_values_and_reject_invalid_source_types() {
    let mut fields=json!({"Vendor":"keep","DensityGraphPositionOffsets":{"FutureMount":7},"DensityGraphLowColor":{"X":0.2,"Y":0.3,"Z":0.4,"W":1,"Future":9}}).as_object().unwrap().clone();
    let p = Preferences::read(&fields).unwrap();
    p.write(&mut fields).unwrap();
    assert_eq!(fields["Vendor"], "keep");
    assert_eq!(fields["DensityGraphPositionOffsets"]["FutureMount"], 7);
    assert_eq!(fields["DensityGraphLowColor"]["Future"], 9);
    for value in [
        json!({"DensityGraphHeight":2147483648u64}),
        json!({"DensityGraphPositionOffsets":{"LeftSideOfWindow":-2147483649i64}}),
        json!({"DensityGraphLowColor":{"X":1e50}}),
    ] {
        assert!(Preferences::read(value.as_object().unwrap()).is_err());
    }
}
fn session() -> Session {
    Session::new(Document::parse(b"#TITLE:Preferences;".as_slice(), Format::Ssc).unwrap())
}
#[test]
fn density_history_owns_changed_nested_leaf_fields_and_keeps_unrelated_updates() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("preferences.json");
    std::fs::write(
        &path,
        serde_json::to_vec(
            &json!({"Version":1,"Vendor":"keep","DensityGraph":{"Future":"preserve"}}),
        )
        .unwrap(),
    )
    .unwrap();
    let mut s = session();
    let file = File::open(&path).unwrap();
    let before = file.density();
    let mut after = before;
    after.height = 100;
    after.low[0] = 0.5;
    s.set_density_preferences(before, after, Some(&file))
        .unwrap();
    assert!(!s.is_dirty());
    s.take_density_preferences_update();
    let mut external = File::open(&path).unwrap();
    let mut p = external.density();
    p.position_offsets[0] = 23;
    external.set_density(p).unwrap();
    let mut audio = external.audio();
    audio.set_music_rate(1.5).unwrap();
    external.set_audio(audio);
    external.save().unwrap();
    s.undo().unwrap();
    let u = s.take_density_preferences_update().unwrap();
    assert_eq!(u.preferences.height, 90);
    assert_eq!(u.preferences.position_offsets[0], 23);
    assert_eq!(u.preferences.low, before.low);
    let loaded = File::open(&path).unwrap();
    assert_eq!(loaded.audio().music_rate(), 1.5);
    let raw: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(raw["Vendor"], "keep");
    assert_eq!(raw["DensityGraph"]["Future"], "preserve");
    s.redo().unwrap();
    let p = File::open(&path).unwrap().density();
    assert_eq!(p.height, 100);
    assert_eq!(p.low[0], 0.5);
    assert_eq!(p.position_offsets[0], 23);
}
#[test]
fn conflicting_owned_density_field_blocks_undo_without_losing_disk_data() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    let file = File::open(&path).unwrap();
    let before = file.density();
    let mut after = before;
    after.height = 100;
    let mut s = session();
    s.set_density_preferences(before, after, Some(&file))
        .unwrap();
    let mut external = File::open(&path).unwrap();
    let mut p = external.density();
    p.height = 123;
    external.set_density(p).unwrap();
    external.save().unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert!(matches!(
        s.undo(),
        Err(deadsync_editor::Error::ExternalChange)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(File::open(&path).unwrap().density().height, 123);
}
#[test]
fn series_rejects_nonfinite_unordered_and_exhausted_budgets_atomically() {
    assert!(
        Series::build(
            [Event {
                row: 0,
                step: true,
                fake: false
            }],
            vec![0.0],
            f64::from_bits(1),
            Default::default(),
            10
        )
        .is_err()
    );
    let e = Event {
        row: 0,
        step: true,
        fake: false,
    };
    assert!(Series::build([e], vec![f64::NAN], 2.0, Default::default(), 10).is_err());
    assert!(Series::build([e], vec![0.0], 2.0, Default::default(), 0).is_err());
    assert!(
        Series::build(
            [Event { row: 192, ..e }, e],
            vec![0.0, 2.0],
            4.0,
            Default::default(),
            10
        )
        .is_err()
    );
    assert!(
        Series::build(
            [Event { row: 192, ..e }],
            vec![0.0],
            2.0,
            Default::default(),
            10
        )
        .is_err()
    );
}
#[test]
fn original_six_density_mount_geometries_match_and_invalid_scaling_refuses() {
    use deadsync_editor::density::layout::{self, Focus, Input, Orientation, Rect};
    use deadsync_editor::density_preferences::Position;
    let f = fixtures();
    for (position, case) in Position::ALL
        .into_iter()
        .zip(f["Geometry"].as_array().unwrap())
    {
        let mut p = Preferences::default();
        p.position = position;
        assert_eq!(position.name(), case["Position"].as_str().unwrap());
        let b = layout::bounds(
            p,
            Input {
                chart_area: Rect {
                    x: 3,
                    y: 5,
                    width: 854,
                    height: 480,
                },
                header: 96,
                focus: Some(Focus {
                    lanes_start: 200,
                    lanes_end: 400,
                    misc_end: 300,
                    misc_scaled_end: 360,
                }),
                dpi_scale: 1.0,
            },
        )
        .unwrap();
        let e = &case["Bounds"];
        assert_eq!(
            b.rect,
            Rect {
                x: e["X"].as_i64().unwrap() as i32,
                y: e["Y"].as_i64().unwrap() as i32,
                width: e["Width"].as_i64().unwrap() as i32,
                height: e["Height"].as_i64().unwrap() as i32
            }
        );
        assert_eq!(
            b.orientation,
            if e["Orientation"] == "Vertical" {
                Orientation::Vertical
            } else {
                Orientation::Horizontal
            }
        );
    }
}
#[test]
fn worker_uses_source_same_row_measure_clock_not_note_or_cursor_phase() {
    use deadsync_editor::density::task::Task;
    use std::sync::atomic::AtomicBool;
    let cases = fixtures();
    for (name, tag) in [
        ("stop-at-measure-boundary", "#STOPS:4=1;"),
        ("warp-at-measure-boundary", "#WARPS:4=4;"),
        ("stop-at-zero-measure", "#STOPS:0=1;"),
    ] {
        let notes = if name == "stop-at-zero-measure" {
            "1000,\n1000"
        } else {
            "1000,\n1000,\n1000"
        };
        let bytes = format!(
            "#VERSION:0.83;#BPMS:0=120;{tag}#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:{notes};"
        );
        let s = Session::new(
            Document::parse(
                std::sync::Arc::<[u8]>::from(bytes.into_bytes()),
                Format::Ssc,
            )
            .unwrap(),
        );
        let r = Task::capture(&s, 0, Default::default())
            .unwrap()
            .run(&AtomicBool::new(false))
            .unwrap();
        let e = cases["Cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["Name"] == name)
            .unwrap();
        assert!(r.is_current(&s, 0));
        assert!(!r.is_current(&session(), 0));
        for (m, e) in r
            .series
            .measures
            .iter()
            .zip(e["Result"]["Measures"].as_array().unwrap())
        {
            assert!(
                (m.start_time - e["StartTime"].as_f64().unwrap()).abs() < 1e-12,
                "{name}: {}",
                m.start_time
            );
        }
        assert!(
            (r.series.final_time
                - if name == "stop-at-measure-boundary" {
                    7.0
                } else if name == "warp-at-measure-boundary" {
                    4.0
                } else {
                    5.0
                })
            .abs()
                < 1e-12
        );
    }
}
#[test]
fn worker_counts_routine_participants_rows_and_filters_source_fake_heads() {
    use deadsync_editor::{
        density::task::Task,
        notes::{Grid, Note, Position},
    };
    use std::sync::{Arc, atomic::AtomicBool};
    let mut grid = Grid::new(8, 2).unwrap();
    for player in 0..2 {
        for lane in 0..2 {
            grid.set(
                Position {
                    row: 0,
                    lane,
                    player,
                },
                Some(Note::tap()),
            )
            .unwrap();
        }
    }
    let mut bytes =
        b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:".to_vec();
    bytes.extend(grid.serialize().unwrap());
    bytes.push(b';');
    let s = Session::new(Document::parse(Arc::<[u8]>::from(bytes), Format::Ssc).unwrap());
    let r = Task::capture(&s, 0, Default::default())
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    assert_eq!(r.series.measures[0].steps, 4);
    assert_eq!(r.series.measures[0].rows, 1);
    assert!(
        Task::capture(&s, 0, Default::default())
            .unwrap()
            .run(&AtomicBool::new(true))
            .is_err()
    );
}
#[test]
fn source_fake_regions_and_warp_heads_are_excluded_without_using_rssp_scan() {
    use deadsync_editor::{
        density::task::Task,
        notes::{Grid, Note, Position},
    };
    use std::sync::{Arc, atomic::AtomicBool};
    let mut grid = Grid::new(4, 1).unwrap();
    for row in [0, 48, 96, 144, 192] {
        grid.set(
            Position {
                row,
                lane: 0,
                player: 0,
            },
            Some(Note::tap()),
        )
        .unwrap();
    }
    let mut bytes=b"#VERSION:0.83;#BPMS:0=120;#WARPS:1=1;#FAKES:3=1;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:".to_vec();
    bytes.extend(grid.serialize().unwrap());
    bytes.push(b';');
    let s = Session::new(Document::parse(Arc::<[u8]>::from(bytes), Format::Ssc).unwrap());
    let r = Task::capture(&s, 0, Default::default())
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    assert_eq!(
        r.series
            .measures
            .iter()
            .map(|m| m.steps)
            .collect::<Vec<_>>(),
        vec![2, 1]
    );
}
#[test]
fn entire_chart_plot_aggregation_retains_global_peak_with_bounded_samples() {
    let count = 9000usize;
    let events = (0..count).flat_map(|m| {
        std::iter::repeat_n(
            Event {
                row: m as u64 * 192,
                step: true,
                fake: false,
            },
            if m == 7777 { 16 } else { 1 },
        )
    });
    let times = (0..count).map(|m| m as f64 * 2.0).collect();
    let s = Series::build(events, times, count as f64 * 2.0, Default::default(), 10000).unwrap();
    let p = s.plot(Accumulation::Step, 2048).unwrap();
    assert!(p.aggregated);
    assert!(p.samples.len() <= 2048);
    assert_eq!(p.peak, 8.0);
    assert!(
        p.samples
            .iter()
            .any(|s| s.time == 7777.0 * 2.0 && s.rate == 8.0 && s.percent == 1.0)
    );
    assert!(s.plot(Accumulation::Step, 0).is_err());
}
