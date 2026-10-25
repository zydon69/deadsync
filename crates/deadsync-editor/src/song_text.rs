//! Song-wide text tags shared by SM and SSC.
use crate::{Error, document::Document};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Title,
    Subtitle,
    Artist,
    Credit,
    Genre,
    Origin,
    TitleTransliteration,
    SubtitleTransliteration,
    ArtistTransliteration,
    Music,
}
impl Field {
    pub const ALL: [Self; 10] = [
        Self::Title,
        Self::Subtitle,
        Self::Artist,
        Self::Credit,
        Self::Genre,
        Self::Origin,
        Self::TitleTransliteration,
        Self::SubtitleTransliteration,
        Self::ArtistTransliteration,
        Self::Music,
    ];
    pub fn tag(self) -> &'static str {
        match self {
            Self::Title => "TITLE",
            Self::Subtitle => "SUBTITLE",
            Self::Artist => "ARTIST",
            Self::Credit => "CREDIT",
            Self::Genre => "GENRE",
            Self::Origin => "ORIGIN",
            Self::TitleTransliteration => "TITLETRANSLIT",
            Self::SubtitleTransliteration => "SUBTITLETRANSLIT",
            Self::ArtistTransliteration => "ARTISTTRANSLIT",
            Self::Music => "MUSIC",
        }
    }
    fn index(self) -> usize {
        Self::ALL.iter().position(|f| *f == self).unwrap()
    }
}
pub struct Text {
    original: [String; 10],
    values: [String; 10],
}
impl Text {
    pub fn read(document: &Document) -> Self {
        let values = Field::ALL.map(|f| document.text(None, f.tag()).unwrap_or_default());
        Self {
            original: values.clone(),
            values,
        }
    }
    pub fn get(&self, field: Field) -> &str {
        &self.values[field.index()]
    }
    pub(crate) fn original(&self, field: Field) -> &str {
        &self.original[field.index()]
    }
    pub fn set(&mut self, field: Field, value: &str) -> Result<(), Error> {
        if value != self.original[field.index()] && value.chars().any(char::is_control) {
            return Err(Error::InvalidEdit(
                "Song text must be a single line without control characters".into(),
            ));
        }
        self.values[field.index()] = value.to_owned();
        Ok(())
    }
    pub fn has_changes(&self) -> bool {
        self.values != self.original
    }
    pub(crate) fn changes(&self) -> Vec<(&'static str, String)> {
        Field::ALL
            .into_iter()
            .filter(|f| self.values[f.index()] != self.original[f.index()])
            .map(|f| (f.tag(), self.get(f).to_owned()))
            .collect()
    }
}
