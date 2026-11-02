//! Song OFFSET/Sync notifications preserve row, including during main playback.
use super::*;
use deadsync_editor::{Error, song_properties::live::Plan};
impl State {
    /// Canonical boundary shared by F11, direct media open and automatic assets.
    /// Read-only/sample metadata retains its exact presentation receipt.
    pub(super) fn commit_song_properties_presented(
        &mut self,
        draft: &deadsync_editor::song_properties::Draft,
    ) -> Result<(), Error> {
        if !draft.requires_live_handoff() {
            let before=self.capture_migration_presentation()?;
            let application=self.workspace.apply_song_properties_with_presentation_receipt(draft)?;
            if let Some(receipt)=application.presentation_receipt(){self.rebind_migration_presentation(before,receipt)?;}
            return Ok(());
        }
        let before = if self.workspace.has_chart() {
            Some(self.capture_timing_presentation()?)
        } else {
            None
        };
        let focus = before
            .as_ref()
            .map(|before| (self.workspace.chart(), before.row()));
        let plan = Plan::prepare(draft, &self.workspace.file().session, focus)?;
        let application = self.workspace.apply_song_properties_live_plan(plan)?;
        self.finish_live_song_properties_application(before, application)
    }
    fn run_live_song_properties_history(&mut self,redo:bool)->Result<(),Error>{
        let before=if self.workspace.has_chart(){Some(self.capture_timing_presentation()?)}else{None};
        let focus=before.as_ref().map(|before|(self.workspace.chart(),before.row()));
        let plan=Plan::prepare_history(&self.workspace.file().session,focus,redo)?;
        let application = self.workspace.apply_song_properties_live_plan(plan)?;
        self.finish_live_song_properties_application(before, application)
    }
    fn finish_live_song_properties_application(
        &mut self,
        before: Option<playback::TimingPresentation>,
        application: deadsync_editor::song_properties::live::Application,
    ) -> Result<(), Error> {
        if application.changed() {
            if application.music_changed() {
                self.song_main_music_notification();
            }
            match (before, application.into_position()) {
                (Some(before), Some(position)) => {
                    self.finish_timing_presentation(before, position)?
                }
                (None, None) => self.rebind_chartless_current_audio(),
                _ => unreachable!("closed Song Properties plan preserves focus presence"),
            }
        }
        Ok(())
    }
    pub(super) fn apply_presented_song_properties(&mut self){
        let Some(draft)=self.song_properties.take()else{return;};
        match self.commit_song_properties_presented(&draft){
            Ok(())=>{self.panel=None;self.refresh();self.status=i18n::tr("Edition","SongPropertiesApplied");},
            Err(error)=>{self.song_properties=Some(draft);self.status=i18n::tr_fmt("Edition","Error",&[("message",&i18n::editor_error(&error))]);},
        }
    }
    pub(super) fn apply_live_song_properties_history(&mut self, redo: bool) {
        let result = self.run_live_song_properties_history(redo);
        self.status = match result {
            Ok(()) => i18n::tr("Edition", "Ready"),
            Err(error) => i18n::editor_error(&error).into(),
        };
        self.refresh();
    }
}
