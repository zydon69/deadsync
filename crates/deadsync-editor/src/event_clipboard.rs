//! Detached mixed selections. Clipboard contents never retain a session token;
//! cut and paste publish through one atomic chart/history gesture.
mod selection;
pub(crate) mod song_widget;
pub use selection::{presets, region};
pub use song_widget::{SongWidget, WidgetKind};
mod patterns;
use crate::{
    Error,
    attacks::inspector::Draft as Attacks,
    history::{Clipboard as Notes, Session},
    notes::Position,
    timing_edit::{self, Scope, clipboard::Clipboard as Timing},
};
pub use selection::{Event, Selection, TimingKind};
use std::collections::BTreeSet;
#[derive(Clone)]
pub struct Clipboard {
    origin: u64,
    source_origin: i64,
    content: bool,
    widgets: Vec<song_widget::Copied>,
    notes: Option<Notes>,
    timing: Timing,
    attacks: Option<(u64, crate::attacks::inspector::Clipboard)>,
    patterns: patterns::Patterns,
    events: BTreeSet<Event>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PasteReport {
    pub changed: bool,
    pub patterns_pasted: usize,
    pub patterns_skipped: usize,
    pub notes_skipped: usize,
}
fn scope(session: &Session) -> Scope {
    if timing_edit::supports_resident_chart_timing(session.document()) {
        Scope::Chart
    } else {
        Scope::Song
    }
}
fn attack_scope(session:&Session)->Scope {
    if crate::attacks::supports_resident_chart_attacks(session.document()) {Scope::Chart} else {Scope::Song}
}
fn snapshot(session: &Session) -> Result<Session, Error> {
    Ok(Session::new(session.snapshot_document()?))
}

impl Clipboard {
    pub(crate) fn preserves_presentation_clock(&self) -> bool {
        self.timing.is_empty()
    }
    pub fn paste_at_fractional_row_with_presentation_receipt(
        &self,
        session: &mut Session,
        chart: usize,
        position: f64,
        player: u32,
    ) -> Result<(PasteReport, crate::song_properties::Application), Error> {
        let row = nearest_paste_row(position)?;
        crate::history::song_presentation::apply_clipboard(self, session, chart, row, player)
    }

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
        let Some(source_origin) = selected.iter().map(|event| event.source_row()).min() else {
            return Ok(None);
        };
        let origin = selected
            .iter()
            .filter_map(|event| event.content_row())
            .min()
            .unwrap_or(0);
        let end = selected
            .iter()
            .filter_map(|event| event.content_row())
            .max()
            .unwrap_or(0);
        let mut timing = Timing::capture_inventory(session, chart, scope(session), origin, end)?;
        timing.retain(selected);
        let attacks = Attacks::capture(session, chart, attack_scope(session))?;
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
            source_origin,
            content: selected.iter().any(|event| event.content_row().is_some()),
            widgets: selected
                .iter()
                .filter_map(|event| match event {
                    Event::SongWidget(widget) => Some(song_widget::Copied {
                        kind: widget.kind(),
                        row: widget.source_row(),
                    }),
                    _ => None,
                })
                .collect(),
            notes,
            timing,
            attacks,
            patterns: patterns::Patterns::capture(session, chart, selected)?,
            events: selected.clone(),
        }))
    }
    pub fn paste(
        &self,
        session: &mut Session,
        chart: usize,
        row: u64,
        player: u32,
    ) -> Result<bool, Error> {
        self.paste_with_report(session, chart, row, player)
            .map(|report| report.changed)
    }
    /// Source GetNearestRow: nonnegative cursor, ties at .5 round down.
    /// Native callers pass their authenticated focus/transport row unchanged.
    pub fn paste_at_fractional_row(
        &self,
        session: &mut Session,
        chart: usize,
        position: f64,
        player: u32,
    ) -> Result<PasteReport, Error> {
        let row = nearest_paste_row(position)?;
        self.paste_with_report(session, chart, row, player)
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
        let content_row = if self.content {
            let shifted =
                i128::from(row) + i128::from(self.origin) - i128::from(self.source_origin);
            if !(0..=i128::from(i32::MAX)).contains(&shifted) {
                return Err(Error::InvalidEdit("Mixed paste exceeds editor rows".into()));
            }
            shifted as u64
        } else {
            row
        };
        let widget_destination = row;
        let row = content_row;
        let mut report = PasteReport::default();
        session.grid(chart)?;
        let selection_before = crate::history::SelectionEffect::capture(session, chart, Vec::new());
        let mut pasted = Vec::new();
        report.changed = session.edit_chart_atomic(chart, "Paste selected events", |staged| {
            let before = snapshot(staged)?;
            let revision = staged.revision();
            if let Some(notes) = &self.notes {
                (_, report.notes_skipped) =
                    notes.paste_force_with_report(staged, chart, row, player)?;
                if let Some(effect) = staged.take_event_selection_update() {
                    pasted.extend_from_slice(effect.events());
                }
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
                let mut attacks = Attacks::capture(staged, chart, attack_scope(staged))?;
                attacks.paste_events(clip, row)?;
                attacks.apply(staged, chart)?;
            }
            (report.patterns_pasted, report.patterns_skipped) =
                self.patterns.paste(staged, chart, self.origin, row)?;
            for widget in &self.widgets {
                widget.apply(staged, chart, self.source_origin, widget_destination)?;
            }
            for event in &self.events {
                let destination = || event.row().checked_sub(self.origin).and_then(|offset| row.checked_add(offset))
                    .ok_or_else(|| Error::InvalidEdit("Paste selection exceeds editor rows".into()));
                pasted.push(match *event {
                    Event::Timing { kind, .. } => Event::Timing { row: destination()?, kind },
                    Event::Attack { .. } => Event::Attack { row: destination()? },
                    Event::Pattern { .. } if report.patterns_skipped == 0 => Event::Pattern { row: destination()? },
                    _ => continue,
                });
            }
            let grid = staged.grid(chart)?;
            let kinds = pasted.iter().filter_map(|event| match event {
                Event::Note(position) => grid.get(*position).map(|note| (*position, note.kind)),
                _ => None,
            }).collect();
            pasted.sort_by_key(|event| event.sort_key(&kinds));
            pasted.dedup();
            Ok(())
        })?;
        let selection_after = crate::history::SelectionEffect::capture(session, chart, pasted);
        session.record_event_selection(crate::history::selection_effect::SelectionHistory { before: selection_before, after: selection_after }, report.changed);
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
            let mut attacks = Attacks::capture(staged, chart, attack_scope(staged))?;
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

fn nearest_paste_row(position: f64) -> Result<u64, Error> {
    if !position.is_finite() || position > i32::MAX as f64 {
        return Err(Error::InvalidEdit("Event paste exceeds editor rows".into()));
    }
    let position = position.max(0.0);
    let integral = position.trunc();
    Ok(integral as u64 + u64::from(position - integral > 0.5))
}

#[cfg(test)]
mod selection_history_tests {
    use super::*;
    use crate::{document::{Document, Format}, notes::Position};

    #[test]
    fn mixed_paste_selects_only_added_targets_undo_clears_and_redo_restores_them() {
        let mut session = Session::new(Document::parse(b"#VERSION:0.83;#BPMS:0=120,2=150;#LABELS:1=marker;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0000;".to_vec(), Format::Ssc).unwrap());
        let mut selected = Selection::capture(&mut session, 0, 48, 96).unwrap();
        selected.select_all(true);
        let clip = selected.copy(&mut session).unwrap().unwrap();
        let before = session.serialize().unwrap();
        assert!(clip.paste_with_report(&mut session, 0, 384, 0).unwrap().changed);
        let after = session.serialize().unwrap();
        let expected = vec![
            Event::Timing { row: 384, kind: TimingKind::Label },
            Event::Note(Position { row: 384, lane: 1, player: 0 }),
            Event::Timing { row: 432, kind: TimingKind::Tempo },
            Event::Note(Position { row: 432, lane: 2, player: 0 }),
        ];
        let effect = session.take_event_selection_update().unwrap();
        assert!(effect.is_current(&session));
        assert_eq!(effect.events(), expected);
        session.undo().unwrap();
        assert_eq!(session.serialize().unwrap(), before);
        assert!(session.take_event_selection_update().unwrap().events().is_empty());
        session.redo().unwrap();
        assert_eq!(session.serialize().unwrap(), after);
        assert_eq!(session.take_event_selection_update().unwrap().events(), expected);
    }

    #[test]
    fn note_paste_selection_uses_accepted_lanes_and_actual_destination_player() {
        for layout in ["dance-single", "dance-routine"] {
            let zeros = if layout == "dance-single" { "0000" } else { "00000000" };
            let text = format!("#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-double;#NOTES:00000001\n10000000\n00000000\n00000000;#NOTEDATA:;#STEPSTYPE:{layout};#NOTES:{zeros};");
            let mut session = Session::new(Document::parse(text.into_bytes(), Format::Ssc).unwrap());
            let clip = Notes::copy(session.grid(0).unwrap(), 0, 48, None).unwrap();
            let before = session.serialize().unwrap();
            let owner = u32::from(layout == "dance-routine");
            let (changed, skipped) = clip.paste_force_with_report(&mut session, 1, 192, owner).unwrap();
            assert!(changed);
            let mut expected = Vec::new();
            if owner == 1 { expected.push(Event::Note(Position { row: 192, lane: 7, player: owner })); }
            expected.push(Event::Note(Position { row: 240, lane: 0, player: owner }));
            assert_eq!(skipped, usize::from(owner == 0));
            let effect = session.take_event_selection_update().unwrap();
            assert!(effect.is_current(&session));
            assert_eq!(effect.chart(), &session.chart_ids()[1]);
            assert_eq!(effect.events(), expected);
            let after = session.serialize().unwrap();
            session.undo().unwrap();
            assert_eq!(session.serialize().unwrap(), before);
            assert!(session.take_event_selection_update().unwrap().events().is_empty());
            session.redo().unwrap();
            assert_eq!(session.serialize().unwrap(), after);
            assert_eq!(session.take_event_selection_update().unwrap().events(), expected);
        }
    }

    #[test]
    fn invalid_paste_destination_refuses_without_a_selection_receipt() {
        let mut session = Session::new(Document::parse(b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;".to_vec(), Format::Ssc).unwrap());
        let clip = Clipboard::copy(&mut session, 0, 0, 0).unwrap().unwrap();
        let before = session.serialize().unwrap();
        assert!(clip.paste_with_report(&mut session, usize::MAX, 0, 0).is_err());
        assert_eq!(session.serialize().unwrap(), before);
        assert!(session.take_event_selection_update().is_none());
    }
}
