use super::*;
use deadsync_editor::{
    playback::Clock,
    projection::{RowProjection, TimeProjection},
    spacing::{Anchor, Speeds},
    spacing_geometry::Cursor,
    spacing_index::Index,
    spacing_search::Settings,
    zoom::SpacingMode,
};

pub(super) struct Cache {
    source: Arc<[u8]>,
    revision: u64,
    chart: usize,
    data: Result<(Clock, Index), String>,
}
pub(super) enum Projection<'a> {
    Rows(RowProjection),
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
        }
    }
    pub(super) fn note_y(&self, row: u64) -> Option<f64> {
        match self {
            Self::Rows(p) => Some(p.y(row)),
            Self::Time { axis, clock, .. } => axis.y(clock.note_time(row).ok()?).ok(),
        }
    }
    pub(super) fn clipped_y(&self, row: u64, top: f32, bottom: f32) -> Option<f64> {
        Some(self.y(row)?.clamp(f64::from(top), f64::from(bottom)))
    }
    pub(super) fn sample_y(&self, row: f64, chart_time: f64) -> Option<f64> {
        match self {
            Self::Rows(p) => row.is_finite().then(|| p.fractional_y(row)),
            Self::Time { axis, .. } => axis.y(chart_time).ok(),
        }
    }
    pub(super) fn music_interval(&self, y: f64, next_y: f64, clock: &Clock) -> Option<(f64, f64)> {
        match self {
            Self::Rows(p) => {
                let first = p.row_at(y);
                let last = p.row_at(next_y);
                if last <= 0.0 {
                    return None;
                }
                Some((
                    clock.music_time(first.max(0.0).floor() as u64).ok()?,
                    clock.music_time(last.ceil() as u64).ok()?,
                ))
            }
            Self::Time { axis, .. } => Some((
                clock.chart_time_to_music(axis.time_at(y).ok()?).ok()?,
                clock.chart_time_to_music(axis.time_at(next_y).ok()?).ok()?,
            )),
        }
    }
    pub(super) fn visible_ranges(
        &self,
        top: f32,
        bottom: f32,
    ) -> Result<Vec<(u64, u64)>, deadsync_editor::Error> {
        match self {
            Self::Rows(p) => Ok(vec![p.visible_rows()]),
            Self::Time { cursor, .. } => {
                let spans = cursor.visible_rows(f64::from(top), f64::from(bottom), u64::MAX)?;
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
    }
}
impl State {
    pub fn spacing_mode(&self) -> SpacingMode {
        if self.time_spacing {
            SpacingMode::ConstantTime
        } else {
            SpacingMode::ConstantRow
        }
    }
    pub(super) fn refresh_time_view(&mut self) {
        if !self.workspace.has_chart() {
            self.time_view = None;
            return;
        }
        let session = &self.workspace.file().session;
        let chart = self.workspace.chart();
        if self.time_view.as_ref().is_some_and(|cache| {
            Arc::ptr_eq(&cache.source, session.document().source())
                && cache.revision == session.revision()
                && cache.chart == chart
        }) {
            return;
        }
        let data = Clock::capture(session, chart)
            .and_then(|clock| {
                let index = Index::new(clock.spacing_sections().collect())?;
                Ok((clock, index))
            })
            .map_err(|error| error.to_string());
        self.time_view = Some(Cache {
            source: Arc::clone(session.document().source()),
            revision: session.revision(),
            chart,
            data,
        });
    }
    pub(super) fn toggle_spacing_mode(&mut self) {
        if !self.time_spacing {
            self.refresh_time_view();
            if let Some(Cache {
                data: Err(error), ..
            }) = &self.time_view
            {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", error)]);
                return;
            }
        }
        self.time_spacing = !self.time_spacing;
        self.status = i18n::tr(
            "Edition",
            if self.time_spacing {
                "SpacingTime"
            } else {
                "SpacingRows"
            },
        );
    }
    pub(super) fn display_projection(
        &self,
        top: f32,
        bottom: f32,
        cursor_y: f32,
    ) -> Result<Projection<'_>, deadsync_editor::Error> {
        if !self.time_spacing {
            return Ok(Projection::Rows(self.row_projection(top, bottom, cursor_y)));
        }
        let (clock, index) = self
            .time_view
            .as_ref()
            .and_then(|cache| cache.data.as_ref().ok())
            .ok_or_else(|| {
                deadsync_editor::Error::InvalidEdit(
                    "Unavailable chart timing for time display".into(),
                )
            })?;
        let time = clock.chart_time(self.workspace.row())?;
        let focal = Anchor {
            y: f64::from(cursor_y),
            time,
            row: self.workspace.row() as f64,
        };
        let settings = Settings {
            mode: SpacingMode::ConstantTime,
            reverse: false,
            speeds: Speeds::default(),
            interpolated_scroll: 1.0,
            zoom: self.zoom.spacing_zoom(),
        };
        let axis = TimeProjection::new(
            time,
            focal.y,
            settings.speeds.time * settings.zoom,
            f64::from(top),
            f64::from(bottom),
            false,
        )?;
        let cursor = Cursor::begin(index, focal, settings)?.ok_or_else(|| {
            deadsync_editor::Error::InvalidEdit("Missing chart timing sections".into())
        })?;
        Ok(Projection::Time {
            axis,
            clock,
            cursor,
        })
    }
}
