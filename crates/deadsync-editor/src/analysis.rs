//! Editor analysis reuses the exact RSSP dependency pinned by DeadSync. The
//! byte-preserving document remains the authority for file writes: an analysis
//! summary does not contain unknown vendor properties or every editable layout.
pub mod task;
mod resident;
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
    if format==Format::Sm {
        if let Some(projection)=resident::project(bytes,chart_index,selected)? {
            let mut summary=chart_bytes(&projection.bytes,Format::Ssc,0,options)?;
            projection.restore_sm_metadata(&mut summary);
            return Ok(summary);
        }
    }
    let summary_index = parsed.notes_list[..chart_index]
        .iter()
        .filter(|entry| {
            entry.field_count >= 5
                && rssp::supported_stepstype_lanes_bytes(entry.fields[0]).is_some()
        })
        .count();
    // RSSP's row scanner requires measure separators on their own lines. Normalize
    // only this selected note field in an analysis copy; source bytes stay intact.
    let normalized = normalized_notes(bytes, selected.note_data)?;
    let mut summary =
        rssp::analyze(&normalized, format.extension(), options).map_err(Error::InvalidFormat)?;
    if summary_index >= summary.charts.len() {
        return Err(Error::InvalidFormat(
            "RSSP could not analyze the selected chart".into(),
        ));
    }
    Ok(summary.charts.remove(summary_index))
}

fn normalized_notes<'a>(
    bytes: &'a [u8],
    notes: &[u8],
) -> Result<std::borrow::Cow<'a, [u8]>, Error> {
    if notes.is_empty() {
        return Ok(std::borrow::Cow::Borrowed(bytes));
    }
    let start = (notes.as_ptr() as usize)
        .checked_sub(bytes.as_ptr() as usize)
        .filter(|start| *start <= bytes.len())
        .ok_or_else(|| Error::InvalidFormat("Analysis note span is outside its source".into()))?;
    let end = start
        .checked_add(notes.len())
        .filter(|end| *end <= bytes.len())
        .ok_or_else(|| Error::InvalidFormat("Analysis note span exceeds its source".into()))?;
    let text = rssp::parse::decode_bytes(notes);
    let blocks = crate::notes::split_outer(&text, ',')?;
    let mut normalized = blocks[0].clone();
    for block in &blocks[1..] {
        if !normalized.ends_with('\n') {
            normalized.push('\n');
        }
        normalized.push(',');
        if !block.starts_with('\n') {
            normalized.push('\n');
        }
        normalized.push_str(block);
    }
    if normalized.as_bytes() == notes {
        return Ok(std::borrow::Cow::Borrowed(bytes));
    }
    let mut snapshot = Vec::with_capacity(bytes.len() - notes.len() + normalized.len());
    snapshot.extend_from_slice(&bytes[..start]);
    snapshot.extend_from_slice(normalized.as_bytes());
    snapshot.extend_from_slice(&bytes[end..]);
    Ok(std::borrow::Cow::Owned(snapshot))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalization_uses_the_note_lexer_and_keeps_attribute_commas_and_other_source_bytes() {
        let source = b"prefix:1{mod,1.5}000, // comma, in comment\n0100:suffix";
        let start = b"prefix:".len();
        let end = source.len() - b":suffix".len();
        let normalized = normalized_notes(source, &source[start..end]).unwrap();
        assert_eq!(
            normalized.as_ref(),
            b"prefix:1{mod,1.5}000\n,\n \n0100:suffix"
        );
        assert_eq!(
            source,
            b"prefix:1{mod,1.5}000, // comma, in comment\n0100:suffix"
        );
    }
    #[test]
    fn malformed_attributes_fail_without_modifying_the_source() {
        let source = b"#NOTES:1{bad,000;";
        let before = source.to_vec();
        assert!(normalized_notes(source, &source[7..source.len() - 1]).is_err());
        assert_eq!(source.as_slice(), before);
    }
}
