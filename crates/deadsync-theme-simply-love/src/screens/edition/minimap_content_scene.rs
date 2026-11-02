//! Isolated native composition adapter. Not connected to edition State yet.
//! The validated viewport is input; the caller owns cache identity and UI status.
use deadsync_editor::{
    Error,
    minimap_content::{AddResult, Raster},
    minimap_event_policy::{self as policy, Budget, Pattern, Shape},
    minimap_palette::Colors,
    minimap_projection::Projection,
    minimap_viewport::Axis,
    notes::{Grid, Note, Position, minimap_query::rows_for_chart_time},
    playback::Clock,
};
#[derive(Clone, Copy)]
pub struct Point {
    pub row: f64,
    pub chart_time: f64,
}
impl Point {
    fn coordinate(self, axis: Axis) -> f64 {
        match axis {
            Axis::Time => self.chart_time,
            Axis::Row => self.row,
        }
    }
}
pub struct Overlays<'a> {
    pub patterns: &'a [Pattern],
    pub preview: Option<(Point, Point)>,
    pub labels: &'a [Point],
    pub show_patterns: bool,
    pub show_preview: bool,
    pub show_labels: bool,
    pub pattern_width: u32,
    pub preview_width: u32,
    pub pattern_color: u32,
    pub preview_color: u32,
    pub label_color: u32,
}
#[derive(Clone, Copy)]
pub struct Limits {
    pub source_notes: usize,
    pub query_heads: usize,
    pub tracks: usize,
    pub markers: usize,
}
pub struct Scene {
    pub raster: Raster,
    pub notes_complete: bool,
    pub markers_complete: bool,
    pub note_count: usize,
    pub tree_queries: usize,
}
/// Build the note/marker foreground using supplied source UI colors. Full
/// UpdateBegin background/rim/fade composition is intentionally still separate.
#[cfg(test)]
pub fn build(
    grid: &Grid,
    clock: &Clock,
    projection: &Projection,
    axis: Axis,
    width: u32,
    note_width: u32,
    note_spacing: u32,
    background: u32,
    overlays: Overlays<'_>,
    limits: Limits,
    colors: impl Fn(Position, &Note) -> Result<Colors, Error>,
) -> Result<Scene, Error> {
    build_internal(
        grid,
        clock,
        projection,
        axis,
        width,
        note_width,
        note_spacing,
        background,
        overlays,
        limits,
        None,
        colors,
    )
}
pub fn build_on_background(
    grid: &Grid,
    clock: &Clock,
    projection: &Projection,
    axis: Axis,
    width: u32,
    note_width: u32,
    note_spacing: u32,
    background: u32,
    overlays: Overlays<'_>,
    limits: Limits,
    base: deadsync_editor::minimap_background::Background,
    colors: impl Fn(Position, &Note) -> Result<Colors, Error>,
) -> Result<Scene, Error> {
    build_internal(
        grid,
        clock,
        projection,
        axis,
        width,
        note_width,
        note_spacing,
        background,
        overlays,
        limits,
        Some(base),
        colors,
    )
}
fn build_internal(
    grid: &Grid,
    clock: &Clock,
    projection: &Projection,
    axis: Axis,
    width: u32,
    note_width: u32,
    note_spacing: u32,
    background: u32,
    overlays: Overlays<'_>,
    limits: Limits,
    base: Option<deadsync_editor::minimap_background::Background>,
    colors: impl Fn(Position, &Note) -> Result<Colors, Error>,
) -> Result<Scene, Error> {
    if projection.visible_height <= 0
        || limits.source_notes > 65535
        || limits.query_heads > 65536
        || limits.markers > 65536
    {
        return Err(Error::InvalidEdit("Invalid minimap scene budget".into()));
    }
    let mut raster = Raster::new(
        width,
        projection.visible_height as u32,
        u32::from(grid.columns()),
        note_width,
        note_spacing,
        projection.quantize,
        background,
    )?;
    if let Some(base) = base {
        raster.replace_pixels(base.pixels)?;
        raster.line(base.content_start_y, 1, width - 2, 0xffffffff, false)?;
        raster.line(base.content_end_y, 1, width - 2, 0xffffffff, false)?;
        raster.line(base.cursor_y, 1, width - 2, 0xffcccccc, true)?;
    }
    let mut marker_count = 0;
    let mut markers_complete = true;
    let marker = |raster: &mut Raster,
                  x: u32,
                  end_x: u32,
                  start: f64,
                  end: f64,
                  color: u32,
                  marker_count: &mut usize,
                  markers_complete: &mut bool|
     -> Result<bool, Error> {
        if *marker_count == limits.markers {
            *markers_complete = false;
            return Ok(false);
        }
        *marker_count += 1;
        let start = Raster::project(projection, start, false)?;
        let end = Raster::project(projection, end, false)?;
        raster.rect(x, end_x, start, end, color)?;
        Ok(true)
    };
    if overlays.show_patterns {
        // Do not allocate/merge an unbounded slice just to obey a draw budget.
        let mut pending: Option<Pattern> = None;
        for (index, pattern) in overlays.patterns.iter().copied().enumerate() {
            if index == limits.markers {
                markers_complete = false;
                break;
            }
            if let Some(previous) = pending.as_mut().filter(|p| p.end_row == pattern.start_row) {
                previous.end_row = pattern.end_row;
                previous.end_time = pattern.end_time;
                continue;
            }
            if let Some(previous) = pending.take() {
                let (start, end) = match axis {
                    Axis::Row => (previous.start_row as f64, previous.end_row as f64),
                    Axis::Time => (previous.start_time, previous.end_time),
                };
                if !marker(
                    &mut raster,
                    1u32.max(width.saturating_sub(overlays.pattern_width.saturating_add(1))),
                    width - 2,
                    start,
                    end,
                    overlays.pattern_color,
                    &mut marker_count,
                    &mut markers_complete,
                )? {
                    break;
                }
            }
            pending = Some(pattern);
        }
        if let Some(previous) = pending {
            let (start, end) = match axis {
                Axis::Row => (previous.start_row as f64, previous.end_row as f64),
                Axis::Time => (previous.start_time, previous.end_time),
            };
            let _ = marker(
                &mut raster,
                1u32.max(width.saturating_sub(overlays.pattern_width.saturating_add(1))),
                width - 2,
                start,
                end,
                overlays.pattern_color,
                &mut marker_count,
                &mut markers_complete,
            )?;
        }
    }
    if overlays.show_preview {
        if let Some((start, end)) = overlays.preview {
            let _ = marker(
                &mut raster,
                1,
                (width - 2).min(overlays.preview_width),
                start.coordinate(axis),
                end.coordinate(axis),
                overlays.preview_color,
                &mut marker_count,
                &mut markers_complete,
            )?;
        }
    }
    if overlays.show_labels {
        for (index, label) in overlays.labels.iter().enumerate() {
            if index == limits.markers {
                markers_complete = false;
                break;
            }
            if label.row == 0.0 {
                continue;
            }
            if marker_count == limits.markers {
                markers_complete = false;
                break;
            }
            marker_count += 1;
            let y = Raster::project(projection, label.coordinate(axis), false)?;
            if raster.line(y, 1, width - 2, overlays.label_color, false)? == AddResult::BelowBottom
            {
                break;
            }
        }
    }
    // Include one raster pixel beyond either edge, preserving the source hold
    // tail's +1 pixel and floor boundary. Clip remains the raster's authority.
    let unit = projection.mini_range / f64::from(projection.visible_height);
    let start = projection.mini_start - unit;
    let end = projection.mini_start + projection.mini_range + unit;
    let rows = match axis {
        Axis::Time => rows_for_chart_time(clock, start, end, grid.last_row())?,
        Axis::Row => {
            let first = start.max(0.0).floor() as u64;
            let last = end.max(0.0).ceil() as u64;
            Some((first, last))
        }
    };
    let mut count = 0;
    let mut queries = 0;
    let mut complete = true;
    if let Some((first, last)) = rows {
        let query = grid.minimap_chronological(first, last, limits.query_heads, limits.tracks)?;
        queries = query.tree_queries;
        complete = query.complete;
        let coordinate = |row: u64| match axis {
            Axis::Row => Ok(row as f64),
            Axis::Time => clock.note_time(row),
        };
        let add = |raster: &mut Raster,
                   position: Position,
                   note: &Note|
         -> Result<Option<AddResult>, Error> {
            let Some(shape) = policy::note_shape(note.kind) else {
                return Ok(None);
            };
            let palette = colors(position, note)?;
            let start = Raster::project(projection, coordinate(position.row)?, false)?;
            Ok(Some(match shape {
                Shape::Short => raster.short_note(position.lane.into(), start, palette.head)?,
                Shape::Mine => raster.short_note(position.lane.into(), start, palette.mine)?,
                Shape::Sustain { roll } => {
                    let end = Raster::project(
                        projection,
                        coordinate(note.end.ok_or_else(|| {
                            Error::InvalidEdit("Missing minimap sustain tail".into())
                        })?)?,
                        false,
                    )?;
                    raster.hold(
                        position.lane.into(),
                        start,
                        end,
                        palette.head,
                        if roll { palette.roll } else { palette.hold },
                    )?
                }
            }))
        };
        for &(position, note) in &query.crossings {
            let _ = add(&mut raster, position, note)?;
        }
        let mut budget = Budget::new(limits.source_notes, query.crossings.len());
        for (index, &(position, note)) in query.heads.iter().enumerate() {
            let result = add(&mut raster, position, note)?;
            if budget.stop_after(note.kind, result == Some(AddResult::BelowBottom)) {
                if result == Some(AddResult::BelowBottom) {
                    complete = true;
                } else if index + 1 < query.heads.len() || !query.complete {
                    complete = false;
                }
                break;
            }
        }
        count = budget.count();
    }
    Ok(Scene {
        raster,
        notes_complete: complete,
        markers_complete,
        note_count: count,
        tree_queries: queries,
    })
}
