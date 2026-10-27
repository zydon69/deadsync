//! Source snap-level cycling with legacy index compatibility and shared settings.
use super::*;
impl State {
    pub(super) fn cycle_snap(&mut self, later: bool) {
        let result = (|| -> Result<(), deadsync_editor::Error> {
            let current = deadsync_editor::snap::Level::native(self.workspace.snap())?;
            let lock = self
                .audio_preferences_file
                .as_ref()
                .map(|f| f.snap().lock)
                .unwrap_or_default();
            let level = current.cycle(later, lock);
            // Save the preference before changing runtime state; conflicts leave
            // both the current subdivision and precise cursor untouched.
            let mut file = self.audio_preferences_file.clone();
            if let Some(file) = &mut file {
                file.set_snap(deadsync_editor::snap::Preferences { level, lock });
                file.save()?;
            }
            self.command(Command::SetSnap(level.native_index()));
            self.audio_preferences_file = file;
            Ok(())
        })();
        if let Err(error) = result {
            self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())]);
        }
    }
}
