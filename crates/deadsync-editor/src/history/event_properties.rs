//! Net-aware, property-only gestures. This deliberately does not change the
//! ordinary note/audio/profile transaction contract.
use super::{Change, Session};
use crate::Error;

impl Session {
    pub(crate) fn edit_event_properties_atomic(
        &mut self,
        chart: usize,
        label: &str,
        operation: impl FnOnce(&mut Session) -> Result<(), Error>,
    ) -> Result<bool, Error> {
        self.edit_chart_atomic(chart, label, |staged| {
            let mut before = Session::staged(staged.document.clone());
            before.grids = staged.grids.clone();
            operation(staged)?;
            fn properties_only(change: &Change) -> bool {
                match change {
                    Change::Value { .. } | Change::AddedProperty { .. } => true,
                    Change::Group(changes) => changes.iter().all(properties_only),
                    _ => false,
                }
            }
            if staged.undo.iter().any(|entry| !properties_only(&entry.change)) {
                return Err(Error::InvalidEdit("Event-property gestures cannot edit notes, participants, structure, files or preferences".into()));
            }
            if staged.document.serialize() != before.document.serialize() {
                return Ok(());
            }
            // Only loaded grids can contain resident state. Newly loaded caches
            // are compared against the original document plus its participant
            // metadata, rather than treated as changes because they were absent.
            for (&chart, grid) in &staged.grids {
                if before.grid(chart)? != grid { return Ok(()); }
            }
            // Each edit_chart_atomic closure starts with an empty history. These
            // are only this gesture's temporary entries; the live undo/redo and
            // revision token have not been touched.
            staged.undo.clear();
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        document::{Document, Format},
        history::Session,
        notes::{Note, Position},
    };
    #[test]
    fn property_only_helper_refuses_note_and_participant_changes_without_publishing() {
        for participants in [false, true] {
            let mut s=Session::new(Document::parse(b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:00000000;".as_slice(),Format::Ssc).unwrap());
            s.set_text(None, "TITLE", "Redo").unwrap();
            s.undo().unwrap();
            let players = s.grid(0).unwrap().players();
            let before = s.serialize().unwrap();
            let revision = s.revision();
            let redo = s.redo_label().unwrap().to_owned();
            let result =
                s.edit_event_properties_atomic(0, "Invalid property operation", |staged| {
                    if participants {
                        staged.set_participants(0, 3)?;
                    } else {
                        staged.edit_notes(
                            0,
                            "Note",
                            [(
                                Position {
                                    player: 0,
                                    lane: 0,
                                    row: 48,
                                },
                                Some(Note::tap()),
                            )],
                        )?;
                    }
                    Ok(())
                });
            assert!(result.is_err());
            assert_eq!(s.serialize().unwrap(), before);
            assert_eq!(s.revision(), revision);
            assert_eq!(s.redo_label(), Some(redo.as_str()));
            assert_eq!(s.grid(0).unwrap().players(), players);
            assert_eq!(s.grid(0).unwrap().iter().count(), 0);
        }
    }
}
