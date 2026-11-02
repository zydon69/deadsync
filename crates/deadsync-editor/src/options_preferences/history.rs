//! Own only changed display_options flags when undoing across unrelated preference edits.
use super::Display;
use crate::{Error, audio_preferences::File};
pub struct Update {
    pub preferences: Display,
    pub file: Option<File>,
}
pub(crate) struct Change {
    pub before: Display,
    pub after: Display,
    pub file: Option<File>,
}
impl std::fmt::Debug for Change {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DisplayOptions")
            .field("before", &self.before)
            .field("after", &self.after)
            .finish_non_exhaustive()
    }
}
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

                let (mut current, mut before, mut after) = (
                    serde_json::Map::new(),
                    serde_json::Map::new(),
                    serde_json::Map::new(),
                );
                file.display_options().write(&mut current);
                expected.write(&mut before);
                target.write(&mut after);
                crate::audio_preferences::publication::merge_fields(&mut current, &before, &after)?;
                target = Display::read(&current)?;
                file.set_display_options(target);

                Ok((target, file))
            },
        )?;
        Ok(Update {
            preferences: target,
            file: self.file.clone(),
        })
    }
}
