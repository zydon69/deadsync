use super::Preferences;
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
        f.debug_struct("NoteColorPreferences")
            .finish_non_exhaustive()
    }
}
impl Change {
    pub fn apply(&mut self, forward: bool) -> Result<Update, Error> {
        let (expected, target) = if forward {
            (&self.before, &self.after)
        } else {
            (&self.after, &self.before)
        };
        let mut preferences = Arc::clone(target);
        if let Some(previous) = &self.file {
            let mut file = File::open(previous.path())?;
            preferences = Arc::new(crate::note_color_inspector::merge::owned(
                &file.note_colors()?,
                expected,
                target,
            )?);
            file.set_note_colors(Arc::clone(&preferences))?;
            file.save()?;
            self.file = Some(file);
        }
        Ok(Update {
            preferences,
            file: self.file.clone(),
        })
    }
}
