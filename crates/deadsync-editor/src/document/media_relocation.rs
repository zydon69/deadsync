//! Relocate known song media and native chart MUSIC overrides without parsing a new Document or changing spans.
use super::{Document, decode_value, escape_text};
use crate::{Error, media_relocation::SourceBase};
use std::{ops::Range, path::Path, sync::Arc};
pub(crate) fn is_media(chart: Option<usize>, name: &str) -> bool {
    (chart.is_none() && name.eq_ignore_ascii_case(crate::song_text::resident::TAG))
        || is_chart_metadata(name)
        || name == "MUSIC"
        || chart.is_none()
            && matches!(
                name,
                "MUSIC"
                    | "PREVIEW"
                    | "LYRICSPATH"
                    | "PREVIEWVID"
                    | "BANNER"
                    | "BACKGROUND"
                    | "JACKET"
                    | "CDIMAGE"
                    | "DISCIMAGE"
                    | "CDTITLE"
            )
}
pub(crate) fn is_media_in(document: &Document, chart: Option<usize>, name: &str) -> bool {
    // A raw SM PREVIEW is preserved opaque data, not an active media reference.
    !(document.format == super::Format::Sm && chart.is_none() && name == "PREVIEW")
        && is_media(chart, name)
}
pub(crate) fn range_is_media(document: &Document, range: &Range<usize>) -> bool {
    document
        .tags
        .iter()
        .any(|tag| tag.value == *range && is_media_in(document, tag.chart, &tag.name))
}
fn is_chart_metadata(name: &str) -> bool {
    name == "GACHARTDATA"
        || name.strip_prefix("GACHARTDATACHART").is_some_and(|suffix| {
            suffix.len() >= 4 && suffix.bytes().all(|byte| byte.is_ascii_digit())
        })
}
#[derive(Clone, Copy)]
enum Payload {
    Path,
    ChartMusic,
    SongPreview,
}
fn payload(name: &str) -> Payload {
    if name.eq_ignore_ascii_case(crate::song_text::resident::TAG) {
        Payload::SongPreview
    } else if is_chart_metadata(&name.to_ascii_uppercase()) {
        Payload::ChartMusic
    } else {
        Payload::Path
    }
}
fn rebase_bytes(
    bytes: &[u8],
    payload: Payload,
    old: SourceBase<'_>,
    new: &Path,
) -> Result<Option<Arc<[u8]>>, Error> {
    if matches!(payload, Payload::SongPreview) {
        let before = crate::song_text::resident::decode(bytes)?;
        let after = old.rebase(new, &before)?;
        return if before == after {
            Ok(None)
        } else {
            crate::song_text::resident::encode(&after).map(Some)
        };
    }
    let text = decode_value(bytes, false);
    if matches!(payload, Payload::ChartMusic) {
        let stripped = crate::generation::jsonc::strip_comments(text.as_bytes())?;
        let mut data: serde_json::Value = serde_json::from_slice(&stripped)
            .map_err(|error| Error::InvalidFormat(error.to_string()))?;
        let Some(extension) = data.get_mut(crate::chart_text::EXTENSION) else {
            return Ok(None);
        };
        crate::chart_text::validate_extension(extension)?;
        let Some(value) = extension
            .get_mut("Fields")
            .and_then(|fields| fields.get_mut("MUSIC"))
        else {
            return Ok(None);
        };
        let before = value
            .as_str()
            .ok_or_else(|| Error::InvalidFormat("Native chart MUSIC must be a string".into()))?;
        let after = old.rebase(new, before)?;
        if after == before {
            return Ok(None);
        };
        *value = after.into();
        let json = serde_json::to_string(&data)
            .map_err(|error| Error::InvalidFormat(error.to_string()))?;
        return Ok(Some(Arc::from(escape_text(&json))));
    }
    let rebased = old.rebase(new, &text)?;
    Ok((rebased != text).then(|| Arc::from(escape_text(&rebased))))
}
pub(crate) fn replacement(
    document: &Document,
    range: &Range<usize>,
    value: &Option<Arc<[u8]>>,
    old: SourceBase<'_>,
    new: &Path,
) -> Result<Option<Arc<[u8]>>, Error> {
    let bytes = value
        .as_deref()
        .unwrap_or_else(|| &document.source[range.clone()]);
    let payload = document
        .tags
        .iter()
        .find(|tag| tag.value == *range)
        .map_or(Payload::Path, |tag| payload(&tag.name));
    Ok(rebase_bytes(bytes, payload, old, new)?.or_else(|| value.clone()))
}
pub(crate) fn added(
    name: &str,
    value: &Option<Arc<[u8]>>,
    old: SourceBase<'_>,
    new: &Path,
) -> Result<Option<Arc<[u8]>>, Error> {
    value
        .as_ref()
        .map(|bytes| {
            Ok(rebase_bytes(bytes, payload(name), old, new)?.unwrap_or_else(|| Arc::clone(bytes)))
        })
        .transpose()
}
pub(crate) fn document(
    source: &Document,
    old: SourceBase<'_>,
    new: &Path,
) -> Result<Document, Error> {
    crate::song_text::resident::read(source)?;
    let mut result = source.clone();
    for tag in &source.tags {
        if is_media_in(source, tag.chart, &tag.name) {
            let value = replacement(
                source,
                &tag.value,
                &source.replacement(&tag.value),
                old,
                new,
            )?;
            result.replace(tag.value.clone(), value)?;
        }
    }
    for ((chart, name), value) in &source.added {
        if is_media_in(source, *chart, name) {
            // Preserve the exact spelling table and avoid rebuilding added tags.
            let value = added(name, &Some(Arc::clone(value)), old, new)?.unwrap();
            result.added.insert((*chart, name.clone()), value);
        }
    }
    Ok(result)
}
