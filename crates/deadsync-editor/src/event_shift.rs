//! Source-faithful row shifts for an explicit mixed event selection.
//! Negative destinations delete movable events; immutable initial nodes remain.
pub mod reviewed;
use crate::{
    Error,
    event_clipboard::{Clipboard, Event, Selection, TimingKind},
    history::{RevisionToken, Session},
};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub changed: bool,
    pub moved: usize,
    pub deleted: usize,
    pub immutable: usize,
    /// Source ordering is retained after a common row offset. This is the
    /// successfully transformed selection, not all events still in the chart.
    pub selection_after: Vec<Event>,
}
pub struct Plan {
    token: RevisionToken,
    chart: usize,
    rows: i64,
    remove: BTreeSet<Event>,
    clipboard: Option<Clipboard>,
    destination: u64,
    player: u32,
    report: Report,
    originals: Vec<Event>,
}
impl Plan {
    pub fn capture(session: &mut Session, selection: &Selection, rows: i64) -> Result<Self, Error> {
        // The public detached-copy boundary validates the caller's live Session
        // and revision; a forged or stale selection cannot supply event keys.
        let _ = selection.copy(session)?;
        let chart = selection.chart();
        let layout = session
            .document()
            .charts()
            .get(chart)
            .ok_or_else(|| invalid("Shift chart does not exist"))?
            .steps_type
            .clone();
        let unsupported_patterns = crate::notes::supports_variable_players(&layout)
            || layout.ends_with("-couple")
            || (layout == "pump-double" && session.grid(chart)?.uses_stepf2());
        let metadata = crate::generation::metadata::ChartData::read(session.document(), chart)?;
        let mut valid = BTreeSet::new();
        let mut remove = BTreeSet::new();
        let mut owners = BTreeSet::new();
        let mut report = Report::default();
        let mut originals = Vec::new();
        for (index, &event) in selection.events().iter().enumerate() {
            if !selection.is_selected(index) {
                continue;
            }
            if matches!(event, Event::SongWidget(_)) {
                report.immutable += 1;
                continue;
            }
            if matches!(
                event,
                Event::Timing {
                    row: 0,
                    kind: TimingKind::Tempo | TimingKind::Signature
                }
            ) {
                report.immutable += 1;
                continue;
            }
            let row = i128::from(event.row()) + i128::from(rows);
            originals.push(event);
            if row < 0 || (unsupported_patterns && matches!(event, Event::Pattern { .. })) {
                remove.insert(event);
                report.deleted += 1;
                continue;
            }
            if row > i128::from(i32::MAX) {
                return Err(invalid("Event shift exceeds editor rows"));
            }
            let row = row as u64;
            let shifted = match event {
                Event::Note(mut position) => {
                    if let Some(end) = session.grid(chart)?.get(position).and_then(|note| note.end)
                    {
                        checked_end(end, rows)?;
                    }
                    owners.insert(position.player);
                    position.row = row;
                    Event::Note(position)
                }
                Event::Pattern { row: head } => {
                    let length = metadata
                        .as_ref()
                        .and_then(|data| data.patterns.as_ref())
                        .and_then(|patterns| patterns.get(&head))
                        .ok_or_else(|| invalid("Selected pattern no longer exists"))?
                        .length;
                    if row
                        .checked_add(length)
                        .is_none_or(|end| end > i32::MAX as u64)
                    {
                        return Err(invalid("Pattern shift exceeds editor rows"));
                    }
                    Event::Pattern { row }
                }
                Event::Timing { kind, .. } => Event::Timing { row, kind },
                Event::Attack { .. } => Event::Attack { row },
                Event::SongWidget(_) => {
                    unreachable!("immutable widgets excluded before row arithmetic")
                }
            };
            valid.insert(event);
            if rows != 0 {
                remove.insert(event);
                report.moved += 1;
            }
            report.selection_after.push(shifted);
        }
        let origin = valid.iter().map(|event| event.row()).min().unwrap_or(0);
        let destination = if valid.is_empty() {
            0
        } else {
            checked_end(origin, rows)?
        };
        let clipboard = if rows == 0 {
            None
        } else {
            choose(session, chart, &valid)?.copy(session)?
        };
        let player = if owners.len() == 1 {
            *owners.first().unwrap()
        } else {
            0
        };
        Ok(Self {
            token: session.revision_token(),
            chart,
            rows,
            remove,
            clipboard,
            destination,
            player,
            report,
            originals,
        })
    }
    pub fn report(&self) -> &Report {
        &self.report
    }
    pub fn apply(self, session: &mut Session) -> Result<Report, Error> {
        if !session.accepts_token(&self.token) {
            return Err(invalid(
                "Event shift belongs to another session or revision",
            ));
        }
        let mut report = self.report;
        let before = crate::history::SelectionEffect::capture(session, self.chart, self.originals);
        if self.remove.is_empty() {
            let after = crate::history::SelectionEffect::capture(session, self.chart, report.selection_after.clone());
            session.record_event_selection(crate::history::selection_effect::SelectionHistory { before, after }, false);
            return Ok(report);
        }
        report.changed =
            session.edit_chart_atomic(self.chart, "Shift selected events", |staged| {
                choose(staged, self.chart, &self.remove)?.delete(staged)?;
                if self.rows != 0 {
                    if let Some(clipboard) = &self.clipboard {
                        let pasted = clipboard.paste_with_report(
                            staged,
                            self.chart,
                            self.destination,
                            self.player,
                        )?;
                        if pasted.notes_skipped != 0 || pasted.patterns_skipped != 0 {
                            return Err(invalid("A validated shifted event could not be restored"));
                        }
                    }
                }
                Ok(())
            })?;
        let after = crate::history::SelectionEffect::capture(session, self.chart, report.selection_after.clone());
        session.record_event_selection(crate::history::selection_effect::SelectionHistory { before, after }, report.changed);
        Ok(report)
    }
}
fn choose(
    session: &mut Session,
    chart: usize,
    events: &BTreeSet<Event>,
) -> Result<Selection, Error> {
    let mut selection = Selection::capture(session, chart, 0, i32::MAX as u64)?;
    selection.select_all(false);
    for index in 0..selection.events().len() {
        if events.contains(&selection.events()[index]) {
            selection.set_selected(index, true)?;
        }
    }
    Ok(selection)
}
fn checked_end(row: u64, rows: i64) -> Result<u64, Error> {
    let row = i128::from(row) + i128::from(rows);
    if !(0..=i128::from(i32::MAX)).contains(&row) {
        return Err(invalid("Shifted event end exceeds editor rows"));
    }
    Ok(row as u64)
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}

