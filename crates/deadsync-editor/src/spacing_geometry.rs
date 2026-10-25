//! Incremental chart geometry using the original active-chart rate transitions.
use crate::{
    Error,
    spacing::{Anchor, Helper},
    spacing_index::Index,
    spacing_search::Settings,
};
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
