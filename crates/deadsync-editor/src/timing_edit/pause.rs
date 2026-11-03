//! Stops and delays share positions, but retain distinct validation and ordering.
use super::{Scope, Target};
use crate::{Error, history::Session};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Stop,
    Delay,
}
impl Kind {
    pub fn tag(self) -> &'static str {
        match self {
            Self::Stop => "STOPS",
            Self::Delay => "DELAYS",
        }
    }
    fn valid(self, seconds: f64) -> bool {
        seconds.is_finite()
            && (seconds as f32).is_finite()
            && match self {
                Self::Stop => seconds != 0.0,
                Self::Delay => seconds >= 0.0,
            }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Pauses {
    kind: Kind,
    values: BTreeMap<u64, f64>,
}
impl Pauses {
    pub fn parse(kind: Kind, text: &str) -> Result<Self, Error> {
        let mut rows = BTreeMap::new();
        // Resolve same-row collisions before discarding invalid lengths, just
        // like Fumen: an invalid later event still masks an earlier valid one.
        for (beat, seconds) in super::tempo::parse_pairs(text)? {
            let row = super::tempo::beat_row(beat)?;
            if row >= 0 {
                rows.insert(row as u64, seconds);
            }
        }
        let mut values = BTreeMap::new();
        for (row, seconds) in rows {
            if (kind == Kind::Stop && crate::number::reference_equal(seconds, 0.0))
                || (kind == Kind::Delay && seconds < 0.0)
            {
                continue;
            }
            if !kind.valid(seconds) {
                return Err(invalid("Pause length is outside the gameplay timing range"));
            }
            values.insert(row, seconds);
        }
        Ok(Self { kind, values })
    }
    pub fn kind(&self) -> Kind {
        self.kind
    }
    pub fn values(&self) -> &BTreeMap<u64, f64> {
        &self.values
    }
    pub fn set(&mut self, row: u64, seconds: f64) -> Result<bool, Error> {
        valid_row(row)?;
        if !self.kind.valid(seconds) {
            return Err(invalid(
                "Stops must be nonzero; delays must be nonnegative; lengths must be finite and representable by gameplay timing",
            ));
        }
        if self
            .values
            .get(&row)
            .is_some_and(|old| crate::number::reference_equal(*old, seconds))
        {
            return Ok(false);
        }
        self.values.insert(row, seconds);
        Ok(true)
    }
    pub fn remove(&mut self, row: u64) -> Result<bool, Error> {
        valid_row(row)?;
        Ok(self.values.remove(&row).is_some())
    }
    pub fn move_node(&mut self, from: u64, to: u64) -> Result<bool, Error> {
        valid_row(from)?;
        valid_row(to)?;
        let seconds = *self
            .values
            .get(&from)
            .ok_or_else(|| invalid("Pause node does not exist"))?;
        if from == to {
            return Ok(false);
        }
        if self.values.contains_key(&to) {
            return Err(invalid(
                "Another pause of this type already occupies the destination row",
            ));
        }
        self.values.remove(&from);
        self.values.insert(to, seconds);
        Ok(true)
    }
    pub fn serialize(&self) -> String {
        self.values
            .iter()
            .map(|(row, seconds)| format!("{}={seconds}", *row as f64 / 48.0))
            .collect::<Vec<_>>()
            .join(",")
    }
}
fn valid_row(row: u64) -> Result<(), Error> {
    if row > i32::MAX as u64 {
        Err(invalid("Pause position exceeds the reference row range"))
    } else {
        Ok(())
    }
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}

/// Both event types are staged together, retaining their before/after-note phase.
/// The untouched type and all other timing tags keep their original raw bytes.
pub struct Draft {
    target: Target,
    original: [Pauses; 2],
    current: [Pauses; 2],
}
impl Draft {
    pub fn capture(session: &Session, chart: usize, scope: Scope) -> Result<Self, Error> {
        let target = Target::capture(session, chart, scope)?;
        let document = session.document();
        let properties = super::resident::Properties::capture(document, target.origin())?;
        let current = [
            Pauses::parse(Kind::Stop, &properties.text("STOPS"))?,
            Pauses::parse(Kind::Delay, &properties.text("DELAYS"))?,
        ];
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
    pub fn pauses(&self, kind: Kind) -> &Pauses {
        &self.current[index(kind)]
    }
    pub fn set(&mut self, kind: Kind, row: u64, seconds: f64) -> Result<bool, Error> {
        self.current[index(kind)].set(row, seconds)
    }
    pub fn remove(&mut self, kind: Kind, row: u64) -> Result<bool, Error> {
        self.current[index(kind)].remove(row)
    }
    pub fn move_node(&mut self, kind: Kind, from: u64, to: u64) -> Result<bool, Error> {
        self.current[index(kind)].move_node(from, to)
    }
    pub fn has_changes(&self) -> bool {
        self.current != self.original
    }
    pub fn apply(&self, session: &mut Session, chart: usize) -> Result<bool, Error> {
        let changes = self
            .current
            .iter()
            .zip(&self.original)
            .filter(|(current, original)| current != original)
            .map(|(current, _)| (current.kind.tag(), current.serialize()))
            .collect::<Vec<_>>();
        self.target
            .apply(session, chart, &changes, "Edit stops and delays")
    }
}
fn index(kind: Kind) -> usize {
    match kind {
        Kind::Stop => 0,
        Kind::Delay => 1,
    }
}
