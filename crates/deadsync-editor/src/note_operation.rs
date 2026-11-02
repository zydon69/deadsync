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
    transformed: Vec<Position>,
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
        let mut transformed = Vec::new();
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
            transformed.push(position);
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
        transformed.sort_by_key(|position| {
            (
                position.row,
                grid.get(*position)
                    .expect("matched source exists")
                    .kind
                    .editor_phase(),
                position.lane,
                position.player,
            )
        });
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
            transformed,
        })
    }
    /// Checked selections retain the source enumeration, including sparse
    /// selections whose order differs from chart row order.
    pub(crate) fn with_selection_order(mut self, order: &[Position]) -> Self {
        let matched: BTreeSet<_> = self.transformed.iter().copied().collect();
        self.transformed = order
            .iter()
            .copied()
            .filter(|position| matched.contains(position))
            .collect();
        self
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
        self.apply_named(session, chart, "Convert or swap reviewed notes")
    }
    pub(crate) fn apply_named(
        &self,
        session: &mut Session,
        chart: usize,
        label: &str,
    ) -> Result<bool, Failure> {
        if !self.is_current(session, chart) {
            return Err(Failure::new(
                Refusal::Stale,
                "Note operation review belongs to another chart, session or history state",
            ));
        }
        let events = self
            .transformed
            .iter()
            .copied()
            .map(crate::event_clipboard::Event::Note)
            .collect::<Vec<_>>();
        let before = crate::history::SelectionEffect::capture(session, chart, events.clone());
        let changed = session.edit_notes(chart, label, self.edits.clone())?;
        let after = crate::history::SelectionEffect::capture(session, chart, events);
        session.record_event_selection(
            crate::history::selection_effect::SelectionHistory { before, after },
            changed,
        );
        Ok(changed)
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

#[cfg(test)]
mod selection_history_tests {
    use super::*;
    use crate::{
        document::{Document, Format},
        event_clipboard::{Event, Selection, TimingKind},
    };

    fn fixture() -> Session {
        Session::new(Document::parse(b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\nM000\n0000;".to_vec(), Format::Ssc).unwrap())
    }
    fn position(row: u64, lane: u16) -> Position {
        Position {
            row,
            lane,
            player: 0,
        }
    }
    fn operation() -> Operation {
        Operation::Convert {
            from: Kind::Tap,
            to: Kind::Mine,
            sustain_rows: 0,
        }
    }

    #[test]
    fn checked_conversion_keeps_only_transformed_notes_in_source_order_through_history() {
        let mut session = fixture();
        let mut selection = Selection::capture(&mut session, 0, 0, 192).unwrap();
        selection.select_all(false);
        for event in [
            Event::Note(position(48, 1)),
            Event::Timing {
                row: 0,
                kind: TimingKind::Tempo,
            },
            Event::Note(position(96, 0)),
            Event::Note(position(0, 0)),
        ] {
            let index = selection
                .events()
                .iter()
                .position(|value| *value == event)
                .unwrap();
            selection.set_selected(index, true).unwrap();
        }
        let before = session.serialize().unwrap();
        let plan = selection
            .prepare_note_operation(&mut session, operation())
            .unwrap();
        assert!(session.take_event_selection_update().is_none());
        assert_eq!(session.serialize().unwrap(), before);
        assert!(plan.apply(&mut session, 0).unwrap());
        let after = session.serialize().unwrap();
        let expected = vec![Event::Note(position(48, 1)), Event::Note(position(0, 0))];
        let update = session.take_event_selection_update().unwrap();
        assert!(update.is_current(&session));
        assert!(!update.is_current(&fixture()));
        assert_eq!(update.events(), expected);
        session.undo().unwrap();
        assert_eq!(session.serialize().unwrap(), before);
        let update = session.take_event_selection_update().unwrap();
        assert!(update.is_current(&session));
        assert_eq!(update.events(), expected);
        session.redo().unwrap();
        assert_eq!(session.serialize().unwrap(), after);
        assert_eq!(
            session.take_event_selection_update().unwrap().events(),
            expected
        );
        session.set_text(None, "TITLE", "unrelated").unwrap();
        assert!(session.take_event_selection_update().is_none());
        session.undo().unwrap();
        assert!(session.take_event_selection_update().is_none());
    }

    #[test]
    fn empty_conversion_clears_selection_without_attaching_to_an_unrelated_undo_entry() {
        let mut session = fixture();
        session.set_text(None, "TITLE", "previous action").unwrap();
        let before = session.serialize().unwrap();
        let plan = Plan::prepare(
            &mut session,
            0,
            Scope::All { player: None },
            Operation::Convert {
                from: Kind::Lift,
                to: Kind::Tap,
                sustain_rows: 0,
            },
        )
        .unwrap();
        assert!(!plan.apply(&mut session, 0).unwrap());
        assert_eq!(session.serialize().unwrap(), before);
        assert!(
            session
                .take_event_selection_update()
                .unwrap()
                .events()
                .is_empty()
        );
        session.undo().unwrap();
        assert!(session.take_event_selection_update().is_none());
        session.redo().unwrap();
        assert!(session.take_event_selection_update().is_none());
    }
}
