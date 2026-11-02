//! Undo owns only changed leaf fields, including individual mount offsets/colors.
use super::Preferences;
use crate::{Error, audio_preferences::File};
use serde_json::{Map, Value};
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
        f.debug_struct("DensityGraphPreferences")
            .field("before", &self.before)
            .field("after", &self.after)
            .finish_non_exhaustive()
    }
}
fn merge(
    current: &mut Map<String, Value>,
    before: &Map<String, Value>,
    after: &Map<String, Value>,
) -> Result<(), Error> {
    for (key, value) in after {
        let old = before.get(key);
        if old == Some(value) {
            continue;
        }
        if let (Some(Value::Object(old)), Value::Object(new), Some(Value::Object(now))) =
            (old, value, current.get_mut(key))
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
impl Change {
    pub fn apply(&mut self, forward: bool, live: Option<&File>) -> Result<Update, Error> {
        let expected = if forward { self.before } else { self.after };
        let mut target = if forward { self.after } else { self.before };
        // Runtime and disk baselines are separate. Compare changed leaves
        // against each, preserving unrelated concurrent preferences.
        let runtime = if let Some(live) = live {
            let (mut current, mut before, mut after) = (Map::new(), Map::new(), Map::new());
            live.density().write(&mut current)?;
            expected.write(&mut before)?;
            target.write(&mut after)?;
            merge(&mut current, &before, &after)?;
            Some(Preferences::read(&current)?)
        } else {
            None
        };
        if let Some(previous) = &self.file {
            let mut file = if previous.path().as_os_str().is_empty() {
                live.unwrap_or(previous).clone()
            } else {
                File::open(previous.path())?
            };
            let (mut current, mut before, mut after) = (Map::new(), Map::new(), Map::new());
            file.density().write(&mut current)?;
            expected.write(&mut before)?;
            target.write(&mut after)?;
            merge(&mut current, &before, &after)?;
            target = Preferences::read(&current)?;
            file.set_density(target)?;
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
