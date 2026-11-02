//! History provenance minted only by the validated Song Properties Draft.
use super::{Change, Session};
use crate::Error;
fn invalid() -> Error {
    Error::InvalidEdit("History is not song properties metadata".into())
}
fn copy(change: &Change) -> Result<Change, Error> {
    Ok(match change {
        Change::Value {
            range,
            before,
            after,
        } => Change::Value {
            range: range.clone(),
            before: before.clone(),
            after: after.clone(),
        },
        Change::AddedProperty {
            chart,
            name,
            before,
            after,
        } => Change::AddedProperty {
            chart: *chart,
            name: name.clone(),
            before: before.clone(),
            after: after.clone(),
        },
        Change::SongTimingChoice { before, after } => Change::SongTimingChoice {
            before: before.clone(),
            after: after.clone(),
        },
        Change::SongTimingProperties(change) => {
            Change::SongTimingProperties(Box::new(copy(change)?))
        }
        Change::SongPropertiesMetadata(change) => {
            Change::SongPropertiesMetadata(Box::new(copy(change)?))
        }
        Change::Group(changes) => {
            Change::Group(changes.iter().map(copy).collect::<Result<_, _>>()?)
        }
        _ => return Err(invalid()),
    })
}
impl Session {
    pub(crate) fn wrap_song_properties_metadata(&mut self) {
        let entry = self
            .undo
            .last_mut()
            .expect("successful Song Properties commit");
        // Keep the pre-existing pure timing-choice producer and its public contract.
        if matches!(
            entry.change,
            Change::SongTimingProperties(_) | Change::SongPropertiesMetadata(_)
        ) {
            return;
        }
        let change = std::mem::replace(&mut entry.change, Change::Group(Vec::new()));
        entry.change = Change::SongPropertiesMetadata(Box::new(change));
    }
    pub fn history_preserves_song_properties_metadata(&self, redo: bool) -> bool {
        (if redo {
            self.redo.last()
        } else {
            self.undo.last()
        })
        .is_some_and(|entry| matches!(entry.change, Change::SongPropertiesMetadata(_)))
    }
    pub(super) fn validate_song_properties_metadata_history(
        &self,
        redo: bool,
    ) -> Result<(), Error> {
        let entry = (if redo {
            self.redo.last()
        } else {
            self.undo.last()
        })
        .filter(|entry| matches!(entry.change, Change::SongPropertiesMetadata(_)))
        .ok_or_else(invalid)?;
        let mut staged = super::timing_live::presentation_candidate(self)?;
        staged.apply(&mut copy(&entry.change)?, redo)?;
        // Defense in depth around the closed producer's unchanged main-clock proof.
        // Dedicated Preview and sample metadata intentionally remain changeable.
        if self.document.text(None, "MUSIC") != staged.document.text(None, "MUSIC")
            || self.document.raw_property(None, "OFFSET")
                != staged.document.raw_property(None, "OFFSET")
            || !crate::number::reference_equal(
                crate::song_properties::sync_offset(&self.document)?,
                crate::song_properties::sync_offset(&staged.document)?,
            )
            || self.chart_ids != staged.chart_ids
        {
            return Err(invalid());
        }
        Ok(())
    }
    pub fn apply_song_properties_metadata_history(
        &mut self,
        redo: bool,
    ) -> Result<crate::song_properties::Application, Error> {
        super::song_presentation::apply_song_metadata_history(self, redo)
    }
}
