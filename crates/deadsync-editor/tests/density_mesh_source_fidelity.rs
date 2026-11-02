use deadsync_editor::{
    density::{Measure, Series, colors::Palette, mesh},
    density_preferences::{Accumulation, ColorMode},
};
use serde_json::Value;
#[test]
fn original_mesh_source_rates_peaks_percent_and_expanded_triangles_match_bits() {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/density-mesh-source.json")).unwrap();
    for case in cases {
        let input = &case["input"];
        let series = Series {
            measures: input["times"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
                .map(|(i, t)| Measure {
                    start_time: t.as_f64().unwrap(),
                    steps: input["steps"][i].as_u64().unwrap() as u32,
                    rows: input["rows"][i].as_u64().unwrap() as u32,
                })
                .collect(),
            final_time: input["end"].as_f64().unwrap(),
            streams: Vec::new(),
            breakdown: None,
            source_byte_overflows: 0,
        };
        let accumulation = if input["accumulation"] == "Step" {
            Accumulation::Step
        } else {
            Accumulation::Row
        };
        let peak = case[if accumulation == Accumulation::Step {
            "peakNps"
        } else {
            "peakRps"
        }]
        .as_f64()
        .unwrap();
        assert_eq!(series.peak(accumulation).to_bits(), peak.to_bits());
        let samples = series.samples(accumulation);
        for (sample, expected) in samples.iter().zip(case["rates"].as_array().unwrap()) {
            assert_eq!(
                sample.rate.to_bits(),
                expected["rate"].as_f64().unwrap().to_bits()
            );
            assert_eq!(
                sample.percent.to_bits(),
                expected["percent"].as_f64().unwrap().to_bits()
            );
        }
        let plot = series.plot(accumulation, usize::MAX).unwrap();
        let mode = if input["mode"] == "ColorByDensity" {
            ColorMode::ColorByDensity
        } else {
            ColorMode::ColorByHeight
        };
        let vertices = mesh::build_world(
            &plot,
            series.final_time,
            input["width"].as_i64().unwrap() as f32,
            input["height"].as_i64().unwrap() as f32,
            Palette::new(
                [0.251, 0.647, 0.419, 1.],
                [0.705, 0.282, 0.282, 1.],
                [0.118, 0.118, 0.118, 1.],
            ),
            mode,
        );
        assert_eq!(
            vertices.len(),
            case["vertices"].as_array().unwrap().len(),
            "{}",
            input["name"]
        );
        for (actual, expected) in vertices.iter().zip(case["vertices"].as_array().unwrap()) {
            assert_eq!(
                actual.position[0].to_bits(),
                (expected["x"].as_f64().unwrap() as f32).to_bits(),
                "{}",
                input["name"]
            );
            assert_eq!(
                actual.position[1].to_bits(),
                (expected["y"].as_f64().unwrap() as f32).to_bits(),
                "{}",
                input["name"]
            );
            let packed = expected["color"].as_u64().unwrap() as u32;
            let rgba = std::array::from_fn(|i| ((packed >> (i * 8)) & 255) as f32 / 255.0);
            assert_eq!(actual.color, rgba, "{}", input["name"]);
        }
    }
}
