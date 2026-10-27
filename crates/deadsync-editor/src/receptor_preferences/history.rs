//! Own only changed receptor flags when undoing across unrelated preference edits.
use super::Preferences;
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
        f.debug_struct("ReceptorPreferences")
            .field("before", &self.before)
            .field("after", &self.after)
            .finish_non_exhaustive()
    }
}
impl Change {
    pub fn apply(&mut self, forward: bool) -> Result<Update, Error> {
        let expected = if forward { self.before } else { self.after };
        let mut target = if forward { self.after } else { self.before };
        if let Some(previous) = &self.file {
            let mut file = File::open(previous.path())?;
            let (mut current, mut before, mut after) = (
                serde_json::Map::new(),
                serde_json::Map::new(),
                serde_json::Map::new(),
            );
            file.receptors().write(&mut current);
            expected.write(&mut before);
            target.write(&mut after);
            for (key, value) in &after {
                if before.get(key) == Some(value) {
                    continue;
                }
                if current.get(key) != before.get(key) {
                    return Err(Error::ExternalChange);
                }
                current.insert(key.clone(), value.clone());
            }
            target = Preferences::read(&current)?;
            file.set_receptors(target);
            file.save()?;
            self.file = Some(file);
        }
        Ok(Update {
            preferences: target,
            file: self.file.clone(),
        })
    }
}
