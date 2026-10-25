//! Chart text fields and their SM/SSC persistence boundaries.
use crate::{
    Error,
    document::{Document, Format},
    history::Session,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Name,
    Description,
    Credit,
    Style,
}
impl Field {
    pub const ALL: [Self; 4] = [Self::Name, Self::Description, Self::Credit, Self::Style];
    fn tag(self) -> &'static str {
        match self {
            Self::Name => "CHARTNAME",
            Self::Description => "DESCRIPTION",
            Self::Credit => "CREDIT",
            Self::Style => "CHARTSTYLE",
        }
    }
    fn index(self) -> usize {
        Self::ALL.iter().position(|field| *field == self).unwrap()
    }
}
pub struct Text {
    format: Format,
    original: [String; 4],
    values: [String; 4],
}
impl Text {
    pub fn read(document: &Document, chart: usize) -> Result<Self, Error> {
        if chart >= document.charts().len() {
            return Err(Error::InvalidEdit("Chart text chart does not exist".into()));
        }
        let values = if document.format == Format::Sm {
            [
                String::new(),
                document.sm_header_text(chart, 1)?,
                document.text(None, "CREDIT").unwrap_or_default(),
                String::new(),
            ]
        } else {
            Field::ALL.map(|field| {
                let value = document.text(Some(chart), field.tag()).unwrap_or_default();
                if field == Field::Credit && value.is_empty() {
                    document.text(None, "CREDIT").unwrap_or_default()
                } else {
                    value
                }
            })
        };
        Ok(Self {
            format: document.format,
            original: values.clone(),
            values,
        })
    }
    pub fn supports(&self, field: Field) -> bool {
        self.format == Format::Ssc || field == Field::Description
    }
    pub fn get(&self, field: Field) -> &str {
        &self.values[field.index()]
    }
    pub fn set(&mut self, field: Field, value: &str) -> Result<(), Error> {
        if !self.supports(field) {
            return Err(Error::Unsupported(
                "SM does not persist this chart text field; use SSC".into(),
            ));
        }
        if value != self.original[field.index()] && value.chars().any(char::is_control) {
            return Err(Error::InvalidEdit(
                "Chart text must be a single line without control characters".into(),
            ));
        }
        self.values[field.index()] = value.to_owned();
        Ok(())
    }
    pub fn has_changes(&self) -> bool {
        self.original != self.values
    }
    pub(crate) fn apply(&self, session: &mut Session, chart: usize) -> Result<(), Error> {
        for field in Field::ALL {
            if self.values[field.index()] == self.original[field.index()] {
                continue;
            }
            if self.format == Format::Sm {
                session.set_sm_header(chart, 1, self.get(field))?;
            } else {
                session.set_text(Some(chart), field.tag(), self.get(field))?;
            }
        }
        Ok(())
    }
}
