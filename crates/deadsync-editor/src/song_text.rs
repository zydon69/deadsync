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
    pub fn requires_ssc(self) -> bool {
        matches!(
            self,
            Self::PreviewMusic
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
    format: crate::document::Format,
    original: [String; Field::ALL.len()],
    values: [String; Field::ALL.len()],
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
        !field.requires_ssc() || self.format == crate::document::Format::Ssc
    }
    pub fn set(&mut self, field: Field, value: &str) -> Result<(), Error> {
        if !self.supports(field) && value != self.original[field.index()] {
            return Err(Error::Unsupported(format!("{} requires SSC", field.tag())));
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
