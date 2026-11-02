//! EditorSong.Selectable enum and Enum.TryParse(..., true), GrooveAuthor 48e18f71.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selectable(i32);
impl Default for Selectable {
    fn default() -> Self {
        Self(0)
    }
}
impl Selectable {
    pub const ALL: [Self; 5] = [Self(0), Self(1), Self(2), Self(3), Self(4)];
    const NAMES: [&'static str; 5] = ["YES", "NO", "ROULETTE", "ES", "OMES"];
    pub fn read(raw: Option<&str>) -> Self {
        let Some(raw) = raw else {
            return Self::default();
        };
        let raw = raw.trim();
        if let Ok(value) = raw.parse::<i32>() {
            return Self(value);
        }
        let ordinal = raw.split(',').try_fold(0, |ordinal, part| {
            Self::NAMES
                .iter()
                .position(|name| name.eq_ignore_ascii_case(part.trim()))
                .map(|index| ordinal | index as i32)
        });
        Self(ordinal.unwrap_or(0))
    }
    pub fn source_name(self) -> String {
        usize::try_from(self.0)
            .ok()
            .and_then(|index| Self::NAMES.get(index))
            .map_or_else(|| self.0.to_string(), |name| (*name).into())
    }
    pub fn step(self, forward: bool) -> Self {
        let Some(index) = Self::ALL.iter().position(|value| *value == self) else {
            return Self::default();
        };
        Self::ALL[(index + if forward { 1 } else { 4 }) % 5]
    }
}
