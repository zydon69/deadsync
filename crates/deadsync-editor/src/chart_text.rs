//! Format-independent chart authoring; source-compatible export owns losses.
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
    Music,
}
impl Field {
    pub const ALL: [Self; 5] = [
        Self::Name,
        Self::Description,
        Self::Credit,
        Self::Style,
        Self::Music,
    ];
    pub fn tag(self) -> &'static str {
        match self {
            Self::Name => "CHARTNAME",
            Self::Description => "DESCRIPTION",
            Self::Credit => "CREDIT",
            Self::Style => "CHARTSTYLE",
            Self::Music => "MUSIC",
        }
    }
    fn index(self) -> usize {
        Self::ALL.iter().position(|field| *field == self).unwrap()
    }
}
pub struct Text {
    original: [String; 5],
    values: [String; 5],
}
impl Text {
    pub fn read(document: &Document, chart: usize) -> Result<Self, Error> {
        if chart >= document.charts().len() {
            return Err(Error::InvalidEdit("Chart text chart does not exist".into()));
        }
        let data=crate::generation::metadata::ChartData::read(document,chart)?;
        let authored=data.as_ref().and_then(|data|data.extra.get(EXTENSION));
        let mut values = std::array::from_fn(|_| String::new());
        for field in Field::ALL {
            let value = local_with_extension(document, chart, field, authored)?.unwrap_or_default();
            values[field.index()] = if field == Field::Credit
                && value.is_empty()
                && authored.and_then(|extension|extension["Fields"].get(field.tag())).is_none()
            {
                document.text(None, field.tag()).unwrap_or_default()
            } else {
                value
            };
        }
        Ok(Self {
            original: values.clone(),
            values,
        })
    }
    pub fn supports(&self, _field: Field) -> bool {
        true
    }
    pub(crate) fn original(&self, field: Field) -> &str { &self.original[field.index()] }
    pub fn get(&self, field: Field) -> &str {
        &self.values[field.index()]
    }
    pub fn set(&mut self, field: Field, value: &str) -> Result<(), Error> {
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
        if !self.has_changes() {
            return Ok(());
        }
        {
            let sm = session.document().format == Format::Sm;
            let mut metadata = crate::generation::metadata::ChartData::existing_or_initial(
                session.document(),
                chart,
            )?;
            let mut extension = metadata
                .extra
                .get(EXTENSION)
                .cloned()
                .unwrap_or_else(|| serde_json::json!({"Version":1,"Fields":{}}));
            validate_extension(&extension)?;
            let fields = extension
                .get_mut("Fields")
                .and_then(serde_json::Value::as_object_mut)
                .unwrap();
            for field in Field::ALL {
                if self.values[field.index()] == self.original[field.index()] {
                    continue;
                }
                fields.insert(field.tag().into(), self.get(field).into());
                if sm {
                    if field == Field::Description {
                        session.set_sm_header(chart, 1, self.get(field))?;
                    }
                } else {
                    session.set_text(Some(chart), field.tag(), self.get(field))?;
                }
            }
            metadata.extra.insert(EXTENSION.into(), extension);
            metadata.save_with_offset(session, chart, None, "Change chart text")?;
        }
        Ok(())
    }
}

/// NativeLossless extension inside the existing indexed chart metadata envelope.
/// Source-compatible writers materialize supported fields and remove this key.
pub const EXTENSION: &str = "DeadSyncChartText";
pub(crate) fn validate_extension(value: &serde_json::Value) -> Result<(), Error> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("Chart text extension must be an object"))?;
    if object.get("Version").and_then(serde_json::Value::as_u64) != Some(1) {
        return Err(Error::Unsupported(
            "Unknown native chart text version".into(),
        ));
    }
    let fields = object
        .get("Fields")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| invalid("Chart text extension has no Fields object"))?;
    for field in Field::ALL {
        if fields
            .get(field.tag())
            .is_some_and(|value| !value.is_string())
        {
            return Err(invalid("Chart text extension field must be a string"));
        }
    }
    Ok(())
}
/// Explicit chart scope, before MUSIC/CREDIT's source song fallback. Invalid
/// metadata is an error, never interpreted as an absent override.
pub fn local_value(
    document: &Document,
    chart: usize,
    field: Field,
) -> Result<Option<String>, Error> {
    if chart >= document.charts().len() {
        return Err(invalid("Chart text chart does not exist"));
    }
    let data=crate::generation::metadata::ChartData::read(document,chart)?;
    local_with_extension(document,chart,field,data.as_ref().and_then(|data|data.extra.get(EXTENSION)))
}
fn local_with_extension(document:&Document,chart:usize,field:Field,extension:Option<&serde_json::Value>)->Result<Option<String>,Error>{
    if let Some(value)=extension.and_then(|value|value["Fields"].get(field.tag())) {
        return Ok(Some(value.as_str().ok_or_else(||invalid("Chart text field must be a string"))?.into()));
    }
    if document.format == Format::Sm {
        return if field == Field::Description {
            document.sm_header_text(chart, 1).map(Some)
        } else {
            Ok(None)
        };
    }
    Ok(document.text(Some(chart), field.tag()))
}
fn invalid(message: &str) -> Error {
    Error::InvalidFormat(message.into())
}
