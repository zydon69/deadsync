//! History provenance for a migration proven to retain every editor clock.
use super::{Change, Session};
use crate::Error;
use std::sync::Arc;
impl Session {
    pub(crate) fn set_timing_migration_properties(
        &mut self,
        properties: &[(Option<usize>, String, Arc<[u8]>)],
    ) -> Result<bool, Error> {
        let changed = self.set_property_bytes_atomic(properties, "Activate SSC chart timing")?;
        if changed {
            let entry = self
                .undo
                .last_mut()
                .expect("a committed migration has one history entry");
            let change = std::mem::replace(&mut entry.change, Change::Group(Vec::new()));
            entry.change = Change::TimingMigration(Box::new(change));
        }
        Ok(changed)
    }
    pub fn history_preserves_timing_migration(&self, redo: bool) -> bool {
        (if redo {
            self.redo.last()
        } else {
            self.undo.last()
        })
        .is_some_and(|entry| matches!(entry.change, Change::TimingMigration(_)))
    }
    pub fn apply_timing_migration_history(
        &mut self,
        redo: bool,
    ) -> Result<crate::song_properties::Application, Error> {
        crate::history::song_presentation::apply_timing_migration_history(self, redo)
    }
}
