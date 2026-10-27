//! Read-only prospective note policy, prepared on input changes, not actor traversal.
use super::State;
use deadsync_editor::{draft_note_policy::Cache, lane_entry::Draft};
impl State {
    pub(super) fn prepare_lane_draft_policy(&mut self) {
        let Some(input) = self.lane_input.model.as_ref() else {
            return;
        };
        let chart = input.chart();
        let session = &self.workspace.file().session;
        if !input.is_current(session) {
            self.lane_input.fake_error = Some("Stale lane draft".into());
            return;
        }
        let drafts: Vec<_> = input.drafts().collect();
        if drafts.is_empty() {
            self.lane_input.fake_error = None;
            return;
        }
        let result = (|| -> Result<(), deadsync_editor::Error> {
            if !self
                .lane_input
                .fake_cache
                .as_ref()
                .is_some_and(|cache| cache.is_current(session, chart))
            {
                self.lane_input.fake_cache = Some(Cache::capture(session, chart)?);
            }
            self.lane_input
                .fake_cache
                .as_mut()
                .unwrap()
                .prepare(session, drafts)
        })();
        self.lane_input.fake_error = result.err().map(|error| error.to_string());
    }
    /// None requests partial/failed presentation, never an invented ordinary note.
    pub(super) fn lane_draft_fake(&self, draft: Draft) -> Option<bool> {
        if self.lane_input.fake_error.is_some() {
            return None;
        }
        self.lane_input
            .fake_cache
            .as_ref()?
            .fake(&self.workspace.file().session, draft)
            .ok()
    }
}
