//! Undo owns only changed selection preference fields.
use super::Preferences;
use crate::{Error, audio_preferences::File};
use serde_json::Map;
pub struct Update {
    pub preferences: Preferences,
    pub file: Option<File>,
}
pub(crate) struct Change {
    pub before: Preferences,
    pub after: Preferences,
    pub file: Option<File>,
}
impl std::fmt::Debug for Change {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SelectionPreferences")
            .field("before", &self.before)
            .field("after", &self.after)
            .finish_non_exhaustive()
    }
}
use crate::audio_preferences::publication::merge_fields as merge;

impl Change {
    pub fn apply(&mut self, forward: bool, live: Option<&File>) -> Result<Update, Error> {
        let expected = if forward { self.before } else { self.after };
        let mut target = if forward { self.after } else { self.before };
        let requested = target.clone();
        target = crate::audio_preferences::publication::publish(
            &mut self.file,
            live,
            requested.clone(),
            |current| {
                let mut file = current.clone();
                let mut target = requested.clone();

                let (mut current, mut before, mut after) = (Map::new(), Map::new(), Map::new());
                file.selection().write(&mut current)?;
                expected.write(&mut before)?;
                target.write(&mut after)?;
                merge(&mut current, &before, &after)?;
                target = Preferences::read(&current)?;
                file.set_selection(target)?;

                Ok((target, file))
            },
        )?;
        Ok(Update {
            preferences: target,
            file: self.file.clone(),
        })
    }
}
