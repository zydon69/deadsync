//! Snapshot-bound editing of saved pattern markers, independent of their notes.
use super::metadata::{ChartData, Definition};
use crate::{
    Error,
    history::{RevisionToken, Session},
};

pub struct Draft {
    token: RevisionToken,
    chart: usize,
    original_start: u64,
    original: Definition,
    pub start: u64,
    pub definition: Definition,
}
impl Draft {
    /// Overlapping markers use the same latest-start selection as F5.
    pub fn at_cursor(session: &Session, chart: usize, cursor: u64) -> Result<Self, Error> {
        let data = ChartData::read(session.document(), chart)?
            .ok_or_else(|| invalid("No saved pattern markers in this chart"))?;
        let (start, definition) = data
            .marker_at(cursor)
            .ok_or_else(|| invalid("No saved pattern marker at the cursor"))?;
        Ok(Self {
            token: session.revision_token(),
            chart,
            original_start: start,
            original: definition.clone(),
            start,
            definition: definition.clone(),
        })
    }
    fn data(&self, session: &Session, chart: usize) -> Result<ChartData, Error> {
        if chart != self.chart || !session.accepts_token(&self.token) {
            return Err(invalid(
                "Pattern editor belongs to another chart or revision",
            ));
        }
        ChartData::read(session.document(), chart)?
            .ok_or_else(|| invalid("Saved pattern metadata is missing"))
    }
    pub fn apply(&self, session: &mut Session, chart: usize) -> Result<bool, Error> {
        let mut data = self.data(session, chart)?;
        if self.start == self.original_start && self.definition == self.original {
            return Ok(false);
        }
        let patterns = data
            .patterns
            .as_mut()
            .ok_or_else(|| invalid("Saved pattern metadata is missing"))?;
        if self.start != self.original_start && patterns.contains_key(&self.start) {
            return Err(invalid("A saved pattern already exists at the new start"));
        }
        patterns.remove(&self.original_start);
        patterns.insert(self.start, self.definition.clone());
        data.save(session, chart)
    }
    /// Deleting a marker does not delete or regenerate its notes.
    pub fn delete(&self, session: &mut Session, chart: usize) -> Result<bool, Error> {
        let mut data = self.data(session, chart)?;
        data.patterns
            .as_mut()
            .ok_or_else(|| invalid("Saved pattern metadata is missing"))?
            .remove(&self.original_start);
        data.save(session, chart)
    }
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
