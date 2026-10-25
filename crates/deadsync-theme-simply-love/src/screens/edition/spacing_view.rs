use super::*;
use deadsync_editor::{
    playback::Clock,
    projection::{RowProjection, TimeProjection},
    spacing::{Anchor, Speeds},
    spacing_geometry::{Cursor, Frame},
    spacing_index::Index,
    spacing_search::Settings,
    zoom::SpacingMode,
};

pub(super) struct Cache {
    source: Arc<[u8]>,
    revision: u64,
    chart: usize,
    data: Result<Data, String>,
}
struct Data {
    clock: Clock,
    index: Index,
    interpolation: Result<deadsync_editor::scroll_interpolation::Timeline, String>,
}
pub(super) enum Projection<'a> {
    Rows(RowProjection),
    Variable {
        geometry: Frame<'a>,
        waveform: TimeProjection,
    },
    Time {
        axis: TimeProjection,
        clock: &'a Clock,
        cursor: Cursor<'a>,
    },
}
impl Projection<'_> {
    pub(super) fn y(&self, row: u64) -> Option<f64> {
        match self {
            Self::Rows(p) => Some(p.y(row)),
            Self::Time { axis, clock, .. } => axis.y(clock.chart_time(row).ok()?).ok(),
            Self::Variable { geometry, .. } => geometry.row_y(row as f64, 0).ok(),
        }
    }
    pub(super) fn note_y(&self, row: u64) -> Option<f64> {
        match self {
            Self::Rows(p) => Some(p.y(row)),
            Self::Time { axis, clock, .. } => axis.y(clock.note_time(row).ok()?).ok(),
            Self::Variable { geometry, .. } => geometry.row_y(row as f64, 11).ok(),
        }
    }
    pub(super) fn clipped_y(&self, row: u64, top: f32, bottom: f32) -> Option<f64> {
        Some(self.y(row)?.clamp(f64::from(top), f64::from(bottom)))
    }
    pub(super) fn sample_y(&self, row: f64, chart_time: f64) -> Option<f64> {
        match self {
            Self::Rows(p) => row.is_finite().then(|| p.fractional_y(row)),
            Self::Time { axis, .. } => axis.y(chart_time).ok(),
            Self::Variable { geometry, .. } => geometry.row_y(row, 0).ok(),
        }
    }
    pub(super) fn music_interval(&self, y: f64, next_y: f64, clock: &Clock) -> Option<(f64, f64)> {
        match self {
            Self::Rows(p) => {
                let a = p.row_at(y);
                let b = p.row_at(next_y);
                let first = a.min(b);
                let last = a.max(b);
                if last <= 0.0 {
                    return None;
                }
                Some((
                    clock.music_time(first.max(0.0).floor() as u64).ok()?,
                    clock.music_time(last.ceil() as u64).ok()?,
                ))
            }
            Self::Time { axis, .. } | Self::Variable { waveform: axis, .. } => {
                let a = axis.time_at(y).ok()?;
                let b = axis.time_at(next_y).ok()?;
                Some((
                    clock.chart_time_to_music(a.min(b)).ok()?,
                    clock.chart_time_to_music(a.max(b)).ok()?,
                ))
            }
        }
    }
    pub(super) fn visible_ranges(
        &self,
        top: f32,
        bottom: f32,
        last_row: u64,
    ) -> Result<Vec<(u64, u64)>, deadsync_editor::Error> {
        let spans = match self {
            Self::Rows(p) => return Ok(vec![p.visible_rows()]),
            Self::Time { cursor, .. } => {
                cursor.visible_rows(f64::from(top), f64::from(bottom), u64::MAX)?
            }
            Self::Variable { geometry, .. } => {
                geometry.visible_rows(f64::from(top), f64::from(bottom), last_row)?
            }
        };
        let mut ranges: Vec<(u64, u64)> = Vec::new();
        for span in spans {
            if let Some(last) = ranges
                .last_mut()
                .filter(|last| span.start <= last.1.saturating_add(1))
            {
                last.1 = last.1.max(span.end);
            } else {
                ranges.push((span.start, span.end));
            }
        }
        Ok(ranges)
    }
}
impl State {
    pub fn display_reversed(&self) -> bool {
        self.reverse_display
    }
    pub(super) fn toggle_reverse_display(&mut self) {
        self.reverse_display = !self.reverse_display;
        self.status = i18n::tr(
            "Edition",
            if self.reverse_display {
                "DisplayReverse"
            } else {
                "DisplayForward"
            },
        );
    }
    pub fn spacing_mode(&self) -> SpacingMode {
        self.spacing_mode
    }
    pub(super) fn refresh_spacing_cache(&mut self) {
        if !self.workspace.has_chart() {
            self.spacing_cache = None;
            return;
        }
        let session = &self.workspace.file().session;
        let chart = self.workspace.chart();
        if self.spacing_cache.as_ref().is_some_and(|cache| {
            Arc::ptr_eq(&cache.source, session.document().source())
                && cache.revision == session.revision()
                && cache.chart == chart
        }) {
            return;
        }
        let data = Clock::capture(session, chart)
            .and_then(|clock| {
                let index = Index::new(clock.spacing_sections().collect())?;
                let interpolation =
                    deadsync_editor::scroll_interpolation::Timeline::capture(session, chart)
                        .map_err(|error| error.to_string());
                Ok(Data {
                    clock,
                    index,
                    interpolation,
                })
            })
            .map_err(|error| error.to_string());
        self.spacing_cache = Some(Cache {
            source: Arc::clone(session.document().source()),
            revision: session.revision(),
            chart,
            data,
        });
    }
    pub(super) fn toggle_spacing_mode(&mut self, variable: bool) {
        let next = if variable {
            if self.spacing_mode == SpacingMode::Variable {
                SpacingMode::ConstantRow
            } else {
                SpacingMode::Variable
            }
        } else if self.spacing_mode == SpacingMode::ConstantTime {
            SpacingMode::ConstantRow
        } else {
            SpacingMode::ConstantTime
        };
        if next != SpacingMode::ConstantRow {
            self.refresh_spacing_cache();
            let error = self
                .spacing_cache
                .as_ref()
                .and_then(|cache| match &cache.data {
                    Err(error) => Some(error),
                    Ok(data) if next == SpacingMode::Variable => data.interpolation.as_ref().err(),
                    _ => None,
                });
            if let Some(error) = error {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", error)]);
                return;
            }
        }
        self.spacing_mode = next;
        self.status = i18n::tr(
            "Edition",
            match next {
                SpacingMode::ConstantRow => "SpacingRows",
                SpacingMode::ConstantTime => "SpacingTime",
                SpacingMode::Variable => "SpacingVariable",
            },
        );
    }
    pub(super) fn display_projection(
        &self,
        top: f32,
        bottom: f32,
        cursor_y: f32,
    ) -> Result<Projection<'_>, deadsync_editor::Error> {
        if self.spacing_mode == SpacingMode::ConstantRow {
            return Ok(Projection::Rows(self.row_projection(top, bottom, cursor_y)));
        }
        let data = self
            .spacing_cache
            .as_ref()
            .and_then(|cache| cache.data.as_ref().ok())
            .ok_or_else(|| {
                deadsync_editor::Error::InvalidEdit(
                    "Unavailable chart timing for time display".into(),
                )
            })?;
        let Data {
            clock,
            index,
            interpolation,
        } = data;
        let time = clock.chart_time(self.workspace.row())?;
        let focal = Anchor {
            y: f64::from(cursor_y),
            time,
            row: self.workspace.row() as f64,
        };
        let settings = Settings {
            mode: self.spacing_mode,
            reverse: self.reverse_display,
            speeds: Speeds::default(),
            interpolated_scroll: if self.spacing_mode == SpacingMode::Variable {
                interpolation
                    .as_ref()
                    .map_err(|error| deadsync_editor::Error::InvalidEdit(error.clone()))?
                    .rate_at(focal.row, time)?
            } else {
                1.0
            },
            zoom: self.zoom.spacing_zoom(),
        };
        let cursor = Cursor::begin(index, focal, settings)?.ok_or_else(|| {
            deadsync_editor::Error::InvalidEdit("Missing chart timing sections".into())
        })?;
        if self.spacing_mode == SpacingMode::Variable {
            // Original CurrentTempo waveform policy. Its temporal axis remains
            // defined when the chart's signed scroll or SPEEDS collapses to zero.
            let active = index
                .active(self.spacing_mode, time, focal.row)?
                .unwrap_or(0);
            let pps =
                settings.speeds.variable * (index.sections()[active].tempo / 300.0) * settings.zoom;
            let waveform = TimeProjection::new(
                time,
                focal.y,
                pps,
                f64::from(top),
                f64::from(bottom),
                self.reverse_display,
            )?;
            return Ok(Projection::Variable {
                geometry: Frame::capture(cursor)?,
                waveform,
            });
        }
        let axis = TimeProjection::new(
            time,
            focal.y,
            settings.speeds.time * settings.zoom,
            f64::from(top),
            f64::from(bottom),
            self.reverse_display,
        )?;

        Ok(Projection::Time {
            axis,
            clock,
            cursor,
        })
    }
}

/// Combine coincident labels while preserving logical row order. A standalone
/// label retains its exact projected Y; a cluster uses its vertical midpoint.
pub(super) fn label_actors(
    actors: &mut Vec<Actor>,
    mut labels: Vec<(u64, f64, String)>,
    x: f32,
    width: f32,
) {
    labels.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    let mut start = 0;
    while start < labels.len() {
        let mut end = start + 1;
        while end < labels.len() && labels[end].1 == labels[start].1 {
            end += 1;
        }
        let y = (labels[start].1 + labels[end - 1].1) * 0.5;
        let group = &mut labels[start..end];
        group.sort_by_key(|item| item.0);
        let label = group
            .iter()
            .map(|item| item.2.as_str())
            .collect::<Vec<_>>()
            .join(" / ");
        actors.push(act!(text: font("miso"): settext(label): align(0.0,0.5): xy(x,y as f32): zoom(0.55): maxwidth((width-x-12.0).max(20.0)): diffuse(0.6,0.9,0.65,1.0): z(8005)));
        start = end;
    }
}
