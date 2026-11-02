use super::*;
impl FileSession {
    pub(super) fn sync_authoring_policy(&mut self) -> Result<(), Error> {
        if let Some(policy) = self.session.pending_authoring_policy() {
            let revision = self.next_policy_revision(policy)?;
            let policy = self
                .session
                .take_authoring_policy()
                .expect("inspected policy");
            self.output_policy = policy;
            self.policy_revision = revision;
        }
        Ok(())
    }
    pub fn prepare_sm_authoring(&mut self) -> Result<crate::history::authoring::Plan, Error> {
        self.sync_authoring_policy()?;
        if self.has_external_change()? {
            return Err(Error::ExternalChange);
        }
        crate::history::authoring::Plan::prepare(&mut self.session)
    }
    pub fn apply_sm_authoring(
        &mut self,
        plan: crate::history::authoring::Plan,
        options: crate::advanced_export::Options,
    ) -> Result<crate::song_properties::Application, Error> {
        self.sync_authoring_policy()?;
        if self.has_external_change()? {
            return Err(Error::ExternalChange);
        }
        let before = self.output_policy.clone();
        let after = match &before {
            crate::output_policy::OutputPolicy::NativeLossless => {
                crate::output_policy::OutputPolicy::AdvancedSongTiming {
                    format: Format::Sm,
                    options,
                }
            }
            _ => before.clone(),
        };
        let revision = self.next_policy_revision(&after)?;
        let application = plan.apply(&mut self.session, before, after.clone())?;
        self.session.take_authoring_policy();
        self.output_policy = after;
        self.policy_revision = revision;
        Ok(application)
    }
    pub(crate) fn authoring_history_with_effect(
        &mut self,
        redo: bool,
    ) -> Result<crate::history::HistoryEffect, Error> {
        self.sync_authoring_policy()?;
        if let Some(policy) = self.session.next_authoring_policy(redo) {
            self.next_policy_revision(policy)?;
        }
        let effect = if redo {
            self.session.redo_with_effect()?
        } else {
            self.session.undo_with_effect()?
        };
        self.sync_authoring_policy()?;
        Ok(effect)
    }
}
