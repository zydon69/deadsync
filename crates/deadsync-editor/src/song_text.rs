//! Song-wide text tags, including SSC-only dedicated preview music.
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
    PreviewMusic,
}
impl Field {
    pub const ALL: [Self; 11] = [
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
        Self::PreviewMusic,
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
            Self::PreviewMusic => "PREVIEW",
        }
    }
    fn index(self) -> usize {
        Self::ALL.iter().position(|f| *f == self).unwrap()
    }
}
pub struct Text {
    format: crate::document::Format,
    original: [String; 11],
    values: [String; 11],
}
impl Text {
    pub fn read(document: &Document) -> Self {
        let values = Field::ALL.map(|f| {
            if f == Field::PreviewMusic && document.format == crate::document::Format::Sm {
                String::new()
            } else {
                document.text(None, f.tag()).unwrap_or_default()
            }
        });
        Self {
            format: document.format,
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
    pub fn supports(&self, field: Field) -> bool {
        field != Field::PreviewMusic || self.format == crate::document::Format::Ssc
    }
    pub fn set(&mut self, field: Field, value: &str) -> Result<(), Error> {
        if !self.supports(field) && value != self.original[field.index()] {
            return Err(Error::Unsupported(
                "Dedicated preview music requires SSC".into(),
            ));
        }
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
