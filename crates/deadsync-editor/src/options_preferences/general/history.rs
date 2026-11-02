//! Changed-leaf ownership for General Options, including source Restore/Undo.
use super::General;
use crate::{Error, audio_preferences::File};
pub struct Update {
    pub preferences: General,
    pub file: Option<File>,
}
pub(crate) struct Change {
    pub before: General,
    pub after: General,
    pub file: Option<File>,
}
impl std::fmt::Debug for Change {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GeneralOptions")
            .field("before", &self.before)
            .field("after", &self.after)
            .finish_non_exhaustive()
    }
}
impl Change {
    pub fn apply(&mut self, forward: bool, live: Option<&File>) -> Result<Update, Error> {
        let expected = if forward { &self.before } else { &self.after };
        let mut target = if forward {
            self.after.clone()
        } else {
            self.before.clone()
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
                file.general_options().write(&mut current)?;
                expected.write(&mut before)?;
                target.write(&mut after)?;
                crate::audio_preferences::publication::merge_fields(&mut current, &before, &after)?;
                target = General::read(&current)?;
                file.set_general_options(target.clone())?;

                Ok((target, file))
            },
        )?;
        Ok(Update {
            preferences: target,
            file: self.file.clone(),
        })
    }
}
