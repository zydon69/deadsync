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
pub fn read(
    bytes: Arc<[u8]>,
    format: Format,
    chart: usize,
    cancelled: &AtomicBool,
) -> Result<SourceChart, Error> {
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
    checkpoint()?;
    if grid.iter().any(|(_, note)| note.kind == Kind::Attack) {
        return Err(Error::Unsupported(
            "Reference interpretation does not support attack notes".into(),
        ));
    }
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
    let timing = GenerationTiming::from_document(&document, chart)?;
    let difficulty = entry
        .meter
        .parse::<f64>()
        .map_err(|_| Error::InvalidFormat("Invalid chart difficulty".into()))?;
    if !difficulty.is_finite() {
        return Err(Error::InvalidFormat(
            "Chart difficulty must be finite".into(),
        ));
    }
    Ok(SourceChart {
        layout: entry.steps_type.clone(),
        grid,
        timing,
        difficulty,
        document,
        chart,
    })
}

/// Parsed once, then shared by source interpretation and pattern preparation.
/// The worker owns these values; no live editor document is borrowed.
pub struct SourceChart {
    pub layout: String,
    pub grid: Grid,
    pub timing: GenerationTiming,
    pub difficulty: f64,
    document: Document,
    chart: usize,
}

pub fn prepare(
    bytes: Arc<[u8]>,
    format: Format,
    chart: usize,
    cancelled: &AtomicBool,
) -> Result<PreparedChart, Error> {
    let source = read(bytes, format, chart, cancelled)?;
    source.prepare_grid(&source.grid, cancelled)
}

/// Native editor conversion mirrors GenerateSmEvents: consume the editor's
/// assigned event times, rather than recompute the standalone Fumen clock.
pub fn prepare_editor(
    bytes: Arc<[u8]>,
    format: Format,
    chart: usize,
    cancelled: &AtomicBool,
) -> Result<PreparedChart, Error> {
    let source = read(bytes, format, chart, cancelled)?;
    source.prepare_editor(cancelled)
}

impl SourceChart {
    pub fn prepare_editor(&self, cancelled: &AtomicBool) -> Result<PreparedChart, Error> {
        let regions = self.pattern_regions(cancelled)?;
        let timeline =
            super::editor_timing::EditorTimeline::build(&self.timing, &self.grid, None, cancelled)?
                .with_markers(&regions, cancelled)?;
        let region = timeline.prepare_region(&self.grid, Some((0, i32::MAX as u64)), cancelled)?;
        let mut mines = timeline
            .heads()
            .iter()
            .filter(|head| head.kind == Kind::Mine)
            .map(|head| SourceMine {
                row: head.position.row,
                lane: head.position.lane,
                time: head.time,
            })
            .collect::<Vec<_>>();
        mines.sort_by_key(|mine| (mine.row, mine.lane));
        if cancelled.load(Ordering::Relaxed) {
            return Err(Error::InvalidEdit(
                "Generation preparation cancelled".into(),
            ));
        }
        Ok(PreparedChart {
            layout: self.layout.clone(),
            frames: region.frames,
            times: region.times,
            event_span: region.event_span,
            difficulty: self.difficulty,
            mines,
            source_notes: self
                .grid
                .iter()
                .filter(|(_, note)| !matches!(note.kind, Kind::Mine | Kind::Keysound))
                .count(),
            ignored_keysounds: self
                .grid
                .iter()
                .filter(|(_, note)| note.kind == Kind::Keysound)
                .count(),
        })
    }

    pub fn initial_editor_metadata(&self) -> Result<super::metadata::ChartData, Error> {
        super::metadata::ChartData::existing_or_initial(&self.document, self.chart)
    }

    /// Only editor consumers interpret custom metadata. Standalone Fumen
    /// preparation ignores editor-specific data, as the original library does.
    pub fn pattern_regions(&self, cancelled: &AtomicBool) -> Result<Vec<(u64, u64)>, Error> {
        if cancelled.load(Ordering::Relaxed) {
            return Err(Error::InvalidEdit("Pattern loading cancelled".into()));
        }
        let data = super::metadata::ChartData::read(&self.document, self.chart)?;
        let mut regions = Vec::new();
        for (row, marker) in data.and_then(|data| data.patterns).unwrap_or_default() {
            if cancelled.load(Ordering::Relaxed) {
                return Err(Error::InvalidEdit("Pattern loading cancelled".into()));
            }
            regions.push((row, row + marker.length));
        }
        Ok(regions)
    }

    /// Recompute Fumen engine timestamps for this grid. GrooveAuthor pattern
    /// contexts instead retain their whole-editor timeline's assigned times;
    /// they must not be built by recomputing a standalone partial-grid clock.
    pub fn prepare_grid(
        &self,
        grid: &Grid,
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
        if grid.players() != 1
            || layout_columns(&self.layout) != Some(grid.columns())
            || !self.difficulty.is_finite()
        {
            return Err(Error::InvalidEdit(
                "Interpretation grid does not match its source layout".into(),
            ));
        }
        let timing = &self.timing;
        let frames = expressed::frames(grid, 0, cancelled)?;
        let times =
            timing.grouped_times_checked(frames.iter().map(|frame| frame.row), checkpoint)?;
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
        Ok(PreparedChart {
            source_notes: grid
                .iter()
                .filter(|(_, note)| !matches!(note.kind, Kind::Mine | Kind::Keysound))
                .count(),
            ignored_keysounds: grid
                .iter()
                .filter(|(_, note)| note.kind == Kind::Keysound)
                .count(),
            layout: self.layout.clone(),
            frames,
            times,
            mines,
            difficulty: self.difficulty,
            event_span,
        })
    }
}
