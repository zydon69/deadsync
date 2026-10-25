//! Shared constant-row chart projection. Time/variable spacing requires a
//! timing-aware projection and is intentionally not represented by this type.
use crate::Error;

#[derive(Clone, Copy, Debug)]
pub struct RowProjection {
    anchor: u64,
    cursor: f64,
    scale: f64,
    top: f64,
    bottom: f64,
}
impl RowProjection {
    pub fn new(
        anchor: u64,
        cursor: f64,
        pixels_per_row: f64,
        top: f64,
        bottom: f64,
    ) -> Result<Self, Error> {
        if ![cursor, pixels_per_row, top, bottom]
            .iter()
            .all(|v| v.is_finite())
            || pixels_per_row <= 0.0
            || top > cursor
            || cursor > bottom
        {
            return Err(Error::InvalidEdit("Invalid chart projection".into()));
        }
        Ok(Self {
            anchor,
            cursor,
            scale: pixels_per_row,
            top,
            bottom,
        })
    }
    /// Subtract integers before conversion to retain nearby-row precision even
    /// when the anchor is too large to be represented exactly by a double.
    pub fn y(self, row: u64) -> f64 {
        let delta = if row >= self.anchor {
            (row - self.anchor) as f64
        } else {
            -((self.anchor - row) as f64)
        };
        self.cursor + delta * self.scale
    }
    pub fn fractional_y(self, row: f64) -> f64 {
        self.cursor + (row - self.anchor as f64) * self.scale
    }
    pub fn row_at(self, y: f64) -> f64 {
        self.anchor as f64 + (y - self.cursor) / self.scale
    }
    pub fn clipped_y(self, row: u64) -> f64 {
        self.y(row).clamp(self.top, self.bottom)
    }
    pub fn clipped_fractional_y(self, row: f64) -> f64 {
        self.fractional_y(row).clamp(self.top, self.bottom)
    }
    /// Include partially visible rows at both edges. Float-to-integer casts
    /// saturate at zero/u64::MAX; negative lead-in remains available via row_at.
    pub fn visible_rows(self) -> (u64, u64) {
        (
            self.anchor
                .saturating_sub(((self.cursor - self.top) / self.scale).ceil() as u64),
            self.anchor
                .saturating_add(((self.bottom - self.cursor) / self.scale).ceil() as u64),
        )
    }
}

/// Inclusive integer bands for bounded overview queries. Every row belongs to
/// exactly one band, even over the entire u64 domain. No chart rows are walked.
#[derive(Clone, Debug)]
pub struct RowBands {
    start: u128,
    span: u128,
    count: u128,
    next: u128,
}
impl RowBands {
    pub fn new(start: u64, end: u64, maximum: usize) -> Result<Self, Error> {
        if start > end || !(1..=4096).contains(&maximum) {
            return Err(Error::InvalidEdit("Invalid overview bands".into()));
        }
        let span = u128::from(end) - u128::from(start) + 1;
        Ok(Self {
            start: u128::from(start),
            span,
            count: span.min(maximum as u128),
            next: 0,
        })
    }
}
impl Iterator for RowBands {
    type Item = (u64, u64);
    fn next(&mut self) -> Option<Self::Item> {
        if self.next == self.count {
            return None;
        }
        let a = self.start + self.span * self.next / self.count;
        self.next += 1;
        let b = self.start + self.span * self.next / self.count - 1;
        Some((a as u64, b as u64))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.count - self.next) as usize;
        (remaining, Some(remaining))
    }
}
impl ExactSizeIterator for RowBands {}
impl std::iter::FusedIterator for RowBands {}
