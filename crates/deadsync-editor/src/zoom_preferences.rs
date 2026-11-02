//! Validated zoom controls from GrooveAuthor PreferencesScroll and its UI.
use crate::{
    Error,
    zoom::{MIN_ZOOM, Manager},
};
use serde_json::{Map, Value};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    duration: f64,
    multiplier: f64,
    cap: f64,
    limited: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            duration: 0.1,
            multiplier: 1.2,
            cap: 1.0,
            limited: false,
        }
    }
}
impl Preferences {
    pub fn duration(self) -> f64 {
        self.duration
    }
    pub fn multiplier(self) -> f64 {
        self.multiplier
    }
    pub fn size_cap(self) -> f64 {
        self.cap
    }
    pub fn limited(self) -> bool {
        self.limited
    }
    pub fn set_duration(&mut self, value: f64) -> Result<(), Error> {
        self.duration = valid(value, 0.0, 1.0)?;
        Ok(())
    }
    pub fn set_multiplier(&mut self, value: f64) -> Result<(), Error> {
        self.multiplier = valid(value, 1.0, 4.0)?;
        Ok(())
    }
    pub fn set_size_cap(&mut self, value: f64) -> Result<(), Error> {
        let value = valid(value, MIN_ZOOM, 1.0)?;
        if !crate::number::reference_equal(self.cap, value) {
            self.cap = value;
        }
        Ok(())
    }
    pub fn set_limited(&mut self, value: bool) {
        self.limited = value;
    }
    /// Source Undo owns duration, multiplier and the limit flag; SizeCap is direct.
    pub(crate) fn undoable_fields_from(self, target: Self) -> Self {
        Self {
            cap: self.cap,
            ..target
        }
    }
    /// Merge only explicitly changed fields, without notifying a runtime manager.
    pub fn merge_changes(self, expected: Self, target: Self) -> Result<Self, Error> {
        let (mut current, mut before, mut after) = (Map::new(), Map::new(), Map::new());
        self.write(&mut current);
        expected.write(&mut before);
        target.write(&mut after);
        crate::audio_preferences::publication::merge_fields(&mut current, &before, &after)?;
        Self::read(&current)
    }
    /// Apply notifications to a clone so failure cannot partially configure a view.
    pub fn apply(self, manager: &mut Manager) -> Result<(), Error> {
        manager.apply_preferences(self.duration, self.multiplier, self.cap, self.limited)
    }
    pub fn initialize(self, manager: &mut Manager) -> Result<(), Error> {
        manager.initialize_preferences(self.duration, self.multiplier, self.cap, self.limited)
    }
    pub(crate) fn read(fields: &Map<String, Value>) -> Result<Self, Error> {
        let mut value = Self::default();
        for (key, index) in [
            ("ScrollInterpolationDuration", 0),
            ("ZoomMultiplier", 1),
            ("SizeCap", 2),
        ] {
            if let Some(field) = fields.get(key) {
                let number = field
                    .as_f64()
                    .ok_or_else(|| Error::InvalidFormat(format!("Invalid {key}")))?;
                match index {
                    0 => value.duration = finite(number)?,
                    1 => value.multiplier = finite(number)?,
                    _ => {
                        let cap = finite(number)?;
                        if !crate::number::reference_equal(value.cap, cap) {
                            value.cap = cap;
                        }
                    }
                }
            }
        }
        if let Some(field) = fields.get("LimitZoomToSize") {
            value.limited = field
                .as_bool()
                .ok_or_else(|| Error::InvalidFormat("Invalid LimitZoomToSize".into()))?;
        }
        Ok(value)
    }
    pub(crate) fn write(self, fields: &mut Map<String, Value>) {
        fields.insert("ScrollInterpolationDuration".into(), self.duration.into());
        fields.insert("ZoomMultiplier".into(), self.multiplier.into());
        fields.insert("SizeCap".into(), self.cap.into());
        fields.insert("LimitZoomToSize".into(), self.limited.into());
    }
}
fn valid(value: f64, min: f64, max: f64) -> Result<f64, Error> {
    if value.is_finite() && (min..=max).contains(&value) {
        Ok(value)
    } else {
        Err(Error::InvalidEdit(
            "Zoom preference is outside its supported range".into(),
        ))
    }
}

fn finite(value: f64) -> Result<f64, Error> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(Error::InvalidFormat(
            "Zoom preference must be finite".into(),
        ))
    }
}
