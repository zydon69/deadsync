//! Shared source Stream preferences; UI opening state is preserved as opaque JSON.
use crate::{Error, density::StreamSettings, density_preferences::Accumulation};
use serde_json::{Map, Value};
pub(crate) mod history;
pub use history::Update;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NoteType {
    Quarter,
    Eighth,
    EighthTriplet,
    #[default]
    Sixteenth,
    SixteenthTriplet,
    ThirtySecond,
    ThirtySecondTriplet,
    SixtyFourth,
    OneHundredNinetySecond,
}
impl NoteType {
    pub const ALL: [Self; 9] = [
        Self::Quarter,
        Self::Eighth,
        Self::EighthTriplet,
        Self::Sixteenth,
        Self::SixteenthTriplet,
        Self::ThirtySecond,
        Self::ThirtySecondTriplet,
        Self::SixtyFourth,
        Self::OneHundredNinetySecond,
    ];
    pub const fn name(self) -> &'static str {
        match self {
            Self::Quarter => "QuarterNotes",
            Self::Eighth => "EighthNotes",
            Self::EighthTriplet => "EighthNoteTriplets",
            Self::Sixteenth => "SixteenthNotes",
            Self::SixteenthTriplet => "SixteenthNoteTriplets",
            Self::ThirtySecond => "ThirtySecondNotes",
            Self::ThirtySecondTriplet => "ThirtySecondNoteTriplets",
            Self::SixtyFourth => "SixtyFourthNotes",
            Self::OneHundredNinetySecond => "OneHundredNinetySecondNotes",
        }
    }
    pub const fn count(self) -> u32 {
        match self {
            Self::Quarter => 4,
            Self::Eighth => 8,
            Self::EighthTriplet => 12,
            Self::Sixteenth => 16,
            Self::SixteenthTriplet => 24,
            Self::ThirtySecond => 32,
            Self::ThirtySecondTriplet => 48,
            Self::SixtyFourth => 64,
            Self::OneHundredNinetySecond => 192,
        }
    }
    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|v| *v == self)
            .expect("complete enum")
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub note_type: NoteType,
    pub accumulation: Accumulation,
    pub show_break_lengths: bool,
    pub minimum_length: i32,
    pub short_break_cutoff: i32,
    pub short_break: char,
    pub long_break: char,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            note_type: NoteType::Sixteenth,
            accumulation: Accumulation::Step,
            show_break_lengths: false,
            minimum_length: 1,
            short_break_cutoff: 4,
            short_break: '-',
            long_break: '|',
        }
    }
}
impl Preferences {
    pub fn validate(self) -> Result<(), Error> {
        self.accumulation.validate()?;
        if self.short_break.len_utf16() != 1 || self.long_break.len_utf16() != 1 {
            return Err(Error::InvalidEdit(
                "Stream marks require one UTF-16 character".into(),
            ));
        }
        Ok(())
    }
    pub fn density_settings(self) -> StreamSettings {
        StreamSettings {
            threshold: self.note_type.count(),
            accumulation: self.accumulation,
            show_break_lengths: self.show_break_lengths,
            minimum_length: self.minimum_length.max(0) as usize,
            short_break_cutoff: self.short_break_cutoff.max(0) as usize,
            short_break: self.short_break,
            long_break: self.long_break,
        }
    }
    pub fn read(f: &Map<String, Value>) -> Result<Self, Error> {
        let mut p = Self::default();
        // Unregistered enum fallback is zero (Quarter); missing field keeps the constructor's Sixteenth default.
        if let Some(v) = f.get("NoteType") {
            let names = NoteType::ALL.map(NoteType::name);
            let index = source_enum(v, &names).unwrap_or(0);
            p.note_type = *NoteType::ALL
                .get(usize::try_from(index).map_err(|_| invalid())?)
                .ok_or_else(invalid)?;
        }
        if let Some(v) = f.get("AccumulationType") {
            p.accumulation = Accumulation::read(Some(v));
        }
        if let Some(v) = f.get("ShowBreakLengths") {
            p.show_break_lengths = v.as_bool().ok_or_else(invalid)?;
        }
        for (name, target) in [
            ("MinimumLengthToConsiderStream", &mut p.minimum_length),
            ("ShortBreakCutoff", &mut p.short_break_cutoff),
        ] {
            if let Some(v) = f.get(name) {
                *target = v
                    .as_i64()
                    .and_then(|v| v.try_into().ok())
                    .ok_or_else(invalid)?;
            }
        }
        for (name, target) in [
            ("ShortBreakCharacter", &mut p.short_break),
            ("LongBreakCharacter", &mut p.long_break),
        ] {
            if let Some(v) = f.get(name) {
                let mut c = v.as_str().ok_or_else(invalid)?.chars();
                *target = c.next().ok_or_else(invalid)?;
                if c.next().is_some() {
                    return Err(invalid());
                }
            }
        }
        p.validate()?;
        Ok(p)
    }
    pub(crate) fn write(self, f: &mut Map<String, Value>) -> Result<(), Error> {
        self.validate()?;
        for (k, v) in [
            ("NoteType", Value::from(self.note_type.name())),
            (
                "AccumulationType",
                Value::from(self.accumulation.name()),
            ),
            ("ShowBreakLengths", Value::from(self.show_break_lengths)),
            (
                "MinimumLengthToConsiderStream",
                Value::from(self.minimum_length),
            ),
            ("ShortBreakCutoff", Value::from(self.short_break_cutoff)),
            (
                "ShortBreakCharacter",
                Value::from(self.short_break.to_string()),
            ),
            (
                "LongBreakCharacter",
                Value::from(self.long_break.to_string()),
            ),
        ] {
            f.insert(k.into(), v);
        }
        Ok(())
    }
}
fn invalid() -> Error {
    Error::InvalidFormat("Invalid Stream preference field".into())
}

// Fumen delegates string values to case-insensitive Enum.TryParse (including name lists).
// Non-string tokens return enum zero. Undefined numeric values are rejected at the domain boundary.
fn source_enum(v: &Value, names: &[&str]) -> Option<i32> {
    let text = v.as_str()?.trim();
    if let Ok(value) = text.parse::<i32>() {
        return Some(value);
    }
    let mut result = 0i32;
    for component in text.split(',') {
        let index = names
            .iter()
            .position(|name| name.eq_ignore_ascii_case(component.trim()))?;
        result |= index as i32;
    }
    Some(result)
}
