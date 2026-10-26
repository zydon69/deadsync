//! Song-level End Hint projected independently into every visible chart.
//! GrooveAuthor EditorLastSecondHintEvent / EditorChart.AddLastSecondHintEvent:
//! positive song seconds only; not a selectable or movable lane event.
use super::{State, spacing_view::Projection, view_layout::Geometry, view_render::Viewport};
use crate::{act, i18n};
use deadlib_present::actors::Actor;
use deadsync_editor::{Error, chart_order::SessionBinding, playback::Clock};
#[derive(Clone, Copy)]
struct Marker {
    seconds: f64,
    row: f64,
    chart_time: f64,
}
impl Marker {
    fn capture(clock: &Clock, seconds: f64) -> Result<Option<Self>, Error> {
        if !seconds.is_finite() {
            return Err(Error::InvalidEdit("Nonfinite end hint".into()));
        }
        if seconds <= 0.0 {
            return Ok(None);
        }
        Ok(Some(Self {
            seconds,
            row: clock.position(seconds)?,
            chart_time: clock.music_to_chart_time(seconds)?,
        }))
    }
}
pub(super) struct Cache {
    binding: SessionBinding,
    values: [u64; 3],
    charts: Vec<usize>,
    markers: Vec<(usize, Marker)>,
}
impl State {
    pub(super) fn refresh_end_hint(&mut self) {
        if !self.workspace.has_chart() {
            self.end_hint = None;
            return;
        }
        let charts: Vec<_> = self
            .visible_view_geometry()
            .iter()
            .map(|view| view.source)
            .collect();
        let session = &self.workspace.file().session;
        let captured;
        let draft = if let Some(draft) = self.song_properties.as_ref() {
            draft
        } else {
            captured = deadsync_editor::song_properties::Draft::capture(session);
            let Ok(draft) = &captured else {
                self.end_hint = None;
                return;
            };
            draft
        };
        let values = [
            draft.last_second_hint(),
            draft.music_offset(),
            draft.sync_offset(),
        ]
        .map(f64::to_bits);
        if self.end_hint.as_ref().is_some_and(|cache| {
            cache.binding.is_current(session) && cache.values == values && cache.charts == charts
        }) {
            return;
        }
        let result = (|| -> Result<Vec<(usize, Marker)>, Error> {
            let mut markers = Vec::new();
            if !draft.supports_last_second_hint() || draft.last_second_hint() <= 0.0 {
                return Ok(markers);
            }
            for &chart in &charts {
                let marker = if self.song_properties.is_some() {
                    Marker::capture(&draft.clock(session, chart)?, draft.last_second_hint())?
                } else {
                    let shared = self
                        .active_views
                        .as_ref()
                        .and_then(|views| {
                            views
                                .entries()
                                .iter()
                                .find(|view| view.source_index() == chart)
                        })
                        .map(|view| view.clock(session))
                        .transpose()?;
                    let clock = match shared {
                        Some(clock) => clock,
                        None => std::sync::Arc::new(Clock::capture(session, chart)?),
                    };
                    Marker::capture(&clock, draft.last_second_hint())?
                };
                if let Some(marker) = marker {
                    markers.push((chart, marker));
                }
            }
            Ok(markers)
        })();
        self.end_hint = result.ok().map(|markers| Cache {
            binding: SessionBinding::capture(session),
            values,
            charts,
            markers,
        });
    }
    pub(super) fn end_hint_actors(
        &self,
        actors: &mut Vec<Actor>,
        projection: &Projection<'_>,
        geometry: Geometry,
        viewport: Viewport,
    ) {
        let Some(marker) = self
            .end_hint
            .as_ref()
            .filter(|cache| cache.binding.is_current(&self.workspace.file().session))
            .and_then(|cache| {
                cache
                    .markers
                    .iter()
                    .find(|(chart, _)| *chart == geometry.source)
            })
            .map(|(_, marker)| marker)
        else {
            return;
        };
        let Some(y) = projection
            .sample_y(marker.row, marker.chart_time)
            .filter(|y| *y >= f64::from(viewport.top) && *y <= f64::from(viewport.bottom))
        else {
            return;
        };
        let x = geometry.right() + 3.0;
        let width = (geometry.clip_right - x).min(26.0);
        if width <= 0.0 {
            return;
        }
        let label = i18n::tr_fmt(
            "Edition",
            "ChartEndHint",
            &[("seconds", &format!("{:.3}", marker.seconds))],
        );
        actors.push(act!(quad: xy(x+width*0.5,y as f32+7.0): zoomto(width,14.0): diffuse(0.8,0.55,0.2,0.4): z(8002)));
        actors.push(act!(text: font("miso"): settext(label): align(0.0,0.0): xy(x,y as f32): zoom(0.3): maxwidth(width): maxheight(14.0): diffuse(1.0,0.8,0.4,1.0): z(8007)));
    }
}
