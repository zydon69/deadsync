//! GrooveAuthor song-level editor metadata. SyncOffset affects preview only.
use crate::{
    Error,
    document::Document,
    history::{RevisionToken, Session},
};
use serde_json::{Map, Value};
pub const DEFAULT_SYNC_OFFSET: f64 = 0.009;
const VERSION: &str = "GASongDataVersion";
const DATA: &str = "GASongData";

fn read(document: &Document) -> Result<(Map<String, Value>, f64), Error> {
    let fields = match (document.text(None, VERSION), document.text(None, DATA)) {
        (None, None) => return Ok((Map::new(), DEFAULT_SYNC_OFFSET)),
        (Some(version), Some(data)) if version.trim().parse::<i32>().ok() == Some(1) => {
            let data = crate::generation::jsonc::strip_comments(data.as_bytes())?;
            match serde_json::from_slice(&data).map_err(|e| Error::InvalidFormat(e.to_string()))? {
                Value::Object(fields) => fields,
                _ => return invalid("GrooveAuthor song metadata must be an object"),
            }
        }
        (Some(_), Some(_)) => {
            return Err(Error::Unsupported(
                "Unknown GrooveAuthor song metadata version".into(),
            ));
        }
        _ => return invalid("Incomplete GrooveAuthor song metadata"),
    };
    // V1's missing field uses the C# double default, not the open-song preference.
    let offset = fields
        .get("SyncOffset")
        .map(|value| {
            value
                .as_f64()
                .filter(|value| value.is_finite())
                .ok_or_else(|| Error::InvalidFormat("Invalid editor sync offset".into()))
        })
        .transpose()?
        .unwrap_or(0.0);
    // Keep the complete object, including future fields and default chart selection.
    Ok((fields, offset))
}
pub fn sync_offset(document: &Document) -> Result<f64, Error> {
    read(document).map(|(_, offset)| offset)
}

pub struct Draft {
    token: RevisionToken,
    original: f64,
    offset: f64,
    fields: Map<String, Value>,
}
impl Draft {
    pub fn capture(session: &Session) -> Result<Self, Error> {
        let (fields, offset) = read(session.document())?;
        Ok(Self {
            token: session.revision_token(),
            original: offset,
            offset,
            fields,
        })
    }
    pub fn sync_offset(&self) -> f64 {
        self.offset
    }
    pub fn set_sync_offset(&mut self, offset: f64) -> Result<(), Error> {
        if !offset.is_finite() {
            return invalid("Editor sync offset must be finite");
        }
        self.offset = offset;
        Ok(())
    }
    pub fn apply(&self, session: &mut Session) -> Result<bool, Error> {
        if !session.accepts_token(&self.token) {
            return invalid("Song properties draft is stale or belongs to another document");
        }
        if crate::number::reference_equal(self.original, self.offset) {
            return Ok(false);
        }
        let mut fields = self.fields.clone();
        fields.insert("SyncOffset".into(), Value::from(self.offset));
        let json =
            serde_json::to_string(&fields).map_err(|e| Error::InvalidFormat(e.to_string()))?;
        session.set_properties_atomic(
            None,
            &[(VERSION, "1"), (DATA, &json)],
            "Change editor sync offset",
        )
    }
}
fn invalid<T>(message: &str) -> Result<T, Error> {
    Err(Error::InvalidEdit(message.into()))
}
