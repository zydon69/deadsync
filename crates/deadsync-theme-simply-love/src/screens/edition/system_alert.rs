//! Source UIModals alerts are host intentions, never actor/audio commands.
use super::*;
use deadlib_platform::system_alert::SystemAlert;
use deadsync_editor::key_binding_preferences::Action;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Modal {
    Update,
    ExternalSong,
    ExternalPack,
    DeletePack,
    Unsaved,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Refusal {
    EditBusy,
    FileIoBusy,
    SaveFailure,
}
#[derive(Default)]
pub(super) struct Runtime {
    current: Option<Modal>,
    announced: bool,
    refusal: Option<Refusal>,
}
impl Runtime {
    fn observe(&mut self, current: Option<Modal>) {
        if self.current != current {
            self.current = current;
            self.announced = false;
        }
    }
    fn take(&mut self) -> Option<SystemAlert> {
        if self.current.is_some() && !self.announced {
            self.announced = true;
            // One sound suffices when a refusal also produced this modal.
            self.refusal = None;
            return Some(SystemAlert::Exclamation);
        }
        self.refusal.take().map(|_| SystemAlert::Exclamation)
    }
}
/// A closed list of source actions guarded by EditEarlyOut, distinct from
/// preference/view/navigation panels. Keyboard auto-repeat is filtered upstream.
pub(super) fn blocked_action(action: Action) -> Option<Refusal> {
    if matches!(
        action,
        Action::FileNew
            | Action::FileOpen
            | Action::FileReload
            | Action::FileRecent
            | Action::CloseEditor
            | Action::Close
            | Action::Exit
            | Action::Save
            | Action::SaveAs
    ) {
        return Some(Refusal::FileIoBusy);
    }
    if action.is_lane_entry()
        || action.is_selected_review()
        || action.is_source_pattern_action()
        || matches!(
            action,
            Action::Undo
                | Action::Redo
                | Action::Cut
                | Action::Paste
                | Action::Delete
                | Action::CutEvents
                | Action::PasteEvents
                | Action::NewChart
                | Action::DeleteChart
                | Action::DuplicateChart
                | Action::RegeneratePattern
                | Action::ShiftAllEarlier
                | Action::ShiftAllLater
                | Action::AddEventTempo
                | Action::AddEventInterpolatedScrollRate
                | Action::AddEventScrollRate
                | Action::AddEventStop
                | Action::AddEventDelay
                | Action::AddEventWarp
                | Action::AddEventFakeRegion
                | Action::AddEventTicks
                | Action::AddEventComboMultipliers
                | Action::AddEventTimeSignature
                | Action::AddEventLabel
                | Action::AddEventAttack
                | Action::AddEventPattern
                | Action::AutoApplyAllSongAssets
                | Action::AutoApplyUnsetSongAssets
        )
    {
        return Some(Refusal::EditBusy);
    }
    None
}
impl State {
    pub(super) fn queue_system_alert(&mut self, reason: Refusal) {
        self.system_alert.refusal = Some(reason);
    }
    fn system_alert_modal(&self) -> Option<Modal> {
        match self.panel {
            Some(Panel::UpdateNotice(_)) => Some(Modal::Update),
            Some(Panel::ExternalFile(_)) => Some(Modal::ExternalSong),
            Some(Panel::FileUnsaved(_)) => Some(Modal::Unsaved),
            Some(Panel::PackProperties) => self.pack_system_alert_modal(),
            _ => None,
        }
    }
    pub(super) fn system_alert_modal_transition(&mut self) {
        let modal = self.system_alert_modal();
        self.system_alert.observe(modal);
    }
    /// Only the active published host consumes this, after admitting/presenting
    /// the panel. Pending modals behind a different panel produce no sound.
    pub fn take_editor_system_alert(&mut self) -> Option<SystemAlert> {
        self.system_alert_modal_transition();
        self.system_alert.take()
    }
    pub(super) fn system_alert_edit_busy(&self) -> bool {
        self.file_io_active()
            || self.pack_file_io_active()
            || self.save_as_pending()
            || self.job.is_some()
            || self.auto_recovery.is_pending()
    }
}
#[cfg(test)]
mod tests;
