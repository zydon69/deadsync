use super::*;
use deadsync_editor::{
    playback::Clock,
    projection::{RowProjection, TimeProjection},
    spacing::Anchor,
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
    common_tempo: Result<f64, String>,
}
pub(super) enum Projection<'a> {
    Rows(RowProjection, Option<TimeProjection>),
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
            Self::Rows(p, _) => Some(p.y(row)),
            Self::Time { axis, clock, .. } => axis.y(clock.chart_time(row).ok()?).ok(),
            Self::Variable { geometry, .. } => geometry.row_y(row as f64, 0).ok(),
        }
    }
    pub(super) fn note_y(&self, row: u64) -> Option<f64> {
        match self {
            Self::Rows(p, _) => Some(p.y(row)),
            Self::Time { axis, clock, .. } => axis.y(clock.note_time(row).ok()?).ok(),
            Self::Variable { geometry, .. } => geometry.row_y(row as f64, 11).ok(),
        }
    }
    pub(super) fn clipped_y(&self, row: u64, top: f32, bottom: f32) -> Option<f64> {
        Some(self.y(row)?.clamp(f64::from(top), f64::from(bottom)))
    }
    pub(super) fn sample_y(&self, row: f64, chart_time: f64) -> Option<f64> {
        match self {
            Self::Rows(p, _) => row.is_finite().then(|| p.fractional_y(row)),
            Self::Time { axis, .. } => axis.y(chart_time).ok(),
            Self::Variable { geometry, .. } => geometry.row_y(row, 0).ok(),
        }
    }
    pub(super) fn attack_y(
        &self,
        marker: &deadsync_editor::attacks::placement::Marker,
    ) -> Option<f64> {
        match self {
            Self::Variable { geometry, .. } => geometry.row_y(marker.row as f64, 6).ok(),
            _ => self.sample_y(marker.row as f64, marker.chart_time),
        }
    }
    pub(super) fn attack_sections(
        &self,
        top: f32,
        bottom: f32,
        last: u64,
    ) -> Vec<deadsync_editor::spacing_geometry::VisibleRows> {
        match self {
            Self::Variable { geometry, .. } => geometry
                .visible_rows(f64::from(top), f64::from(bottom), last)
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }
    pub(super) fn attack_segments(
        &self,
        marker: &deadsync_editor::attacks::placement::Marker,
        sections: &[deadsync_editor::spacing_geometry::VisibleRows],
        top: f32,
        bottom: f32,
        limit: usize,
    ) -> Vec<(f64, f64)> {
        if marker.end_time <= marker.chart_time {
            return Vec::new();
        }
        if let Self::Variable { geometry, .. } = self {
            return geometry
                .region_segments(
                    marker.row as f64,
                    marker.end_row,
                    sections,
                    f64::from(top),
                    f64::from(bottom),
                    limit,
                )
                .unwrap_or_default();
        }
        let (Some(a), Some(b)) = (
            self.attack_y(marker),
            self.sample_y(marker.end_row, marker.end_time),
        ) else {
            return Vec::new();
        };
        if a.max(b) < f64::from(top) || a.min(b) > f64::from(bottom) {
            return Vec::new();
        }
        vec![(
            a.min(b).clamp(f64::from(top), f64::from(bottom)),
            a.max(b).clamp(f64::from(top), f64::from(bottom)),
        )]
    }
    pub(super) fn music_interval(&self, y: f64, next_y: f64, clock: &Clock) -> Option<(f64, f64)> {
        match self {
            Self::Rows(_, None) => None,
            Self::Rows(_, Some(axis))
            | Self::Time { axis, .. }
            | Self::Variable { waveform: axis, .. } => {
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
            Self::Rows(p, _) => {
                return Ok(vec![p.visible_rows_in(f64::from(top), f64::from(bottom))?]);
            }
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
        let mut settings = self.scroll_preferences;
        settings.set_reverse(!settings.reverse());
        match self.commit_scroll_preferences(settings) {
            Ok(_) => {
                self.status = i18n::tr(
                    "Edition",
                    if self.reverse_display {
                        "DisplayReverse"
                    } else {
                        "DisplayForward"
                    },
                )
            }
            Err(_) => self.status = i18n::tr("Edition", "ScrollValuesSaveFailed"),
        }
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
        let chart = self
            .preview_source_chart()
            .unwrap_or_else(|| self.workspace.chart());
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
                let common_tempo = deadsync_editor::waveform_spacing::most_common_tempo(&index)
                    .map_err(|error| error.to_string())
                    .and_then(|tempo| tempo.ok_or_else(|| "No chart rate events".into()));
                Ok(Data {
                    clock,
                    index,
                    interpolation,
                    common_tempo,
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
        self.set_spacing_mode(next);
    }
    pub(super) fn cycle_source_spacing_mode(&mut self) {
        self.set_raw_spacing_mode(self.spacing_preference.next());
    }
    fn set_spacing_mode(&mut self, next: SpacingMode) {
        self.set_raw_spacing_mode(
            deadsync_editor::scroll_preferences::spacing::Preference::from_effective(next),
        );
        self.status = i18n::tr(
            "Edition",
            match next {
                SpacingMode::ConstantRow => "SpacingRows",
                SpacingMode::ConstantTime => "SpacingTime",
                SpacingMode::Variable => "SpacingVariable",
            },
        );
    }
    fn display_position(&self, clock: &Clock) -> Result<(f64, f64), deadsync_editor::Error> {
        if let Some(position) = self.preview_position()? {
            return Ok((position.chart_time(), position.row()));
        }
        if let Some(position) = self.playback_position() {
            return Ok(position);
        }
        Ok((
            clock.chart_time(self.workspace.row())?,
            self.workspace.row() as f64,
        ))
    }
    fn waveform_axis(
        &self,
        data: &Data,
        top: f32,
        bottom: f32,
        cursor_y: f32,
    ) -> Result<TimeProjection, deadsync_editor::Error> {
        let (time, row) = self.display_position(&data.clock)?;
        let mode = self.spacing_mode;
        let policy = self.waveform_preferences.scroll_mode();
        let active = data
            .index
            .active(mode, time, row)?
            .ok_or_else(|| deadsync_editor::Error::InvalidEdit("No chart rate events".into()))?;
        let common = if mode != SpacingMode::ConstantTime
            && policy == deadsync_editor::waveform_spacing::ScrollMode::MostCommonTempo
        {
            *data
                .common_tempo
                .as_ref()
                .map_err(|error| deadsync_editor::Error::InvalidEdit(error.clone()))?
        } else {
            0.0
        };
        let interpolation = if mode == SpacingMode::Variable
            && policy == deadsync_editor::waveform_spacing::ScrollMode::CurrentTempoAndRate
        {
            data.interpolation
                .as_ref()
                .map_err(|error| deadsync_editor::Error::InvalidEdit(error.clone()))?
                .rate_at(row, time)?
        } else {
            1.0
        };
        let speeds = self.current_spacing_speeds();
        let pps = deadsync_editor::waveform_spacing::pixels_per_second(
            mode,
            policy,
            data.index.sections()[active].spacing_rate(),
            speeds,
            common,
            interpolation,
        )? * self.zoom.spacing_zoom();
        TimeProjection::new(
            time,
            f64::from(cursor_y),
            pps,
            f64::from(top),
            f64::from(bottom),
            self.reverse_display,
        )
    }
    pub(super) fn display_projection(
        &self,
        top: f32,
        bottom: f32,
        cursor_y: f32,
    ) -> Result<Projection<'_>, deadsync_editor::Error> {
        if self.preview.is_some() {
            self.preview_position()?;
        }
        if self.spacing_mode == SpacingMode::ConstantRow {
            let waveform = self
                .spacing_cache
                .as_ref()
                .and_then(|cache| cache.data.as_ref().ok())
                .and_then(|data| self.waveform_axis(data, top, bottom, cursor_y).ok());
            return Ok(Projection::Rows(
                self.row_projection(top, bottom, cursor_y),
                waveform,
            ));
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
            ..
        } = data;
        let (time, row) = self.display_position(clock)?;
        let focal = Anchor {
            y: f64::from(cursor_y),
            time,
            row,
        };
        let settings = Settings {
            mode: self.spacing_mode,
            reverse: self.reverse_display,
            speeds: self.current_spacing_speeds(),
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
            let waveform = self.waveform_axis(data, top, bottom, cursor_y)?;
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

impl State {
    pub fn spacing_preference(&self) -> deadsync_editor::scroll_preferences::spacing::Preference {
        self.spacing_preference
    }
    pub fn configure_spacing_preference(
        &mut self,
        value: deadsync_editor::scroll_preferences::spacing::Preference,
    ) {
        self.spacing_preference = value;
        self.spacing_mode = value.effective();
    }
    fn set_raw_spacing_mode(
        &mut self,
        value: deadsync_editor::scroll_preferences::spacing::Preference,
    ) {
        self.configure_spacing_preference(value);
        if let Some(file) = self.application_preferences_owner_mut() {
            file.set_spacing_preference(value);
        }
        self.status = match value {
            deadsync_editor::scroll_preferences::spacing::Preference::Unknown(_) => i18n::tr_fmt(
                "Edition",
                "SpacingRawMode",
                &[("value", &value.source_name())],
            ),
            deadsync_editor::scroll_preferences::spacing::Preference::ConstantTime => {
                i18n::tr("Edition", "SpacingTime")
            }
            deadsync_editor::scroll_preferences::spacing::Preference::ConstantRow => {
                i18n::tr("Edition", "SpacingRows")
            }
            deadsync_editor::scroll_preferences::spacing::Preference::Variable => {
                i18n::tr("Edition", "SpacingVariable")
            }
        };
    }
}
