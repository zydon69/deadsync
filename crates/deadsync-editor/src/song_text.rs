//! Song-wide authoring properties. Format restrictions belong to reviewed export.
pub mod resident;
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
    Banner,
    Background,
    CdTitle,
    Jacket,
    CdImage,
    DiscImage,
    PreviewVideo,
    Lyrics,
}
impl Field {
    pub const ALL: [Self; 19] = [
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
        Self::Banner,
        Self::Background,
        Self::CdTitle,
        Self::Jacket,
        Self::CdImage,
        Self::DiscImage,
        Self::PreviewVideo,
        Self::Lyrics,
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
            Self::Banner => "BANNER",
            Self::Background => "BACKGROUND",
            Self::CdTitle => "CDTITLE",
            Self::Jacket => "JACKET",
            Self::CdImage => "CDIMAGE",
            Self::DiscImage => "DISCIMAGE",
            Self::PreviewVideo => "PREVIEWVID",
            Self::Lyrics => "LYRICSPATH",
        }
    }
    /// Export advice only, never an authoring/editability restriction.
    pub fn requires_ssc(self) -> bool { self.omitted_by_source_sm() }
    pub fn omitted_by_source_sm(self) -> bool {
        matches!(
            self,
            Self::Origin | Self::PreviewMusic
                | Self::Jacket
                | Self::CdImage
                | Self::DiscImage
                | Self::PreviewVideo
        )
    }
    pub fn file_kind(self) -> Option<crate::music_file::Kind> {
        use crate::music_file::Kind;
        match self {
            Self::Music | Self::PreviewMusic => Some(Kind::Audio),
            Self::Banner | Self::CdTitle | Self::Jacket | Self::CdImage | Self::DiscImage => {
                Some(Kind::Image)
            }
            Self::Background => Some(Kind::ImageOrVideo),
            Self::PreviewVideo => Some(Kind::Video),
            Self::Lyrics => Some(Kind::Lyrics),
            _ => None,
        }
    }
    fn index(self) -> usize {
        Self::ALL.iter().position(|f| *f == self).unwrap()
    }
}
pub struct Text {
    original: [String; Field::ALL.len()],
    values: [String; Field::ALL.len()],
    preview_clear_needed: bool,
    preview_explicit: bool,
}
impl Text {
    pub fn read(document: &Document) -> Self {
        // Infallible display readers never activate ignored raw SM media or
        // malformed native authoring state. Mutation boundaries use try_read.
        Self::from_preview(document, resident::effective(document).unwrap_or_default())
    }
    pub fn try_read(document: &Document) -> Result<Self, Error> {
        Ok(Self::from_preview(document, resident::effective(document)?))
    }
    fn from_preview(document: &Document, preview: String) -> Self {
        let mut values = Field::ALL.map(|f|document.text(None,f.tag()).unwrap_or_default());
        values[Field::PreviewMusic.index()] = preview;
        let preview_clear_needed = document.format == crate::document::Format::Sm
            && document.raw_property(None, resident::TAG).is_none()
            && document.text(None, "PREVIEW").is_some_and(|value| !value.is_empty());
        Self { original: values.clone(), values, preview_clear_needed, preview_explicit: false }
    }
    pub fn get(&self, field: Field) -> &str {
        &self.values[field.index()]
    }
    pub(crate) fn original(&self, field: Field) -> &str {
        &self.original[field.index()]
    }
    pub fn supports(&self, _field: Field) -> bool { true }
    pub fn set(&mut self, field: Field, value: &str) -> Result<(), Error> {
        if value != self.original[field.index()] && value.chars().any(char::is_control) {
            return Err(Error::InvalidEdit(
                "Song text must be a single line without control characters".into(),
            ));
        }
        if field == Field::PreviewMusic { self.preview_explicit = true; }
        self.values[field.index()] = value.to_owned();
        Ok(())
    }
    pub fn has_changes(&self) -> bool {
        self.values != self.original || self.preview_explicit && self.preview_clear_needed
    }
    pub(crate) fn changes(&self) -> Vec<(&'static str, String)> {
        Field::ALL
            .into_iter()
            .filter(|f| self.values[f.index()] != self.original[f.index()]
                || *f == Field::PreviewMusic && self.preview_explicit && self.preview_clear_needed)
            .map(|f| (f.tag(), self.get(f).to_owned()))
            .collect()
    }
}
