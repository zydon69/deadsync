//! Interpolated scroll rates, distinct from instant SCROLLS and beat/time clocks.
use super::{Scope, Target, tempo::beat_row};
use crate::{Error, document::Format, history::Session};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Period {
    Rows(i32),
    Seconds(f64),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Speed {
    pub rate: f64,
    pub period: Period,
}
impl Default for Speed {
    fn default() -> Self {
        Self {
            rate: 1.0,
            period: Period::Rows(0),
        }
    }
}
impl Speed {
    pub fn fits_at(self, row: u64) -> bool {
        self.validate(row, false).is_ok()
    }
    pub fn parse(text: &str) -> Result<Self, Error> {
        let text = text.strip_suffix('\n').unwrap_or(text);
        let (rate, period) = text
            .split_once("x/")
            .ok_or_else(|| error("Expected ratex/lengthrows or ratex/seconds s"))?;
        let decimal = |text: &str| -> Result<f64, Error> {
            let mut points = 0;
            let mut digits = 0;
            for c in text.bytes() {
                if c == b'.' {
                    points += 1;
                } else if c.is_ascii_digit() {
                    digits += 1;
                } else {
                    return Err(error("Speed input requires unsigned decimal numbers"));
                }
            }
            if points > 1 || digits == 0 {
                return Err(error("Invalid decimal speed value"));
            }
            let value = text
                .parse::<f64>()
                .map_err(|_| error("Invalid decimal speed value"))?;
            finite(value)?;
            Ok(value)
        };
        let rate = decimal(rate)?;
        let period = if let Some(rows) = period.strip_suffix("rows") {
            decimal(rows)?;
            Period::Rows(
                rows.parse::<i32>()
                    .map_err(|_| error("Speed row duration must be a nonnegative integer"))?,
            )
        } else if let Some(seconds) = period.strip_suffix('s') {
            Period::Seconds(decimal(seconds)?)
        } else {
            return Err(error("Speed duration must use rows or s"));
        };
        let value = Self { rate, period };
        value.validate(0, true)?;
        Ok(value)
    }
    fn validate(self, row: u64, new: bool) -> Result<(), Error> {
        valid_row(row)?;
        finite(self.rate)?;
        if new && self.rate < 0.0 {
            return Err(error("New speed targets must be nonnegative"));
        }
        match self.period {
            Period::Rows(length) => {
                if new && length < 0 {
                    return Err(error("New speed row lengths must be nonnegative"));
                }
                if i32::try_from(row as i64 + i64::from(length)).is_err() {
                    return Err(error("Speed end row exceeds the reference integer limit"));
                }
            }
            Period::Seconds(length) => {
                finite(length)?;
                if new && length < 0.0 {
                    return Err(error("New speed time lengths must be nonnegative"));
                }
            }
        }
        Ok(())
    }
    /// Exact editor interpolation formula, including instant and legacy negative periods.
    pub fn interpolate(
        self,
        previous: f64,
        event_row: u64,
        event_time: f64,
        row: f64,
        time: f64,
    ) -> Result<f64, Error> {
        self.validate(event_row, false)?;
        finite(previous)?;
        if !event_time.is_finite() || !row.is_finite() || !time.is_finite() {
            return Err(error("Nonfinite speed query"));
        }
        let (start, end, current) = match self.period {
            Period::Rows(length) => (event_row as f64, event_row as f64 + f64::from(length), row),
            Period::Seconds(length) => (event_time, event_time + length, time),
        };
        if !end.is_finite() {
            return Err(error("Speed interpolation endpoint overflows"));
        }
        let value = if crate::number::reference_equal(end, start) {
            if current >= end { self.rate } else { previous }
        } else {
            let value = previous + (current - start) / (end - start) * (self.rate - previous);
            value.clamp(previous.min(self.rate), previous.max(self.rate))
        };
        if !value.is_finite() {
            return Err(error("Nonfinite interpolated speed"));
        }
        Ok(value)
    }
}
impl std::fmt::Display for Speed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.period {
            Period::Rows(rows) => write!(f, "{}x/{rows}rows", self.rate),
            Period::Seconds(seconds) => write!(f, "{}x/{seconds}s", self.rate),
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Speeds {
    values: BTreeMap<u64, Speed>,
}
impl Speeds {
    pub fn parse(text: &str) -> Result<Self, Error> {
        let cleaned = text
            .lines()
            .map(|line| line.split_once("//").map_or(line, |(value, _)| value))
            .collect::<Vec<_>>()
            .join("");
        let mut entries = Vec::new();
        for entry in cleaned.split(',') {
            let fields: Vec<_> = entry.split('=').map(str::trim).collect();
            if fields.len() != 4 {
                continue;
            }
            let (Ok(beat), Ok(rate), Ok(length), Ok(mode)) = (
                fields[0].parse::<f64>(),
                fields[1].parse::<f64>(),
                fields[2].parse::<f64>(),
                fields[3].parse::<i32>(),
            ) else {
                continue;
            };
            if !beat.is_finite() {
                return Err(error("Nonfinite speed event beat"));
            }
            entries.push((beat, rate, length, mode));
        }
        entries.sort_by(|a, b| a.0.partial_cmp(&b.0).expect("Finite speed beats"));
        let mut source = BTreeMap::new();
        for (beat, rate, length, mode) in entries {
            let row = beat_row(beat)?;
            if row >= 0 {
                source.insert(row as u64, (rate, length, mode));
            }
        }
        let mut values = BTreeMap::new();
        for (row, (rate, length, mode)) in source {
            let period = if mode != 0 {
                Period::Seconds(length)
            } else {
                Period::Rows(beat_row(length)? as i32)
            };
            let value = Speed { rate, period };
            value.validate(row, false)?;
            values.insert(row, value);
        }
        Ok(Self { values })
    }
    pub fn values(&self) -> &BTreeMap<u64, Speed> {
        &self.values
    }
    pub fn set(&mut self, row: u64, value: Speed) -> Result<bool, Error> {
        value.validate(row, true)?;
        if self.values.get(&row) == Some(&value) {
            return Ok(false);
        }
        self.values.insert(row, value);
        Ok(true)
    }
    pub fn remove(&mut self, row: u64) -> Result<bool, Error> {
        valid_row(row)?;
        Ok(self.values.remove(&row).is_some())
    }
    pub fn move_node(&mut self, from: u64, to: u64) -> Result<bool, Error> {
        valid_row(from)?;
        let value = *self
            .values
            .get(&from)
            .ok_or_else(|| error("Speed event does not exist"))?;
        value.validate(to, false)?;
        if from == to {
            return Ok(false);
        }
        if self.values.contains_key(&to) {
            return Err(error("Another speed event occupies the destination"));
        }
        self.values.remove(&from);
        self.values.insert(to, value);
        Ok(true)
    }
    /// Select by chart row; seconds-based events use the supplied chart-time mapping.
    pub fn rate_at(
        &self,
        row: f64,
        time: f64,
        event_time: impl FnOnce(u64) -> Result<f64, Error>,
    ) -> Result<f64, Error> {
        if !row.is_finite() || row > i32::MAX as f64 || !time.is_finite() {
            return Err(error("Invalid speed position query"));
        }
        let Some((&start, &value)) = self
            .values
            .range(..=row.max(0.0).floor() as u64)
            .next_back()
            .or_else(|| self.values.first_key_value())
        else {
            return Ok(1.0);
        };
        let previous = self
            .values
            .range(..start)
            .next_back()
            .map_or(value.rate, |(_, value)| value.rate);
        let origin = if matches!(value.period, Period::Seconds(_)) {
            event_time(start)?
        } else {
            0.0
        };
        value.interpolate(previous, start, origin, row, time)
    }
    pub fn serialize(&self) -> String {
        self.values
            .iter()
            .map(|(row, value)| {
                let (length, mode) = match value.period {
                    Period::Rows(rows) => (f64::from(rows) / 48.0, 0),
                    Period::Seconds(seconds) => (seconds, 1),
                };
                format!("{}={}={length}={mode}", *row as f64 / 48.0, value.rate)
            })
            .collect::<Vec<_>>()
            .join(",")
    }
}
pub struct Draft {
    target: Target,
    original: Speeds,
    current: Speeds,
}
impl Draft {
    pub fn capture(session: &Session, chart: usize, scope: Scope) -> Result<Self, Error> {
        let target = Target::capture(session, chart, scope)?;
        if session.document().format == Format::Sm {
            return Err(error("Interpolated speed export requires SSC"));
        }
        let current = Speeds::parse(
            &session
                .document()
                .text(target.origin(), "SPEEDS")
                .unwrap_or_default(),
        )?;
        Ok(Self {
            target,
            original: current.clone(),
            current,
        })
    }
    pub fn scope(&self) -> Scope {
        self.target.scope
    }
    pub fn inherited(&self) -> bool {
        self.target.inherited
    }
    pub fn affected_charts(&self) -> &[usize] {
        &self.target.affected
    }
    pub fn speeds(&self) -> &Speeds {
        &self.current
    }
    pub fn has_changes(&self) -> bool {
        self.current != self.original
    }
    pub fn set(&mut self, row: u64, value: Speed) -> Result<bool, Error> {
        self.current.set(row, value)
    }
    pub fn remove(&mut self, row: u64) -> Result<bool, Error> {
        self.current.remove(row)
    }
    pub fn move_node(&mut self, from: u64, to: u64) -> Result<bool, Error> {
        self.current.move_node(from, to)
    }
    pub fn apply(&self, session: &mut Session, chart: usize) -> Result<bool, Error> {
        let changes = if self.has_changes() {
            vec![("SPEEDS", self.current.serialize())]
        } else {
            vec![]
        };
        self.target
            .apply(session, chart, &changes, "Edit interpolated scroll speeds")
    }
}
fn finite(value: f64) -> Result<(), Error> {
    if value.is_finite() && (value as f32).is_finite() {
        Ok(())
    } else {
        Err(error(
            "Speed values must be finite and within the gameplay float range",
        ))
    }
}
fn valid_row(row: u64) -> Result<(), Error> {
    if row <= i32::MAX as u64 {
        Ok(())
    } else {
        Err(error("Speed row exceeds the reference limit"))
    }
}
fn error(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
