//! Original MiniMap content options; viewport/ranges remain a separate contract.
use crate::Error;
use serde_json::{Map, Value};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub visible: bool,
    pub width: u32,
    pub note_width: u32,
    pub note_spacing: u32,
    pub patterns: bool,
    pub patterns_width: u32,
    pub preview: bool,
    pub preview_width: u32,
    pub labels: bool,
    pub max_notes: usize,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            visible: true,
            width: 90,
            note_width: 2,
            note_spacing: 3,
            patterns: true,
            patterns_width: 8,
            preview: true,
            preview_width: 8,
            labels: true,
            max_notes: 6144,
        }
    }
}
impl Preferences {
    pub fn read(minimap: &Map<String, Value>, options: &Map<String, Value>) -> Result<Self, Error> {
        let mut result = Self::default();
        for (name, target) in [
            ("ShowMiniMap", &mut result.visible),
            ("ShowPatterns", &mut result.patterns),
            ("ShowPreview", &mut result.preview),
            ("ShowLabels", &mut result.labels),
        ] {
            if let Some(value) = minimap.get(name) {
                *target = value
                    .as_bool()
                    .ok_or_else(|| Error::InvalidFormat(format!("Invalid {name}")))?;
            }
        }
        for (name, target) in [
            ("MiniMapWidth", &mut result.width),
            ("MiniMapNoteWidth", &mut result.note_width),
            ("MiniMapNoteSpacing", &mut result.note_spacing),
            ("PatternsWidth", &mut result.patterns_width),
            ("PreviewWidth", &mut result.preview_width),
        ] {
            if let Some(value) = minimap.get(name) {
                *target = u32::try_from(
                    value
                        .as_u64()
                        .ok_or_else(|| Error::InvalidFormat(format!("Invalid {name}")))?,
                )
                .map_err(|_| Error::InvalidFormat(format!("Invalid {name}")))?;
            }
        }
        if let Some(value) = options.get("MiniMapMaxNotesToDraw") {
            result.max_notes = usize::try_from(
                value
                    .as_u64()
                    .ok_or_else(|| Error::InvalidFormat("Invalid MiniMapMaxNotesToDraw".into()))?,
            )
            .map_err(|_| Error::InvalidFormat("Invalid MiniMapMaxNotesToDraw".into()))?;
        }
        if !(3..=1024).contains(&result.width)
            || result.note_width > result.width - 2
            || result.note_spacing > 1024
            || result.patterns_width > 1024
            || result.preview_width > 1024
            || result.max_notes > 65535
        {
            return Err(Error::InvalidFormat(
                "Minimap content exceeds renderer resource bounds".into(),
            ));
        }
        Ok(result)
    }
}
