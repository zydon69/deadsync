//! Editor analysis reuses the exact RSSP dependency pinned by DeadSync. The
//! byte-preserving document remains the authority for file writes: an analysis
//! summary does not contain unknown vendor properties or every editable layout.
use crate::{Error, document::Format, history::Session};

/// Analyze a snapshot rather than the on-disk file, so unsaved notes and timing
/// edits are reflected. Call from a worker for interactive long-file analysis.
pub fn chart(
    session: &Session,
    chart_index: usize,
    options: &rssp::AnalysisOptions,
) -> Result<rssp::ChartSummary, Error> {
    let bytes = session.serialize()?;
    chart_bytes(&bytes, session.document().format, chart_index, options)
}

pub fn chart_bytes(
    bytes: &[u8],
    format: Format,
    chart_index: usize,
    options: &rssp::AnalysisOptions,
) -> Result<rssp::ChartSummary, Error> {
    let parsed = rssp::parse::extract_sections(bytes, format.extension())?;
    let selected = parsed
        .notes_list
        .get(chart_index)
        .ok_or_else(|| Error::InvalidEdit("Chart does not exist".into()))?;
    if rssp::supported_stepstype_lanes_bytes(selected.fields[0]).is_none() {
        return Err(Error::Unsupported(format!(
            "RSSP analysis does not support layout {} directly",
            rssp::parse::decode_unescape_trim(selected.fields[0])
        )));
    }
    let summary_index = parsed.notes_list[..chart_index]
        .iter()
        .filter(|entry| {
            entry.field_count >= 5
                && rssp::supported_stepstype_lanes_bytes(entry.fields[0]).is_some()
        })
        .count();
    let mut summary =
        rssp::analyze(bytes, format.extension(), options).map_err(Error::InvalidFormat)?;
    if summary_index >= summary.charts.len() {
        return Err(Error::InvalidFormat(
            "RSSP could not analyze the selected chart".into(),
        ));
    }
    Ok(summary.charts.remove(summary_index))
}
