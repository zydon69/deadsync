//! Explicit review before legacy SSC timing becomes active in gameplay.
use super::*;
impl State {
    pub(in crate::screens::edition) fn current_timing_for_generation(&mut self) -> bool {
        if deadsync_editor::timing_edit::migration::has_legacy_chart_timing(
            self.workspace.file().session.document(),
        ) {
            self.open_timing_migration();
            false
        } else {
            true
        }
    }
    pub(in crate::screens::edition) fn open_timing_migration(&mut self) {
        match self.workspace.timing_migration() {
            Ok(draft) => {
                self.timing_migration = Some(draft);
                self.panel = Some(Panel::TimingMigration(0));
                self.status = i18n::tr("Edition", "LegacyTimingHelp");
            }
            Err(error) => self.timing_error(error),
        }
    }
    pub(in crate::screens::edition) fn timing_migration_key(
        &mut self,
        index: usize,
        code: KeyCode,
    ) {
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
            self.panel = Some(Panel::TimingMigration(1 - index));
        } else if matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) {
            if index == 1 {
                self.timing_migration = None;
                self.panel = None;
                self.status = i18n::tr("Edition", "LegacyTimingDiscarded");
                return;
            }
            let Some(draft) = &self.timing_migration else {
                return;
            };
            match self.workspace.apply_timing_migration(draft) {
                Ok(_) => {
                    self.timing_migration = None;
                    self.panel = None;
                    self.refresh();
                    self.status = i18n::tr("Edition", "LegacyTimingApplied");
                }
                Err(error) => self.timing_error(error),
            }
        }
    }
    pub(in crate::screens::edition) fn timing_migration_summary(&self) -> Arc<str> {
        let Some(draft) = &self.timing_migration else {
            return i18n::tr("Edition", "LegacyTimingHelp");
        };
        let missing = i18n::tr("Edition", "LegacyTimingMissingVersion");
        i18n::tr_fmt(
            "Edition",
            "LegacyTimingSummary",
            &[
                ("old", draft.previous_version().unwrap_or(&missing)),
                ("version", draft.target_version()),
                (
                    "local",
                    &draft
                        .charts()
                        .iter()
                        .filter(|chart| chart.previously_local)
                        .count()
                        .to_string(),
                ),
                (
                    "tempos",
                    &draft
                        .charts()
                        .iter()
                        .filter(|chart| chart.materializes_tempo)
                        .count()
                        .to_string(),
                ),
            ],
        )
    }
}
