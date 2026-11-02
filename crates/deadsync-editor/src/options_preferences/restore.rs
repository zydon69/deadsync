//! Read-only projection across existing Options owners. No duplicate mutable state.
use super::{Display, General};
use crate::{Error, audio_preferences::File, minimap_palette::ColorMethod};
use serde_json::{Map, Value};
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub general: General,
    pub display: Display,
    pub recent_maximum: usize,
    pub step_color_method: ColorMethod,
    pub suppress_external_song: bool,
}
impl Snapshot {
    pub fn capture(file: &File) -> Self {
        Self {
            general: file.general_options().clone(),
            display: file.display_options(),
            recent_maximum: file.recent_songs().maximum,
            step_color_method: file.note_colors_arc().step_color_method,
            suppress_external_song: file.suppress_external_song_notifications(),
        }
    }
    /// Original Options.RestoreDefaults resets22 business fields, preserving layout
    /// and all four hidden budgets (three Display, one separately owned MiniMap).
    pub fn restored(&self) -> Self {
        Self {
            general: General::default(),
            display: self.display.restore_defaults(),
            recent_maximum: 20,
            step_color_method: ColorMethod::Stepmania,
            suppress_external_song: false,
        }
    }
    pub fn write(&self, fields: &mut Map<String, Value>) -> Result<(), Error> {
        self.general.write(fields)?;
        self.display.write(fields);
        fields.insert("RecentFilesHistorySize".into(), self.recent_maximum.into());
        fields.insert(
            "StepColorMethodValue".into(),
            self.step_color_method.source_name().into(),
        );
        fields.insert(
            "SuppressExternalSongModificationNotification".into(),
            self.suppress_external_song.into(),
        );
        Ok(())
    }
    pub fn install(&self, file: &mut File) -> Result<(), Error> {
        // All validation happens on a detached clone before the caller publishes it.
        file.set_general_options(self.general.clone())?;
        file.set_display_options(self.display);
        let mut recent = file.recent_songs().clone();
        recent.maximum = self.recent_maximum;
        file.set_recent_songs(recent)?;
        let mut colors = file.note_colors_arc().as_ref().clone();
        colors.step_color_method = self.step_color_method;
        file.set_note_colors(std::sync::Arc::new(colors))?;
        file.set_suppress_external_song_notifications(self.suppress_external_song);
        Ok(())
    }
    pub(crate) fn merged(
        current: &File,
        expected: &Self,
        target: &Self,
    ) -> Result<(Self, File), Error> {
        let (mut fields, mut before, mut after) = (Map::new(), Map::new(), Map::new());
        Self::capture(current).write(&mut fields)?;
        expected.write(&mut before)?;
        target.write(&mut after)?;
        crate::audio_preferences::publication::merge_fields(&mut fields, &before, &after)?;
        let recent_maximum = fields["RecentFilesHistorySize"]
            .as_u64()
            .and_then(|v| usize::try_from(v).ok())
            .ok_or_else(|| Error::InvalidFormat("Invalid RecentFilesHistorySize".into()))?;
        let merged = Self {
            general: General::read(&fields)?,
            display: Display::read(&fields)?,
            recent_maximum,
            step_color_method: ColorMethod::read_source(fields.get("StepColorMethodValue"))?,
            suppress_external_song: fields["SuppressExternalSongModificationNotification"]
                .as_bool()
                .ok_or_else(|| {
                    Error::InvalidFormat(
                        "Invalid SuppressExternalSongModificationNotification".into(),
                    )
                })?,
        };
        let mut file = current.clone();
        merged.install(&mut file)?;
        Ok((merged, file))
    }
}
pub struct Update {
    pub preferences: Snapshot,
    pub file: Option<File>,
}
pub(crate) struct Change {
    pub before: Snapshot,
    pub after: Snapshot,
    pub file: Option<File>,
}
impl std::fmt::Debug for Change {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RestoreOptions").finish_non_exhaustive()
    }
}
impl Change {
    pub fn apply(&mut self, forward: bool, live: Option<&File>) -> Result<Update, Error> {
        let (expected, target) = if forward {
            (&self.before, &self.after)
        } else {
            (&self.after, &self.before)
        };
        let mut preferences = target.clone();
        preferences = crate::audio_preferences::publication::publish(
            &mut self.file, live, preferences,
            |current| Snapshot::merged(current, expected, target),
        )?;
        Ok(Update {
            preferences,
            file: self.file.clone(),
        })
    }
}
