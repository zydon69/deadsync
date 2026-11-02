//! Explicit local attack lists in SM. Presence, including an empty list, detaches
//! a resident chart from the original song-level ATTACKS fallback.
use crate::{
    Error,
    document::{Document, Format, escape_text},
    history::Session,
};
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, sync::Arc};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    version: u32,
    source: Vec<u8>,
}
pub(crate) fn supports_chart(document: &Document) -> bool {
    document.format == Format::Sm || crate::timing_edit::supports_chart_timing(document)
}
pub(crate) fn tag(chart: usize) -> Result<String, Error> {
    if chart > 9999 {
        return Err(invalid(
            "SM attack chart exceeds four-digit metadata limits",
        ));
    }
    Ok(format!("DSChartAttacksCHART{chart:04}"))
}
pub(crate) fn is_owner_tag(name: &str) -> bool {
    name.to_ascii_uppercase()
        .strip_prefix("DSCHARTATTACKSCHART")
        .is_some_and(|index| index.len() == 4 && index.bytes().all(|byte| byte.is_ascii_digit()))
}
pub(crate) fn read(
    document: &Document,
    chart: Option<usize>,
) -> Result<Option<Cow<'_, [u8]>>, Error> {
    let Some(chart) = chart.filter(|_| document.format == Format::Sm) else {
        return Ok(document.raw_property(chart, "ATTACKS").map(Cow::Borrowed));
    };
    if chart >= document.charts().len() {
        return Err(invalid("Local attack chart does not exist"));
    }
    let name = tag(chart)?;
    if document
        .tags()
        .iter()
        .filter(|entry| entry.chart.is_none() && entry.name.eq_ignore_ascii_case(&name))
        .count()
        > 1
    {
        return Err(invalid(
            "Repeated local attack owners require explicit repair",
        ));
    }
    let Some(text) = document.text(None, &name) else {
        return Ok(None);
    };
    let value: Stored =
        serde_json::from_str(&text).map_err(|_| invalid("Malformed local SM attack owner"))?;
    if value.version != 1 {
        return Err(invalid("Unsupported local SM attack owner version"));
    }
    super::Attacks::parse(&value.source)?;
    Ok(Some(Cow::Owned(value.source)))
}
pub(crate) fn change(
    document: &Document,
    chart: Option<usize>,
    source: Arc<[u8]>,
) -> Result<(Option<usize>, String, Arc<[u8]>), Error> {
    if let Some(chart) = chart.filter(|_| document.format == Format::Sm) {
        if chart >= document.charts().len() {
            return Err(invalid("Local attack chart does not exist"));
        }
        super::Attacks::parse(&source)?;
        let name = tag(chart)?;
        // Never replace a future/malformed owner implicitly.
        read(document, Some(chart))?;
        let text = serde_json::to_string(&Stored {
            version: 1,
            source: source.to_vec(),
        })
        .map_err(|_| invalid("Cannot encode local SM attacks"))?;
        Ok((None, name, Arc::from(escape_text(&text))))
    } else {
        Ok((chart, "ATTACKS".into(), source))
    }
}
pub(crate) fn write(
    session: &mut Session,
    chart: Option<usize>,
    source: Arc<[u8]>,
) -> Result<bool, Error> {
    let (scope, name, value) = change(session.document(), chart, source)?;
    session.set_property_bytes(scope, &name, value)
}
pub(crate) fn empty(document: &mut Document, chart: usize) -> Result<(), Error> {
    let (scope, name, value) = change(document, Some(chart), Arc::from(&b""[..]))?;
    if document.raw_property(scope, &name).is_some() {
        return Err(invalid("Local attacks occupy the new chart index"));
    }
    document.set_added_property(scope, &name, Some(value))
}
pub(crate) fn clone_chart(
    source: &Document,
    from: usize,
    target: &mut Document,
    to: usize,
) -> Result<(), Error> {
    read(source, Some(from))?;
    for prefix in [
        "DSChartAttacksCHART",
        "DSAttackRowsLocalCHART",
        "DSAttackRowsCHART",
    ] {
        let origin = format!("{prefix}{from:04}");
        let destination = format!("{prefix}{to:04}");
        if target.raw_property(None, &destination).is_some() {
            return Err(invalid("Attack metadata occupies the new chart index"));
        }
        if let Some(raw) = source.raw_property(None, &origin) {
            target.set_added_property(None, &destination, Some(Arc::from(raw)))?;
        }
    }
    Ok(())
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
