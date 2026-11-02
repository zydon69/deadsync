//! Authored editor timing is independent of the original SM reader's tag set.
//! Ordinary unsupported SM tags remain opaque. Only this versioned owner is
//! authoritative, and it is never exposed through Document::raw_property.
use crate::{
    Error,
    document::{Document, Format, escape_text},
    history::Session,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};
const TAGS: [&str; 13] = [
    "BPMS",
    "STOPS",
    "DELAYS",
    "TIMESIGNATURES",
    "TICKCOUNTS",
    "WARPS",
    "FAKES",
    "COMBOS",
    "LABELS",
    "SCROLLS",
    "SPEEDS",
    "OFFSET",
    "DISPLAYBPM",
];
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    version: u32,
    fields: BTreeMap<String, String>,
}
/// One parsed read snapshot per clock/inspector, never a filesystem operation.
pub(crate) struct Properties {
    fields: BTreeMap<String, String>,
    resident: bool,
}
pub(crate) fn tag(chart: Option<usize>) -> Result<String, Error> {
    match chart {
        None => Ok("DSTimingSong".into()),
        Some(chart) if chart <= 9999 => Ok(format!("DSTimingCHART{chart:04}")),
        _ => Err(invalid(
            "Resident timing chart exceeds four-digit metadata limits",
        )),
    }
}
pub(crate) fn has_local(document: &Document, chart: usize) -> bool {
    document.format == Format::Sm
        && tag(Some(chart))
            .ok()
            .is_some_and(|tag| document.raw_property(None, &tag).is_some())
}
impl Properties {
    pub fn capture(document: &Document, origin: Option<usize>) -> Result<Self, Error> {
        if origin.is_some_and(|chart| chart >= document.charts().len()) {
            return Err(invalid("Resident timing chart does not exist"));
        }
        if document.format == Format::Sm {
            let tag = tag(origin)?;
            if document
                .tags()
                .iter()
                .filter(|entry| entry.chart.is_none() && entry.name.eq_ignore_ascii_case(&tag))
                .count()
                > 1
            {
                return Err(invalid(
                    "Repeated resident timing owners require explicit repair",
                ));
            }
            if let Some(text) = document.text(None, &tag) {
                let stored: Stored = serde_json::from_str(&text)
                    .map_err(|_| invalid("Malformed resident timing owner"))?;
                if stored.version != 1
                    || stored.fields.len() != TAGS.len()
                    || !TAGS.iter().all(|tag| stored.fields.contains_key(*tag))
                {
                    return Err(invalid("Unsupported or incomplete resident timing owner"));
                }
                return Ok(Self {
                    fields: stored.fields,
                    resident: true,
                });
            }
        }
        let fields = TAGS
            .iter()
            .map(|&tag| {
                let extended = matches!(
                    tag,
                    "WARPS" | "FAKES" | "COMBOS" | "LABELS" | "SCROLLS" | "SPEEDS"
                );
                let value = if document.format == Format::Sm && (origin.is_some() || extended) {
                    String::new()
                } else {
                    document
                        .text(origin, tag)
                        .or_else(|| {
                            (tag == "STOPS" && document.format == Format::Sm)
                                .then(|| document.text(None, "FREEZES"))
                                .flatten()
                        })
                        .unwrap_or_default()
                };
                let value = if document.format == Format::Sm && origin.is_none() && tag == "STOPS" {
                    let parts: Vec<_> = document
                        .tags()
                        .iter()
                        .filter(|entry| {
                            entry.chart.is_none()
                                && matches!(entry.name.as_str(), "STOPS" | "FREEZES")
                        })
                        .map(|entry| {
                            rssp::parse::decode_unescape(document.value(entry.value.clone()))
                                .into_owned()
                        })
                        .collect();
                    if parts.is_empty() {
                        value
                    } else {
                        parts.join(",")
                    }
                } else {
                    value
                };
                (tag.to_owned(), value)
            })
            .collect();
        Ok(Self {
            fields,
            resident: false,
        })
    }
    pub fn text(&self, tag: &str) -> String {
        self.fields.get(tag).cloned().unwrap_or_default()
    }
    pub fn is_resident(&self) -> bool {
        self.resident
    }
}
pub(crate) fn text(document: &Document, origin: Option<usize>, tag: &str) -> Result<String, Error> {
    Ok(Properties::capture(document, origin)?.text(tag))
}
/// Detachment snapshots the effective owner, including empty extended fields;
/// it never activates an unsupported tag retained in the original SM bytes.
pub(crate) fn apply(
    session: &mut Session,
    chart: usize,
    scope: super::Scope,
    changes: &[(&str, String)],
    label: &str,
) -> Result<bool, Error> {
    let target = (scope == super::Scope::Chart).then_some(chart);
    let origin = target.filter(|&chart| has_local(session.document(), chart));
    let mut fields = Properties::capture(session.document(), origin)?.fields;
    for (name, value) in changes {
        if !TAGS.contains(name) {
            return Err(invalid("Unknown resident timing field"));
        }
        fields.insert((*name).into(), value.clone());
    }
    let value = serde_json::to_string(&Stored { version: 1, fields })
        .map_err(|_| invalid("Cannot encode resident timing owner"))?;
    session.set_property_bytes_atomic(
        &[(None, tag(target)?, Arc::from(escape_text(&value)))],
        label,
    )
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}

pub(crate) fn is_tag(name: &str) -> bool {
    name.eq_ignore_ascii_case("DSTimingSong")
        || name
            .to_ascii_uppercase()
            .strip_prefix("DSTIMINGCHART")
            .is_some_and(|suffix| {
                suffix.len() == 4 && suffix.bytes().all(|byte| byte.is_ascii_digit())
            })
}
pub(crate) fn clone_chart_owner(
    source: &Document,
    from: usize,
    target: &mut Document,
    to: usize,
) -> Result<(), Error> {
    let origin = tag(Some(from))?;
    let destination = tag(Some(to))?;
    if target.raw_property(None, &destination).is_some() {
        return Err(invalid("Resident timing occupies the new chart index"));
    }
    if let Some(raw) = source.raw_property(None, &origin) {
        // Parse before copying, so malformed authoring state cannot be silently
        // attached to a newly generated chart.
        Properties::capture(source, Some(from))?;
        target.set_added_property(None, &destination, Some(Arc::from(raw)))?;
    }
    Ok(())
}

pub(crate) fn initial_chart(
    tempo: f64,
    signature: super::signature::Signature,
    offset: f64,
) -> Result<Arc<[u8]>, Error> {
    let mut fields: BTreeMap<String, String> = TAGS
        .iter()
        .map(|tag| ((*tag).to_owned(), String::new()))
        .collect();
    for (tag, value) in [
        ("BPMS", format!("0={tempo}")),
        (
            "TIMESIGNATURES",
            format!("0={}={}", signature.numerator, signature.denominator),
        ),
        ("OFFSET", offset.to_string()),
    ] {
        fields.insert(tag.into(), value);
    }
    let json = serde_json::to_string(&Stored { version: 1, fields })
        .map_err(|_| invalid("Cannot encode initial resident timing"))?;
    Ok(Arc::from(escape_text(&json)))
}
