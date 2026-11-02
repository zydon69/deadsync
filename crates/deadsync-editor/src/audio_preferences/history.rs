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
    pub fn apply(
        &mut self,
        forward: bool,
        live: Option<Audio>,
        fresh: Option<&File>,
    ) -> Result<Update, Error> {
        let expected = if forward { self.before } else { self.after };
        let target = if forward { self.after } else { self.before };
        fn merged(current: Audio, expected: Audio, target: Audio) -> Result<Audio, Error> {
            let (mut fields, mut before, mut after) = (
                serde_json::Map::new(),
                serde_json::Map::new(),
                serde_json::Map::new(),
            );
            current.write(&mut fields);
            expected.write(&mut before);
            target.write(&mut after);
            super::publication::merge_fields(&mut fields, &before, &after)?;
            Audio::read(&fields)
        }
        // The Session's scalar baseline remains authoritative even if a caller
        // captured an obsolete File after a direct, non-undoable tick edit.
        let scalar = live.unwrap_or_else(|| fresh.map(File::audio).unwrap_or(expected));
        let runtime = merged(scalar, expected, target)?;
        // Fresh File proves changed-leaf ownership independently. Its unrelated
        // Audio values may be stale after an untracked scalar-only tick edit.
        if self.file.is_some() {
            if let Some(fresh) = fresh {
                merged(fresh.audio(), expected, target)?;
            }
        }
        let mut runtime_file = if self.file.is_some() {
            fresh.cloned().or_else(|| {
                self.file
                    .as_ref()
                    .filter(|f| f.path().as_os_str().is_empty())
                    .cloned()
            })
        } else {
            None
        };
        if let Some(file) = runtime_file.as_mut() {
            file.set_audio(scalar);
        }
        // Disk-only legacy callers retain fresh disk disjoint values; memory
        // owners always retain the authoritative scalar even without a File.
        let fresh = runtime_file.as_ref();
        let audio = super::publication::publish(&mut self.file, fresh, runtime, |current| {
            let mut file = current.clone();
            let audio = merged(file.audio(), expected, target)?;
            file.set_audio(audio);
            Ok((audio, file))
        })?;
        Ok(Update {
            audio,
            file: self.file.clone(),
        })
    }
}
