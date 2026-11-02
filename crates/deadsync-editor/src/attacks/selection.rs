//! LastSelectedAttackEvent follows event lifetime, independently of Undo state.
use crate::{generation::metadata::Guid, timing_edit::Scope};
use std::collections::BTreeMap;
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Selected {
    pub scope: Scope,
    pub row: u64,
}
pub(crate) type Memory = BTreeMap<Guid, Selected>;
/// Origin relationships recorded by the resident inspector. Missing origins are
/// genuine deletion/replacement; a retained origin at another row is a move.
#[derive(Clone, Debug)]
pub(crate) struct Lifecycle {
    pub chart: usize,
    pub identity: Guid,
    pub scope: Scope,
    pub forward: BTreeMap<u64, Option<u64>>,
    pub backward: BTreeMap<u64, Option<u64>>,
}
impl Lifecycle {
    pub fn from_sources(
        chart: usize,
        identity: Guid,
        scope: Scope,
        original: &BTreeMap<usize, u64>,
        current: &BTreeMap<usize, u64>,
        rows: impl Iterator<Item = u64>,
    ) -> Self {
        let mut forward: BTreeMap<_, _> = original.values().map(|&row| (row, None)).collect();
        let mut backward: BTreeMap<_, _> = rows.map(|row| (row, None)).collect();
        for (source, &old) in original {
            if let Some(&new) = current.get(source) {
                forward.insert(old, Some(new));
                backward.insert(new, Some(old));
            }
        }
        Self {
            chart,
            identity,
            scope,
            forward,
            backward,
        }
    }
    pub fn has_changes(&self) -> bool {
        self.forward
            .iter()
            .chain(&self.backward)
            .any(|(&row, &other)| other != Some(row))
    }
}
