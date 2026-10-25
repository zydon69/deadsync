//! Incremental chart geometry using the original active-chart rate transitions.
use crate::{
    Error,
    spacing::{Anchor, Helper},
    spacing_index::Index,
    spacing_search::Settings,
    zoom::SpacingMode,
};
/// Conservative inclusive candidate rows for one rate section. Adjacent spans
/// may overlap at event phases; callers still filter exact event Y and phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisibleRows {
    pub section: usize,
    pub start: u64,
    pub end: u64,
}

#[derive(Clone, Copy)]
pub struct Cursor<'a> {
    index: &'a Index,
    section: usize,
    anchor: Anchor,
    helper: Helper,
    settings: Settings,
}
impl<'a> Cursor<'a> {
    pub fn begin(
        index: &'a Index,
        focal: Anchor,
        settings: Settings,
    ) -> Result<Option<Self>, Error> {
        if ![focal.y, focal.time, focal.row]
            .iter()
            .all(|v| v.is_finite())
        {
            return Err(Error::InvalidEdit(
                "Nonfinite geometry focal position".into(),
            ));
        }
        let Some(section) = index.active(settings.mode, focal.time, focal.row)? else {
            return Ok(None);
        };
        let rate = index.sections()[section];
        let mut helper = Helper::new(settings.mode, settings.reverse);
        helper.update(
            rate.spacing_rate(),
            settings.speeds,
            settings.interpolated_scroll,
            settings.zoom,
        )?;
        let anchor = Anchor {
            y: helper.y(rate.mapping.time(), rate.mapping.row(), focal)?,
            time: rate.mapping.time(),
            row: rate.mapping.row(),
        };
        Ok(Some(Self {
            index,
            section,
            anchor,
            helper,
            settings,
        }))
    }
    pub fn section(&self) -> usize {
        self.section
    }
    pub fn rate_anchor(&self) -> Anchor {
        self.anchor
    }
    /// Move coefficients and anchor atomically, retaining original forward vs
    /// backward arithmetic order. No allocations are made during traversal.
    pub fn move_to(&mut self, section: usize) -> Result<(), Error> {
        let mut next = *self;
        next.walk_to(section)?;
        *self = next;
        Ok(())
    }
    fn walk_to(&mut self, target: usize) -> Result<(), Error> {
        if target >= self.index.sections().len() {
            return Err(Error::InvalidEdit("Invalid geometry section".into()));
        }
        while self.section < target {
            let next = self.index.sections()[self.section + 1];
            let y = self
                .helper
                .y(next.mapping.time(), next.mapping.row(), self.anchor)?;
            self.helper.update(
                next.spacing_rate(),
                self.settings.speeds,
                self.settings.interpolated_scroll,
                self.settings.zoom,
            )?;
            self.section += 1;
            self.anchor = Anchor {
                y,
                time: next.mapping.time(),
                row: next.mapping.row(),
            };
        }
        while self.section > target {
            let previous = self.index.sections()[self.section - 1];
            self.helper.update(
                previous.spacing_rate(),
                self.settings.speeds,
                self.settings.interpolated_scroll,
                self.settings.zoom,
            )?;
            let y = self
                .helper
                .y(previous.mapping.time(), previous.mapping.row(), self.anchor)?;
            self.section -= 1;
            self.anchor = Anchor {
                y,
                time: previous.mapping.time(),
                row: previous.mapping.row(),
            };
        }
        Ok(())
    }
    /// Visit every rate section, retaining all viewport crossings when variable
    /// scrolling folds back on itself. Work is proportional to rate events,
    /// never chart rows. Collapsed sections need no inverse division.
    ///
    /// The supplied last row bounds the chart domain. Results are conservative
    /// candidates for bounded note/grid queries, not a replacement for exact
    /// event projection. In particular same-row timing phases may overlap.
    pub fn visible_rows(
        &self,
        top: f64,
        bottom: f64,
        last_row: u64,
    ) -> Result<Vec<VisibleRows>, Error> {
        if !top.is_finite() || !bottom.is_finite() || top > bottom {
            return Err(Error::InvalidEdit("Invalid geometry viewport".into()));
        }
        let mut cursor = *self;
        cursor.walk_to(0)?;
        let mut result = Vec::new();
        for section in 0..self.index.sections().len() {
            cursor.walk_to(section)?;
            let rate = self.index.sections()[section];
            // Include neighboring integer rows to retain exact-boundary events
            // despite floating-point inverse rounding, including large rows.
            let lower = if section == 0 {
                0
            } else {
                rate.mapping.row().next_down().floor().max(0.0) as u64
            };
            let upper = self
                .index
                .sections()
                .get(section + 1)
                .map(|next| next.mapping.row().next_up().ceil().max(0.0) as u64)
                .unwrap_or(last_row)
                .min(last_row);
            if lower > upper {
                continue;
            }
            let coefficient = if self.settings.mode == SpacingMode::ConstantTime {
                cursor.helper.pixels_per_second()
            } else {
                cursor.helper.pixels_per_row()
            };
            let (start, end) = if coefficient == 0.0 {
                if cursor.anchor.y < top || cursor.anchor.y > bottom {
                    continue;
                }
                (lower, upper)
            } else if self.settings.mode == SpacingMode::ConstantTime {
                // time_at is monotone within a section but not injective:
                // warps and negative stops collapse entire row intervals.
                // Find both ends of that preimage instead of using position_at,
                // whose single answer would discard collapsed visible notes.
                let direction = if self.settings.reverse { -1.0 } else { 1.0 };
                let minimum = (top * direction).min(bottom * direction);
                let maximum = (top * direction).max(bottom * direction);
                let first = |threshold: f64, strict: bool| -> Result<u128, Error> {
                    let mut lo = u128::from(lower);
                    let mut hi = u128::from(upper) + 1;
                    while lo < hi {
                        let mid = lo + (hi - lo) / 2;
                        let y = cursor
                            .helper
                            .row_y(mid as f64, cursor.anchor.y, |row| rate.mapping.time_at(row))?
                            * direction;
                        if y < threshold || (strict && y == threshold) {
                            lo = mid + 1;
                        } else {
                            hi = mid;
                        }
                    }
                    Ok(lo)
                };
                let start = first(minimum, false)?;
                let end = first(maximum, true)?;
                if start >= end {
                    continue;
                }
                (start as u64, (end - 1) as u64)
            } else {
                let row_at = |y| {
                    cursor
                        .helper
                        .location(
                            y,
                            cursor.anchor,
                            false,
                            |time| rate.mapping.position_at(time),
                            |row| rate.mapping.time_at(row),
                        )
                        .map(|(_, row)| row)
                };
                let a = row_at(top)?;
                let b = row_at(bottom)?;
                let start = (a.min(b).next_down().floor().max(0.0) as u64).max(lower);
                let end = (a.max(b).next_up().ceil().max(0.0) as u64).min(upper);
                if start > end {
                    continue;
                }
                (start, end)
            };
            result.push(VisibleRows {
                section,
                start,
                end,
            });
        }
        Ok(result)
    }

