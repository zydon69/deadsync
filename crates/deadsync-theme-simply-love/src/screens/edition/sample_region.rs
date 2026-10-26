use super::*;

pub(super) struct Cache {
    source: Arc<[u8]>,
    revision: u64,
    chart: usize,
    values: [u64; 4],
    positions: Option<[f64; 2]>,
    times: Option<[f64; 2]>,
}
impl State {
    pub(super) fn refresh_sample_region(&mut self) {
        self.refresh_end_hint();
        self.refresh_waveform();
        if !self.workspace.has_chart() {
            self.sample_region = None;
            return;
        }
        let session = &self.workspace.file().session;
        let captured;
        let draft = if let Some(draft) = self.song_properties.as_ref() {
            draft
        } else {
            captured = deadsync_editor::song_properties::Draft::capture(session);
            let Ok(draft) = &captured else {
                self.sample_region = None;
                return;
            };
            draft
        };
        let values = [
            draft.sample_start(),
            draft.sample_length(),
            draft.music_offset(),
            draft.sync_offset(),
        ]
        .map(f64::to_bits);
        let chart = self.workspace.chart();
        if self.sample_region.as_ref().is_some_and(|cache| {
            Arc::ptr_eq(&cache.source, session.document().source())
                && cache.revision == session.revision()
                && cache.chart == chart
                && cache.values == values
        }) {
            return;
        }
        let interval = if draft.sample_length() > 0.0 {
            draft
                .sample_end()
                .ok()
                .and_then(|end| draft.clock(session, chart).ok().map(|clock| (clock, end)))
        } else {
            None
        };
        let times = interval.as_ref().and_then(|(clock, end)| {
            Some([
                clock.music_to_chart_time(draft.sample_start()).ok()?,
                clock.music_to_chart_time(*end).ok()?,
            ])
        });
        let positions = interval.as_ref().and_then(|(clock, end)| {
            Some([
                clock.position(draft.sample_start()).ok()?,
                clock.position(*end).ok()?,
            ])
        });
        self.sample_region = Some(Cache {
            source: Arc::clone(session.document().source()),
            revision: session.revision(),
            chart,
            values,
            times,
            positions,
        });
    }
    pub(super) fn sample_region_actors(
        &self,
        actors: &mut Vec<Actor>,
        x: f32,
        width: f32,
        top: f32,
        bottom: f32,
        projection: &spacing_view::Projection<'_>,
    ) {
        let Some(cache) = &self.sample_region else {
            return;
        };
        let Some(times) = cache.times else {
            return;
        };
        let positions = cache.positions.unwrap_or([f64::NAN; 2]);
        let Some(start) = projection.sample_y(positions[0], times[0]) else {
            return;
        };
        let Some(end) = projection.sample_y(positions[1], times[1]) else {
            return;
        };
        let a = start.min(end).clamp(f64::from(top), f64::from(bottom)) as f32;
        let b = start.max(end).clamp(f64::from(top), f64::from(bottom)) as f32;
        if b <= a {
            return;
        }
        actors.push(
            act!(quad: xy(x,(a+b)*0.5): zoomto(width,b-a): diffuse(0.2,0.85,0.65,0.14): z(8001)),
        );
        for boundary in [start, end] {
            let position = boundary;
            if position >= f64::from(top) && position <= f64::from(bottom) {
                actors.push(act!(quad: xy(x,position as f32): zoomto(width,1.0): diffuse(0.2,0.85,0.65,0.8): z(8002)));
            }
        }
        text(
            actors,
            &i18n::tr("Edition", "SongSampleRegion"),
            x - width * 0.5 - 100.0,
            a,
            0.45,
            [0.2, 0.85, 0.65, 1.0],
        );
    }
}
