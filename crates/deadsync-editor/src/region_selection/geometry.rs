//! Original selection overlap predicates. Notes intentionally have asymmetric
//! exact-edge lane rules; miscellaneous widgets use inclusive rectangles.
use super::misc_layout::Rect;
use crate::{
    Error, selection_preferences::Overlap, spacing::Helper, spacing_index::Index,
    spacing_search::Settings,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Time,
    Position,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Range {
    pub min: f64,
    pub max: f64,
}
impl Range {
    pub fn normalized(a: f64, b: f64) -> Result<Self, Error> {
        if !a.is_finite() || !b.is_finite() {
            return invalid("Nonfinite region boundary");
        }
        Ok(Self {
            min: a.min(b),
            max: a.max(b),
        })
    }
}
/// Native center spacing can differ from drawn arrow width after size zoom.
/// This adaptation uses those real dimensions; equal spacing/width reproduces
/// source GetSelectedLanes after translation from centered chart coordinates.
pub fn selected_lanes(
    x: Range,
    center_spacing: f64,
    arrow_width: f64,
    mode: Overlap,
) -> Result<(i32, i32), Error> {
    if ![x.min, x.max, center_spacing, arrow_width]
        .iter()
        .all(|x| x.is_finite())
        || x.min > x.max
        || center_spacing <= 0.0
        || arrow_width <= 0.0
    {
        return invalid("Invalid lane selection geometry");
    }
    let half_gap = (center_spacing - arrow_width) * 0.5;
    let (lo, hi) = match mode.effective() {
        Overlap::Any => (
            ((x.min + half_gap) / center_spacing).floor(),
            ((x.max - half_gap) / center_spacing).floor(),
        ),
        Overlap::All => (
            ((x.min - half_gap) / center_spacing).ceil(),
            ((x.max + half_gap) / center_spacing).floor() - 1.0,
        ),
        _ => (
            ((x.min + center_spacing * 0.5) / center_spacing).floor(),
            ((x.max - center_spacing * 0.5) / center_spacing).floor(),
        ),
    };
    if lo < i32::MIN as f64 || lo > i32::MAX as f64 || hi < i32::MIN as f64 || hi > i32::MAX as f64
    {
        return invalid("Lane region exceeds editor geometry");
    }
    Ok((lo as i32, hi as i32))
}
pub fn contains_rect(mode: Overlap, region: Rect, event: Rect) -> bool {
    let (x0, y0, x1, y1) = (
        region.x,
        region.y,
        region.x + region.width,
        region.y + region.height,
    );
    match mode.effective() {
        Overlap::Any => {
            event.x <= x1
                && event.x + event.width >= x0
                && event.y <= y1
                && event.y + event.height >= y0
        }
        Overlap::All => {
            event.x >= x0
                && event.x + event.width <= x1
                && event.y >= y0
                && event.y + event.height <= y1
        }
        _ => {
            let (x, y) = (event.x + event.width * 0.5, event.y + event.height * 0.5);
            x >= x0 && x <= x1 && y >= y0 && y <= y1
        }
    }
}
/// Source selects the last rate <= each endpoint, unlike FindBest's strict
/// focal lookup. Endpoint-local padding is intentionally not integrated across
/// rates, matching original AdjustSelection*Range behavior.
pub fn adjusted_range(
    index: &Index,
    settings: Settings,
    axis: Axis,
    bounds: Range,
    half_height: f64,
    mode: Overlap,
) -> Result<Range, Error> {
    if ![bounds.min, bounds.max, half_height]
        .iter()
        .all(|x| x.is_finite())
        || bounds.min > bounds.max
        || half_height < 0.0
    {
        return invalid("Invalid region padding");
    }
    if !matches!(mode.effective(), Overlap::Any | Overlap::All) {
        return Ok(bounds);
    }
    let padding = |value: f64| -> Result<f64, Error> {
        let sections = index.sections();
        if sections.is_empty() {
            return invalid("Missing initialized region timing sections");
        }
        let n = sections.partition_point(|s| match axis {
            Axis::Time => s.mapping.time() <= value,
            Axis::Position => s.mapping.row() <= value,
        });
        let rate = sections[n.saturating_sub(1)];
        let mut helper = Helper::new(settings.mode, settings.reverse);
        helper.update(
            rate.spacing_rate(),
            settings.speeds,
            settings.interpolated_scroll,
            settings.zoom,
        )?;
        // Keep signed infinities/NaN when a variable rate collapses to zero.
        // The original strict range predicate then decides whether to select.
        Ok(half_height
            / match axis {
                Axis::Time => helper.pixels_per_second(),
                Axis::Position => helper.pixels_per_row(),
            })
    };
    let lower = padding(bounds.min)?;
    let upper = padding(bounds.max)?;
    Ok(if mode.effective() == Overlap::Any {
        Range {
            min: bounds.min - lower,
            max: bounds.max + upper,
        }
    } else {
        Range {
            min: bounds.min + lower,
            max: bounds.max - upper,
        }
    })
}
/// Lower bound strictly excluded, upper bound included. Holds are tested at
/// their head position only by the consumer, never by body intersection.
pub fn contains_head(bounds: Range, value: f64) -> bool {
    bounds.min < bounds.max && value > bounds.min && value <= bounds.max
}
fn invalid<T>(message: &str) -> Result<T, Error> {
    Err(Error::InvalidEdit(message.into()))
}
