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
    crate::timing_edit::migration::require_compatible(&document)?;
    let entry = document
        .charts()
        .get(chart)
        .ok_or_else(|| Error::InvalidEdit("Chart does not exist".into()))?;
    let grid = Grid::parse_layout(document.value(entry.notes.clone()), &entry.steps_type)?;
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
    let timing = GenerationTiming::from_document(&document, chart)?;
    let session = crate::history::Session::new(Document::parse(bytes, format)?);
    let attack_rows = crate::attacks::placement::Timeline::capture(&session, chart)?
        .markers()
        .iter()
        .map(|marker| marker.row)
        .collect();
    let scope = if crate::timing_edit::supports_chart_timing(&document) {
        crate::timing_edit::Scope::Chart
    } else {
        crate::timing_edit::Scope::Song
    };
    let file_attacks =
        crate::attacks::placement::Timeline::capture_file_scope(&session, chart, scope)?
            .markers()
            .iter()
            .map(|marker| (marker.row, marker.loaded_time))
            .collect();
    checkpoint()?;
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
        attack_rows,
        file_attacks,
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
    attack_rows: Vec<u64>,
    file_attacks: Vec<(u64, f64)>,
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
    pub(crate) fn editor_timeline(
        &self,
        cancelled: &AtomicBool,
    ) -> Result<super::editor_timing::EditorTimeline, Error> {
        super::editor_timing::EditorTimeline::build_with_attacks(
            &self.timing,
            &self.grid,
            None,
            &self.attack_rows,
            cancelled,
        )
    }
    pub fn prepare_editor(&self, cancelled: &AtomicBool) -> Result<PreparedChart, Error> {
        let regions = self.pattern_regions(cancelled)?;
        let timeline = self
            .editor_timeline(cancelled)?
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
        let timing = self.timing.clone().with_file_attacks(&self.file_attacks)?;
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