#[cfg(test)]
mod selection_history_tests {
    use super::*;
    use crate::{document::{Document, Format}, notes::Position};
    #[test]
    fn mixed_shift_history_restores_deleted_originals_and_excludes_initial_tempo() {
        let mut session = Session::new(Document::parse(b"#VERSION:0.83;#BPMS:0=120,2=150;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0000\n0000;".to_vec(), Format::Ssc).unwrap());
        let mut selected = Selection::capture(&mut session, 0, 0, 192).unwrap();
        selected.select_all(true);
        let before = session.serialize().unwrap();
        let report = Plan::capture(&mut session, &selected, -48).unwrap().apply(&mut session).unwrap();
        assert!(report.changed);
        assert_eq!(report.deleted, 1);
        let after = session.serialize().unwrap();
        let effect = session.take_event_selection_update().unwrap();
        assert!(effect.is_current(&session));
        assert_eq!(effect.events(), &report.selection_after);
        session.undo().unwrap();
        assert_eq!(session.serialize().unwrap(), before);
        assert_eq!(session.take_event_selection_update().unwrap().events(), &[
            Event::Note(Position { row: 0, lane: 0, player: 0 }),
            Event::Note(Position { row: 48, lane: 1, player: 0 }),
            Event::Timing { row: 96, kind: TimingKind::Tempo },
        ]);
        session.redo().unwrap();
        assert_eq!(session.serialize().unwrap(), after);
        assert_eq!(session.take_event_selection_update().unwrap().events(), &report.selection_after);
    }
}
