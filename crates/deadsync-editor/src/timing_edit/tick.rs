//! Hold tick events. Imported signed values are retained; new values must be nonnegative.
use super::{Scope, Target, tempo::beat_row};
use crate::{Error, history::Session};
use std::collections::BTreeMap;
pub const DEFAULT_TICKS: i32 = 4;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TickCounts {
    values: BTreeMap<u64, i32>,
}
impl TickCounts {
    pub fn parse(text: &str) -> Result<Self, Error> {
        let cleaned = text
            .lines()
            .map(|line| line.split_once("//").map_or(line, |(v, _)| v))
            .collect::<Vec<_>>()
            .join("");
        let mut entries = Vec::new();
        for part in cleaned.split(',') {
            let fields: Vec<_> = part.split('=').map(str::trim).collect();
            if fields.len() != 2 {
                continue;
            }
            let (Ok(beat), Ok(value)) = (fields[0].parse::<f64>(), fields[1].parse::<i32>()) else {
                continue;
            };
            if !beat.is_finite() {
                return Err(error("Nonfinite tick count beat"));
            }
            entries.push((beat, value));
        }
        entries.sort_by(|a, b| a.0.partial_cmp(&b.0).expect("Finite tick beats"));
        let mut values = BTreeMap::new();
        for (beat, value) in entries {
            let row = beat_row(beat)?;
            if row >= 0 {
                values.insert(row as u64, value);
            }
        }
        Ok(Self { values })
    }
    pub fn values(&self) -> &BTreeMap<u64, i32> {
        &self.values
    }
    pub fn set(&mut self, row: u64, value: i32) -> Result<bool, Error> {
        valid_row(row)?;
        if value < 0 {
            return Err(error("New tick counts must be nonnegative integers"));
        }
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
        valid_row(to)?;
        let value = *self
            .values
            .get(&from)
            .ok_or_else(|| error("Tick count node does not exist"))?;
        if from == to {
            return Ok(false);
        }
        if self.values.contains_key(&to) {
            return Err(error("Another tick count occupies this row"));
        }
        self.values.remove(&from);
        self.values.insert(to, value);
        Ok(true)
    }
    pub fn serialize(&self) -> String {
        self.values
            .iter()
            .map(|(row, value)| format!("{}={value}", *row as f64 / 48.0))
            .collect::<Vec<_>>()
            .join(",")
    }
}
pub struct Draft {
    target: Target,
    original: TickCounts,
    current: TickCounts,
}
impl Draft {
    pub fn capture(session: &Session, chart: usize, scope: Scope) -> Result<Self, Error> {
        let target = Target::capture(session, chart, scope)?;
        let current = TickCounts::parse(
            &session
                .document()
                .text(target.origin(), "TICKCOUNTS")
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
    pub fn counts(&self) -> &TickCounts {
        &self.current
    }
    pub fn has_changes(&self) -> bool {
        self.current != self.original
    }
    pub fn set(&mut self, row: u64, value: i32) -> Result<bool, Error> {
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
            vec![("TICKCOUNTS", self.current.serialize())]
        } else {
            vec![]
        };
        self.target
            .apply(session, chart, &changes, "Edit hold tick counts")
    }
}
fn valid_row(row: u64) -> Result<(), Error> {
    if row <= i32::MAX as u64 {
        Ok(())
    } else {
        Err(error("Tick count row exceeds the reference limit"))
    }
}
fn error(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
