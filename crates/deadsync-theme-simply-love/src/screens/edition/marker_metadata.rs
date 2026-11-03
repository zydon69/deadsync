//! Pattern inspector metadata commits do not own navigation or transport.
use super::*;
impl State {
    pub(super) fn commit_marker(&mut self,delete:bool){
        let Some(draft)=self.marker.take()else{return;};
        let result=(|| {
            // Reuse the exact clock/audio receipt handoff, including a paused
            // or closed focused view; no row/time is reconstructed from start.
            let before=self.capture_migration_presentation()?;
            let application=self.workspace.apply_marker_metadata_with_presentation_receipt(&draft,delete)?;
            if let Some(receipt)=application.presentation_receipt(){self.rebind_migration_presentation(before,receipt)?;}
            Ok::<_,deadsync_editor::Error>(())
        })();
        self.marker=Some(draft);
        match result {
            Ok(())=>{self.marker=None;self.panel=None;self.refresh();self.status=i18n::tr("Edition",if delete{"MarkerDeleted"}else{"MarkerEdited"});},
            Err(error)=>self.status=i18n::tr_fmt("Edition","Error",&[("message",&i18n::editor_error(&error))]),
        }
    }
    pub(super) fn apply_marker_metadata_history(&mut self,redo:bool){
        let result=(|| {
            let before=self.capture_migration_presentation()?;
            let application=self.workspace.apply_marker_metadata_history(redo)?;
            if let Some(receipt)=application.presentation_receipt(){self.rebind_migration_presentation(before,receipt)?;}
            Ok::<_,deadsync_editor::Error>(())
        })();
        self.status=match result{Ok(())=>i18n::tr("Edition","Ready"),Err(error)=>i18n::tr_fmt("Edition","Error",&[("message",&i18n::editor_error(&error))])};
        self.refresh();
    }
}
