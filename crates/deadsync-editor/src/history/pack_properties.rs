//! A source cached field publishes exactly one global action at deactivation.
use super::{Change, Session};
use crate::Error;
impl Session {
    /// The owner/token/Data baseline was captured before text editing. Validation
    /// and the actual mutation both authenticate it; no fallback recaptures stale data.
    pub fn edit_pack_properties_reviewed(
        &mut self,
        review: &crate::pack::storage::history::Review,
        after: crate::pack::Data,
    ) -> Result<bool, Error> {
        if !self.allows_external_edits {
            return Err(Error::ExternalChange);
        }
        let Some(edit) = crate::pack::storage::history::Edit::capture_reviewed(review, after)?
        else {
            return Ok(false);
        };
        self.execute(
            "Edit pack properties".into(),
            Change::PackProperties(Box::new(edit)),
        )?;
        Ok(true)
    }
    pub fn history_changes_pack_properties(&self, redo: bool) -> bool {
        (if redo {
            self.redo.last()
        } else {
            self.undo.last()
        })
        .is_some_and(|entry| matches!(entry.change, Change::PackProperties(_)))
    }
}
