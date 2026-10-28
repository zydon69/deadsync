//! Relocate known song media and native chart MUSIC overrides without parsing a new Document or changing spans.
use super::{Document, decode, escape_text};
use crate::{Error,media_relocation::SourceBase};
use std::{ops::Range, path::Path, sync::Arc};
pub(crate) fn is_media(chart: Option<usize>, name: &str) -> bool {
    name == "MUSIC"
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
pub(crate) fn range_is_media(document: &Document, range: &Range<usize>) -> bool {
    document
        .tags
        .iter()
        .any(|tag| tag.value == *range && is_media(tag.chart, &tag.name))
}
fn rebase_bytes(bytes: &[u8], old: SourceBase<'_>, new: &Path) -> Result<Option<Arc<[u8]>>, Error> {
    let text = decode(bytes);
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
    Ok(rebase_bytes(bytes, old, new)?.or_else(|| value.clone()))
}
pub(crate) fn added(
    value: &Option<Arc<[u8]>>,
    old: SourceBase<'_>,
    new: &Path,
) -> Result<Option<Arc<[u8]>>, Error> {
    value
        .as_ref()
        .map(|bytes| Ok(rebase_bytes(bytes, old, new)?.unwrap_or_else(|| Arc::clone(bytes))))
        .transpose()
}
pub(crate) fn document(source: &Document, old: SourceBase<'_>, new: &Path) -> Result<Document, Error> {
    let mut result = source.clone();
    for tag in &source.tags {
        if is_media(tag.chart, &tag.name) {
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
        if is_media(*chart, name) {
            // Preserve the exact spelling table and avoid rebuilding added tags.
            let value = added(&Some(Arc::clone(value)), old, new)?.unwrap();
            result.added.insert((*chart, name.clone()), value);
        }
    }
    Ok(result)
}
