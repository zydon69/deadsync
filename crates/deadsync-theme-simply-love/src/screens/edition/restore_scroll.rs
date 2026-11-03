//! One source Scroll Restore over the existing preference owners.
use super::*;
use deadsync_editor::{scroll_preferences::restore::Snapshot, zoom::SpacingMode};
impl State {
    pub fn scroll_snapshot(&self) -> Snapshot {
        Snapshot {
            scroll: self.scroll_preferences,
            zoom: self.zoom_preferences,
            spacing: self.spacing_preference(),
            waveform_mode: self.waveform_preferences.scroll_mode(),
        }
    }
    pub(super) fn sync_restore_scroll_history(&mut self) -> Result<(), deadsync_editor::Error> {
        if let Some(update) = self.workspace.take_restore_scroll_update() {
            // Prepare every fallible numerical application before publishing UI caches.
            let p = update.preferences;
            let mut zoom = self.zoom.clone();
            p.zoom.apply(&mut zoom)?;
            for (index, mode) in [
                SpacingMode::ConstantTime,
                SpacingMode::ConstantRow,
                SpacingMode::Variable,
            ]
            .into_iter()
            .enumerate()
            {
                let speed=p.scroll.speed(index).expect("three source speeds");
                if self.scroll_preferences.speed(index)!=Some(speed){zoom.set_spacing(mode,speed)?;}
            }
            let mut waveform = update
                .file
                .as_ref()
                .map(|file| file.waveform())
                .unwrap_or(self.waveform_preferences);
            waveform.set_scroll_mode(p.waveform_mode);
            self.zoom = zoom;
            self.zoom_preferences = p.zoom;
            self.scroll_preferences = p.scroll;
            self.reverse_display = p.scroll.reverse();
            self.configure_spacing_preference(p.spacing);
            if self.waveform_preferences!=waveform{self.waveform_preferences=waveform;self.apply_waveform_preferences();}
            if let Some(file) = update.file {
                self.publish_application_preferences_owner(file);
            }
            self.scroll_preferences_draft = None;
            self.zoom_preferences_draft = None;
            self.zoom_preferences_edit=None;
            self.waveform_preferences_draft = None;
        }
        Ok(())
    }
    pub(super) fn restore_source_scroll(&mut self) {
        let before = self.scroll_snapshot();
        let result = self.capture_application_preferences_file().and_then(|file|
            self.workspace.restore_scroll(before, Some(&file)))
            .and_then(|changed| {
                self.sync_restore_scroll_history()?;
                Ok(changed)
            });
        self.status = match result {
            Ok(_) => i18n::tr("Edition", "ScrollAllDefaultsSaved"),
            Err(error) => i18n::tr_fmt(
                "Edition",
                "ScrollRestoreFailedReason",
                &[("error", &i18n::editor_error(&error))],
            ),
        };
    }
}
