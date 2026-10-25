//! GrooveAuthor EditorRateAlteringEvent conversions (48e18f71, MIT).
//! A mapping belongs to one rate event. Selecting the appropriate event is
//! a separate timeline operation; this type does not search across sections.
use crate::Error;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RateMapping {
    row: f64,
    time: f64,
    warp: f64,
    stop: f64,
    seconds_per_row: f64,
    rows_per_second: f64,
}
impl RateMapping {
    pub fn new(
        row: f64,
        time: f64,
        warp: f64,
        stop: f64,
        seconds_per_row: f64,
    ) -> Result<Self, Error> {
        for value in [row, time, warp, stop, seconds_per_row] {
            finite(value)?;
        }
        if row < 0.0 || warp < 0.0 || seconds_per_row <= 0.0 {
            return invalid("Invalid timing rate mapping");
        }
        let rows_per_second = finite(1.0 / seconds_per_row)?;
        Ok(Self {
            row,
            time,
            warp,
            stop,
            seconds_per_row,
            rows_per_second,
        })
    }
    pub fn row(self) -> f64 {
        self.row
    }
    pub fn time(self) -> f64 {
        self.time
    }
    pub fn warp(self) -> f64 {
        self.warp
    }
    pub fn stop(self) -> f64 {
        self.stop
    }
    pub fn seconds_per_row(self) -> f64 {
        self.seconds_per_row
    }
    pub fn rows_per_second(self) -> f64 {
        self.rows_per_second
    }
    pub fn time_at(self, position: f64) -> Result<f64, Error> {
        finite(position)?;
        finite(rate_time(
            self.row,
            self.time,
            self.warp,
            self.stop,
            self.seconds_per_row,
            position,
        ))
    }
    pub fn position_at(self, time: f64) -> Result<f64, Error> {
        finite(time)?;
        finite(rate_position(
            self.row,
            self.time,
            self.warp,
            self.stop,
            self.rows_per_second,
            time,
        ))
    }
}
// Shared by already-validated timeline rates and public validated snapshots.
pub(crate) fn rate_time(
    row: f64,
    time: f64,
    warp: f64,
    stop: f64,
    seconds_per_row: f64,
    position: f64,
) -> f64 {
    let relative = position - (row + warp);
    let relative = if position >= 0.0 {
        relative.max(0.0)
    } else {
        relative
    };
    let seconds = relative * seconds_per_row + stop;
    let seconds = if position >= 0.0 {
        seconds.max(0.0)
    } else {
        seconds
    };
    time + seconds
}
pub(crate) fn rate_position(
    row: f64,
    time: f64,
    warp: f64,
    stop: f64,
    rows_per_second: f64,
    position_time: f64,
) -> f64 {
    let relative = position_time - (time + stop);
    let relative = if position_time >= 0.0 {
        relative.max(0.0)
    } else {
        relative
    };
    row + relative * rows_per_second + warp
}
fn finite(value: f64) -> Result<f64, Error> {
    if value.is_finite() {
        Ok(value)
    } else {
        invalid("Nonfinite timing rate mapping")
    }
}
fn invalid<T>(message: &str) -> Result<T, Error> {
    Err(Error::InvalidEdit(message.into()))
}
