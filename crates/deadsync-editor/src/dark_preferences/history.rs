//! Own only changed dark flags when undoing across unrelated preference edits.
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
        f.debug_struct("DarkPreferences")
            .field("before", &self.before)
            .field("after", &self.after)
            .finish_non_exhaustive()
    }
}
impl Change {
    pub fn apply(&mut self, forward: bool, live: Option<&File>) -> Result<Update, Error> {
        let expected = if forward { self.before } else { self.after };
        let mut target = if forward { self.after } else { self.before };
        let runtime = if let Some(live) = live {
            Some(merge_preferences(live.dark(), expected, target)?)
        } else {
            None
        };
        if let Some(previous) = &self.file {
            let mut file = if previous.path().as_os_str().is_empty() {
                live.unwrap_or(previous).clone()
            } else {
                File::open(previous.path())?
            };
            target = merge_preferences(file.dark(), expected, target)?;
            file.set_dark(target)?;
            if file.path().as_os_str().is_empty() {
                file.commit_memory_preferences()?;
            } else {
                file.save()?;
            }
            self.file = Some(file);
        }
        Ok(Update {
            preferences: runtime.unwrap_or(target),
            file: self.file.clone(),
        })
    }
}

fn merge_preferences(
    current: Preferences,
    before: Preferences,
    after: Preferences,
) -> Result<Preferences, Error> {
    fn merge(
        current: &mut serde_json::Map<String, serde_json::Value>,
        before: &serde_json::Map<String, serde_json::Value>,
        after: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), Error> {
        for (key, value) in after {
            let old = before.get(key);
            if old == Some(value) {
                continue;
            }
            if let (
                Some(serde_json::Value::Object(old)),
                serde_json::Value::Object(new),
                Some(serde_json::Value::Object(now)),
            ) = (old, value, current.get_mut(key))
            {
                merge(now, old, new)?;
            } else {
                if current.get(key) != old {
                    return Err(Error::ExternalChange);
                }
                current.insert(key.clone(), value.clone());
            }
        }
        Ok(())
    }
    let (mut now, mut old, mut new) = (
        serde_json::Map::new(),
        serde_json::Map::new(),
        serde_json::Map::new(),
    );
    current.write(&mut now);
    before.write(&mut old);
    after.write(&mut new);
    merge(&mut now, &old, &new)?;
    Preferences::read(&now)
}
