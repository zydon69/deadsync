//! Vertical MouseDown/Move selection policy from pinned MiniMap.cs.
//! The caller supplies a virtual keyboard pointer inside the map's horizontal
//! bounds. Cursor fraction stays separate from the grabbed fraction.
use crate::{minimap_position_preferences::SelectMode, minimap_projection::Projection};
#[derive(Default, Debug)]
pub struct Capture {
    grabbed: Option<f64>,
}
impl Capture {
    pub fn grabbed(&self) -> bool {
        self.grabbed.is_some()
    }
    pub fn clear(&mut self) {
        self.grabbed = None;
    }
    pub fn begin(
        &mut self,
        projection: &mut Projection,
        screen_y: i32,
        cursor: f64,
        mode: SelectMode,
    ) -> Option<bool> {
        let mut next = *projection;
        next.initialized()?;
        if !next.mini_unquantized.is_finite() {
            return None;
        }
        if !cursor.is_finite() || next.editor_end <= next.editor_start {
            return None;
        }
        let y = next.reflect_y(screen_y)?;
        let relative = f64::from(y) - f64::from(next.bounds_y);
        if relative < 0.0 || relative > f64::from(next.visible_height) {
            return Some(false);
        }
        let start = next.project_editor(next.editor_start);
        let end = next.project_editor(next.editor_end);
        let mut grabbed = if relative >= start && relative <= end {
            Some((relative - start) / (end - start))
        } else {
            let cursor_fraction =
                (cursor - next.editor_start) / (next.editor_end - next.editor_start);
            match mode {
                SelectMode::MoveToCursor => Some(cursor_fraction),
                SelectMode::MoveToSelectedArea => {
                    let selected = next.position_at_pixel(f64::from(y));
                    let range = next.editor_end - next.editor_start;
                    next.editor_start = selected - (cursor - next.editor_start);
                    next.editor_end = next.editor_start + range;
                    let full = next.full_end - next.full_start;
                    next.mini_start = if next.mini_range >= full {
                        next.full_start
                    } else {
                        let percentage = (next.editor_start - next.full_start) / (full - range);
                        next.full_start + percentage.clamp(0.0, 1.0) * (full - next.mini_range)
                    };
                    next = next.initialized()?;
                    let a = next.project_editor(next.editor_start);
                    let b = next.project_editor(next.editor_end);
                    (relative >= a && relative <= b).then_some((relative - a) / (b - a))
                }
            }
        };
        if grabbed.is_some_and(|fraction| !fraction.is_finite()) {
            return None;
        }
        if let Some(fraction) = grabbed {
            let cursor_fraction = next.grabbed_percentage;
            next.grabbed_percentage = fraction;
            next.drag(screen_y)?;
            next.grabbed_percentage = cursor_fraction;
        }
        if !next.editor_start.is_finite() || !next.editor_end.is_finite() {
            return None;
        }
        *projection = next;
        self.grabbed = grabbed.take();
        Some(true)
    }
    pub fn move_to(&self, projection: &mut Projection, screen_y: i32) -> Option<()> {
        let Some(grabbed) = self.grabbed else {
            return Some(());
        };
        let mut next = *projection;
        let cursor_fraction = next.grabbed_percentage;
        next.grabbed_percentage = grabbed;
        next.drag(screen_y)?;
        next.grabbed_percentage = cursor_fraction;
        *projection = next;
        Some(())
    }
}
