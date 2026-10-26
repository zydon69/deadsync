//! Combo multipliers: preserve signed source fields, validate new values like GrooveAuthor.
use super::{Scope, Target, tempo::beat_row};
use crate::{Error, document::Format, history::Session};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Multipliers {
    pub hit: i32,
    pub miss: i32,
}
impl Default for Multipliers {
    fn default() -> Self {
        Self { hit: 1, miss: 1 }
    }
}
impl Multipliers {
    pub fn parse(text: &str) -> Result<Self, Error> {
        // .NET's regex $ also accepts one final newline, but no other whitespace.
        let text = text.strip_suffix('\n').unwrap_or(text);
        let (hit, miss) = text
            .split_once("x/")
            .ok_or_else(|| error("Expected hitx/missx combo multipliers"))?;
        let miss = miss
            .strip_suffix('x')
            .ok_or_else(|| error("Expected hitx/missx combo multipliers"))?;
        let number = |part: &str| -> Result<i32, Error> {
            if part.is_empty() || !part.bytes().all(|c| c.is_ascii_digit()) {
                return Err(error("Combo multipliers must be nonnegative integers"));
            }
            part.parse::<i32>()
                .map_err(|_| error("Combo multiplier exceeds the signed integer limit"))
        };
        Ok(Self {
            hit: number(hit)?,
            miss: number(miss)?,
        })
    }
}
impl std::fmt::Display for Multipliers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x/{}x", self.hit, self.miss)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Combos {
    values: BTreeMap<u64, Multipliers>,
}
impl Combos {
    pub fn parse(text: &str) -> Result<Self, Error> {
        let cleaned = text
            .lines()
            .map(|line| line.split_once("//").map_or(line, |(v, _)| v))
            .collect::<Vec<_>>()
            .join("");
        let mut entries = Vec::new();
        for entry in cleaned.split(',') {
            let parts: Vec<_> = entry.split('=').map(str::trim).collect();
            if parts.len() != 2 && parts.len() != 3 {
                continue;
            }
            let (Ok(beat), Ok(hit)) = (parts[0].parse::<f64>(), parts[1].parse::<i32>()) else {
                continue;
            };
            let miss = if parts.len() == 3 {
                let Ok(value) = parts[2].parse::<i32>() else {
                    continue;
                };
                value
            } else {
                hit
            };
            if !beat.is_finite() {
                return Err(error("Nonfinite combo event beat"));
            }
            entries.push((beat, Multipliers { hit, miss }));
        }
        entries.sort_by(|a, b| a.0.partial_cmp(&b.0).expect("Finite combo beats"));
        let mut values = BTreeMap::new();
        for (beat, value) in entries {
            let row = beat_row(beat)?;
            if row >= 0 {
                values.insert(row as u64, value);
            }
        }
        Ok(Self { values })
    }
    pub fn values(&self) -> &BTreeMap<u64, Multipliers> {
        &self.values
    }
    pub fn set(&mut self, row: u64, value: Multipliers) -> Result<bool, Error> {
        valid_row(row)?;
        if value.hit < 0 || value.miss < 0 {
            return Err(error("New combo multipliers must be nonnegative integers"));
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
    pub(super) fn clone_at(&mut self, row: u64, value: Multipliers) -> Result<(), Error> {
        valid_row(row)?;
        self.values.insert(row, value);
        Ok(())
    }
    pub fn move_node(&mut self, from: u64, to: u64) -> Result<bool, Error> {
        valid_row(from)?;
        valid_row(to)?;
        let value = *self
            .values
            .get(&from)
            .ok_or_else(|| error("Combo event does not exist"))?;
        if from == to {
            return Ok(false);
        }
        if self.values.contains_key(&to) {
            return Err(error("Another combo event occupies this row"));
        }
        self.values.remove(&from);
        self.values.insert(to, value);
        Ok(true)
    }
    pub fn serialize(&self) -> String {
        self.values
            .iter()
            .map(|(row, value)| format!("{}={}={}", *row as f64 / 48.0, value.hit, value.miss))
            .collect::<Vec<_>>()
            .join(",")
    }
}
pub struct Draft {
    target: Target,
    original: Combos,
    current: Combos,
}
impl Draft {
    pub fn capture(session: &Session, chart: usize, scope: Scope) -> Result<Self, Error> {
        let target = Target::capture(session, chart, scope)?;
        if session.document().format == Format::Sm {
            return Err(error("Combo multiplier export requires SSC"));
        }
        let current = Combos::parse(
            &session
                .document()
                .text(target.origin(), "COMBOS")
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
    pub fn combos(&self) -> &Combos {
        &self.current
    }
    pub fn has_changes(&self) -> bool {
        self.current != self.original
    }
    pub fn set(&mut self, row: u64, value: Multipliers) -> Result<bool, Error> {
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
            vec![("COMBOS", self.current.serialize())]
        } else {
            vec![]
        };
        self.target
            .apply(session, chart, &changes, "Edit combo multipliers")
    }
}
fn valid_row(row: u64) -> Result<(), Error> {
    if row <= i32::MAX as u64 {
        Ok(())
    } else {
        Err(error("Combo row exceeds reference limits"))
    }
}
fn error(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
