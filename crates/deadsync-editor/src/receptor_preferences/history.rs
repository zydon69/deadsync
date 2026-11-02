//! Own only changed receptor flags when undoing across unrelated preference edits.
use super::Preferences;
use crate::{Error, audio_preferences::File};
pub struct Update {
    pub preferences: Preferences,
    pub file: Option<File>,
    pub placement_changed: [bool; 6],
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
                file.receptors().write(&mut current);
                expected.write(&mut before);
                target.write(&mut after);
                crate::audio_preferences::publication::merge_fields(&mut current, &before, &after)?;
                target = Preferences::read(&current)?;
                file.set_receptors(target);

                Ok((target, file))
            },
        )?;
        Ok(Update {
            preferences: target,
            file: self.file.clone(),
            placement_changed: [
                self.before.placement.x != self.after.placement.x,
                self.before.placement.y != self.after.placement.y,
                self.before.placement.center != self.after.placement.center,
                self.before.placement.lock_x != self.after.placement.lock_x,
                self.before.placement.lock_y != self.after.placement.lock_y,
                self.before.placement.allow_header != self.after.placement.allow_header,
            ],
        })
    }
}
