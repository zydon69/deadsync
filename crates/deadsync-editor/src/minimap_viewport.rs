//! Source-based viewport assembly for the next minimap increment. Not wired yet.
use crate::{Error, minimap_navigation::Position, minimap_projection::Projection, playback::Clock};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Time,
    Row,
}
#[derive(Clone, Copy, Debug)]
pub struct Viewport {
    pub top: f64,
    pub bottom: f64,
    pub focal: f64,
}
impl Viewport {
    pub fn range(self, pixels_per_unit: f64) -> Result<f64, Error> {
        if ![self.top, self.bottom, self.focal, pixels_per_unit]
            .into_iter()
            .all(f64::is_finite)
            || self.top >= self.bottom
            || pixels_per_unit <= 0.0
        {
            return Err(Error::InvalidEdit("Invalid minimap viewport".into()));
        }
        let range = (self.bottom - self.top) / pixels_per_unit;
        if !range.is_finite() || range <= 0.0 {
            return Err(Error::InvalidEdit("Invalid minimap viewport range".into()));
        }
        Ok(range)
    }
    pub fn earlier_distance(self, reverse: bool) -> f64 {
        if reverse {
            self.bottom - self.focal
        } else {
            self.focal - self.top
        }
    }
}
/// Port of SetMiniMapAreaFromEditorArea; quantization is applied by Projection.
pub fn area_start(
    full_start: f64,
    full_end: f64,
    editor_start: f64,
    editor_end: f64,
    mini_range: f64,
) -> Result<f64, Error> {
    if ![full_start, full_end, editor_start, editor_end, mini_range]
        .into_iter()
        .all(f64::is_finite)
        || full_end <= full_start
        || editor_end < editor_start
        || mini_range <= 0.0
    {
        return Err(Error::InvalidEdit("Invalid minimap area".into()));
    }
    if mini_range >= full_end - full_start {
        return Ok(full_start);
    }
    let denominator = full_end - full_start - (editor_end - editor_start);
    if denominator == 0.0 {
        return Err(Error::InvalidEdit("Degenerate minimap area".into()));
    }
    let percentage = (editor_start - full_start) / denominator;
    Ok(full_start + percentage.clamp(0.0, 1.0) * (full_end - full_start - mini_range))
}
pub struct Geometry {
    pub axis: Axis,
    pub projection: Projection,
}
impl Geometry {
    /// `content` and `focus` are in the selected axis. Bounds come from the real
    /// rendered viewport, rather than assumed screen-height/focal percentages.
    pub fn build(
        axis: Axis,
        viewport: Viewport,
        pixels_per_unit: f64,
        focus: f64,
        content: (f64, f64),
        visible_range: f64,
        map_y: i32,
        map_height: i32,
        reverse: bool,
        quantize: bool,
    ) -> Result<Self, Error> {
        if content.0 > content.1 {
            return Err(Error::InvalidEdit("Inverted minimap content bounds".into()));
        }
        let editor_range = viewport.range(pixels_per_unit)?;
        let distance = viewport.earlier_distance(reverse);
        let editor_start = focus - distance / pixels_per_unit;
        let editor_end = editor_start + editor_range;
        let full_start = content.0 - editor_range;
        let full_end = content.1 + editor_range;
        let mini_start = area_start(
            full_start,
            full_end,
            editor_start,
            editor_end,
            visible_range,
        )?;
        let projection = Projection {
            full_start,
            full_end,
            editor_start,
            editor_end,
            mini_start,
            mini_unquantized: mini_start,
            mini_range: visible_range,
            bounds_y: map_y,
            visible_height: map_height,
            grabbed_percentage: distance / (viewport.bottom - viewport.top),
            reverse,
            quantize,
        }
        .initialized()
        .ok_or_else(|| Error::InvalidEdit("Invalid minimap projection".into()))?;
        Ok(Self { axis, projection })
    }
    pub fn focused_coordinate(&self) -> f64 {
        self.projection.editor_start
            + (self.projection.editor_end - self.projection.editor_start)
                * self.projection.grabbed_percentage
    }
    /// Preserve the chosen row even inside a warp. Time-to-row inversion would
    /// select a different equal-time row and is not an equivalent operation.
    pub fn position(&self, clock: &Clock) -> Result<Position, Error> {
        let coordinate = self.focused_coordinate();
        match self.axis {
            Axis::Time => {
                let music_time = clock.chart_time_to_music(coordinate)?;
                Ok(Position {
                    chart_time: coordinate,
                    music_time,
                    row: clock.position(music_time)?,
                })
            }
            Axis::Row => Ok(Position {
                row: coordinate,
                chart_time: clock.fractional_chart_time(coordinate)?,
                music_time: clock.fractional_music_time(coordinate)?,
            }),
        }
    }
}
