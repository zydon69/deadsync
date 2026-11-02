//! Time signatures retain their written ratio (2/2 and 4/4 are distinct).
use super::{Scope, Target, tempo::beat_row};
use crate::{Error, history::Session};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Signature {
    pub numerator: u32,
    pub denominator: u32,
}
impl Signature {
    /// GrooveAuthor's interactive validator, distinct from Fumen's file reader.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let parts: Vec<_> = text.split('/').collect();
        if parts.len() != 2 {
            return invalid("Expected numerator/denominator");
        }
        let numerator = parts[0]
            .trim()
            .parse::<i32>()
            .map_err(|_| error("Invalid time signature numerator"))?;
        let denominator = parts[1]
            .trim()
            .parse::<i32>()
            .map_err(|_| error("Invalid time signature denominator"))?;
        if numerator < 1
            || denominator < 1
            || denominator > 48
            || !(denominator as u32).is_power_of_two()
        {
            return invalid(
                "Time signature values must be positive; denominator must be a power of two no greater than 48",
            );
        }
        Ok(Self {
            numerator: numerator as u32,
            denominator: denominator as u32,
        })
    }
    fn rows(self) -> Result<u64, Error> {
        if self.numerator == 0 || self.denominator == 0 || 192 % self.denominator != 0 {
            return invalid("Time signature cannot be represented by StepMania rows");
        }
        let product = 192u64 * u64::from(self.numerator);
        // The reference uses signed int multiplication before dividing. Refuse
        // overflow rather than exporting values that corrupt measure arithmetic.
        if product > i32::MAX as u64 {
            return invalid("Time signature exceeds the reference measure limit");
        }
        Ok(product / u64::from(self.denominator))
    }
}
impl std::fmt::Display for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.numerator, self.denominator)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeasurePosition {
    pub measure: u64,
    pub row_in_measure: u64,
    pub rows_per_measure: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Signatures {
    values: BTreeMap<u64, Signature>,
    measures: Vec<(u64, u64, u64)>,
}
impl Signatures {
    pub fn from_document(
        document: &crate::document::Document,
        chart: usize,
    ) -> Result<Self, Error> {
        if chart >= document.charts().len() {
            return invalid("Time signature chart does not exist");
        }
        let scope = super::reference_uses_chart_timing(document, chart).then_some(chart);
        let scope=if document.format==crate::document::Format::Sm {super::uses_chart_timing(document,chart).then_some(chart)} else {scope};
        Self::parse(&super::resident::text(document,scope,"TIMESIGNATURES")?)
    }
    pub fn parse(text: &str) -> Result<Self, Error> {
        let mut source = Vec::new();
        let cleaned = text
            .lines()
            .map(|line| line.split_once("//").map_or(line, |(value, _)| value))
            .collect::<Vec<_>>()
            .join("");
        for entry in cleaned.split(',') {
            if entry.trim().is_empty() {
                continue;
            }
            let parts: Vec<_> = entry.split('=').map(str::trim).collect();
            if parts.len() != 3 {
                continue;
            }
            let (Ok(beat), Ok(n), Ok(d)) = (
                parts[0].parse::<f64>(),
                parts[1].parse::<i32>(),
                parts[2].parse::<i32>(),
            ) else {
                continue;
            };
            if !beat.is_finite() {
                return invalid("Nonfinite time signature beat");
            }
            source.push((beat, n, d));
        }
        source.sort_by(|a, b| a.0.partial_cmp(&b.0).expect("Finite signature beats"));
        let mut rows = BTreeMap::new();
        for (beat, n, d) in source {
            let row = beat_row(beat)?;
            if row >= 0 {
                rows.insert(row as u64, (n, d));
            }
        }
        let mut values = BTreeMap::new();
        for (row, (n, d)) in rows {
            // Resolve same-row collisions before filtering invalid values.
            if n > 0 && d > 0 && 192 % d == 0 {
                values.insert(
                    row,
                    Signature {
                        numerator: n as u32,
                        denominator: d as u32,
                    },
                );
            }
        }
        values.entry(0).or_insert(Signature {
            numerator: 4,
            denominator: 4,
        });
        Self::from_values(values)
    }
    fn from_values(values: BTreeMap<u64, Signature>) -> Result<Self, Error> {
        let mut measures = Vec::with_capacity(values.len());
        let (mut previous, mut measure, mut length) = (0, 0, 192);
        for (&row, &signature) in &values {
            valid_row(row)?;
            measure += (row - previous).div_ceil(length);
            length = signature.rows()?;
            measures.push((row, measure, length));
            previous = row;
        }
        Ok(Self { values, measures })
    }
    pub fn values(&self) -> &BTreeMap<u64, Signature> {
        &self.values
    }
    pub fn at(&self, row: u64) -> Signature {
        *self
            .values
            .range(..=row)
            .next_back()
            .expect("Initial signature")
            .1
    }
    /// Destination one measure away using the signature at the departure row,
    /// including a negative destination before row zero (as in GrooveAuthor).
    pub fn measure_destination(&self, row: u64, previous: bool) -> Result<i64, Error> {
        valid_row(row)?;
        let length = self.at(row).rows()? as i64;
        Ok(row as i64 + if previous { -length } else { length })
    }
    pub fn measure_position(&self, row: u64) -> Result<MeasurePosition, Error> {
        valid_row(row)?;
        let index = self.measures.partition_point(|&(start, _, _)| start <= row) - 1;
        let (start, measure, length) = self.measures[index];
        Ok(MeasurePosition {
            measure: measure + (row - start) / length,
            row_in_measure: (row - start) % length,
            rows_per_measure: length,
        })
    }
    /// Visible measure boundaries, including a new signature in mid-measure.
    pub fn measure_starts(
        &self,
        start: u64,
        end: u64,
    ) -> Result<impl Iterator<Item = (u64, u64)> + '_, Error> {
        valid_row(start)?;
        valid_row(end)?;
        if start > end {
            return invalid("Invalid measure range");
        }
        let index = self.measures.partition_point(|&(row, _, _)| row <= start) - 1;
        Ok(self.measures[index..]
            .iter()
            .enumerate()
            .take_while(move |(_, (row, _, _))| *row <= end)
            .flat_map(move |(offset, &(row, measure, length))| {
                let limit = self
                    .measures
                    .get(index + offset + 1)
                    .map_or(end + 1, |&(next, _, _)| next.min(end + 1));
                let first = row + start.saturating_sub(row).div_ceil(length) * length;
                (first..limit)
                    .step_by(length as usize)
                    .map(move |current| (current, measure + (current - row) / length))
            }))
    }
    pub fn set(&mut self, row: u64, signature: Signature) -> Result<bool, Error> {
        valid_row(row)?;
        Signature::parse(&signature.to_string())?;
        if self.values.get(&row) == Some(&signature) {
            return Ok(false);
        }
        let mut values = self.values.clone();
        values.insert(row, signature);
        *self = Self::from_values(values)?;
        Ok(true)
    }
    pub fn remove(&mut self, row: u64) -> Result<bool, Error> {
        valid_row(row)?;
        if row == 0 {
            return invalid("The initial time signature cannot be deleted");
        }
        if !self.values.contains_key(&row) {
            return Ok(false);
        }
        let mut values = self.values.clone();
        values.remove(&row);
        *self = Self::from_values(values)?;
        Ok(true)
    }
    pub fn move_node(&mut self, from: u64, to: u64) -> Result<bool, Error> {
        valid_row(from)?;
        valid_row(to)?;
        if from == 0 && to != 0 {
            return invalid("The initial time signature cannot be moved");
        }
        let signature = *self
            .values
            .get(&from)
            .ok_or_else(|| error("Time signature does not exist"))?;
        if from == to {
            return Ok(false);
        }
        if self.values.contains_key(&to) {
            return invalid("Another time signature occupies this row");
        }
        let mut values = self.values.clone();
        values.remove(&from);
        values.insert(to, signature);
        *self = Self::from_values(values)?;
        Ok(true)
    }
    pub fn serialize(&self) -> String {
        self.values
            .iter()
            .map(|(row, s)| format!("{}={}={}", *row as f64 / 48.0, s.numerator, s.denominator))
            .collect::<Vec<_>>()
            .join(",")
    }
}
pub struct Draft {
    target: Target,
    original: Signatures,
    current: Signatures,
}
impl Draft {
    pub fn capture(session: &Session, chart: usize, scope: Scope) -> Result<Self, Error> {
        let target = Target::capture(session, chart, scope)?;
        let current = Signatures::parse(
            &super::resident::text(session.document(),target.origin(),"TIMESIGNATURES")?,
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
    pub fn signatures(&self) -> &Signatures {
        &self.current
    }
    pub fn set(&mut self, row: u64, signature: Signature) -> Result<bool, Error> {
        self.current.set(row, signature)
    }
    pub fn remove(&mut self, row: u64) -> Result<bool, Error> {
        self.current.remove(row)
    }
    pub fn move_node(&mut self, from: u64, to: u64) -> Result<bool, Error> {
        self.current.move_node(from, to)
    }
    pub fn has_changes(&self) -> bool {
        self.current != self.original
    }
    pub fn apply(&self, session: &mut Session, chart: usize) -> Result<bool, Error> {
        let changes = if self.has_changes() {
            vec![("TIMESIGNATURES", self.current.serialize())]
        } else {
            vec![]
        };
        self.target
            .apply(session, chart, &changes, "Edit time signatures")
    }
}
fn valid_row(row: u64) -> Result<(), Error> {
    if row > i32::MAX as u64 {
        invalid("Time signature row exceeds reference limit")
    } else {
        Ok(())
    }
}
fn error(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
fn invalid<T>(message: &str) -> Result<T, Error> {
    Err(error(message))
}
