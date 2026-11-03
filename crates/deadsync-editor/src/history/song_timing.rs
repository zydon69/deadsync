//! Resident song timing ownership. Lazy initialization never conflates null with
//! a choice not yet resolved from the immutable open context.
use super::*;
use crate::{chart_order::Snapshot, song_timing_chart::Choice};
impl Session {
    fn initial_song_timing_choice(&self) -> Result<Choice, Error> {
        let mut temporary = Self::staged(self.document.reparse(self.serialize()?)?);
        temporary.chart_ids = self.chart_ids.clone();
        let snapshot = Snapshot::loadable(&mut temporary)?.snapshot;
        let preferences =
            crate::open_preferences::Preferences::from_load_context(self.document.load_context());
        let fields = crate::song_properties::timing_custom_fields(&self.document);
        Choice::load(&snapshot, fields.as_ref(), &preferences)
    }
    pub fn song_timing_choice(&self) -> Result<Choice, Error> {
        self.song_timing
            .clone()
            .map_or_else(|| self.initial_song_timing_choice(), Ok)
    }
    pub(crate) fn ensure_song_timing_choice(&mut self) -> Result<(), Error> {
        if self.song_timing.is_none() {
            self.song_timing = Some(self.initial_song_timing_choice()?);
        }
        Ok(())
    }
    pub fn song_timing_chart(&self) -> Result<Option<usize>, Error> {
        let choice = self.song_timing_choice()?;
        choice
            .identity()
            .map(|id| {
                self.chart_ids
                    .iter()
                    .position(|candidate| candidate == id)
                    .ok_or_else(|| {
                        Error::InvalidEdit(
                            "Song timing chart no longer belongs to this song".into(),
                        )
                    })
            })
            .transpose()
    }
    pub(crate) fn install_song_timing_choice(&mut self, choice: Choice) -> Result<(), Error> {
        let snapshot = Snapshot::loadable(self)?.snapshot;
        if choice
            .identity()
            .is_some_and(|id| !snapshot.keys().iter().any(|key| &key.guid == id))
        {
            return Err(Error::InvalidFormat(
                "Recovered song timing chart is not loadable".into(),
            ));
        }
        self.song_timing = Some(choice);
        Ok(())
    }
    pub(crate) fn set_song_timing_choice(&mut self, choice: Choice) -> Result<bool, Error> {
        self.ensure_song_timing_choice()?;
        if self.song_timing.as_ref() == Some(&choice) {
            return Ok(false);
        }
        let snapshot = Snapshot::loadable(self)?.snapshot;
        if choice
            .identity()
            .is_some_and(|id| !snapshot.keys().iter().any(|key| &key.guid == id))
        {
            return Err(Error::InvalidEdit(
                "Song timing chart is not loadable".into(),
            ));
        }
        // Value changes retain both directions, including when their staged history
        // is composed into the live Song Properties transaction.
        let before = self
            .song_timing
            .as_ref()
            .expect("choice initialized above")
            .clone();
        self.execute(
            "Change song timing chart".into(),
            Change::SongTimingChoice {
                before,
                after: choice,
            },
        )?;
        Ok(true)
    }
}
impl Session {
    pub fn song_timing_snapshot(&self) -> Result<Snapshot, Error> {
        let mut temporary = Self::staged(self.document.reparse(self.serialize()?)?);
        temporary.chart_ids = self.chart_ids.clone();
        Ok(Snapshot::loadable(&mut temporary)?.snapshot)
    }
    pub(crate) fn set_song_properties_and_timing(
        &mut self,
        properties: &[(Option<usize>, String, Arc<[u8]>)],
        choice: Choice,
        label: &str,
    ) -> Result<bool, Error> {
        self.ensure_song_timing_choice()?;
        let mut temporary = Self::staged(self.document.clone());
        temporary.chart_ids = self.chart_ids.clone();
        temporary.song_timing = self.song_timing.clone();
        temporary.set_property_bytes_atomic(properties, label)?;
        let choice_changed = temporary.set_song_timing_choice(choice)?;
        let metadata_keys = [
            crate::song_properties::custom_property_key(&self.document, "GASongData"),
            crate::song_properties::custom_property_key(&self.document, "GASongDataVersion"),
        ];
        let pure = choice_changed
            && properties.iter().all(|(scope, name, _)| {
                scope.is_none()
                    && metadata_keys
                        .iter()
                        .any(|key| key.eq_ignore_ascii_case(name))
            })
            && crate::number::reference_equal(
                crate::song_properties::sync_offset(&self.document)?,
                crate::song_properties::sync_offset(&temporary.document)?,
            );
        let changes = temporary
            .undo
            .into_iter()
            .map(|entry| entry.change)
            .collect::<Vec<_>>();
        if changes.is_empty() {
            return Ok(false);
        }
        let change = Change::Group(changes);
        self.execute(
            label.into(),
            if pure {
                Change::SongTimingProperties(Box::new(change))
            } else {
                change
            },
        )?;
        Ok(true)
    }
}
impl Session {
    pub(crate) fn materialize_song_timing_state(
        mut document: Document,
        identities: &[crate::generation::metadata::Guid],
        choice: &Choice,
    ) -> Result<Document, Error> {
        let mut staged = Self::staged(document.clone());
        staged.chart_ids = identities.to_vec();
        let snapshot = Snapshot::loadable(&mut staged)?.snapshot;
        let fields = choice.output_fields(&snapshot, snapshot.source_indices())?;
        crate::song_properties::materialize_output_context(
            &mut document,
            fields,
            crate::song_properties::OutputContext::NativeLossless,
        )?;
        Ok(document)
    }
}

impl Session {
    pub fn song_timing_history_preserves_presentation(&self, redo: bool) -> bool {
        let entry = if redo {
            self.redo.last()
        } else {
            self.undo.last()
        };
        entry.is_some_and(|entry| matches!(entry.change, Change::SongTimingProperties(_)))
    }
    pub fn apply_song_timing_history(
        &mut self,
        redo: bool,
    ) -> Result<crate::song_properties::Application, Error> {
        crate::history::song_presentation::apply_timing_history(self, redo)
    }
}
impl Session {
    pub(crate) fn initialize_song_timing_choice_checked(
        &mut self,
        cancelled: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<(), Error> {
        if self.song_timing.is_some() {
            return Ok(());
        }
        let snapshot = match cancelled {
            Some(cancelled) => Snapshot::loadable_checked(self, cancelled)?.snapshot,
            None => Snapshot::loadable(self)?.snapshot,
        };
        let preferences =
            crate::open_preferences::Preferences::from_load_context(self.document.load_context());
        let fields = crate::song_properties::timing_custom_fields(&self.document);
        self.song_timing = Some(Choice::load(&snapshot, fields.as_ref(), &preferences)?);
        Ok(())
    }
}
