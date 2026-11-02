//! Non-document history for the complete Waveform and MiniMap preference owners.
use crate::{Error, audio_preferences::File};
use serde_json::{Map, Value};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MiniMap {
    pub viewport: crate::minimap_preferences::Preferences,
    pub position: crate::minimap_position_preferences::Preferences,
    pub content: crate::minimap_content_preferences::Preferences,
}
impl MiniMap {
    pub fn capture(file: &File) -> Result<Self, Error> {
        Ok(Self {
            viewport: file.minimap(),
            position: file.minimap_position(),
            content: file.minimap_content()?,
        })
    }
    /// MiniMapMaxNotesToDraw belongs to Options and is intentionally excluded
    /// from source PreferencesMiniMap.RestoreDefaults.
    pub fn restored(self) -> Self {
        let mut defaults = Self::default();
        defaults.content.max_notes = self.content.max_notes;
        defaults
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Snapshot {
    Waveform(crate::waveform_preferences::Preferences),
    MiniMap(MiniMap),
}
impl Snapshot {
    pub fn capture_like(self, file: &File) -> Result<Self, Error> {
        Ok(match self {
            Self::Waveform(_) => Self::Waveform(file.waveform()),
            Self::MiniMap(_) => Self::MiniMap(MiniMap::capture(file)?),
        })
    }
    fn fields(self) -> Result<Value, Error> {
        let mut groups = Map::new();
        let mut fields = Map::new();
        match self {
            Self::Waveform(p) => {
                p.write(&mut fields);
                groups.insert("Waveform".into(), fields.into());
            }
            Self::MiniMap(p) => {
                let mut options = Map::new();
                p.viewport.write(&mut fields);
                p.position.write(&mut fields);
                p.content.write(&mut fields, &mut options)?;
                groups.insert("MiniMap".into(), fields.into());
                groups.insert("Options".into(), options.into());
            }
        }
        Ok(groups.into())
    }
    fn read_like(self, groups: &Value) -> Result<Self, Error> {
        Ok(match self {
            Self::Waveform(_) => Self::Waveform(crate::waveform_preferences::Preferences::read(
                groups["Waveform"].as_object().unwrap(),
            )?),
            Self::MiniMap(_) => {
                let fields = groups["MiniMap"].as_object().unwrap();
                Self::MiniMap(MiniMap {
                    viewport: crate::minimap_preferences::Preferences::read(fields)?,
                    position: crate::minimap_position_preferences::Preferences::read(fields)?,
                    content: crate::minimap_content_preferences::Preferences::read(
                        fields,
                        groups["Options"].as_object().unwrap(),
                    )?,
                })
            }
        })
    }
    pub(crate) fn validate(self) -> Result<(), Error> {
        self.read_like(&self.fields()?).map(|_| ())
    }
    fn install(self, file: &mut File) -> Result<(), Error> {
        match self {
            Self::Waveform(p) => file.set_waveform(p),
            Self::MiniMap(p) => {
                file.set_minimap(p.viewport);
                file.set_minimap_position(p.position);
                file.set_minimap_content(p.content)?;
            }
        }
        Ok(())
    }
}
pub struct Update {
    pub preferences: Snapshot,
    pub file: Option<File>,
}
pub(crate) struct Change {
    pub before: Snapshot,
    pub after: Snapshot,
    pub file: Option<File>,
}
impl std::fmt::Debug for Change {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VisualPreferences")
            .field("before", &self.before)
            .field("after", &self.after)
            .finish_non_exhaustive()
    }
}
// Return a fresh merged tree. A conflict never partially mutates an owner/file.
fn merge(current: &Value, before: &Value, after: &Value) -> Result<Value, Error> {
    if before == after {
        return Ok(current.clone());
    }
    if let (Some(current), Some(before), Some(after)) =
        (current.as_object(), before.as_object(), after.as_object())
    {
        let mut result = current.clone();
        for key in before
            .keys()
            .chain(after.keys())
            .collect::<std::collections::BTreeSet<_>>()
        {
            let value = merge(
                current.get(key).unwrap_or(&Value::Null),
                before.get(key).unwrap_or(&Value::Null),
                after.get(key).unwrap_or(&Value::Null),
            )?;
            if value.is_null() {
                result.remove(key);
            } else {
                result.insert(key.clone(), value);
            }
        }
        return Ok(result.into());
    }
    if current != before {
        return Err(Error::ExternalChange);
    }
    Ok(after.clone())
}
impl Change {
    pub fn apply(&mut self, forward: bool, live: Option<&File>) -> Result<Update, Error> {
        let expected = if forward { self.before } else { self.after };
        let target = if forward { self.after } else { self.before };
        let preferences = crate::audio_preferences::publication::publish(
            &mut self.file,
            live,
            target,
            |current| {
                let merged = target.read_like(&merge(
                    &expected.capture_like(current)?.fields()?,
                    &expected.fields()?,
                    &target.fields()?,
                )?)?;
                let mut file = current.clone();
                merged.install(&mut file)?;
                Ok((merged, file))
            },
        )?;
        Ok(Update {
            preferences,
            file: self.file.clone(),
        })
    }
}
