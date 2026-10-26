//! Retained immutable native frames, bound to live Session/chart identity.
use super::minimap_content_render::Mesh;
use deadsync_editor::{
    Error, chart_order::Binding, history::Session, minimap_projection::Projection,
    minimap_viewport::Axis,
};
use std::{cell::RefCell, sync::Arc};
#[derive(Clone, PartialEq, Eq)]
pub struct Key {
    pub axis: Axis,
    pub projection: [u64; 9],
    pub width: u32,
    pub height: u32,
    pub focused: bool,
    pub captured: bool,
    pub selection: Option<(u64, u64)>,
    pub player: u32,
    pub palette_revision: u64,
    pub options_revision: u64,
}
impl Key {
    pub fn projection_values(projection: &Projection) -> [u64; 9] {
        [
            projection.full_start.to_bits(),
            projection.full_end.to_bits(),
            projection.mini_start.to_bits(),
            projection.mini_unquantized.to_bits(),
            projection.mini_range.to_bits(),
            projection.editor_start.to_bits(),
            projection.editor_end.to_bits(),
            projection.grabbed_percentage.to_bits(),
            u64::from(projection.quantize),
        ]
    }
}
pub struct Frame {
    pub mesh: Mesh,
    pub notes_complete: bool,
    pub markers_complete: bool,
    pub note_count: usize,
    pub tree_queries: usize,
}
impl Frame {
    pub fn complete(&self) -> bool {
        self.notes_complete && self.markers_complete && self.mesh.complete
    }
}
struct Entry {
    binding: Binding,
    key: Key,
    result: Result<Arc<Frame>, Arc<str>>,
}
#[derive(Default)]
pub struct Cache {
    entry: RefCell<Option<Entry>>,
}
impl Cache {
    pub fn clear(&mut self) {
        *self.entry.get_mut() = None;
    }
    pub fn current(
        &self,
        session: &Session,
        chart: usize,
        key: &Key,
    ) -> Option<Result<Arc<Frame>, Arc<str>>> {
        self.entry
            .borrow()
            .as_ref()
            .filter(|entry| &entry.key == key && entry.binding.is_current(session, chart))
            .map(|entry| entry.result.clone())
    }
    pub fn frame(
        &self,
        session: &Session,
        chart: usize,
        key: Key,
        build: impl FnOnce() -> Result<Frame, Error>,
    ) -> Result<Arc<Frame>, Arc<str>> {
        let mut entry = self.entry.borrow_mut();
        if let Some(current) = entry
            .as_ref()
            .filter(|entry| entry.key == key && entry.binding.is_current(session, chart))
        {
            return current.result.clone();
        }
        let binding = Binding::capture(session, chart)
            .map_err(|error| Arc::<str>::from(error.to_string()))?;
        let result = build()
            .map(Arc::new)
            .map_err(|error| Arc::<str>::from(error.to_string()));
        *entry = Some(Entry {
            binding,
            key,
            result: result.clone(),
        });
        result
    }
}
