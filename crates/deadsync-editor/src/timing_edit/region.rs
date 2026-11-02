//! Warp and fake regions use integer row lengths, including constructor clamping.
use super::{
    Scope, Target,
    tempo::{beat_row, parse_pairs},
};
use crate::{Error, history::Session};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Warp,
    Fake,
}
impl Kind {
    pub fn tag(self) -> &'static str {
        match self {
            Self::Warp => "WARPS",
            Self::Fake => "FAKES",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Regions {
    kind: Kind,
    values: BTreeMap<u64, u64>,
}
impl Regions {
    pub fn parse(kind: Kind, text: &str) -> Result<Self, Error> {
        let mut rows = BTreeMap::new();
        for (beat, length) in parse_pairs(text)? {
            let row = beat_row(beat)?;
            if row >= 0 {
                rows.insert(row as u64, length);
            }
        }
        let mut result = Self {
            kind,
            values: BTreeMap::new(),
        };
        for (row, length) in rows {
            if length <= 0.0 {
                continue;
            }
            // Fumen rounds lengths like absolute beats; EditorWarpEvent and
            // EditorFakeSegmentEvent constructors then clamp positive zero-row
            // lengths to one row. This is independent of position rounding.
            let length = beat_row(length)?.max(1) as u64;
            result.set(row, length)?;
        }
        Ok(result)
    }
    pub fn kind(&self) -> Kind {
        self.kind
    }
    pub fn values(&self) -> &BTreeMap<u64, u64> {
        &self.values
    }
    pub fn set(&mut self, row: u64, length: u64) -> Result<bool, Error> {
        valid_region(row, length)?;
        if self.values.get(&row) == Some(&length) {
            return Ok(false);
        }
        self.values.insert(row, length);
        Ok(true)
    }
    pub fn remove(&mut self, row: u64) -> Result<bool, Error> {
        valid_row(row)?;
        Ok(self.values.remove(&row).is_some())
    }
    pub fn move_node(&mut self, from: u64, to: u64) -> Result<bool, Error> {
        valid_row(from)?;
        valid_row(to)?;
        let length = *self
            .values
            .get(&from)
            .ok_or_else(|| invalid("Region does not exist"))?;
        valid_region(to, length)?;
        if from == to {
            return Ok(false);
        }
        if self.values.contains_key(&to) {
            return Err(invalid(
                "Another region of this type starts at the destination row",
            ));
        }
        self.values.remove(&from);
        self.values.insert(to, length);
        Ok(true)
    }
    pub fn serialize(&self) -> String {
        self.values
            .iter()
            .map(|(row, length)| format!("{}={}", *row as f64 / 48.0, *length as f64 / 48.0))
            .collect::<Vec<_>>()
            .join(",")
    }
}
fn valid_row(row: u64) -> Result<(), Error> {
    if row > i32::MAX as u64 {
        Err(invalid("Region position exceeds the supported row range"))
    } else {
        Ok(())
    }
}
fn valid_region(row: u64, length: u64) -> Result<(), Error> {
    valid_row(row)?;
    if length == 0
        || length > i32::MAX as u64
        || row
            .checked_add(length)
            .is_none_or(|end| end > i32::MAX as u64)
    {
        return Err(invalid(
            "Region length must be at least one row and its end must fit in the supported row range",
        ));
    }
    Ok(())
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
/// A single transaction can change both region types; the other timing tags
/// are copied losslessly when a previously inherited chart becomes independent.
pub struct Draft {
    target: Target,
    original: [Regions; 2],
    current: [Regions; 2],
}
impl Draft {
    pub fn capture(session: &Session, chart: usize, scope: Scope) -> Result<Self, Error> {
        let target = Target::capture(session, chart, scope)?;
        let document = session.document();
        let properties = super::resident::Properties::capture(document,target.origin())?;

        let current = [
            Regions::parse(
                Kind::Warp,
                &properties.text("WARPS"),
            )?,
            Regions::parse(
                Kind::Fake,
                &properties.text("FAKES"),
            )?,
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
    pub fn regions(&self, kind: Kind) -> &Regions {
        &self.current[index(kind)]
    }
    pub fn set(&mut self, kind: Kind, row: u64, length: u64) -> Result<bool, Error> {
        self.current[index(kind)].set(row, length)
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
            .apply(session, chart, &changes, "Edit warps and fake regions")
    }
}
fn index(kind: Kind) -> usize {
    match kind {
        Kind::Warp => 0,
        Kind::Fake => 1,
    }
}
