//! Detached mixed selections. Clipboard contents never retain a session token;
//! cut and paste publish through one atomic chart/history gesture.
mod selection;
pub use selection::presets;
mod patterns;
use crate::{
    Error,
    attacks::inspector::Draft as Attacks,
    document::Document,
    history::{Clipboard as Notes, Session},
    notes::Position,
    timing_edit::{self, Scope, clipboard::Clipboard as Timing},
};
pub use selection::{Event, Selection, TimingKind};
use std::collections::BTreeSet;
#[derive(Clone)]
pub struct Clipboard {
    origin: u64,
    notes: Option<Notes>,
    timing: Timing,
    attacks: Option<(u64, crate::attacks::inspector::Clipboard)>,
    patterns: patterns::Patterns,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PasteReport {
    pub changed: bool,
    pub patterns_pasted: usize,
    pub patterns_skipped: usize,
    pub notes_skipped: usize,
}
fn scope(session: &Session) -> Scope {
    if timing_edit::supports_chart_timing(session.document()) {
        Scope::Chart
    } else {
        Scope::Song
    }
}
fn snapshot(session: &Session) -> Result<Session, Error> {
    Ok(Session::new(Document::parse(
        session.serialize()?,
        session.document().format,
    )?))
}
impl Clipboard {
    pub fn copy(
        session: &mut Session,
        chart: usize,
        start: u64,
        end: u64,
    ) -> Result<Option<Self>, Error> {
        Selection::capture(session, chart, start, end)?.copy(session)
    }
    fn copy_selected(
        session: &mut Session,
        chart: usize,
        selected: &BTreeSet<Event>,
    ) -> Result<Option<Self>, Error> {
        let Some(origin) = selected.iter().map(|event| event.row()).min() else {
            return Ok(None);
        };
        let end = selected
            .iter()
            .map(|event| event.row())
            .max()
            .expect("Nonempty selection");
        let mut timing = Timing::capture(session, chart, scope(session), origin, end)?;
        timing.retain(selected);
        let attacks = Attacks::capture(session, chart, scope(session))?;
        let indices: Vec<_> = attacks
            .attacks()
            .events()
            .iter()
            .enumerate()
            .filter_map(|(index, event)| {
                selected
                    .contains(&Event::Attack { row: event.row })
                    .then_some(index)
            })
            .collect();
        let attacks = indices
            .first()
            .map(|&index| {
                let row = attacks.attacks().events()[index].row;
                attacks.copy_events(&indices).map(|clip| (row, clip))
            })
            .transpose()?;
        let positions: BTreeSet<_> = selected
            .iter()
            .filter_map(|event| match event {
                Event::Note(p) => Some(*p),
                _ => None,
            })
            .collect();
        let notes = if positions.is_empty() {
            None
        } else {
            Some(Notes::copy_positions(
                session.grid(chart)?,
                origin,
                &positions,
            )?)
        };
        Ok(Some(Self {
            origin,
            notes,
            timing,
            attacks,
            patterns: patterns::Patterns::capture(session, chart, selected)?,
        }))
    }
    pub fn paste(
        &self,
        session: &mut Session,
        chart: usize,
        row: u64,
        player: u32,
    ) -> Result<bool, Error> {
        self.paste_with_report(session, chart, row, player).map(|report| report.changed)
    }
    pub fn paste_with_report(
        &self,
        session: &mut Session,
        chart: usize,
        row: u64,
        player: u32,
    ) -> Result<PasteReport, Error> {
        if row > i32::MAX as u64 {
            return Err(Error::InvalidEdit("Event paste exceeds editor rows".into()));
        }
        let mut report = PasteReport::default();
        report.changed = session.edit_chart_atomic(chart, "Paste selected events", |staged| {
            let before = snapshot(staged)?;
            let revision = staged.revision();
            if let Some(notes) = &self.notes {
                (_, report.notes_skipped) = notes.paste_force_with_report(staged, chart, row, player)?;
            }
            self.timing
                .paste(staged, chart, scope(staged), self.origin, row)?;
            // Note heads affect the file loader's attack placement around timing
            // phases, so preserve the original resident rows after both edits.
            if staged.revision() != revision {
                crate::attacks::anchoring::preserve(&before, staged)?;
            }
            if let Some((first, clip)) = &self.attacks {
                let row = row
                    .checked_add(first - self.origin)
                    .ok_or_else(|| Error::InvalidEdit("Attack paste overflow".into()))?;
                let mut attacks = Attacks::capture(staged, chart, scope(staged))?;
                attacks.paste_events(clip, row)?;
                attacks.apply(staged, chart)?;
            }
            (report.patterns_pasted, report.patterns_skipped) = self.patterns.paste(staged, chart, self.origin, row)?;
            Ok(())
        })?;
        Ok(report)
    }
    /// Empty selections retain the caller's previous clipboard. A failed cut
    /// changes neither notes/properties nor either history branch.
    pub fn cut(
        session: &mut Session,
        chart: usize,
        start: u64,
        end: u64,
    ) -> Result<Option<Self>, Error> {
        Selection::capture(session, chart, start, end)?.cut(session)
    }
    fn remove_selected(
        &self,
        session: &mut Session,
        chart: usize,
        selected: &BTreeSet<Event>,
        label: &str,
    ) -> Result<bool, Error> {
        session.edit_chart_atomic(chart, label, |staged| {
            let mut attacks = Attacks::capture(staged, chart, scope(staged))?;
            let attack_indices: Vec<_> = attacks
                .attacks()
                .events()
                .iter()
                .enumerate()
                .filter_map(|(index, event)| {
                    selected
                        .contains(&Event::Attack { row: event.row })
                        .then_some(index)
                })
                .collect();
            attacks.remove_events(&attack_indices)?;
            attacks.apply(staged, chart)?;
            let remaining = snapshot(staged)?;
            let revision = staged.revision();
            let positions: Vec<Position> = selected
                .iter()
                .filter_map(|event| match event {
                    Event::Note(p) => Some(*p),
                    _ => None,
                })
                .collect();
            staged.edit_notes(
                chart,
                "Delete selected notes",
                positions.into_iter().map(|p| (p, None)),
            )?;
            self.timing.delete(staged, chart, scope(staged))?;
            if staged.revision() != revision {
                crate::attacks::anchoring::preserve(&remaining, staged)?;
            }
            self.patterns.remove(staged, chart)?;
            Ok(())
        })
    }
}
