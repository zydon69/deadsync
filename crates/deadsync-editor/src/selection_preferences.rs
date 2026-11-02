//! Selection policy, independent from simfile format and chart content history.
//! Native Version1 group is `Selection`; source group decoding is explicit and
//! does not claim to import an entire GrooveAuthor Preferences file.
use crate::Error;
use serde_json::{Map, Value};
pub(crate) mod history;
pub use history::Update;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Overlap {
    Any,
    #[default]
    Center,
    All,
    Unknown(i32),
}
impl Overlap {
    pub const ALL: [Self; 3] = [Self::Any, Self::Center, Self::All];
    pub fn value(self) -> i32 {
        match self {
            Self::Any => 0,
            Self::Center => 1,
            Self::All => 2,
            Self::Unknown(value) => value,
        }
    }
    pub fn from_value(value: i32) -> Self {
        match value {
            0 => Self::Any,
            1 => Self::Center,
            2 => Self::All,
            other => Self::Unknown(other),
        }
    }
    pub fn name(self) -> String {
        match self {
            Self::Any => "OverlapAny".into(),
            Self::Center => "OverlapCenter".into(),
            Self::All => "OverlapAll".into(),
            Self::Unknown(value) => value.to_string(),
        }
    }
    pub fn effective(self) -> Self {
        match self {
            Self::Unknown(_) => Self::Center,
            known => known,
        }
    }
    pub fn index(self) -> usize {
        self.effective().value() as usize
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RegionMode {
    #[default]
    TimeOrPosition,
    TimeOrPositionAndLane,
    Unknown(i32),
}
impl RegionMode {
    pub const ALL: [Self; 2] = [Self::TimeOrPosition, Self::TimeOrPositionAndLane];
    pub fn value(self) -> i32 {
        match self {
            Self::TimeOrPosition => 0,
            Self::TimeOrPositionAndLane => 1,
            Self::Unknown(value) => value,
        }
    }
    pub fn from_value(value: i32) -> Self {
        match value {
            0 => Self::TimeOrPosition,
            1 => Self::TimeOrPositionAndLane,
            other => Self::Unknown(other),
        }
    }
    pub fn name(self) -> String {
        match self {
            Self::TimeOrPosition => "TimeOrPosition".into(),
            Self::TimeOrPositionAndLane => "TimeOrPositionAndLane".into(),
            Self::Unknown(value) => value.to_string(),
        }
    }
    pub fn uses_lanes(self) -> bool {
        self == Self::TimeOrPositionAndLane
    }
    pub fn index(self) -> usize {
        usize::from(self.uses_lanes())
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Preferences {
    pub overlap: Overlap,
    pub region: RegionMode,
}
impl Preferences {
    pub fn is_using_defaults(self) -> bool {
        self == Self::default()
    }
    pub fn validate(self) -> Result<(), Error> {
        if matches!(self.overlap, Overlap::Unknown(0..=2))
            || matches!(self.region, RegionMode::Unknown(0..=1))
        {
            return Err(invalid());
        }
        Ok(())
    }
    pub fn read(fields: &Map<String, Value>) -> Result<Self, Error> {
        Self::read_fields(fields, false)
    }
    /// Exactly the original permissive enum converter for these two source fields.
    /// Unknown numeric enum strings remain representable and round-trip; source
    /// switch fallthrough is applied only by effective policy, not during storage.
    pub fn read_source_fields(fields: &Map<String, Value>) -> Result<Self, Error> {
        Self::read_fields(fields, true)
    }
    fn read_fields(fields: &Map<String, Value>, source: bool) -> Result<Self, Error> {
        let mut result = Self::default();
        if let Some(value) = fields.get("Mode") {
            result.overlap = Overlap::from_value(read_enum(
                value,
                &["OverlapAny", "OverlapCenter", "OverlapAll"],
                1,
                source,
            )?);
        }
        if let Some(value) = fields.get("RegionMode") {
            result.region = RegionMode::from_value(read_enum(
                value,
                &["TimeOrPosition", "TimeOrPositionAndLane"],
                0,
                source,
            )?);
        }
        result.validate()?;
        Ok(result)
    }
    pub fn write(self, fields: &mut Map<String, Value>) -> Result<(), Error> {
        self.validate()?;
        fields.insert("Mode".into(), self.overlap.name().into());
        fields.insert("RegionMode".into(), self.region.name().into());
        Ok(())
    }
}
fn read_enum(value: &Value, names: &[&str], fallback: i32, source: bool) -> Result<i32, Error> {
    if let Some(text) = value.as_str() {
        let text = text.trim();
        if let Ok(value) = text.parse::<i32>() {
            return Ok(value);
        }
        let mut combined = 0;
        for part in text.split(',') {
            let Some(index) = names
                .iter()
                .position(|name| name.eq_ignore_ascii_case(part.trim()))
            else {
                return Ok(fallback);
            };
            combined |= index as i32;
        }
        return Ok(combined);
    }
    if source {
        return if value.is_array() || value.is_object() {
            Err(invalid())
        } else {
            Ok(0)
        };
    }
    value
        .as_i64()
        .and_then(|number| i32::try_from(number).ok())
        .ok_or_else(invalid)
}
fn invalid() -> Error {
    Error::InvalidFormat("Invalid selection preference enum".into())
}
