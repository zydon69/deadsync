//! Presentation adapter for typed timing transactions. Region lengths are shown
//! in beats; their authoritative storage and edits remain integer rows.
use super::Workspace;
use deadsync_editor::{
    Error,
    timing_edit::{
        Scope, TempoDraft,
        combo::{self, Multipliers},
        label,
        pause::{self, Kind as PauseKind},
        region::{self, Kind as RegionKind},
        scroll,
        signature::{self, Signature},
        speed, tick,
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
    Label,
    Speed,
    Combo,
    Tick,
    Scroll,
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
            Self::Scroll => "TimingTypeScroll",
            Self::Tick => "TimingTypeTick",
            Self::Combo => "TimingTypeCombo",
            Self::Speed => "TimingTypeSpeed",
            Self::Label => "TimingTypeLabel",
        }
    }
    pub(super) fn step(self, backward: bool) -> Self {
        let kinds = [
            Self::Bpm,
            Self::Stop,
            Self::Delay,
            Self::Warp,
            Self::Fake,
            Self::Label,
            Self::Speed,
            Self::Combo,
            Self::Tick,
            Self::Scroll,
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
    Scrolls(scroll::Draft),
    Ticks(tick::Draft),
    Combos(combo::Draft),
    Speeds(speed::Draft),
    Labels(label::Draft),
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
            EventKind::Label => {
                label::Draft::capture(&workspace.file().session, workspace.chart(), scope)
                    .map(Self::Labels)
            }
            EventKind::Speed => {
                speed::Draft::capture(&workspace.file().session, workspace.chart(), scope)
                    .map(Self::Speeds)
            }
            EventKind::Combo => {
                combo::Draft::capture(&workspace.file().session, workspace.chart(), scope)
                    .map(Self::Combos)
            }
            EventKind::Tick => {
                tick::Draft::capture(&workspace.file().session, workspace.chart(), scope)
                    .map(Self::Ticks)
            }
            EventKind::Scroll => {
                scroll::Draft::capture(&workspace.file().session, workspace.chart(), scope)
                    .map(Self::Scrolls)
            }
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
            Self::Scrolls(_) => EventKind::Scroll,
            Self::Ticks(_) => EventKind::Tick,
            Self::Combos(_) => EventKind::Combo,
            Self::Speeds(_) => EventKind::Speed,
            Self::Labels(_) => EventKind::Label,
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
            Self::Labels(d) => d.scope(),
            Self::Speeds(d) => d.scope(),
            Self::Combos(d) => d.scope(),
            Self::Ticks(d) => d.scope(),
            Self::Scrolls(d) => d.scope(),
            Self::Signatures(d) => d.scope(),
            Self::Pauses { draft, .. } => draft.scope(),
            Self::Regions { draft, .. } => draft.scope(),
        }
    }
    pub(super) fn inherited(&self) -> bool {
        match self {
            Self::Bpm(d) => d.inherited(),
            Self::Labels(d) => d.inherited(),
            Self::Speeds(d) => d.inherited(),
            Self::Combos(d) => d.inherited(),
            Self::Ticks(d) => d.inherited(),
            Self::Scrolls(d) => d.inherited(),
            Self::Signatures(d) => d.inherited(),
            Self::Pauses { draft, .. } => draft.inherited(),
            Self::Regions { draft, .. } => draft.inherited(),
        }
    }
    pub(super) fn affected_charts(&self) -> &[usize] {
        match self {
            Self::Bpm(d) => d.affected_charts(),
            Self::Labels(d) => d.affected_charts(),
            Self::Speeds(d) => d.affected_charts(),
            Self::Combos(d) => d.affected_charts(),
            Self::Ticks(d) => d.affected_charts(),
            Self::Scrolls(d) => d.affected_charts(),
            Self::Signatures(d) => d.affected_charts(),
            Self::Pauses { draft, .. } => draft.affected_charts(),
            Self::Regions { draft, .. } => draft.affected_charts(),
        }
    }
    pub(super) fn has_changes(&self) -> bool {
        match self {
            Self::Bpm(d) => d.has_changes(),
            Self::Labels(d) => d.has_changes(),
            Self::Speeds(d) => d.has_changes(),
            Self::Combos(d) => d.has_changes(),
            Self::Ticks(d) => d.has_changes(),
            Self::Scrolls(d) => d.has_changes(),
            Self::Signatures(d) => d.has_changes(),
            Self::Pauses { draft, .. } => draft.has_changes(),
            Self::Regions { draft, .. } => draft.has_changes(),
        }
    }
    pub(super) fn node(&self, row: u64) -> Option<String> {
        match self {
            Self::Signatures(d) => d.signatures().values().get(&row).map(ToString::to_string),
            Self::Labels(d) => d.labels().values().get(&row).cloned(),
            Self::Speeds(d) => d.speeds().values().get(&row).map(ToString::to_string),
            Self::Combos(d) => d.combos().values().get(&row).map(ToString::to_string),
            Self::Scrolls(d) => d.scrolls().values().get(&row).map(ToString::to_string),
            Self::Bpm(d) => d.tempos().values().get(&row).map(ToString::to_string),
            Self::Ticks(d) => d.counts().values().get(&row).map(ToString::to_string),
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
            Self::Labels(d) => d
                .labels()
                .values()
                .get(&row)
                .cloned()
                .unwrap_or_else(|| crate::i18n::tr("Edition", "LabelDefault").to_string()),
            Self::Speeds(d) => d
                .speeds()
                .values()
                .get(&row)
                .copied()
                .unwrap_or_default()
                .to_string(),
            Self::Combos(d) => d
                .combos()
                .values()
                .get(&row)
                .copied()
                .unwrap_or_default()
                .to_string(),
            Self::Ticks(d) => d
                .counts()
                .values()
                .get(&row)
                .copied()
                .unwrap_or(tick::DEFAULT_TICKS)
                .to_string(),
            Self::Scrolls(d) => d.scrolls().at(row).to_string(),
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
            Self::Labels(d) => neighbor(d.labels().values(), row, forward),
            Self::Speeds(d) => neighbor(d.speeds().values(), row, forward),
            Self::Combos(d) => neighbor(d.combos().values(), row, forward),
            Self::Ticks(d) => neighbor(d.counts().values(), row, forward),
            Self::Scrolls(d) => neighbor(d.scrolls().values(), row, forward),
            Self::Pauses { draft, kind } => neighbor(draft.pauses(*kind).values(), row, forward),
            Self::Regions { draft, kind } => neighbor(draft.regions(*kind).values(), row, forward),
        }?;
        Some((row, self.value(row)))
    }
    pub(super) fn count(&self) -> usize {
        match self {
            Self::Bpm(d) => d.tempos().values().len(),
            Self::Labels(d) => d.labels().values().len(),
            Self::Speeds(d) => d.speeds().values().len(),
            Self::Combos(d) => d.combos().values().len(),
            Self::Ticks(d) => d.counts().values().len(),
            Self::Scrolls(d) => d.scrolls().values().len(),
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
            Self::Labels(d) => d.set(row, text),
            Self::Speeds(d) => d.set(row, speed::Speed::parse(text)?),
            Self::Combos(d) => d.set(row, Multipliers::parse(text)?),
            Self::Ticks(d) => d.set(
                row,
                text.parse::<i32>()
                    .map_err(|_| Error::InvalidEdit("Invalid integer tick count".into()))?,
            ),
            Self::Scrolls(d) => d.set(row, number()?),
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
            Self::Labels(d) => d.remove(row),
            Self::Speeds(d) => d.remove(row),
            Self::Combos(d) => d.remove(row),
            Self::Ticks(d) => d.remove(row),
            Self::Scrolls(d) => d.remove(row),
            Self::Signatures(d) => d.remove(row),
            Self::Pauses { draft, kind } => draft.remove(*kind, row),
            Self::Regions { draft, kind } => draft.remove(*kind, row),
        }
    }
    pub(super) fn move_node(&mut self, from: u64, to: u64) -> Result<bool, Error> {
        match self {
            Self::Bpm(d) => d.move_node(from, to),
            Self::Labels(d) => d.move_node(from, to),
            Self::Speeds(d) => d.move_node(from, to),
            Self::Combos(d) => d.move_node(from, to),
            Self::Ticks(d) => d.move_node(from, to),
            Self::Scrolls(d) => d.move_node(from, to),
            Self::Signatures(d) => d.move_node(from, to),
            Self::Pauses { draft, kind } => draft.move_node(*kind, from, to),
            Self::Regions { draft, kind } => draft.move_node(*kind, from, to),
        }
    }
    pub(super) fn apply(&self, workspace: &mut Workspace) -> Result<bool, Error> {
        match self {
            Self::Bpm(d) => workspace.apply_tempo(d),
            Self::Labels(d) => workspace.apply_labels(d),
            Self::Speeds(d) => workspace.apply_speeds(d),
            Self::Combos(d) => workspace.apply_combos(d),
            Self::Ticks(d) => workspace.apply_ticks(d),
            Self::Scrolls(d) => workspace.apply_scrolls(d),
            Self::Signatures(d) => workspace.apply_signatures(d),
            Self::Pauses { draft, .. } => workspace.apply_pauses(draft),
            Self::Regions { draft, .. } => workspace.apply_regions(draft),
        }
    }
    pub(super) fn valid_value_at(&self, row: u64, text: &str) -> bool {
        if self.kind() == EventKind::Label {
            return !label::sanitize(text).contains('=');
        }
        if self.kind() == EventKind::Speed {
            return speed::Speed::parse(text).is_ok_and(|value| value.fits_at(row));
        }
        if self.kind() == EventKind::Combo {
            return Multipliers::parse(text).is_ok();
        }
        if self.kind() == EventKind::Tick {
            return text.parse::<i32>().is_ok_and(|value| value >= 0);
        }
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
                EventKind::Scroll => true,
                EventKind::Delay => value >= 0.0,
                _ => region_length(value).is_some_and(|length| fits(row, length)),
            }
    }
    pub(super) fn valid_destination(&self, from: u64, to: u64) -> bool {
        match self {
            Self::Speeds(d) => d
                .speeds()
                .values()
                .get(&from)
                .is_some_and(|value| value.fits_at(to)),
            Self::Regions { draft, kind } => draft
                .regions(*kind)
                .values()
                .get(&from)
                .is_some_and(|length| fits(to, *length)),
            _ => true,
        }
    }
    pub(super) fn same_value(&self, row: u64, text: &str) -> bool {
        if let Self::Labels(d) = self {
            return d
                .labels()
                .values()
                .get(&row)
                .is_some_and(|value| value == text);
        }
        if let Self::Speeds(d) = self {
            return d.speeds().values().get(&row).is_some_and(|value| {
                text == value.to_string() || speed::Speed::parse(text).ok() == Some(*value)
            });
        }
        if let Self::Combos(d) = self {
            return d.combos().values().get(&row).is_some_and(|value| {
                text == value.to_string() || Multipliers::parse(text).ok() == Some(*value)
            });
        }
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
        if self.kind() == EventKind::Label {
            return match key {
                "TimingBpm" => "LabelValue",
                "TimingInvalidBpm" => "LabelInvalid",
                "TimingSet" => "LabelSet",
                "TimingRemove" => "LabelRemove",
                "TimingMove" => "LabelMove",
                "TimingPrevious" => "LabelPrevious",
                "TimingNext" => "LabelNext",
                "TimingAccept" => "LabelAccept",
                "TimingStaged" => "LabelStaged",
                "TimingRemoved" => "LabelRemoved",
                "TimingMissing" => "LabelMissing",
                "TimingCollision" => "LabelCollision",
                "TimingMoveValue" => "LabelMoveValue",
                "TimingMoved" => "LabelMoved",
                "TimingInputPending" => "LabelInputPending",
                _ => key,
            };
        }
        if self.kind() == EventKind::Speed {
            return match key {
                "TimingBpm" => "SpeedValue",
                "TimingInvalidBpm" => "SpeedInvalid",
                "TimingSet" => "SpeedSet",
                "TimingRemove" => "SpeedRemove",
                "TimingMove" => "SpeedMove",
                "TimingPrevious" => "SpeedPrevious",
                "TimingNext" => "SpeedNext",
                "TimingAccept" => "SpeedAccept",
                "TimingStaged" => "SpeedStaged",
                "TimingRemoved" => "SpeedRemoved",
                "TimingMissing" => "SpeedMissing",
                "TimingCollision" => "SpeedCollision",
                "TimingMoveValue" => "SpeedMoveValue",
                "TimingMoved" => "SpeedMoved",
                "TimingInputPending" => "SpeedInputPending",
                _ => key,
            };
        }
        if self.kind() == EventKind::Combo {
            return match key {
                "TimingBpm" => "ComboValue",
                "TimingInvalidBpm" => "ComboInvalid",
                "TimingSet" => "ComboSet",
                "TimingRemove" => "ComboRemove",
                "TimingMove" => "ComboMove",
                "TimingPrevious" => "ComboPrevious",
                "TimingNext" => "ComboNext",
                "TimingAccept" => "ComboAccept",
                "TimingStaged" => "ComboStaged",
                "TimingRemoved" => "ComboRemoved",
                "TimingMissing" => "ComboMissing",
                "TimingCollision" => "ComboCollision",
                "TimingMoveValue" => "ComboMoveValue",
                "TimingMoved" => "ComboMoved",
                "TimingInputPending" => "ComboInputPending",
                _ => key,
            };
        }
        if self.kind() == EventKind::Tick {
            return match key {
                "TimingBpm" => "TickValue",
                "TimingInvalidBpm" => "TickInvalid",
                "TimingSet" => "TickSet",
                "TimingRemove" => "TickRemove",
                "TimingMove" => "TickMove",
                "TimingPrevious" => "TickPrevious",
                "TimingNext" => "TickNext",
                "TimingAccept" => "TickAccept",
                "TimingStaged" => "TickStaged",
                "TimingRemoved" => "TickRemoved",
                "TimingMissing" => "TickMissing",
                "TimingCollision" => "TickCollision",
                "TimingMoveValue" => "TickMoveValue",
                "TimingMoved" => "TickMoved",
                "TimingInputPending" => "TickInputPending",
                _ => key,
            };
        }
        if self.kind() == EventKind::Scroll {
            return match key {
                "TimingBpm" => "ScrollRate",
                "TimingInvalidBpm" => "ScrollInvalid",
                "TimingSet" => "ScrollSet",
                "TimingRemove" => "ScrollRemove",
                "TimingMove" => "ScrollMove",
                "TimingPrevious" => "ScrollPrevious",
                "TimingNext" => "ScrollNext",
                "TimingAccept" => "ScrollAccept",
                "TimingStaged" => "ScrollStaged",
                "TimingRemoved" => "ScrollRemoved",
                "TimingMissing" => "ScrollMissing",
                "TimingCollision" => "ScrollCollision",
                "TimingMoveValue" => "ScrollMoveValue",
                "TimingMoved" => "ScrollMoved",
                "TimingInputPending" => "ScrollInputPending",
                _ => key,
            };
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
