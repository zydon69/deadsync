//! Native lossless ownership for explicitly authored SM preview media.
//! The original SM PREVIEW tag remains opaque and is never activated on load.
use crate::{
    Error,
    document::{Document, Format, escape_text},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
pub const TAG: &str = "DSSongPreview";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    version: u32,
    path: String,
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
fn validate_path(path: &str) -> Result<(), Error> {
    if path.chars().any(char::is_control) {
        return Err(invalid("Resident song preview path must be a single line"));
    }
    Ok(())
}
fn parse(text: &str) -> Result<String, Error> {
    let stored: Stored =
        serde_json::from_str(text).map_err(|_| invalid("Malformed resident song preview owner"))?;
    if stored.version != 1 {
        return Err(invalid("Unsupported resident song preview owner version"));
    }
    validate_path(&stored.path)?;
    Ok(stored.path)
}
/// Decode an escaped property payload, including history before/after values.
pub fn decode(bytes: &[u8]) -> Result<String, Error> {
    let mut source = format!("#{TAG}:").into_bytes();
    let start = source.len();
    source.extend_from_slice(bytes);
    source.push(b';');
    let document = Document::parse(source, Format::Sm)?;
    let tag = document
        .tag(None, TAG)
        .ok_or_else(|| invalid("Malformed resident song preview owner"))?;
    if document.tags().len() != 1 || tag.value != (start..start + bytes.len()) {
        return Err(invalid("Malformed resident song preview owner"));
    }
    parse(&document.text(None, TAG).unwrap_or_default())
}
/// None is an unowned source field; Some("") is an authored Clear.
pub fn read(document: &Document) -> Result<Option<String>, Error> {
    let owners: Vec<_> = document
        .tags()
        .iter()
        .filter(|tag| tag.name.eq_ignore_ascii_case(TAG))
        .collect();
    if owners.len() > 1
        || owners.iter().any(|tag| tag.chart.is_some())
        || (0..document.charts().len())
            .any(|chart| document.raw_property(Some(chart), TAG).is_some())
    {
        return Err(invalid(
            "Repeated or chart-scoped resident song preview owners require explicit repair",
        ));
    }
    document.raw_property(None, TAG).map(decode).transpose()
}
pub fn effective(document: &Document) -> Result<String, Error> {
    let authored = read(document)?;
    Ok(if document.format == Format::Sm {
        authored.unwrap_or_default()
    } else {
        document.text(None, "PREVIEW").unwrap_or_default()
    })
}
pub fn encode(path: &str) -> Result<Arc<[u8]>, Error> {
    validate_path(path)?;
    let json = serde_json::to_string(&Stored {
        version: 1,
        path: path.to_owned(),
    })
    .map_err(|_| invalid("Cannot encode resident song preview owner"))?;
    Ok(Arc::from(escape_text(&json)))
}
fn set(document: &mut Document, name: &str, value: Arc<[u8]>) -> Result<(), Error> {
    // Match Document::raw_property precedence: an existing added override
    // remains authoritative even when the original physical tag also exists.
    if document.added_property(None, name).is_some() {
        if let Some(tag) = document.tag(None, name) {
            let range = tag.value.clone();
            document.set_added_property(None, name, None)?;
            document.replace(range, Some(value))
        } else {
            document.set_added_property(None, name, Some(value))
        }
    } else if let Some(tag) = document.tag(None, name) {
        document.replace(tag.value.clone(), Some(value))
    } else {
        document.set_added_property(None, name, Some(value))
    }
}
pub fn apply(document: &Document, path: &str) -> Result<Document, Error> {
    read(document)?;
    if document.format != Format::Sm {
        return Err(invalid("Resident song preview owner requires SM authoring"));
    }
    let mut output = document.clone();
    set(&mut output, TAG, encode(path)?)?;
    Ok(output)
}
/// Prepare source-effective media for export; the writer excludes the native
/// owner. In particular, SM opaque PREVIEW must not become active through SSC.
pub fn materialize_for_output(document: &Document, target: Format) -> Result<Document, Error> {
    read(document)?;
    let mut output = document.clone();
    if target == Format::Ssc && document.format == Format::Sm {
        let value = effective(document)?;
        set(&mut output, "PREVIEW", Arc::from(escape_text(&value)))?;
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn installing_existing_added_owner_wins_over_original_physical_tag() {
        let raw = encode("physical.ogg").unwrap();
        let bytes = [b"#DSSongPreview:".as_slice(), raw.as_ref(), b";"].concat();
        let mut document = Document::parse(bytes, Format::Sm).unwrap();
        document
            .set_added_property(None, TAG, Some(encode("added.ogg").unwrap()))
            .unwrap();
        let changed = apply(&document, "accepted.ogg").unwrap();
        assert_eq!(effective(&changed).unwrap(), "accepted.ogg");
        let reparsed = changed.reparse(changed.serialize()).unwrap();
        assert_eq!(effective(&reparsed).unwrap(), "accepted.ogg");
        assert_eq!(
            reparsed
                .tags()
                .iter()
                .filter(|tag| tag.name.eq_ignore_ascii_case(TAG))
                .count(),
            1
        );
    }
    #[test]
    fn chart_scoped_added_owner_is_rejected_before_serialization() {
        let mut document = Document::parse(
            b"#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;".as_slice(),
            Format::Ssc,
        )
        .unwrap();
        document
            .set_added_property(Some(0), TAG, Some(encode("wrong-scope.ogg").unwrap()))
            .unwrap();
        assert!(read(&document).is_err());
    }
    #[test]
    fn export_preparation_replaces_added_preview_instead_of_stale_physical_value() {
        let mut document =
            Document::parse(b"#PREVIEW:physical.ogg;".as_slice(), Format::Sm).unwrap();
        document
            .set_added_property(
                None,
                "PREVIEW",
                Some(Arc::from(escape_text("added-opaque.ogg"))),
            )
            .unwrap();
        let output = materialize_for_output(&document, Format::Ssc).unwrap();
        assert_eq!(output.text(None, "PREVIEW").as_deref(), Some(""));
        let reparsed = output.reparse(output.serialize()).unwrap();
        assert_eq!(reparsed.text(None, "PREVIEW").as_deref(), Some(""));
        assert_eq!(
            reparsed
                .tags()
                .iter()
                .filter(|tag| tag.name == "PREVIEW")
                .count(),
            1
        );
    }
}
