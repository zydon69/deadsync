//! Detached mixed selections. Clipboard contents never retain a session token;
//! cut and paste publish through one atomic chart/history gesture.
use crate::{
    Error,
    attacks::inspector::Draft as Attacks,
    document::Document,
    history::{Clipboard as Notes, Session},
    notes::Position,
    timing_edit::{self, Scope, clipboard::Clipboard as Timing},
};
#[derive(Clone)]
pub struct Clipboard {
    origin: u64,
    notes: Option<Notes>,
    timing: Timing,
    attacks: Option<(u64, crate::attacks::inspector::Clipboard)>,
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
        if start > end || end > i32::MAX as u64 {
            return Err(Error::InvalidEdit("Invalid event selection bounds".into()));
        }
        let timing = Timing::capture(session, chart, scope(session), start, end)?;
        let attacks = Attacks::capture(session, chart, scope(session))?;
        let indices: Vec<_> = attacks
            .attacks()
            .events()
            .iter()
            .enumerate()
            .filter_map(|(index, event)| (start..=end).contains(&event.row).then_some(index))
            .collect();
        let attack_row = indices
            .first()
            .map(|&index| attacks.attacks().events()[index].row);
        let grid = session.grid(chart)?;
        let note_row = grid
            .iter()
            .filter_map(|(position, _)| {
                (start..=end)
                    .contains(&position.row)
                    .then_some(position.row)
            })
            .min();
        let Some(origin) = [timing.first_row(), attack_row, note_row]
            .into_iter()
            .flatten()
            .min()
        else {
            return Ok(None);
        };
        let notes = note_row
            .map(|_| Notes::copy(grid, origin, end, None))
            .transpose()?;
        let attacks = attack_row
            .map(|row| attacks.copy_events(&indices).map(|clip| (row, clip)))
            .transpose()?;
        Ok(Some(Self {
            origin,
            notes,
            timing,
            attacks,
        }))
    }
    pub fn paste(
        &self,
        session: &mut Session,
        chart: usize,
        row: u64,
        player: u32,
    ) -> Result<bool, Error> {
        if row > i32::MAX as u64 {
            return Err(Error::InvalidEdit("Event paste exceeds editor rows".into()));
        }
        session.edit_chart_atomic(chart, "Paste selected events", |staged| {
            let before = snapshot(staged)?;
            if let Some(notes) = &self.notes {
                notes.paste_force(staged, chart, row, player)?;
            }
            self.timing
                .paste(staged, chart, scope(staged), self.origin, row)?;
            // Note heads affect the file loader's attack placement around timing
            // phases, so preserve the original resident rows after both edits.
            crate::attacks::anchoring::preserve(&before, staged)?;
            if let Some((first, clip)) = &self.attacks {
                let row = row
                    .checked_add(first - self.origin)
                    .ok_or_else(|| Error::InvalidEdit("Attack paste overflow".into()))?;
                let mut attacks = Attacks::capture(staged, chart, scope(staged))?;
                attacks.paste_events(clip, row)?;
                attacks.apply(staged, chart)?;
            }
            Ok(())
        })
    }
    /// Empty selections retain the caller's previous clipboard. A failed cut
    /// changes neither notes/properties nor either history branch.
    pub fn cut(
        session: &mut Session,
        chart: usize,
        start: u64,
        end: u64,
    ) -> Result<Option<Self>, Error> {
        let Some(clipboard) = Self::copy(session, chart, start, end)? else {
            return Ok(None);
        };
        session.edit_chart_atomic(chart, "Cut selected events", |staged| {
            let mut attacks = Attacks::capture(staged, chart, scope(staged))?;
            let selected: Vec<_> = attacks
                .attacks()
                .events()
                .iter()
                .enumerate()
                .filter_map(|(index, event)| (start..=end).contains(&event.row).then_some(index))
                .collect();
            attacks.remove_events(&selected)?;
            attacks.apply(staged, chart)?;
            let remaining = snapshot(staged)?;
            let positions: Vec<Position> = staged
                .grid(chart)?
                .iter()
                .filter_map(|(position, _)| {
                    (start..=end).contains(&position.row).then_some(position)
                })
                .collect();
            staged.edit_notes(
                chart,
                "Cut selected notes",
                positions.into_iter().map(|p| (p, None)),
            )?;
            clipboard.timing.delete(staged, chart, scope(staged))?;
            crate::attacks::anchoring::preserve(&remaining, staged)?;
            Ok(())
        })?;
        Ok(Some(clipboard))
    }
}
