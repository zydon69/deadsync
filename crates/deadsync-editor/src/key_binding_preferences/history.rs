//! Global binding changes do not edit document history content or replay input.
use super::{Preferences, inspector};
use crate::{Error, audio_preferences::File};
use std::sync::Arc;
pub struct Update {
    pub preferences: Arc<Preferences>,
    pub file: Option<File>,
}
pub(crate) struct Change {
    pub before: Arc<Preferences>,
    pub after: Arc<Preferences>,
    pub file: Option<File>,
}
impl std::fmt::Debug for Change {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyBindingPreferences")
            .field("before", &self.before)
            .field("after", &self.after)
            .finish_non_exhaustive()
    }
}
impl Change {
    pub fn apply(&mut self, forward: bool, live: Option<&File>) -> Result<Update, Error> {
        let (expected, target) = if forward {
            (&self.before, &self.after)
        } else {
            (&self.after, &self.before)
        };
        let preferences = crate::audio_preferences::publication::publish(
            &mut self.file,
            live,
            Arc::clone(target),
            |current| {
                let mut file = current.clone();
                let preferences =
                    Arc::new(inspector::merge(&file.key_bindings(), expected, target)?);
                file.set_key_bindings(Arc::clone(&preferences))?;

                Ok((preferences, file))
            },
        )?;
        Ok(Update {
            preferences,
            file: self.file.clone(),
        })
    }
}
