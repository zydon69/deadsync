//! Shared constant-row chart projection. Time/variable spacing requires a
//! timing-aware projection and is intentionally not represented by this type.
use crate::Error;

#[derive(Clone, Copy, Debug)]
pub struct RowProjection {
    anchor: u64,
    offset: f64,
    cursor: f64,
    scale: f64,
    reverse: bool,
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
            offset: 0.0,
            cursor,
            scale: pixels_per_row,
            reverse: false,
            top,
            bottom,
        })
    }
    /// Change direction without changing the positive spacing magnitude.
    pub fn with_reverse(mut self, reverse: bool) -> Self {
        self.reverse = reverse;
        self
    }
    /// Fractional displacement from the integer anchor. Subtraction of integer
    /// rows still happens before float conversion, preserving large-row precision.
    pub fn with_row_offset(mut self, offset: f64) -> Result<Self, Error> {
        if !offset.is_finite() {
            return Err(Error::InvalidEdit("Nonfinite row projection offset".into()));
        }
        self.offset = offset;
        Ok(self)
    }
    fn signed_scale(self) -> f64 {
        self.scale * if self.reverse { -1.0 } else { 1.0 }
    }
    /// Subtract integers before conversion to retain nearby-row precision even
    /// when the anchor is too large to be represented exactly by a double.
    pub fn y(self, row: u64) -> f64 {
        let delta = if row >= self.anchor {
            (row - self.anchor) as f64
        } else {
            -((self.anchor - row) as f64)
        };
        self.cursor + (delta - self.offset) * self.signed_scale()
    }
    pub fn fractional_y(self, row: f64) -> f64 {
        self.cursor + (row - self.anchor as f64 - self.offset) * self.signed_scale()
    }
    pub fn row_at(self, y: f64) -> f64 {
        self.anchor as f64 + self.offset + (y - self.cursor) / self.signed_scale()
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
        let a = (self.top - self.cursor) / self.signed_scale() + self.offset;
        let b = (self.bottom - self.cursor) / self.signed_scale() + self.offset;
        let shift = |delta: f64| {
            if delta < 0.0 {
                self.anchor.saturating_sub((-delta) as u64)
            } else {
                self.anchor.saturating_add(delta as u64)
            }
        };
        (shift(a.min(b).floor()), shift(a.max(b).ceil()))
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

/// Linear chart-time axis. Audio/sample projection uses this directly so a
/// pause retains its screen height instead of collapsing through a row inverse.
#[derive(Clone, Copy, Debug)]
pub struct TimeProjection {
    time: f64,
    cursor: f64,
    scale: f64,
    top: f64,
    bottom: f64,
}
impl TimeProjection {
    pub fn new(
        time: f64,
        cursor: f64,
        pixels_per_second: f64,
        top: f64,
        bottom: f64,
        reverse: bool,
    ) -> Result<Self, Error> {
        if ![time, cursor, pixels_per_second, top, bottom]
            .iter()
            .all(|v| v.is_finite())
            || pixels_per_second <= 0.0
            || top > cursor
            || cursor > bottom
        {
            return Err(Error::InvalidEdit("Invalid time projection".into()));
        }
        Ok(Self {
            time,
            cursor,
            scale: pixels_per_second * if reverse { -1.0 } else { 1.0 },
            top,
            bottom,
        })
    }
    pub fn y(self, time: f64) -> Result<f64, Error> {
        self.finite(self.cursor + (time - self.time) * self.scale)
    }
    pub fn time_at(self, y: f64) -> Result<f64, Error> {
        self.finite(self.time + (y - self.cursor) / self.scale)
    }
    pub fn clipped_y(self, time: f64) -> Result<f64, Error> {
        Ok(self.y(time)?.clamp(self.top, self.bottom))
    }
    fn finite(self, value: f64) -> Result<f64, Error> {
        if value.is_finite() {
            Ok(value)
        } else {
            Err(Error::InvalidEdit("Nonfinite time projection".into()))
        }
    }
}
