use super::*;

pub(super) struct Cache {
    source: Arc<[u8]>,
    revision: u64,
    chart: usize,
    values: [u64; 4],
    positions: Option<[f64; 2]>,
}
impl State {
    pub(super) fn refresh_sample_region(&mut self) {
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
        self.sample_region = Some(Cache {
            source: Arc::clone(session.document().source()),
            revision: session.revision(),
            chart,
            values,
            positions: if draft.sample_length() > 0.0 {
                draft.sample_positions(session, chart).ok()
            } else {
                None
            },
        });
    }
    pub(super) fn sample_region_actors(
        &self,
        actors: &mut Vec<Actor>,
        x: f32,
        width: f32,
        top: f32,
        bottom: f32,
        cursor_y: f32,
    ) {
        let Some([start, end]) = self
            .sample_region
            .as_ref()
            .and_then(|cache| cache.positions)
        else {
            return;
        };
        let y = |position: f64| {
            (f64::from(cursor_y) + position - self.workspace.row() as f64)
                .clamp(f64::from(top), f64::from(bottom)) as f32
        };
        let a = y(start);
        let b = y(end);
        if b <= a {
            return;
        }
        actors.push(
            act!(quad: xy(x,(a+b)*0.5): zoomto(width,b-a): diffuse(0.2,0.85,0.65,0.14): z(8001)),
        );
        for boundary in [start, end] {
            let position = f64::from(cursor_y) + boundary - self.workspace.row() as f64;
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
