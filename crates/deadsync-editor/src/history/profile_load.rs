//! Source load normalization establishes a baseline, never a hidden Undo entry.
use super::*;
impl Session {
    pub(crate) fn adopt_profile_load(
        &mut self,
        token: &RevisionToken,
        document: Document,
        marker_repairs: bool,
    ) -> Result<(), Error> {
        if !self.accepts_token(token)
            || !self.undo.is_empty()
            || !self.redo.is_empty()
            || self.revision != 0
            || self.content_revision != 0
            || document.charts().len() != self.chart_ids.len()
        {
            return Err(Error::InvalidEdit(
                "Profile normalization requires an unchanged freshly loaded song".into(),
            ));
        }
        self.document = document;
        self.application_review_epoch = Arc::new(());
        self.grids.clear();
        self.touched_grids.clear();
        if marker_repairs {
            self.revision = self.next_revision;
            self.content_revision = self.next_revision;
            self.next_revision += 1;
            // saved_revision remains the original raw-file baseline. No entry
            // exists to undo source loader repairs; later user edits do.
        }
        Ok(())
    }
}
