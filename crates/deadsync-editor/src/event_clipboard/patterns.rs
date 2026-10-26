//! Detached marker definitions. Clipboard ForceAdd replaces a marker with the
//! same head row; it does not clear generated notes or overlapping regions.
use super::Event;
use crate::{
    Error,
    generation::metadata::{ChartData, Definition},
    history::Session,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Default)]
pub(super) struct Patterns(BTreeMap<u64, Definition>);

impl Patterns {
    pub(super) fn capture(
        session: &Session,
        chart: usize,
        selected: &BTreeSet<Event>,
    ) -> Result<Self, Error> {
        let rows: Vec<_> = selected
            .iter()
            .filter_map(|event| match event {
                Event::Pattern { row } => Some(*row),
                _ => None,
            })
            .collect();
        if rows.is_empty() {
            return Ok(Self::default());
        }
        let data = ChartData::read(session.document(), chart)?
            .ok_or_else(|| Error::InvalidEdit("Selected pattern metadata is missing".into()))?;
        let mut definitions = BTreeMap::new();
        for row in rows {
            let definition = data
                .patterns
                .as_ref()
                .and_then(|patterns| patterns.get(&row))
                .ok_or_else(|| Error::InvalidEdit("Selected pattern no longer exists".into()))?;
            definitions.insert(row, definition.clone());
        }
        Ok(Self(definitions))
    }

    /// Returns accepted/skipped marker counts. Unsupported markers do not
    /// prevent other compatible events in the same clipboard from being pasted.
    pub(super) fn paste(
        &self,
        session: &mut Session,
        chart: usize,
        origin: u64,
        row: u64,
    ) -> Result<(usize, usize), Error> {
        if self.0.is_empty() {
            return Ok((0, 0));
        }
        let layout = &session
            .document()
            .charts()
            .get(chart)
            .ok_or_else(|| Error::InvalidEdit("Chart does not exist".into()))?
            .steps_type;
        if crate::notes::supports_variable_players(layout)
            || layout.ends_with("-couple")
            || (layout == "pump-double" && session.grid(chart)?.uses_stepf2())
        {
            return Ok((0, self.0.len()));
        }
        let mut data = ChartData::existing_or_initial(session.document(), chart)?;
        let before = data.patterns.clone();
        let patterns = data.patterns.get_or_insert_with(BTreeMap::new);
        for (&head, definition) in &self.0 {
            let head = head
                .checked_sub(origin)
                .and_then(|offset| row.checked_add(offset))
                .ok_or_else(|| Error::InvalidEdit("Pattern paste exceeds editor rows".into()))?;
            patterns.insert(head, definition.clone());
        }
        data.validate()?;
        if data.patterns != before {
            data.save(session, chart)?;
        }
        Ok((self.0.len(), 0))
    }

    pub(super) fn remove(&self, session: &mut Session, chart: usize) -> Result<(), Error> {
        if self.0.is_empty() {
            return Ok(());
        }
        let mut data = ChartData::read(session.document(), chart)?
            .ok_or_else(|| Error::InvalidEdit("Selected pattern metadata is missing".into()))?;
        let patterns = data
            .patterns
            .as_mut()
            .ok_or_else(|| Error::InvalidEdit("Selected pattern metadata is missing".into()))?;
        for row in self.0.keys() {
            if patterns.remove(row).is_none() {
                return Err(Error::InvalidEdit(
                    "Selected pattern no longer exists".into(),
                ));
            }
        }
        data.save(session, chart)?;
        Ok(())
    }
}
