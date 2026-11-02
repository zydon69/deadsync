//! Original PreferencesPerformance fields. UI limits apply when editing;
//! stored out-of-range integers and unknown enum values remain distinguishable.
use crate::Error;
use serde_json::{Map, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scale {
    Independent,
    Shared,
    Explicit,
    Unknown(i32),
}

impl Scale {
    pub fn from_raw(value: i32) -> Self {
        match value {
            0 => Self::Independent,
            1 => Self::Shared,
            2 => Self::Explicit,
            value => Self::Unknown(value),
        }
    }
    pub fn source_name(self) -> String {
        match self {
            Self::Independent => "Independent".into(),
            Self::Shared => "Shared".into(),
            Self::Explicit => "Explicit".into(),
            Self::Unknown(value) => value.to_string(),
        }
    }
    pub fn read_source(value: Option<&Value>) -> Result<Self, Error> {
        let Some(value) = value else {
            return Ok(Self::Shared);
        };
        let Some(text) = value.as_str() else {
            if value.is_array() || value.is_object() {
                return Err(Error::InvalidFormat(
                    "Performance FrameMaxTime must be a scalar".into(),
                ));
            }
            return Ok(Self::Independent);
        };
        let text = text.trim();
        if let Ok(raw) = text.parse::<i32>() {
            return Ok(Self::from_raw(raw));
        }
        let names = ["Independent", "Shared", "Explicit"];
        let mut raw = 0;
        for part in text.split(',') {
            let Some(index) = names
                .iter()
                .position(|name| name.eq_ignore_ascii_case(part.trim()))
            else {
                return Ok(Self::Shared);
            };
            raw |= index as i32;
        }
        Ok(Self::from_raw(raw))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    pub visible: bool,
    pub max_frames: i32,
    pub scale: Scale,
    pub explicit_seconds: f64,
    pub paused: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            visible: false,
            max_frames: 512,
            scale: Scale::Shared,
            explicit_seconds: 1.0 / 60.0,
            paused: false,
        }
    }
}

impl Preferences {
    pub fn read(fields: &Map<String, Value>) -> Result<Self, Error> {
        let mut result = Self::default();
        for (name, target) in [
            ("ShowPerformanceWindow", &mut result.visible),
            ("PerformanceMonitorPaused", &mut result.paused),
        ] {
            if let Some(value) = fields.get(name) {
                *target = value.as_bool().ok_or_else(|| {
                    Error::InvalidFormat(format!("Performance {name} must be boolean"))
                })?;
            }
        }
        if let Some(value) = fields.get("MaxFramesToDraw") {
            result.max_frames = value
                .as_i64()
                .and_then(|value| i32::try_from(value).ok())
                .ok_or_else(|| {
                    Error::InvalidFormat("Performance MaxFramesToDraw must be an i32".into())
                })?;
        }
        result.scale = if let Some(value) = fields.get("FrameMaxTime").and_then(Value::as_i64) {
            Scale::from_raw(i32::try_from(value).map_err(|_| {
                Error::InvalidFormat("Performance FrameMaxTime must fit i32".into())
            })?)
        } else {
            Scale::read_source(fields.get("FrameMaxTime"))?
        };
        if let Some(value) = fields.get("ExplicitFrameMaxTime") {
            result.explicit_seconds = value
                .as_f64()
                .filter(|value| value.is_finite())
                .ok_or_else(|| {
                    Error::InvalidFormat("Performance ExplicitFrameMaxTime must be finite".into())
                })?;
        }
        Ok(result)
    }

    pub fn validate(self) -> Result<(), Error> {
        if !self.explicit_seconds.is_finite() {
            return Err(Error::InvalidEdit(
                "Performance ExplicitFrameMaxTime must be finite".into(),
            ));
        }
        Ok(())
    }
    pub fn write(self, fields: &mut Map<String, Value>) -> Result<(), Error> {
        self.validate()?;
        fields.insert("ShowPerformanceWindow".into(), self.visible.into());
        fields.insert("MaxFramesToDraw".into(), self.max_frames.into());
        fields.insert("FrameMaxTime".into(), self.scale.source_name().into());
        fields.insert("ExplicitFrameMaxTime".into(), self.explicit_seconds.into());
        fields.insert("PerformanceMonitorPaused".into(), self.paused.into());
        Ok(())
    }

    pub fn restoration_needed(self) -> bool {
        let defaults = Self::default();
        self.max_frames != defaults.max_frames
            || self.scale != defaults.scale
            || !crate::number::reference_equal(self.explicit_seconds, defaults.explicit_seconds)
            || self.paused != defaults.paused
    }

    pub fn restored(self) -> Self {
        Self {
            visible: self.visible,
            ..Self::default()
        }
    }
}
