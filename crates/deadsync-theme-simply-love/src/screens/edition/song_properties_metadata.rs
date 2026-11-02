//! Song metadata history preserves the exact main presentation coordinate.
use super::*;
impl State {
    pub(super) fn apply_song_properties_metadata_history(&mut self, redo: bool) {
        let result = (|| {
            let before = self.capture_migration_presentation()?;
            let application = self
                .workspace
                .apply_song_properties_metadata_history(redo)?;
            if let Some(receipt) = application.presentation_receipt() {
                self.rebind_migration_presentation(before, receipt)?;
            }
            Ok::<_, deadsync_editor::Error>(())
        })();
        self.status = match result {
            Ok(()) => i18n::tr("Edition", "Ready"),
            Err(error) => i18n::tr_fmt(
                "Edition",
                "Error",
                &[("message", &i18n::editor_error(&error))],
            ),
        };
        // This runs the actual dedicated Preview registration and live sample-region
        // notifications, which are distinct from preserving Main's clock and asset.
        self.refresh();
    }
}
