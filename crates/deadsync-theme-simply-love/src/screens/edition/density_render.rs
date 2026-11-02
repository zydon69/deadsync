//! Bounded retained mesh for the source per-measure density graph.
use deadlib_present::actors::{Actor, SizeSpec};
use deadlib_render_core::{BlendMode, MeshVertex};
use deadsync_editor::{
    density::Plot,
    density::layout::Orientation,
    density_preferences::{ColorMode, Preferences},
};
use std::sync::Arc;
pub(super) struct Mesh {
    pub vertices: Arc<[MeshVertex]>,
    pub peak: f64,
    pub sampled: bool,
}
impl Mesh {
    pub fn build(
        plot: &Plot,
        final_time: f64,
        p: Preferences,
        width: i32,
        height: i32,
        orientation: Orientation,
        reverse: bool,
    ) -> Self {
        let palette = deadsync_editor::density::colors::Palette::new(p.low, p.high, p.background);
        let (horizontal, vertical) = if orientation == Orientation::Horizontal {
            (width as f32, height as f32)
        } else {
            (height as f32, width as f32)
        };
        let vertices = deadsync_editor::density::mesh::build_world(
            plot,
            final_time,
            horizontal,
            vertical,
            palette,
            p.color_mode,
        )
        .into_iter()
        .map(|vertex| {
            let [x, y] = vertex.position;
            MeshVertex {
                pos: if orientation == Orientation::Horizontal {
                    [x, vertical - y]
                } else {
                    [vertical - y, if reverse { horizontal - x } else { x }]
                },
                color: vertex.color,
            }
        })
        .collect::<Vec<_>>();
        Self {
            vertices: vertices.into(),
            peak: plot.peak,
            sampled: plot.aggregated,
        }
    }
    pub fn actor(&self, x: f32, y: f32, width: f32, height: f32) -> Actor {
        Actor::Mesh {
            align: [0.0, 0.0],
            offset: [x, y],
            size: [SizeSpec::Px(width), SizeSpec::Px(height)],
            tint: [1.0; 4],
            vertices: Arc::clone(&self.vertices),
            visible: true,
            blend: BlendMode::Alpha,
            z: 8005,
        }
    }
}

#[cfg(test)]
mod source_color_tests {
    use super::*;
    use deadsync_editor::density::Sample;
    #[test]
    fn actual_mesh_vertices_use_source_quantized_colors_in_both_modes_and_orientations() {
        let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../deadsync-editor/tests/fixtures/density-raw-source.json"
        )))
        .unwrap();
        for case in cases {
            let mut p = Preferences::read(
                case["value"]["PreferencesDensityGraph"]
                    .as_object()
                    .unwrap(),
            )
            .unwrap();
            let expected: Vec<[f32; 4]> = case["colors"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| std::array::from_fn(|i| c["bytes"][i].as_u64().unwrap() as f32 / 255.0))
                .collect();
            let plot = Plot {
                samples: (0..5)
                    .map(|i| Sample {
                        has_steps: i != 0,
                        time: i as f64,
                        rate: i as f64,
                        percent: i as f64 / 4.0,
                    })
                    .collect(),
                peak: 4.0,
                aggregated: false,
            };
            for mode in [ColorMode::ColorByDensity, ColorMode::ColorByHeight] {
                p.color_mode = mode;
                for orientation in [Orientation::Horizontal, Orientation::Vertical] {
                    for reverse in [false, true] {
                        let mesh =
                            Mesh::build(&plot, 5.0, p, i32::MAX, i32::MAX, orientation, reverse);
                        assert!(!mesh.vertices.is_empty());
                        assert!(mesh.vertices.len() <= plot.samples.len() * 6 + 3);
                        for vertex in mesh.vertices.iter() {
                            assert!(vertex.pos.into_iter().all(f32::is_finite));
                            assert!(
                                expected.contains(&vertex.color),
                                "{} {:?}",
                                case["name"],
                                vertex.color
                            );
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod original_mesh_projection_tests {
    use super::*;
    use deadsync_editor::{
        density::{Measure, Series},
        density_preferences::Accumulation,
    };
    #[test]
    fn actual_mesh_maps_original_triangles_and_colors_for_both_axes_and_reverse() {
        let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../deadsync-editor/tests/fixtures/density-mesh-source.json"
        )))
        .unwrap();
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
            let mode = if input["accumulation"] == "Step" {
                Accumulation::Step
            } else {
                Accumulation::Row
            };
            let plot = series.plot(mode, usize::MAX).unwrap();
            let mut p = Preferences::default();
            p.color_mode = if input["mode"] == "ColorByDensity" {
                ColorMode::ColorByDensity
            } else {
                ColorMode::ColorByHeight
            };
            let width = input["width"].as_i64().unwrap() as i32;
            let height = input["height"].as_i64().unwrap() as i32;
            for orientation in [Orientation::Horizontal, Orientation::Vertical] {
                for reverse in [false, true] {
                    let (w, h) = if orientation == Orientation::Horizontal {
                        (width, height)
                    } else {
                        (height, width)
                    };
                    let mesh = Mesh::build(&plot, series.final_time, p, w, h, orientation, reverse);
                    assert_eq!(
                        mesh.vertices.len(),
                        case["vertices"].as_array().unwrap().len()
                    );
                    for (actual, expected) in mesh
                        .vertices
                        .iter()
                        .zip(case["vertices"].as_array().unwrap())
                    {
                        let x = expected["x"].as_f64().unwrap() as f32;
                        let y = expected["y"].as_f64().unwrap() as f32;
                        let pos = if orientation == Orientation::Horizontal {
                            [x, height as f32 - y]
                        } else {
                            [
                                height as f32 - y,
                                if reverse { width as f32 - x } else { x },
                            ]
                        };
                        assert_eq!(actual.pos, pos, "{}", input["name"]);
                        let packed = expected["color"].as_u64().unwrap() as u32;
                        assert_eq!(
                            actual.color,
                            std::array::from_fn(|i| ((packed >> (i * 8)) & 255) as f32 / 255.0)
                        );
                    }
                }
            }
        }
    }
}
