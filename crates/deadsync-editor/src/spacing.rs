//! Per-rate numerical spacing from GrooveAuthor EventSpacingHelper (MIT).
//! Rate-to-chart conversions are explicit callbacks; callers must supply the
//! actual timeline semantics for stops, delays and warps, not linear estimates.
use crate::{Error, zoom::SpacingMode};

#[derive(Clone, Copy, Debug)]
pub struct Rate {
    pub row: f64,
    pub time: f64,
    pub tempo: f64,
    pub seconds_per_row: f64,
    pub rows_per_second: f64,
    pub scroll: f64,
}
#[derive(Clone, Copy, Debug)]
pub struct Speeds {
    pub time: f64,
    pub row: f64,
    pub variable: f64,
}
impl Default for Speeds {
    fn default() -> Self {
        Self {
            time: 300.0,
            row: 2.0,
            variable: 300.0,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Anchor {
    pub y: f64,
    pub time: f64,
    pub row: f64,
}
#[derive(Clone, Copy, Debug)]
pub struct Region {
    pub time: f64,
    pub row: f64,
    pub time_duration: f64,
    pub row_duration: f64,
    pub selection: bool,
    pub drawn_y: f64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Helper {
    mode: SpacingMode,
    direction: f64,
    pps: f64,
    ppr: f64,
    previous_pps: f64,
    previous_ppr: f64,
    rate_time: f64,
    rate_row: f64,
    ready: bool,
}
impl Helper {
    pub fn new(mode: SpacingMode, reverse: bool) -> Self {
        Self {
            mode,
            direction: if reverse { -1.0 } else { 1.0 },
            pps: 1.0,
            ppr: 1.0,
            previous_pps: 1.0,
            previous_ppr: 1.0,
            rate_time: 0.0,
            rate_row: 0.0,
            ready: false,
        }
    }
    pub fn pixels_per_second(self) -> f64 {
        self.pps
    }
    pub fn pixels_per_row(self) -> f64 {
        self.ppr
    }
    pub fn negative_scroll(self) -> bool {
        self.pps < 0.0 || self.ppr < 0.0
    }
    pub fn update(
        &mut self,
        rate: Rate,
        speeds: Speeds,
        interpolated_scroll: f64,
        zoom: f64,
    ) -> Result<(), Error> {
        for value in [
            rate.row,
            rate.time,
            rate.tempo,
            rate.seconds_per_row,
            rate.rows_per_second,
            rate.scroll,
            speeds.time,
            speeds.row,
            speeds.variable,
            interpolated_scroll,
            zoom,
        ] {
            finite(value)?;
        }
        if [
            rate.tempo,
            rate.seconds_per_row,
            rate.rows_per_second,
            speeds.time,
            speeds.row,
            speeds.variable,
            zoom,
        ]
        .iter()
        .any(|v| *v <= 0.0)
        {
            return invalid("Spacing rates and speeds must be positive");
        }
        let (pps, ppr) = match self.mode {
            SpacingMode::ConstantTime => {
                let pps = speeds.time * zoom;
                (pps, pps * rate.seconds_per_row)
            }
            SpacingMode::ConstantRow => {
                let ppr = speeds.row * zoom;
                (ppr * rate.rows_per_second, ppr)
            }
            SpacingMode::Variable => {
                let scroll = rate.scroll * interpolated_scroll;
                let pps = speeds.variable * (rate.tempo / 300.0) * scroll * zoom;
                (pps, pps * rate.seconds_per_row)
            }
        };
        finite(pps)?;
        finite(ppr)?;
        self.previous_pps = self.pps;
        self.previous_ppr = self.ppr;
        self.pps = pps;
        self.ppr = ppr;
        self.rate_time = rate.time;
        self.rate_row = rate.row;
        self.ready = true;
        Ok(())
    }
    pub fn relative_y(self, time: f64, row: f64, anchor_y: f64) -> Result<f64, Error> {
        finite(time)?;
        finite(row)?;
        finite(anchor_y)?;
        finite(
            anchor_y
                + if self.mode == SpacingMode::ConstantTime {
                    time * self.pps * self.direction
                } else {
                    row * self.ppr * self.direction
                },
        )
    }
    pub fn y(self, time: f64, row: f64, anchor: Anchor) -> Result<f64, Error> {
        finite(time)?;
        finite(row)?;
        finite(anchor.time)?;
        finite(anchor.row)?;
        self.relative_y(time - anchor.time, row - anchor.row, anchor.y)
    }
    pub fn y_from_rate(self, time: f64, row: f64, rate_y: f64) -> Result<f64, Error> {
        self.require_rate()?;
        self.y(
            time,
            row,
            Anchor {
                y: rate_y,
                time: self.rate_time,
                row: self.rate_row,
            },
        )
    }
    pub fn row_y(
        self,
        row: f64,
        rate_y: f64,
        row_to_time: impl FnOnce(f64) -> Result<f64, Error>,
    ) -> Result<f64, Error> {
        self.require_rate()?;
        if self.mode == SpacingMode::ConstantTime {
            self.y_from_rate(row_to_time(row)?, row, rate_y)
        } else {
            self.relative_y(0.0, row - self.rate_row, rate_y)
        }
    }
    pub fn location(
        self,
        y: f64,
        anchor: Anchor,
        previous: bool,
        time_to_row: impl FnOnce(f64) -> Result<f64, Error>,
        row_to_time: impl FnOnce(f64) -> Result<f64, Error>,
    ) -> Result<(f64, f64), Error> {
        self.require_rate()?;
        for value in [y, anchor.y, anchor.time, anchor.row] {
            finite(value)?;
        }
        let time_mode = self.mode == SpacingMode::ConstantTime;
        let speed = match (time_mode, previous) {
            (true, false) => self.pps,
            (true, true) => self.previous_pps,
            (false, false) => self.ppr,
            (false, true) => self.previous_ppr,
        } * self.direction;
        if speed == 0.0 {
            return invalid("Zero spacing rate has no unique inverse");
        }
        let (time, row) = if time_mode {
            let time = finite(anchor.time + (y - anchor.y) / speed)?;
            (time, time_to_row(time)?)
        } else {
            let row = finite(anchor.row + (y - anchor.y) / speed)?;
            (row_to_time(row)?, row)
        };
        finite(time)?;
        finite(row)?;
        Ok((time, row))
    }
    pub fn region_ends_before(self, region: Region, time: f64, row: f64) -> Result<bool, Error> {
        validate_region(region)?;
        finite(time)?;
        finite(row)?;
        Ok(if self.mode == SpacingMode::ConstantTime {
            time > finite(region.time + region.time_duration)?
        } else {
            row > finite(region.row + region.row_duration)?
        })
    }
    pub fn skip_region(self, region: Region) -> Result<bool, Error> {
        validate_region(region)?;
        Ok(!region.selection
            && if self.mode == SpacingMode::ConstantTime {
                region.time_duration <= 0.0
            } else {
                region.row_duration <= 0.0
            })
    }
    pub fn region_y(self, region: Region, rate_y: f64) -> Result<f64, Error> {
        self.require_rate()?;
        validate_region(region)?;
        self.relative_y(
            (region.time - self.rate_time).max(0.0),
            (region.row - self.rate_row).max(0.0),
            rate_y,
        )
    }
    /// Signed height is intentional. Reverse/negative scroll consumers must
    /// normalize the resulting rectangle when drawing, as in the original.
    pub fn region_height(self, region: Region, rate_y: f64) -> Result<f64, Error> {
        self.require_rate()?;
        validate_region(region)?;
        let time = finite(region.time + region.time_duration - self.rate_time)?.max(0.0);
        let row = finite(region.row + region.row_duration - self.rate_row)?.max(0.0);
        finite(self.relative_y(time, row, rate_y)? - region.drawn_y)
    }
    fn require_rate(self) -> Result<(), Error> {
        if self.ready {
            Ok(())
        } else {
            invalid("Spacing rate has not been initialized")
        }
    }
}
fn validate_region(region: Region) -> Result<(), Error> {
    for v in [
        region.time,
        region.row,
        region.time_duration,
        region.row_duration,
        region.drawn_y,
    ] {
        finite(v)?;
    }
    Ok(())
}
fn finite(value: f64) -> Result<f64, Error> {
    if value.is_finite() {
        Ok(value)
    } else {
        invalid("Nonfinite spacing value")
    }
}
fn invalid<T>(message: &str) -> Result<T, Error> {
    Err(Error::InvalidEdit(message.into()))
}
