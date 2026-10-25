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
