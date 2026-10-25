//! Instant scroll rates, including zero and reverse scroll. Initial nodes are mutable.
use super::{
    Scope, Target,
    tempo::{beat_row, parse_pairs},
};
use crate::{Error, document::Format, history::Session};
use std::collections::BTreeMap;
#[derive(Clone, Debug, PartialEq)]
pub struct Scrolls {
    values: BTreeMap<u64, f64>,
}
impl Scrolls {
    pub fn parse(text: &str) -> Result<Self, Error> {
        let mut values = BTreeMap::new();
        for (beat, rate) in parse_pairs(text)? {
            let row = beat_row(beat)?;
            if row >= 0 {
                values.insert(row as u64, rate);
            }
        }
        for &rate in values.values() {
            valid_rate(rate)?;
        }
        Ok(Self { values })
    }
    pub fn values(&self) -> &BTreeMap<u64, f64> {
        &self.values
    }
    /// The editor propagates its first scroll rate backward to preceding events.
    pub fn at(&self, row: u64) -> f64 {
        self.values
            .range(..=row)
            .next_back()
            .or_else(|| self.values.first_key_value())
            .map_or(1.0, |(_, rate)| *rate)
    }
    pub fn set(&mut self, row: u64, rate: f64) -> Result<bool, Error> {
        valid_row(row)?;
        valid_rate(rate)?;
        if self
            .values
            .get(&row)
            .is_some_and(|old| crate::number::reference_equal(*old, rate))
        {
            return Ok(false);
        }
        self.values.insert(row, rate);
        Ok(true)
    }
    pub fn remove(&mut self, row: u64) -> Result<bool, Error> {
        valid_row(row)?;
        Ok(self.values.remove(&row).is_some())
    }
    pub fn move_node(&mut self, from: u64, to: u64) -> Result<bool, Error> {
        valid_row(from)?;
        valid_row(to)?;
        let rate = *self
            .values
            .get(&from)
            .ok_or_else(|| error("Scroll node does not exist"))?;
        if from == to {
            return Ok(false);
        }
        if self.values.contains_key(&to) {
            return Err(error("Another scroll node occupies this row"));
        }
        self.values.remove(&from);
        self.values.insert(to, rate);
        Ok(true)
    }
    pub fn serialize(&self) -> String {
        self.values
            .iter()
            .map(|(row, rate)| format!("{}={rate}", *row as f64 / 48.0))
            .collect::<Vec<_>>()
            .join(",")
    }
}
pub struct Draft {
    target: Target,
    original: Scrolls,
    current: Scrolls,
}
impl Draft {
    pub fn capture(session: &Session, chart: usize, scope: Scope) -> Result<Self, Error> {
        let target = Target::capture(session, chart, scope)?;
        if session.document().format == Format::Sm {
            return Err(error(
                "Scroll export requires SSC; convert the file before editing scroll events",
            ));
        }
        let current = Scrolls::parse(
            &session
                .document()
                .text(target.origin(), "SCROLLS")
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
    pub fn scrolls(&self) -> &Scrolls {
        &self.current
    }
    pub fn has_changes(&self) -> bool {
        self.current != self.original
    }
    pub fn set(&mut self, row: u64, rate: f64) -> Result<bool, Error> {
        self.current.set(row, rate)
    }
    pub fn remove(&mut self, row: u64) -> Result<bool, Error> {
        self.current.remove(row)
    }
    pub fn move_node(&mut self, from: u64, to: u64) -> Result<bool, Error> {
        self.current.move_node(from, to)
    }
    pub fn apply(&self, session: &mut Session, chart: usize) -> Result<bool, Error> {
        let changes = if self.has_changes() {
            vec![("SCROLLS", self.current.serialize())]
        } else {
            vec![]
        };
        self.target
            .apply(session, chart, &changes, "Edit scroll rates")
    }
}
fn valid_rate(value: f64) -> Result<(), Error> {
    if value.is_finite() && (value as f32).is_finite() {
        Ok(())
    } else {
        Err(error(
            "Scroll rate must be finite and representable by gameplay",
        ))
    }
}
fn valid_row(row: u64) -> Result<(), Error> {
    if row <= i32::MAX as u64 {
        Ok(())
    } else {
        Err(error("Scroll row exceeds the reference limit"))
    }
}
fn error(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
