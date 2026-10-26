use super::{Audio, File};
use crate::Error;
/// A committed application-preference effect for the native transport/UI.
pub struct Update {
    pub audio: Audio,
    pub file: Option<File>,
}
pub(crate) struct Change {
    pub before: Audio,
    pub after: Audio,
    pub file: Option<File>,
}
impl std::fmt::Debug for Change {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioPreferences")
            .field("before", &self.before)
            .field("after", &self.after)
            .finish_non_exhaustive()
    }
}
impl Change {
    pub fn apply(&mut self, forward: bool) -> Result<Update, Error> {
        let expected = if forward { self.before } else { self.after };
        let mut audio = if forward { self.after } else { self.before };
        if let Some(previous) = &self.file {
            // Own only changed fields. Other audio controls, other preference
            // groups and unknown fields may have been edited since this action.
            let mut file = File::open(previous.path())?;
            let mut merged = file.audio();
            for (current, before, after) in [
                (&mut merged.fade_in, expected.fade_in, audio.fade_in),
                (&mut merged.fade_out, expected.fade_out, audio.fade_out),
                (
                    &mut merged.music_rate,
                    expected.music_rate,
                    audio.music_rate,
                ),
            ] {
                if before == after {
                    continue;
                }
                if *current != before {
                    return Err(Error::ExternalChange);
                }
                *current = after;
            }
            file.set_audio(merged);
            file.save()?;
            audio = merged;
            self.file = Some(file);
        }
        Ok(Update {
            audio,
            file: self.file.clone(),
        })
    }
}
