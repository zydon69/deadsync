//! Original StepDensityEffect measure-mesh construction in source world space.
//! Bounded plot aggregation remains the caller's policy, not a mesh dimension.
use super::{Plot, colors::Palette};
use crate::density_preferences::ColorMode;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
}
pub fn build_world(
    plot: &Plot,
    final_time: f64,
    width: f32,
    height: f32,
    palette: Palette,
    mode: ColorMode,
) -> Vec<Vertex> {
    if width <= 2.0 || height <= 2.0 || plot.samples.is_empty() {
        return Vec::new();
    }
    let low = super::colors::rgba(palette.low);
    let mut vertices: Vec<Vertex> = Vec::with_capacity(plot.samples.len() * 2 + 1);
    let mut indices = Vec::with_capacity(plot.samples.len() * 6 + 3);
    let (mut previous_high, mut previous_low) = (0, 0);
    let (mut previous_rate, mut previous_previous_rate) = (0.0, 0.0);
    let step_height = height - 2.0;
    let step_width = width - 2.0;
    for (i, sample) in plot.samples.iter().enumerate() {
        // Source casts AFTER the double product; it does not first round the
        // ratio to float32 and does not clamp valid negative chart times/rates.
        let x = 1.0 + (sample.time / final_time * step_width as f64) as f32;
        let y = 1.0 + (sample.percent * step_height as f64) as f32;
        if !sample.has_steps {
            if i == 0 || !plot.samples[i - 1].has_steps {
                continue;
            }
            vertices.push(Vertex {
                position: [x, y],
                color: low,
            });
            indices.extend([previous_low, previous_high, vertices.len() - 1]);
            previous_low = vertices.len() - 1;
            previous_high = vertices.len() - 1;
            previous_previous_rate = previous_rate;
            previous_rate = sample.rate;
            continue;
        }
        if i >= 2
            && plot.samples[i - 1].has_steps
            && plot.samples[i - 2].has_steps
            && (sample.rate - previous_rate).abs() <= 0.0001
            && (sample.rate - previous_previous_rate).abs() <= 0.0001
        {
            vertices[previous_low].position[0] = x;
            vertices[previous_high].position[0] = x;
            previous_previous_rate = previous_rate;
            previous_rate = sample.rate;
            continue;
        }
        let color = palette.color(sample.percent as f32);
        vertices.push(Vertex {
            position: [x, 1.0],
            color: if mode == ColorMode::ColorByDensity {
                color
            } else {
                low
            },
        });
        vertices.push(Vertex {
            position: [x, y],
            color,
        });
        if i != 0 && plot.samples[i - 1].has_steps {
            indices.extend([
                previous_low,
                previous_high,
                vertices.len() - 2,
                previous_high,
                vertices.len() - 1,
                vertices.len() - 2,
            ]);
        }
        previous_low = vertices.len() - 2;
        previous_high = vertices.len() - 1;
        previous_previous_rate = previous_rate;
        previous_rate = sample.rate;
    }
    if plot.samples.last().is_some_and(|sample| sample.has_steps) {
        // The original endpoint is stepWidth, without the minX prefix.
        vertices.push(Vertex {
            position: [step_width, 1.0],
            color: low,
        });
        indices.extend([previous_low, previous_high, vertices.len() - 1]);
    }
    let mut triangles = Vec::with_capacity(indices.len());
    for triangle in indices.chunks_exact(3) {
        // Original invalid/zero-peak timelines can yield NaN positions. Their
        // GPU behavior is undefined; the native consumer emits no such triangle.
        if triangle
            .iter()
            .all(|i| vertices[*i].position.into_iter().all(f32::is_finite))
        {
            triangles.extend(triangle.iter().map(|i| vertices[*i]));
        }
    }
    triangles
}
