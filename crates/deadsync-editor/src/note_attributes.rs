//! Snapshot-bound editing of a single note's opaque simfile attributes.
use crate::{
    Error,
    history::{RevisionToken, Session},
    notes::{Note, Position},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Head,
    Tail,
}
pub struct Draft {
    token: RevisionToken,
    chart: usize,
    position: Position,
    note: Note,
    stepf2: bool,
}
impl Draft {
    pub fn capture(session: &mut Session, chart: usize, position: Position) -> Result<Self, Error> {
        let note = session
            .grid(chart)?
            .get(position)
            .cloned()
            .ok_or_else(|| Error::InvalidEdit("No note head at this position".into()))?;
        Ok(Self::from_note(session, chart, position, &note))
    }
    pub(crate) fn from_note(
        session: &Session,
        chart: usize,
        position: Position,
        note: &Note,
    ) -> Self {
        Self {
            token: session.revision_token(),
            chart,
            position,
            note: note.clone(),
            stepf2: session
                .loaded_grid(chart)
                .is_some_and(|grid| grid.uses_stepf2()),
        }
    }
    pub fn position(&self) -> Position {
        self.position
    }
    pub fn note(&self) -> &Note {
        &self.note
    }
    pub fn text(&self, field: Field) -> &str {
        match field {
            Field::Head => &self.note.attributes,
            Field::Tail => &self.note.tail_attributes,
        }
    }
    /// Typed StepF2 fields are available only in a decoded co-op chart. They
    /// share the same draft/history gesture as the existing opaque suffixes.
    pub fn stepf2_attributes(
        &self,
        field: Field,
    ) -> Result<Option<crate::stepf2::Attributes>, Error> {
        if !self.stepf2 || (field == Field::Tail && self.note.end.is_none()) {
            return Ok(None);
        }
        Ok(Some(
            crate::stepf2::Attributes::read(self.text(field))?.map_or_else(
                || crate::stepf2::Attributes {
                    fake: field == Field::Head && self.note.kind == crate::notes::Kind::Fake,
                    ..Default::default()
                },
                |(attributes, _)| attributes,
            ),
        ))
    }

    pub fn set_stepf2_attributes(
        &mut self,
        field: Field,
        attributes: crate::stepf2::Attributes,
    ) -> Result<(), Error> {
        if !self.stepf2 || (field == Field::Tail && self.note.end.is_none()) {
            return Err(Error::InvalidEdit(
                "StepF2 attributes are unavailable for this note endpoint".into(),
            ));
        }
        if self.stepf2_attributes(field)? == Some(attributes) {
            return Ok(());
        }
        let current = self.text(field);
        let trailing =
            crate::stepf2::Attributes::read(current)?.map_or(current, |(_, trailing)| trailing);
        let suffix = attributes.suffix('N', trailing)?;
        self.set_text(field, &suffix)?;
        if field == Field::Head
            && matches!(
                self.note.kind,
                crate::notes::Kind::Tap | crate::notes::Kind::Fake
            )
        {
            self.note.kind = if attributes.fake {
                crate::notes::Kind::Fake
            } else {
                crate::notes::Kind::Tap
            };
        }
        Ok(())
    }
    /// Syntax is validated atomically at application, allowing incomplete input while typing.
    pub fn set_text(&mut self, field: Field, value: &str) -> Result<(), Error> {
        if value.chars().any(char::is_control) {
            return Err(Error::InvalidEdit(
                "Note attributes must occupy one line".into(),
            ));
        }
        match field {
            Field::Head => self.note.attributes = value.into(),
            Field::Tail => self.note.tail_attributes = value.into(),
        }
        if self.stepf2
            && field == Field::Head
            && matches!(
                self.note.kind,
                crate::notes::Kind::Tap | crate::notes::Kind::Fake
            )
        {
            // Raw edits remain permissive while typing. Reclassify only once
            // the compound itself is complete and valid; apply rejects any
            // remaining syntax errors before changing history.
            if let Ok(Some((attributes, _))) = crate::stepf2::Attributes::read(value) {
                self.note.kind = if attributes.fake {
                    crate::notes::Kind::Fake
                } else {
                    crate::notes::Kind::Tap
                };
            }
        }
        Ok(())
    }
    pub fn apply(&self, session: &mut Session, chart: usize) -> Result<bool, Error> {
        if chart != self.chart || !session.accepts_token(&self.token) {
            return Err(Error::InvalidEdit("Note attribute draft is stale".into()));
        }
        if self.stepf2 {
            crate::stepf2::validate_attributes(&self.note)?;
        }
        session.edit_notes(
            chart,
            "Edit note attributes",
            [(self.position, Some(self.note.clone()))],
        )
    }
}
