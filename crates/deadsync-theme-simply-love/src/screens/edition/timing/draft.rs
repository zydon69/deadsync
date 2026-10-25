//! Presentation adapter for typed timing transactions. Region lengths are shown
//! in beats; their authoritative storage and edits remain integer rows.
use super::Workspace;
use deadsync_editor::{
    Error,
    timing_edit::{
        Scope, TempoDraft,
        pause::{self, Kind as PauseKind},
        region::{self, Kind as RegionKind},
        signature::{self, Signature},
    },
};
use std::collections::BTreeMap;
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum EventKind {
    Bpm,
    Stop,
    Delay,
    Warp,
    Fake,
    Signature,
}
impl EventKind {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Bpm => "TimingTypeBpm",
            Self::Stop => "TimingTypeStop",
            Self::Delay => "TimingTypeDelay",
            Self::Warp => "TimingTypeWarp",
            Self::Fake => "TimingTypeFake",
            Self::Signature => "TimingTypeSignature",
        }
    }
    pub(super) fn step(self, backward: bool) -> Self {
        let kinds = [
            Self::Bpm,
            Self::Stop,
            Self::Delay,
            Self::Warp,
            Self::Fake,
            Self::Signature,
        ];
        let index = kinds
            .iter()
            .position(|kind| *kind == self)
            .expect("Timing type");
        kinds[(index + if backward { kinds.len() - 1 } else { 1 }) % kinds.len()]
    }
    pub(super) fn is_region(self) -> bool {
        matches!(self, Self::Warp | Self::Fake)
    }
}
pub(super) enum Draft {
    Bpm(TempoDraft),
    Signatures(signature::Draft),
    Pauses {
        draft: pause::Draft,
        kind: PauseKind,
    },
    Regions {
        draft: region::Draft,
        kind: RegionKind,
    },
}
impl Draft {
    pub(super) fn capture(
        workspace: &Workspace,
        scope: Scope,
        kind: EventKind,
    ) -> Result<Self, Error> {
        match kind {
            EventKind::Bpm => workspace.tempo_draft(scope).map(Self::Bpm),
            EventKind::Signature => {
                signature::Draft::capture(&workspace.file().session, workspace.chart(), scope)
                    .map(Self::Signatures)
            }
            EventKind::Stop | EventKind::Delay => {
                workspace.pause_draft(scope).map(|draft| Self::Pauses {
                    draft,
                    kind: if kind == EventKind::Stop {
                        PauseKind::Stop
                    } else {
                        PauseKind::Delay
                    },
                })
            }
            EventKind::Warp | EventKind::Fake => {
                workspace.region_draft(scope).map(|draft| Self::Regions {
                    draft,
                    kind: if kind == EventKind::Warp {
                        RegionKind::Warp
                    } else {
                        RegionKind::Fake
                    },
                })
            }
        }
    }
    pub(super) fn kind(&self) -> EventKind {
        match self {
            Self::Bpm(_) => EventKind::Bpm,
            Self::Signatures(_) => EventKind::Signature,
            Self::Pauses {
                kind: PauseKind::Stop,
                ..
            } => EventKind::Stop,
            Self::Pauses { .. } => EventKind::Delay,
            Self::Regions {
                kind: RegionKind::Warp,
                ..
            } => EventKind::Warp,
            Self::Regions { .. } => EventKind::Fake,
        }
    }
    /// Switching paired types preserves the entire combined draft.
    pub(super) fn select_type(&mut self, next: EventKind) -> bool {
        match self {
            Self::Pauses { kind, .. } if matches!(next, EventKind::Stop | EventKind::Delay) => {
                *kind = if next == EventKind::Stop {
                    PauseKind::Stop
                } else {
                    PauseKind::Delay
                };
                true
            }
            Self::Regions { kind, .. } if next.is_region() => {
                *kind = if next == EventKind::Warp {
                    RegionKind::Warp
                } else {
                    RegionKind::Fake
                };
                true
            }
            _ => false,
        }
    }
    pub(super) fn scope(&self) -> Scope {
        match self {
            Self::Bpm(d) => d.scope(),
            Self::Signatures(d) => d.scope(),
            Self::Pauses { draft, .. } => draft.scope(),
            Self::Regions { draft, .. } => draft.scope(),
        }
    }
    pub(super) fn inherited(&self) -> bool {
        match self {
            Self::Bpm(d) => d.inherited(),
            Self::Signatures(d) => d.inherited(),
            Self::Pauses { draft, .. } => draft.inherited(),
            Self::Regions { draft, .. } => draft.inherited(),
        }
    }
    pub(super) fn affected_charts(&self) -> &[usize] {
        match self {
            Self::Bpm(d) => d.affected_charts(),
            Self::Signatures(d) => d.affected_charts(),
            Self::Pauses { draft, .. } => draft.affected_charts(),
            Self::Regions { draft, .. } => draft.affected_charts(),
        }
    }
    pub(super) fn has_changes(&self) -> bool {
        match self {
            Self::Bpm(d) => d.has_changes(),
            Self::Signatures(d) => d.has_changes(),
            Self::Pauses { draft, .. } => draft.has_changes(),
            Self::Regions { draft, .. } => draft.has_changes(),
        }
    }
    pub(super) fn node(&self, row: u64) -> Option<String> {
        match self {
            Self::Signatures(d) => d.signatures().values().get(&row).map(ToString::to_string),
            Self::Bpm(d) => d.tempos().values().get(&row).map(ToString::to_string),
            Self::Pauses { draft, kind } => draft
                .pauses(*kind)
                .values()
                .get(&row)
                .map(ToString::to_string),
            Self::Regions { draft, kind } => draft
                .regions(*kind)
                .values()
                .get(&row)
                .map(|length| (*length as f64 / 48.0).to_string()),
        }
    }
    pub(super) fn value(&self, row: u64) -> String {
        match self {
            Self::Bpm(d) => d.tempos().at(row).to_string(),
            Self::Signatures(d) => d.signatures().at(row).to_string(),
            _ => self
                .node(row)
                .unwrap_or_else(|| if self.kind().is_region() { "1" } else { "0.5" }.to_owned()),
        }
    }
    pub(super) fn neighbor(&self, row: u64, forward: bool) -> Option<(u64, String)> {
        let row = match self {
            Self::Signatures(d) => neighbor(d.signatures().values(), row, forward),
            Self::Bpm(d) => neighbor(d.tempos().values(), row, forward),
            Self::Pauses { draft, kind } => neighbor(draft.pauses(*kind).values(), row, forward),
            Self::Regions { draft, kind } => neighbor(draft.regions(*kind).values(), row, forward),
        }?;
        Some((row, self.value(row)))
    }
    pub(super) fn count(&self) -> usize {
        match self {
            Self::Bpm(d) => d.tempos().values().len(),
            Self::Signatures(d) => d.signatures().values().len(),
            Self::Pauses { draft, .. } => {
                draft.pauses(PauseKind::Stop).values().len()
                    + draft.pauses(PauseKind::Delay).values().len()
            }
            Self::Regions { draft, .. } => {
                draft.regions(RegionKind::Warp).values().len()
                    + draft.regions(RegionKind::Fake).values().len()
            }
        }
    }
    pub(super) fn set(&mut self, row: u64, text: &str) -> Result<bool, Error> {
        let number = || {
            text.parse::<f64>()
                .map_err(|_| Error::InvalidEdit("Invalid timing value".into()))
        };
        match self {
            Self::Signatures(d) => d.set(row, Signature::parse(text)?),
            Self::Bpm(d) => d.set(row, number()?),
            Self::Pauses { draft, kind } => draft.set(*kind, row, number()?),
            Self::Regions { draft, kind } => draft.set(
                *kind,
                row,
                region_length(number()?)
                    .ok_or_else(|| Error::InvalidEdit("Invalid region length".into()))?,
            ),
        }
    }
    pub(super) fn remove(&mut self, row: u64) -> Result<bool, Error> {
        match self {
            Self::Bpm(d) => d.remove(row),
            Self::Signatures(d) => d.remove(row),
            Self::Pauses { draft, kind } => draft.remove(*kind, row),
            Self::Regions { draft, kind } => draft.remove(*kind, row),
        }
    }
    pub(super) fn move_node(&mut self, from: u64, to: u64) -> Result<bool, Error> {
        match self {
            Self::Bpm(d) => d.move_node(from, to),
            Self::Signatures(d) => d.move_node(from, to),
            Self::Pauses { draft, kind } => draft.move_node(*kind, from, to),
            Self::Regions { draft, kind } => draft.move_node(*kind, from, to),
        }
    }
    pub(super) fn apply(&self, workspace: &mut Workspace) -> Result<bool, Error> {
        match self {
            Self::Bpm(d) => workspace.apply_tempo(d),
            Self::Signatures(d) => workspace.apply_signatures(d),
            Self::Pauses { draft, .. } => workspace.apply_pauses(draft),
            Self::Regions { draft, .. } => workspace.apply_regions(draft),
        }
    }
    pub(super) fn valid_value_at(&self, row: u64, text: &str) -> bool {
        if self.kind() == EventKind::Signature {
            return Signature::parse(text).is_ok();
        }
        let Ok(value) = text.parse::<f64>() else {
            return false;
        };
        value.is_finite()
            && (value as f32).is_finite()
            && match self.kind() {
                EventKind::Bpm => value >= deadsync_editor::timing_edit::tempo::MIN_BPM,
                EventKind::Stop => value != 0.0,
                EventKind::Delay => value >= 0.0,
                _ => region_length(value).is_some_and(|length| fits(row, length)),
            }
    }
    pub(super) fn valid_destination(&self, from: u64, to: u64) -> bool {
        match self {
            Self::Regions { draft, kind } => draft
                .regions(*kind)
                .values()
                .get(&from)
                .is_some_and(|length| fits(to, *length)),
            _ => true,
        }
    }
    pub(super) fn same_value(&self, row: u64, text: &str) -> bool {
        if let Self::Signatures(d) = self {
            return d.signatures().values().get(&row).is_some_and(|signature| {
                text == signature.to_string() || Signature::parse(text).ok() == Some(*signature)
            });
        }
        let Ok(value) = text.parse::<f64>() else {
            return false;
        };
        match self {
            Self::Regions { draft, kind } => {
                region_length(value) == draft.regions(*kind).values().get(&row).copied()
            }
            _ => self.value(row).parse::<f64>().ok() == Some(value),
        }
    }
    pub(super) fn key<'a>(&self, key: &'a str) -> &'a str {
        if self.kind() == EventKind::Bpm {
            return key;
        }
        if self.kind() == EventKind::Signature {
            return match key {
                "TimingBpm" => "SignatureValue",
                "TimingInvalidBpm" => "SignatureInvalid",
                "TimingSet" => "SignatureSet",
                "TimingRemove" => "SignatureRemove",
                "TimingMove" => "SignatureMove",
                "TimingPrevious" => "SignaturePrevious",
                "TimingNext" => "SignatureNext",
                "TimingAccept" => "SignatureAccept",
                "TimingStaged" => "SignatureStaged",
                "TimingRemoved" => "SignatureRemoved",
                "TimingMissing" => "SignatureMissing",
                "TimingCollision" => "SignatureCollision",
                "TimingMoveValue" => "SignatureMoveValue",
                "TimingMoved" => "SignatureMoved",
                "TimingInputPending" => "SignatureInputPending",
                _ => key,
            };
        }
        if self.kind().is_region() {
            return match key {
                "TimingBpm" => "RegionLength",
                "TimingSet" => "RegionSet",
                "TimingRemove" => "RegionRemove",
                "TimingMove" => "RegionMove",
                "TimingPrevious" => "RegionPrevious",
                "TimingNext" => "RegionNext",
                "TimingAccept" => "RegionAccept",
                "TimingStaged" => "RegionStaged",
                "TimingRemoved" => "RegionRemoved",
                "TimingMissing" => "RegionMissing",
                "TimingCollision" => "RegionCollision",
                "TimingMoveValue" => "RegionMoveValue",
                "TimingMoved" => "RegionMoved",
                "TimingInputPending" => "RegionInputPending",
                "TimingInvalidBpm" => "RegionInvalidLength",
                _ => key,
            };
        }
        match key {
            "TimingBpm" => "PauseSeconds",
            "TimingSet" => "PauseSet",
            "TimingRemove" => "PauseRemove",
            "TimingMove" => "PauseMove",
            "TimingPrevious" => "PausePrevious",
            "TimingNext" => "PauseNext",
            "TimingAccept" => "PauseAccept",
            "TimingStaged" => "PauseStaged",
            "TimingRemoved" => "PauseRemoved",
            "TimingMissing" => "PauseMissing",
            "TimingCollision" => "PauseCollision",
            "TimingMoveValue" => "PauseMoveValue",
            "TimingMoved" => "PauseMoved",
            "TimingInputPending" => "PauseInputPending",
            "TimingInvalidBpm" => {
                if self.kind() == EventKind::Stop {
                    "PauseInvalidStop"
                } else {
                    "PauseInvalidDelay"
                }
            }
            _ => key,
        }
    }
}
fn region_length(value: f64) -> Option<u64> {
    let rows = (value * 48.0).round();
    (value.is_finite() && value > 0.0 && rows >= 1.0 && rows <= i32::MAX as f64)
        .then_some(rows as u64)
}
fn fits(row: u64, length: u64) -> bool {
    row.checked_add(length)
        .is_some_and(|end| end <= i32::MAX as u64)
}
fn neighbor<T>(values: &BTreeMap<u64, T>, row: u64, forward: bool) -> Option<u64> {
    let entry = if forward {
        values
            .range((std::ops::Bound::Excluded(row), std::ops::Bound::Unbounded))
            .next()
            .or_else(|| values.first_key_value())
    } else {
        values
            .range(..row)
            .next_back()
            .or_else(|| values.last_key_value())
    };
    entry.map(|(row, _)| *row)
}
