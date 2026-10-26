//! One retained mesh for raster runs; no actor or texture allocation per pixel.
use deadlib_present::actors::{Actor, SizeSpec};
use deadlib_render_core::{BlendMode, MeshVertex};
use deadsync_editor::Error;
use std::{collections::BTreeMap, sync::Arc};
#[derive(Clone, Copy)]
struct Run {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    color: u32,
}
pub struct Mesh {
    pub vertices: Arc<[MeshVertex]>,
    reversed_vertices: Arc<[MeshVertex]>,
    pub complete: bool,
    pub rectangles: usize,
    width: u32,
    height: u32,
}
impl Mesh {
    /// A truncated mesh is explicitly incomplete; the caller must show loading
    /// or a partial-result notice, rather than present it as the entire chart.
    pub fn build(width: u32, height: u32, pixels: &[u32], limit: usize) -> Result<Self, Error> {
        if width == 0
            || height == 0
            || width > 1024
            || height > 16384
            || pixels.len() != width as usize * height as usize
            || limit > 65536
        {
            return Err(Error::InvalidEdit(
                "Invalid minimap mesh dimensions or budget".into(),
            ));
        }
        let mut active = BTreeMap::<(u32, u32, u32), Run>::new();
        let mut rectangles = Vec::new();
        let mut complete = true;
        'rows: for y in 0..height {
            let mut next = BTreeMap::new();
            let mut x = 0;
            while x < width {
                let color = pixels[y as usize * width as usize + x as usize];
                let start = x;
                x += 1;
                while x < width && pixels[y as usize * width as usize + x as usize] == color {
                    x += 1;
                }
                if color >> 24 == 0 {
                    continue;
                }
                let key = (start, x - start, color);
                let run = if let Some(mut run) = active.remove(&key) {
                    run.height += 1;
                    run
                } else {
                    Run {
                        x: start,
                        y,
                        width: x - start,
                        height: 1,
                        color,
                    }
                };
                next.insert(key, run);
            }
            for run in active.into_values() {
                if rectangles.len() == limit {
                    complete = false;
                    active = next;
                    break 'rows;
                }
                rectangles.push(run);
            }
            active = next;
        }
        for run in active.into_values() {
            if rectangles.len() == limit {
                complete = false;
                break;
            }
            rectangles.push(run);
        }
        let count = rectangles.len();
        let mut vertices = Vec::with_capacity(count * 6);
        for run in rectangles {
            let x = run.x as f32;
            let y = run.y as f32;
            let right = x + run.width as f32;
            let bottom = y + run.height as f32;
            let color = [
                (run.color & 255) as f32 / 255.0,
                ((run.color >> 8) & 255) as f32 / 255.0,
                ((run.color >> 16) & 255) as f32 / 255.0,
                (run.color >> 24) as f32 / 255.0,
            ];
            for pos in [
                [x, y],
                [right, y],
                [right, bottom],
                [x, y],
                [right, bottom],
                [x, bottom],
            ] {
                vertices.push(MeshVertex { pos, color });
            }
        }
        let reversed_vertices = vertices
            .iter()
            .map(|vertex| MeshVertex {
                pos: [vertex.pos[0], height as f32 - vertex.pos[1]],
                color: vertex.color,
            })
            .collect::<Vec<_>>()
            .into();
        Ok(Self {
            vertices: vertices.into(),
            reversed_vertices,
            complete,
            rectangles: count,
            width,
            height,
        })
    }
    pub fn actor(&self, x: f32, y: f32, reverse: bool, z: i16) -> Actor {
        // Both orientations share retained vertex buffers across actor builds.
        let vertices = Arc::clone(if reverse {
            &self.reversed_vertices
        } else {
            &self.vertices
        });
        Actor::Mesh {
            align: [0.0, 0.0],
            offset: [x, y],
            size: [
                SizeSpec::Px(self.width as f32),
                SizeSpec::Px(self.height as f32),
            ],
            tint: [1.0; 4],
            vertices,
            visible: true,
            blend: BlendMode::Alpha,
            z,
        }
    }
}
