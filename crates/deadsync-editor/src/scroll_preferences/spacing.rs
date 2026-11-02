//! Source raw SpacingMode, separate from the renderer's effective spacing.
use crate::{Error, zoom::SpacingMode};
use serde_json::Value;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Preference {
    #[default]
    ConstantTime,
    ConstantRow,
    Variable,
    Unknown(i32),
}
impl Preference {
    pub fn from_raw(value: i32) -> Self {
        match value {
            0 => Self::ConstantTime,
            1 => Self::ConstantRow,
            2 => Self::Variable,
            value => Self::Unknown(value),
        }
    }
    pub fn from_effective(value: SpacingMode) -> Self {
        match value {
            SpacingMode::ConstantTime => Self::ConstantTime,
            SpacingMode::ConstantRow => Self::ConstantRow,
            SpacingMode::Variable => Self::Variable,
        }
    }
    pub fn read_source(value: Option<&Value>) -> Result<Self, Error> {
        let names = [
            "ConstantTime".to_owned(),
            "ConstantRow".to_owned(),
            "Variable".to_owned(),
        ];
        Ok(Self::from_raw(crate::open_preferences::enum_value(
            value, &names, 0,
        )?))
    }
    /// Native Version1 has historically accepted numeric enum values directly.
    /// The explicit source importer normalizes source numeric tokens before here.
    pub fn read_native(value: Option<&Value>) -> Result<Self, Error> {
        if let Some(Value::Number(value)) = value {
            if let Some(value) = value.as_i64().and_then(|v| i32::try_from(v).ok()) {
                return Ok(Self::from_raw(value));
            }
        }
        Self::read_source(value)
    }
    pub fn raw(self) -> i32 {
        match self {
            Self::ConstantTime => 0,
            Self::ConstantRow => 1,
            Self::Variable => 2,
            Self::Unknown(value) => value,
        }
    }
    pub fn source_name(self) -> String {
        match self {
            Self::ConstantTime => "ConstantTime".into(),
            Self::ConstantRow => "ConstantRow".into(),
            Self::Variable => "Variable".into(),
            Self::Unknown(value) => value.to_string(),
        }
    }
    /// Original EventSpacingHelper factory defaults unknown enum values to time.
    pub fn effective(self) -> SpacingMode {
        match self {
            Self::ConstantRow => SpacingMode::ConstantRow,
            Self::Variable => SpacingMode::Variable,
            _ => SpacingMode::ConstantTime,
        }
    }
    /// Original unchecked int addition and signed remainder, including negatives.
    pub fn next(self) -> Self {
        Self::from_raw(self.raw().wrapping_add(1) % 3)
    }
}
