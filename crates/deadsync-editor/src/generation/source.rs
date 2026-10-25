//! Immutable SM/SSC adapter for the original expressed-chart interpreter.
//! Movement frames use first-event-of-row times; mines retain note times.
use super::{
    expressed::{self, Frame, PolicyRequest},
    mines::SourceMine,
    timing::GenerationTiming,
};
use crate::{
    Error,
    document::{Document, Format},
    notes::{Grid, Kind, layout_columns},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub struct PreparedChart {
    pub layout: String,
    pub frames: Vec<Frame>,
    pub times: Vec<f64>,
    pub mines: Vec<SourceMine>,
    pub ignored_keysounds: usize,
    pub source_notes: usize,
    pub difficulty: f64,
    pub event_span: [f64; 2],
}
impl PreparedChart {
    pub fn request(&self) -> PolicyRequest<'_> {
        PolicyRequest {
            frames: &self.frames,
            times: &self.times,
            difficulty: self.difficulty,
            event_span: self.event_span,
        }
    }
}

/// Call with a serialized editor snapshot so unsaved notes, timing and metadata
/// are included together. The snapshot is never mutated during preparation.
pub fn prepare(
    bytes: Arc<[u8]>,
    format: Format,
    chart: usize,
    cancelled: &AtomicBool,
) -> Result<PreparedChart, Error> {
    let checkpoint = || {
        if cancelled.load(Ordering::Relaxed) {
            Err(Error::InvalidEdit(
                "Generation preparation cancelled".into(),
            ))
        } else {
            Ok(())
        }
    };
    checkpoint()?;
    let document = Document::parse(bytes.clone(), format)?;
    let entry = document
        .charts()
        .get(chart)
        .ok_or_else(|| Error::InvalidEdit("Chart does not exist".into()))?;
    let columns = layout_columns(&entry.steps_type)
        .ok_or_else(|| Error::Unsupported("Unknown generation layout".into()))?;
    let grid = Grid::parse(document.value(entry.notes.clone()), columns)?;
    if grid.players() != 1 {
        return Err(Error::Unsupported(
            "Generation requires a chart with one participant".into(),
        ));
    }
    let parsed = rssp::parse::extract_sections(&bytes, format.extension())?;
    let parsed_chart = parsed
        .notes_list
        .get(chart)
        .ok_or_else(|| Error::InvalidFormat("RSSP did not parse the selected chart".into()))?;
    if [
        parsed.attacks.as_deref(),
        parsed_chart.chart_attacks.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|value| !rssp::parse::decode_unescape(value).trim().is_empty())
    {
        return Err(Error::Unsupported(
            "Attack event timing is not ported for generation yet".into(),
        ));
    }
    let timing = GenerationTiming::from_simfile(&bytes, format, chart)?;
    let frames = expressed::frames(&grid, 0, cancelled)?;
    let times = timing.grouped_times_checked(frames.iter().map(|frame| frame.row), checkpoint)?;
    let mut note_rows = Vec::new();
    let mut mine_positions = Vec::new();
    for (position, note) in grid.iter() {
        checkpoint()?;
        note_rows.push(position.row);
        if let Some(end) = note.end {
            note_rows.push(end);
        }
        if note.kind == Kind::Mine {
            mine_positions.push((position.row, position.lane));
        }
    }
    mine_positions.sort_unstable();
    let mine_times =
        timing.times_checked(mine_positions.iter().map(|(row, _)| *row), checkpoint)?;
    let mines = mine_positions
        .into_iter()
        .zip(mine_times)
        .map(|((row, lane), time)| SourceMine { row, lane, time })
        .collect();
    let event_span = timing.event_span_checked(note_rows, checkpoint)?;
    let difficulty = entry
        .meter
        .parse::<f64>()
        .map_err(|_| Error::InvalidFormat("Invalid chart difficulty".into()))?;
    if !difficulty.is_finite() {
        return Err(Error::InvalidFormat(
            "Chart difficulty must be finite".into(),
        ));
    }
    Ok(PreparedChart {
        source_notes: grid
            .iter()
            .filter(|(_, note)| !matches!(note.kind, Kind::Mine | Kind::Keysound))
            .count(),
        ignored_keysounds: grid
            .iter()
            .filter(|(_, note)| note.kind == Kind::Keysound)
            .count(),
        layout: entry.steps_type.clone(),
        frames,
        times,
        mines,
        difficulty,
        event_span,
    })
}
