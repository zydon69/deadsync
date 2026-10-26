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
        Ok(())
    }
    pub fn apply(&self, session: &mut Session, chart: usize) -> Result<bool, Error> {
        if chart != self.chart || !session.accepts_token(&self.token) {
            return Err(Error::InvalidEdit("Note attribute draft is stale".into()));
        }
        session.edit_notes(
            chart,
            "Edit note attributes",
            [(self.position, Some(self.note.clone()))],
        )
    }
}