    /// Original CheckForCompletingHold endpoint: the tail's phase is 11,
    /// and the cap follows Reverse, independently of a negative scroll rate.
    /// Cap height is already scaled by the caller's note-size zoom.
    pub fn hold_end_y(&mut self, row: f64, cap_height: f64) -> Result<f64, Error> {
        if !cap_height.is_finite() || cap_height < 0.0 {
            return Err(Error::InvalidEdit("Invalid hold cap height".into()));
        }
        let mut next = *self;
        let section = next
            .index
            .event_active(row, 11)?
            .ok_or_else(|| Error::InvalidEdit("Missing geometry section".into()))?;
        next.walk_to(section)?;
        let mapping = next.index.sections()[section].mapping;
        let y = next
            .helper
            .row_y(row, next.anchor.y, |row| mapping.time_at(row))?
            + cap_height * if next.settings.reverse { -1.0 } else { 1.0 };
        if !y.is_finite() {
            return Err(Error::InvalidEdit("Nonfinite hold endpoint".into()));
        }
        *self = next;
        Ok(y)
    }
    pub fn y_in_section(&self, time: f64, row: f64) -> Result<f64, Error> {
        self.helper.y(time, row, self.anchor)
    }
    /// Select by the event's original row/order key, not the strict focal-point
    /// lookup. A rate boundary itself uses its preceding section's coefficients.
    pub fn event_y(&mut self, time: f64, row: f64, order: u8) -> Result<f64, Error> {
        let mut next = *self;
        let section = next
            .index
            .event_active(row, order)?
            .ok_or_else(|| Error::InvalidEdit("Missing geometry section".into()))?;
        next.walk_to(section)?;
        let y = next.y_in_section(time, row)?;
        *self = next;
        Ok(y)
    }
}

/// Stable per-frame section anchors. Point queries use binary event lookup and
/// constant work, without rewalking sections for every lane or overlay.
pub struct Frame<'a> {
    origin: Cursor<'a>,
    anchors: Vec<f64>,
}
impl<'a> Frame<'a> {
    pub fn capture(origin: Cursor<'a>) -> Result<Self, Error> {
        let mut cursor = origin;
        cursor.walk_to(0)?;
        let mut anchors = Vec::with_capacity(origin.index.sections().len());
        for section in 0..origin.index.sections().len() {
            cursor.walk_to(section)?;
            anchors.push(cursor.anchor.y);
        }
        Ok(Self { origin, anchors })
    }
    pub fn row_y(&self, row: f64, order: u8) -> Result<f64, Error> {
        let section = self
            .origin
            .index
            .event_active(row, order)?
            .ok_or_else(|| Error::InvalidEdit("Missing geometry section".into()))?;
        let rate = self.origin.index.sections()[section];
        let settings = self.origin.settings;
        let mut helper = Helper::new(settings.mode, settings.reverse);
        helper.update(
            rate.spacing_rate(),
            settings.speeds,
            settings.interpolated_scroll,
            settings.zoom,
        )?;
        helper.row_y(row, self.anchors[section], |row| rate.mapping.time_at(row))
    }
    pub fn visible_rows(
        &self,
        top: f64,
        bottom: f64,
        last_row: u64,
    ) -> Result<Vec<VisibleRows>, Error> {
        self.origin.visible_rows(top, bottom, last_row)
    }
}
