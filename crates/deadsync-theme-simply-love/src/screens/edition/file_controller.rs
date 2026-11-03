//! Keyboard File actions retain explicit review authority across asynchronous
//! Save As; cancellation destroys that authority. No I/O or state replacement.
use deadsync_editor::{
    Error,
    history::file_intent::IntentGuard,
    storage::{FileSession, RelocationReceipt},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    Review,
    Saving,
    Ready,
}
struct Pending<T> {
    action: T,
    guard: IntentGuard,
    phase: Phase,
    policy_revision: u64,
}
pub(super) struct Controller<T> {
    pending: Option<Pending<T>>,
}
impl<T> Default for Controller<T> {
    fn default() -> Self {
        Self { pending: None }
    }
}
impl<T> Controller<T> {
    pub(super) fn is_action(&self, predicate: impl FnOnce(&T) -> bool) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|pending| predicate(&pending.action))
    }
    #[cfg(test)]
    pub(super) fn begin(&mut self, action: T, file: &FileSession) {
        self.begin_with_unsaved(action, file, file.has_unsaved_document());
    }
    pub(super) fn begin_with_unsaved(&mut self, action: T, file: &FileSession, unsaved: bool) {
        self.pending = Some(Pending {
            action,
            guard: IntentGuard::capture(&file.session),
            policy_revision: file.output_policy_revision(),
            phase: if unsaved { Phase::Review } else { Phase::Ready },
        });
    }
    pub(super) fn phase(&self) -> Option<Phase> {
        self.pending.as_ref().map(|pending| pending.phase)
    }
    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }
    fn current(&mut self, file: &FileSession) -> Result<&mut Pending<T>, Error> {
        if !self.pending.as_ref().is_some_and(|pending| {
            pending.guard.is_current(&file.session)
                && pending.policy_revision == file.output_policy_revision()
        }) {
            self.cancel();
            return Err(Error::InvalidEdit(
                "File action no longer matches the source history".into(),
            ));
        }
        Ok(self.pending.as_mut().expect("validated pending action"))
    }
    /// Caller obtained an explicit Save choice in the review panel.
    pub(super) fn choose_save(&mut self, file: &FileSession) -> Result<(), Error> {
        let pending = self.current(file)?;
        if pending.phase != Phase::Review {
            return Err(Error::InvalidEdit(
                "File action is not awaiting review".into(),
            ));
        }
        pending.phase = Phase::Saving;
        Ok(())
    }
    /// Caller obtained an explicit Discard confirmation, distinct from Cancel.
    pub(super) fn confirm_discard(&mut self, file: &FileSession) -> Result<(), Error> {
        let pending = self.current(file)?;
        if pending.phase != Phase::Review {
            return Err(Error::InvalidEdit(
                "File action is not awaiting discard confirmation".into(),
            ));
        }
        pending.phase = Phase::Ready;
        Ok(())
    }
    pub(super) fn relocated(
        &mut self,
        file: &FileSession,
        receipt: &RelocationReceipt,
    ) -> Result<(), Error> {
        let result = (|| {
            let pending = self
                .pending
                .as_mut()
                .ok_or_else(|| Error::InvalidEdit("File action was cancelled".into()))?;
            if pending.phase != Phase::Saving {
                return Err(Error::InvalidEdit(
                    "File action is not awaiting save".into(),
                ));
            }
            if !receipt.accepts_output_policy_transition(
                pending.policy_revision,
                file.output_policy_revision(),
            ) {
                return Err(Error::InvalidEdit(
                    "File action output policy changed without its publication proof".into(),
                ));
            }
            pending.guard = pending.guard.rebind(&file.session, receipt)?;
            pending.policy_revision = file.output_policy_revision();
            Ok(())
        })();
        if result.is_err() {
            self.cancel();
        }
        result
    }
    pub(super) fn saved(
        &mut self,
        file: &FileSession,
        receipt: Option<&RelocationReceipt>,
    ) -> Result<(), Error> {
        let result = (|| {
            let pending = self
                .pending
                .as_mut()
                .ok_or_else(|| Error::InvalidEdit("File action was cancelled".into()))?;
            if pending.phase != Phase::Saving {
                return Err(Error::InvalidEdit(
                    "File action is not awaiting save".into(),
                ));
            }
            if let Some(receipt) = receipt {
                if !receipt.accepts_output_policy_transition(
                    pending.policy_revision,
                    file.output_policy_revision(),
                ) {
                    return Err(Error::InvalidEdit(
                        "File action output policy changed without its publication proof".into(),
                    ));
                }
                pending.guard = pending.guard.rebind(&file.session, receipt)?;
                pending.policy_revision = file.output_policy_revision();
            }
            if !pending.guard.is_current(&file.session)
                || pending.policy_revision != file.output_policy_revision()
            {
                return Err(Error::InvalidEdit(
                    "File action history changed while saving".into(),
                ));
            }
            if file.has_unsaved_document() {
                return Err(Error::InvalidEdit(
                    "File action still has unsaved content".into(),
                ));
            }
            pending.phase = Phase::Ready;
            Ok(())
        })();
        if result.is_err() {
            self.cancel();
        }
        result
    }
    /// Reclassify a ready action only after the same currentness checks as
    /// publication. A transform cannot bypass the source review/receipt chain.
    pub(super) fn map_ready(
        &mut self,
        file: &FileSession,
        transform: impl FnOnce(&mut T) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let result = (|| {
            let pending = self.current(file)?;
            if pending.phase != Phase::Ready {
                return Err(Error::InvalidEdit(
                    "File action is not ready for publication".into(),
                ));
            }
            transform(&mut pending.action)
        })();
        if result.is_err() {
            self.cancel();
        }
        result
    }
    /// All native Pack/job/pending-durability guards must still pass immediately
    /// before publication. False retains the intention for an explicit retry.
    pub(super) fn take_ready(
        &mut self,
        file: &FileSession,
        publication_allowed: bool,
    ) -> Result<Option<T>, Error> {
        if self.phase() != Some(Phase::Ready) {
            return Ok(None);
        }
        let pending = self.current(file)?;
        if pending.phase != Phase::Ready || !publication_allowed {
            return Ok(None);
        }
        Ok(self.pending.take().map(|pending| pending.action))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn file() -> (tempfile::TempDir, FileSession) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("source.ssc");
        std::fs::write(&path, b"#VERSION:0.83;#TITLE:Original;").unwrap();
        let file = FileSession::open(&path).unwrap();
        (dir, file)
    }
    #[test]
    fn clean_action_waits_for_publication_guards_and_is_consumed_once() {
        let (_root, file) = file();
        let mut controller = Controller::default();
        controller.begin("open", &file);
        assert_eq!(controller.phase(), Some(Phase::Ready));
        assert_eq!(controller.take_ready(&file, false).unwrap(), None);
        assert_eq!(controller.take_ready(&file, true).unwrap(), Some("open"));
        assert_eq!(controller.take_ready(&file, true).unwrap(), None);
    }
    #[test]
    fn cancelled_save_cannot_close_after_a_later_save() {
        let (_root, mut file) = file();
        file.session.set_text(None, "TITLE", "Changed").unwrap();
        let mut controller = Controller::default();
        controller.begin("close", &file);
        controller.choose_save(&file).unwrap();
        controller.cancel();
        assert!(controller.saved(&file, None).is_err());
        assert!(controller.phase().is_none());
    }
    #[test]
    fn edit_then_undo_revokes_even_a_confirmed_discard() {
        let (_root, mut file) = file();
        file.session.set_text(None, "TITLE", "Changed").unwrap();
        let mut controller = Controller::default();
        controller.begin("new", &file);
        controller.confirm_discard(&file).unwrap();
        file.session.set_text(None, "TITLE", "Intervening").unwrap();
        file.session.undo().unwrap();
        assert!(controller.take_ready(&file, true).is_err());
        assert!(controller.phase().is_none());
    }
    #[test]
    fn untouched_untitled_still_needs_explicit_review() {
        let root = tempfile::tempdir().unwrap();
        let file = FileSession::from_new(
            deadsync_editor::file_origin::NewSong::prepare(root.path(), 0.009).unwrap(),
        )
        .unwrap();
        let mut controller = Controller::default();
        controller.begin("close", &file);
        assert_eq!(controller.phase(), Some(Phase::Review));
        assert_eq!(controller.take_ready(&file, true).unwrap(), None);
        controller.confirm_discard(&file).unwrap();
        assert_eq!(controller.take_ready(&file, true).unwrap(), Some("close"));
    }
    #[test]
    #[cfg(unix)]
    fn exact_first_save_receipt_resumes_the_reviewed_action() {
        let root = tempfile::tempdir().unwrap();
        let mut file = FileSession::from_new(
            deadsync_editor::file_origin::NewSong::prepare(root.path(), 0.009).unwrap(),
        )
        .unwrap();
        let mut controller = Controller::default();
        controller.begin("close", &file);
        controller.choose_save(&file).unwrap();
        let review = file.prepare_save_as(&root.path().join("new.ssc")).unwrap();
        let receipt = review.relocation().clone();
        assert!(matches!(
            file.save_as(review).unwrap(),
            deadsync_editor::storage::SaveAsOutcome::Durable { .. }
        ));
        controller.saved(&file, Some(&receipt)).unwrap();
        assert_eq!(controller.take_ready(&file, true).unwrap(), Some("close"));
    }
    #[test]
    fn unsuccessful_save_does_not_leave_authority_for_an_unrelated_later_save() {
        let (_root, mut file) = file();
        file.session.set_text(None, "TITLE", "Changed").unwrap();
        let mut controller = Controller::default();
        controller.begin("close", &file);
        controller.choose_save(&file).unwrap();
        assert!(controller.saved(&file, None).is_err());
        assert!(controller.phase().is_none());
        assert_eq!(controller.take_ready(&file, true).unwrap(), None);
    }
    #[test]
    fn edit_then_undo_after_output_rejects_an_ordinary_save_receipt() {
        let (_root, mut file) = file();
        file.session.set_text(None, "TITLE", "Changed").unwrap();
        let mut controller = Controller::default();
        controller.begin("close", &file);
        controller.choose_save(&file).unwrap();
        file.session.set_text(None, "TITLE", "Intervening").unwrap();
        file.session.undo().unwrap();
        assert!(controller.saved(&file, None).is_err());
        assert!(controller.phase().is_none());
    }
    #[test]
    fn policy_change_without_matching_publication_revokes_a_file_action() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("source.ssc");
        std::fs::write(&path,"#VERSION:0.83;#TITLE:Original;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        let mut file = FileSession::open(&path).unwrap();
        file.session.set_text(None, "TITLE", "Changed").unwrap();
        let mut controller = Controller::default();
        controller.begin("close", &file);
        controller.choose_save(&file).unwrap();
        let policy = deadsync_editor::output_policy::OutputPolicy::Advanced {
            format: deadsync_editor::document::Format::Ssc,
            options: Default::default(),
            timing_chart: file.session.chart_ids()[0].clone(),
        };
        let review = file.prepare_save_with_policy(policy).unwrap();
        assert!(
            !review
                .issues()
                .iter()
                .any(|issue| issue.severity == deadsync_editor::advanced_export::Severity::Error)
        );
        let ack = review
            .issues()
            .iter()
            .enumerate()
            .filter(|(_, issue)| {
                issue.severity == deadsync_editor::advanced_export::Severity::Warning
            })
            .map(|(index, _)| index)
            .collect();
        assert!(matches!(
            file.save_reviewed(review, &ack).unwrap(),
            deadsync_editor::storage::SaveAttempt::Saved
        ));
        assert!(controller.saved(&file, None).is_err());
        assert!(controller.phase().is_none());
    }
}
