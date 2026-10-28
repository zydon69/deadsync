//! Reviewed whole-chart or exact-head note conversions. Existing row-range
//! commands remain compatible; all entry points share one note conversion rule.
use crate::{
    Error,
    history::{Session, file_intent::IntentGuard},
    note_swap::Pair,
    notes::{Kind, Note, Position},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::{AtomicBool, Ordering},
};
#[derive(Clone, Copy, Debug)]
pub enum Operation {
    Convert {
        from: Kind,
        to: Kind,
        sustain_rows: u64,
    },
    Swap(Pair),
}
#[derive(Clone, Debug)]
pub enum Scope {
    All {
        player: Option<u32>,
    },
    Range {
        start: u64,
        end: u64,
        player: Option<u32>,
    },
    Heads(BTreeSet<Position>),
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub selected: usize,
    pub matched: usize,
    pub changed: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    ZeroDurationSustain,
    Stale,
    Invalid,
}
#[derive(Debug)]
pub struct Failure {
    kind: Refusal,
    error: Error,
}
impl Failure {
    pub fn kind(&self) -> Refusal {
        self.kind
    }
    fn new(kind: Refusal, message: &str) -> Self {
        Self {
            kind,
            error: invalid(message),
        }
    }
}
impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        Self {
            kind: Refusal::Invalid,
            error,
        }
    }
}
impl From<Failure> for Error {
    fn from(value: Failure) -> Self {
        value.error
    }
}
pub struct Plan {
    guard: IntentGuard,
    chart: usize,
    identity: crate::generation::metadata::Guid,
    report: Report,
    edits: BTreeMap<Position, Option<Note>>,
}
impl Plan {
    pub fn prepare(
        session: &mut Session,
        chart: usize,
        scope: Scope,
        operation: Operation,
    ) -> Result<Self, Failure> {
        Self::prepare_bounded(
            session,
            chart,
            scope,
            operation,
            2_000_000,
            &AtomicBool::new(false),
        )
    }
    /// Bounded sparse-head scan. Grid cloning/loading remains synchronous; this
    /// is cooperative cancellation at scan/validation boundaries, not a peak
    /// memory limit or an interruptible native Grid allocator.
    pub fn prepare_bounded(
        session: &mut Session,
        chart: usize,
        scope: Scope,
        operation: Operation,
        maximum_notes: usize,
        cancelled: &AtomicBool,
    ) -> Result<Self, Failure> {
        check(cancelled)?;
        if let Scope::Range { start, end, .. } = &scope {
            if start > end || *end > i32::MAX as u64 {
                return Err(invalid("Invalid note operation range").into());
            }
        }
        let identity = session
            .chart_ids()
            .get(chart)
            .cloned()
            .ok_or_else(|| invalid("No chart for note operation"))?;
        let guard = IntentGuard::capture(session);
        let grid = session.grid(chart)?;
        let stepf2 = grid.uses_stepf2();
        let player = match &scope {
            Scope::All { player } | Scope::Range { player, .. } => *player,
            Scope::Heads(_) => None,
        };
        if player.is_some_and(|p| p >= grid.players()) {
            return Err(invalid("Selected note player does not exist").into());
        }
        let mut report = Report::default();
        let mut edits = BTreeMap::new();
        for (index, (position, note)) in grid.iter().enumerate() {
            if index >= maximum_notes {
                return Err(invalid("Note operation scan limit exceeded").into());
            }
            if index % 1024 == 0 {
                check(cancelled)?;
            }
            let selected = match &scope {
                Scope::All { player } => player.is_none_or(|p| p == position.player),
                Scope::Range { start, end, player } => {
                    (*start..=*end).contains(&position.row)
                        && player.is_none_or(|p| p == position.player)
                }
                Scope::Heads(heads) => heads.contains(&position),
            };
            if !selected {
                continue;
            }
            report.selected += 1;
            let (to, duration) = match operation {
                Operation::Convert {
                    from,
                    to,
                    sustain_rows,
                } if note.kind == from => (to, sustain_rows),
                Operation::Convert { .. } => continue,
                Operation::Swap(pair) => {
                    let (a, b) = pair.kinds();
                    if note.kind == a {
                        (b, 0)
                    } else if note.kind == b {
                        (a, 0)
                    } else {
                        continue;
                    }
                }
            };
            report.matched += 1;
            if matches!(to, Kind::Hold | Kind::Roll) && note.end.is_none() && duration == 0 {
                return Err(Failure::new(
                    Refusal::ZeroDurationSustain,
                    "Operation would create a zero-duration hold or roll; choose conversion with an explicit sustain duration",
                ));
            }
            let converted = convert_note(note, position.row, to, duration, stepf2)?;
            if &converted != note {
                report.changed += 1;
                edits.insert(position, Some(converted));
            }
        }
        // Validate sustain bounds/collisions using the established sparse Grid
        // mutation contract before exposing any review or touching live history.
        check(cancelled)?;
        let mut candidate = grid.clone();
        check(cancelled)?;
        for position in edits.keys() {
            candidate.set(*position, None)?;
        }
        for (index, (position, note)) in edits.iter().enumerate() {
            if index % 1024 == 0 {
                check(cancelled)?;
            }
            if !stepf2 {
                if let Some(note) = note {
                    crate::stepf2::require_standard(note)?;
                }
            }
            candidate.set(*position, note.clone())?;
        }
        check(cancelled)?;
        Ok(Self {
            guard,
            chart,
            identity,
            report,
            edits,
        })
    }
    pub fn chart(&self) -> usize {
        self.chart
    }
    pub fn report(&self) -> Report {
        self.report
    }
    pub fn is_current(&self, session: &Session, chart: usize) -> bool {
        chart == self.chart
            && session.chart_ids().get(chart) == Some(&self.identity)
            && self.guard.is_current(session)
    }
    pub fn apply(&self, session: &mut Session, chart: usize) -> Result<bool, Failure> {
        if !self.is_current(session, chart) {
            return Err(Failure::new(
                Refusal::Stale,
                "Note operation review belongs to another chart, session or history state",
            ));
        }
        Ok(session.edit_notes(chart, "Convert or swap reviewed notes", self.edits.clone())?)
    }
}
/// Single reference rule used by both existing Transform::Convert and reviewed
/// exact-head operations. Attributes and positive existing sustain ends survive.
pub(crate) fn convert_note(
    note: &Note,
    row: u64,
    to: Kind,
    sustain_rows: u64,
    stepf2: bool,
) -> Result<Note, Error> {
    if matches!(to, Kind::Keysound | Kind::Attack) {
        return Err(Error::Unsupported(
            "Conversion to an attributed note needs explicit attributes".into(),
        ));
    }
    let mut note = note.clone();
    if stepf2 {
        crate::stepf2::change_kind(&mut note, to)?;
    } else {
        note.kind = to;
    }
    note.end = if matches!(to, Kind::Hold | Kind::Roll) {
        Some(match note.end {
            Some(end) => end,
            None => row
                .checked_add(sustain_rows)
                .ok_or_else(|| invalid("Sustain exceeds row range"))?,
        })
    } else {
        note.tail_attributes = "".into();
        None
    };
    Ok(note)
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}

fn check(cancelled: &AtomicBool) -> Result<(), Error> {
    if cancelled.load(Ordering::Relaxed) {
        Err(invalid("Note operation preparation cancelled"))
    } else {
        Ok(())
    }
}
