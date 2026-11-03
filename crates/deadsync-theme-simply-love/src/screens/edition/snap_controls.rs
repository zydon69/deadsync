//! Source snap-level cycling with legacy index compatibility and shared settings.
use super::*;
impl State {
    pub(super) fn cycle_snap(&mut self, later: bool) {
        if self.audio_output_busy() {
            self.status = i18n::tr("Edition", "AudioOutputBusy");
            return;
        }
        let result = (|| -> Result<(), deadsync_editor::Error> {
            self.flush_wheel_settings()?;
            let current = deadsync_editor::snap::Level::native(self.workspace.snap())?;
            let lock = self
                .application_preferences_owner()
                .map(|f| f.snap().lock)
                .unwrap_or_default();
            let level = current.cycle(later, lock);
            // Save the preference before changing runtime state; conflicts leave
            // both the current subdivision and precise cursor untouched.
            let current_file = self.capture_application_preferences_file()?;
            let mut owner = Some(current_file.clone());
            deadsync_editor::audio_preferences::publication::publish(
                &mut owner,
                Some(&current_file),
                (),
                |current_file| {
                    let mut file = current_file.clone();
                    let mut snap = file.snap();
                    if snap.level != current {
                        return Err(deadsync_editor::Error::ExternalChange);
                    }
                    snap.level = level;
                    file.set_snap(snap);
                    Ok(((), file))
                },
            )?;
            // SetSnap is explicitly supported by the chartless Workspace. The
            // song-only UI command gate must not refuse this preference update.
            self.workspace
                .execute(Command::SetSnap(level.native_index()))?;
            if let Some(file) = owner {
                self.publish_application_preferences_owner(file);
            }
            self.refresh();
            Ok(())
        })();
        if let Err(error) = result {
            self.status = i18n::tr_fmt(
                "Edition",
                "Error",
                &[("message", &crate::i18n::editor_error(&error))],
            );
        }
    }
}

impl State {
    /// Original DrawRowSnapLockLevels reconciliation; no cursor/audio action.
    pub(super) fn set_hotbar_snap_lock(
        &mut self,
        lock: deadsync_editor::snap::Level,
    ) -> Result<bool, deadsync_editor::Error> {
        if self.audio_output_busy() {
            self.status = i18n::tr("Edition", "AudioOutputBusy");
            return Ok(false);
        }
        self.flush_wheel_settings()?;
        let level = deadsync_editor::snap::Level::native(self.workspace.snap())?;
        let current = self.capture_application_preferences_file()?;
        let before = current.snap();
        if before.lock == lock {
            return Ok(false);
        }
        let mut next = level;
        if !next.allowed(lock) {
            next = next.cycle(true, lock);
            if next.source_index() == 0 && lock.source_index() != 0 {
                next = lock;
            }
        }
        if before.level != level {
            return Err(deadsync_editor::Error::ExternalChange);
        }
        let mut owner = Some(current.clone());
        deadsync_editor::audio_preferences::publication::publish(
            &mut owner,
            Some(&current),
            (),
            |current| {
                if current.snap() != before {
                    return Err(deadsync_editor::Error::ExternalChange);
                }
                let mut file = current.clone();
                file.set_snap(deadsync_editor::snap::Preferences { level: next, lock });
                Ok(((), file))
            },
        )?;
        self.workspace
            .execute(Command::SetSnap(next.native_index()))?;
        if let Some(file) = owner {
            self.publish_application_preferences_owner(file);
        }
        self.refresh();
        Ok(true)
    }
}
