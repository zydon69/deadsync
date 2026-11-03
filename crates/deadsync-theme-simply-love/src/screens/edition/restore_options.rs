//! One original Options Restore action over the existing application owners.
use super::*;
use deadsync_editor::options_preferences::restore::Snapshot;
impl State {
    pub fn options_snapshot(&self) -> Snapshot {
        Snapshot {
            general: self.general_options.clone(),
            display: self.display_options,
            recent_maximum: self.file_menu.recent_maximum(),
            step_color_method: self.note_colors.step_color_method,
            suppress_external_song: self.application_preferences_owner()
                .is_some_and(|file| file.suppress_external_song_notifications()),
        }
    }
    pub(super) fn sync_restore_options_history(&mut self) -> Result<(), deadsync_editor::Error> {
        if let Some(update) = self.workspace.take_restore_options_update() {
            let mut colors = update
                .file
                .as_ref()
                .map(|file| file.note_colors_arc())
                .unwrap_or_else(|| Arc::clone(&self.note_colors));
            Arc::make_mut(&mut colors).step_color_method = update.preferences.step_color_method;
            // File parsing and the core action validated the unchanged palette before publication.
            self.configure_note_color_arc(colors)?;
            if let Some(memory)=self.source_preferences_import.memory.as_mut(){memory.set_suppress_external_song_notifications(update.preferences.suppress_external_song);}
            self.general_options = update.preferences.general;
            self.configure_display_options(update.preferences.display);
            self.file_menu.set_recent_maximum(update.preferences.recent_maximum);
            if let Some(file) = update.file {
                // This action owns the limit, not RecentFiles. Preserve the live
                // record list independently from the persisted disk baseline.
                self.publish_application_preferences_owner(file);
            }
        }
        Ok(())
    }
    pub(super) fn restore_all_options(&mut self) {
        let before = self.options_snapshot();
        let result = self.capture_application_preferences_file().and_then(|file|
            self.workspace.restore_options(before, Some(&file)))
            .and_then(|_| self.sync_restore_options_history());
        match result {
            Ok(()) => {
                self.status = i18n::tr("Edition", "GeneralAllDefaultsSaved");
            }
            Err(error) => {
                self.status = i18n::tr_fmt(
                    "Edition",
                    "GeneralOptionsSaveFailedReason",
                    &[("error", &crate::i18n::editor_error(&error))],
                )
            }
        }
    }
}
