//! Difficulty/rating fields, including SM positional headers.
use crate::{
    Error,
    document::{Document, Format},
    history::Session,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Difficulty {
    Beginner,
    Easy,
    Medium,
    Hard,
    Challenge,
    Edit,
}
impl Difficulty {
    pub const ALL: [Self; 6] = [
        Self::Beginner,
        Self::Easy,
        Self::Medium,
        Self::Hard,
        Self::Challenge,
        Self::Edit,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Beginner => "Beginner",
            Self::Easy => "Easy",
            Self::Medium => "Medium",
            Self::Hard => "Hard",
            Self::Challenge => "Challenge",
            Self::Edit => "Edit",
        }
    }
    pub fn step(self, forward: bool) -> Self {
        let i = Self::ALL.iter().position(|v| *v == self).unwrap();
        Self::ALL[(i + if forward { 1 } else { 5 }) % 6]
    }
    fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|v| v.as_str() == value.trim())
            .or_else(|| {
                value
                    .trim()
                    .parse::<usize>()
                    .ok()
                    .and_then(|i| Self::ALL.get(i).copied())
            })
    }
}
pub struct Identity {
    raw_difficulty: String,
    original_difficulty: Option<Difficulty>,
    difficulty: Option<Difficulty>,
    original_rating: i32,
    rating: i32,
}
impl Identity {
    pub fn read(document: &Document, chart: usize) -> Result<Self, Error> {
        if chart >= document.charts().len() {
            return Err(invalid("Chart properties chart does not exist"));
        }
        let field = |name: &str, index| -> Result<String, Error> {
            if document.format == Format::Sm {
                document.sm_header_text(chart, index)
            } else {
                Ok(document.text(Some(chart), name).unwrap_or_default())
            }
        };
        let raw_difficulty = field("DIFFICULTY", 2)?;
        let difficulty = Difficulty::parse(&raw_difficulty);
        let raw_rating = field("METER", 3)?;
        let rating = if document.format == Format::Sm {
            raw_rating.trim().parse::<i32>().unwrap_or(0)
        } else {
            let number = raw_rating
                .trim()
                .replace(',', "")
                .parse::<f64>()
                .unwrap_or(0.0);
            // Rust's saturating float-to-int conversion also retains the
            // pinned .NET reader/constructor result for NaN.
            number as i32
        };
        Ok(Self {
            raw_difficulty,
            original_difficulty: difficulty,
            difficulty,
            original_rating: rating,
            rating,
        })
    }
    pub fn difficulty(&self) -> Option<Difficulty> {
        self.difficulty
    }
    pub fn raw_difficulty(&self) -> &str {
        &self.raw_difficulty
    }
    pub fn rating(&self) -> i32 {
        self.rating
    }
    pub fn set_difficulty(&mut self, value: Difficulty) {
        self.difficulty = Some(value);
    }
    pub fn set_rating(&mut self, value: i32) -> Result<(), Error> {
        if value < 1 {
            return Err(invalid("Chart rating must be at least 1"));
        }
        self.rating = value;
        Ok(())
    }
    pub fn has_changes(&self) -> bool {
        self.difficulty != self.original_difficulty || self.rating != self.original_rating
    }
    pub(crate) fn apply(&self, session: &mut Session, chart: usize) -> Result<(), Error> {
        for (name, index, value) in [
            (
                "DIFFICULTY",
                2,
                (self.difficulty != self.original_difficulty)
                    .then(|| self.difficulty.unwrap().as_str().to_owned()),
            ),
            (
                "METER",
                3,
                (self.rating != self.original_rating).then(|| self.rating.to_string()),
            ),
        ] {
            if let Some(value) = value {
                if session.document().format == Format::Sm {
                    session.set_sm_header(chart, index, &value)?;
                } else {
                    session.set_text(Some(chart), name, &value)?;
                }
            }
        }
        Ok(())
    }
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
