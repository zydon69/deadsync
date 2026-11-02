//! Application-only performance settings with changed-field conflict checks.
use super::preferences::Preferences;
use crate::{Error, audio_preferences::File};

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
        f.debug_struct("PerformancePreferences")
            .field("before", &self.before)
            .field("after", &self.after)
            .finish_non_exhaustive()
    }
}

impl Change {
    pub fn apply(&mut self, forward: bool, live: Option<&File>) -> Result<Update, Error> {
        let (expected, mut target) = if forward {
            (self.before, self.after)
        } else {
            (self.after, self.before)
        };
        let requested = target.clone();
        target = crate::audio_preferences::publication::publish(
            &mut self.file,
            live,
            requested.clone(),
            |current| {
                let mut file = current.clone();
                let mut target = requested.clone();

                let (mut current, mut before, mut after) = (
                    serde_json::Map::new(),
                    serde_json::Map::new(),
                    serde_json::Map::new(),
                );
                file.performance().write(&mut current)?;
                expected.write(&mut before)?;
                target.write(&mut after)?;
                crate::audio_preferences::publication::merge_fields(&mut current, &before, &after)?;
                target = Preferences::read(&current)?;
                file.set_performance(target)?;

                Ok((target, file))
            },
        )?;
        Ok(Update {
            preferences: target,
            file: self.file.clone(),
        })
    }
}
