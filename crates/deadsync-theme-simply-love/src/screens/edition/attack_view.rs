//! Snapshot-built attack widgets; actor construction never parses a simfile.
use super::*;
use deadsync_editor::attacks::placement::Timeline;
const WIDGET_BUDGET: usize = 64;
const SEGMENT_BUDGET: usize = 16;
pub(super) struct Cache {
    source: Arc<[u8]>,
    revision: u64,
    chart: usize,
    data: Result<Timeline, String>,
}
impl State {
    pub(super) fn refresh_attack_view(&mut self) {
        if !self.workspace.has_chart() {
            self.attack_view = None;
            return;
        }
        let session = &self.workspace.file().session;
        let chart = self.workspace.chart();
        if self.attack_view.as_ref().is_some_and(|cache| {
            Arc::ptr_eq(&cache.source, session.document().source())
                && cache.revision == session.revision()
                && cache.chart == chart
        }) {
            return;
        }
        self.attack_view = Some(Cache {
            source: Arc::clone(session.document().source()),
            revision: session.revision(),
            chart,
            data: Timeline::capture(session, chart).map_err(|error| error.to_string()),
        });
    }
    pub(super) fn attack_last_row(&self) -> u64 {
        self.attack_view
            .as_ref()
            .and_then(|cache| cache.data.as_ref().ok())
            .map_or(0, Timeline::last_row)
    }
    pub(super) fn navigate_attack(&mut self, forward: bool) {
        self.refresh_attack_view();
        if self
            .attack_view
            .as_ref()
            .is_some_and(|cache| cache.data.is_err())
        {
            self.status = i18n::tr("Edition", "AttackWidgetUnavailable");
            return;
        }
        let row = self
            .attack_view
            .as_ref()
            .and_then(|cache| cache.data.as_ref().ok())
            .and_then(|timeline| timeline.neighbor(self.workspace.row(), forward));
        if let Some(row) = row {
            self.command(Command::Seek(row));
            self.status = i18n::tr("Edition", "AttackWidgetNavigation");
        } else {
            self.status = i18n::tr("Edition", "AttackWidgetMissing");
        }
    }
    pub(super) fn open_attack_at_cursor(&mut self) {
        self.refresh_attack_view();
        if self
            .attack_view
            .as_ref()
            .is_some_and(|cache| cache.data.is_err())
        {
            self.status = i18n::tr("Edition", "AttackWidgetUnavailable");
            return;
        }
        let index = self
            .attack_view
            .as_ref()
            .and_then(|cache| cache.data.as_ref().ok())
            .and_then(|timeline| timeline.at(self.workspace.row()))
            .and_then(|marker| marker.source_indices.first())
            .copied();
        let Some(index) = index else {
            self.status = i18n::tr("Edition", "AttackWidgetMissing");
            return;
        };
        let scope = if deadsync_editor::attacks::supports_resident_chart_attacks(
            self.workspace.file().session.document(),
        ) {
            deadsync_editor::timing_edit::Scope::Chart
        } else {
            deadsync_editor::timing_edit::Scope::Song
        };
        self.open_attacks(scope);
        self.select_attack_source(index);
    }
    pub(super) fn attack_actors(
        &self,
        actors: &mut Vec<Actor>,
        left: f32,
        top: f32,
        bottom: f32,
        projection: &spacing_view::Projection<'_>,
        ranges: &[(u64, u64)],
    ) {
        if !self.render_policy().misc_events || self.preview.is_some() || bottom <= top {
            return;
        }
        let Some(cache) = &self.attack_view else {
            return;
        };
        let Ok(timeline) = &cache.data else {
            actors.push(act!(text: font("miso"): settext(i18n::tr("Edition","AttackWidgetUnavailable")): align(0.0,0.0): xy(12.0,top): zoom(0.45): maxwidth((left-45.0).max(20.0)): maxheight(18.0): diffuse(1.0,0.6,0.25,1.0): z(8005)));
            return;
        };
        if timeline.markers().is_empty() {
            return;
        }
        let sections = projection.attack_sections(
            top,
            bottom,
            self.attack_last_row()
                .max(self.workspace.row().saturating_add(768)),
        );
        let mut seen = std::collections::BTreeSet::new();
        let mut bands = std::collections::BTreeSet::new();
        for &(start, end) in ranges {
            let remaining = WIDGET_BUDGET.saturating_sub(seen.len());
            if remaining == 0 {
                break;
            }
            for marker in timeline.visible(start, end, remaining).unwrap_or_default() {
                if !seen.insert(marker.row) {
                    continue;
                }
                let segments =
                    projection.attack_segments(marker, &sections, top, bottom, SEGMENT_BUDGET);
                for &(a, b) in &segments {
                    if bands.insert((a.to_bits(), b.to_bits())) {
                        let height = ((b - a) as f32).max(1.0).min(bottom - top);
                        let y = (((a + b) * 0.5) as f32)
                            .clamp(top + height * 0.5, bottom - height * 0.5);
                        actors.push(act!(quad: xy(left-18.0,y): zoomto(3.0,height): diffuse(1.0,0.55,0.15,0.45): z(8002)));
                    }
                }

            }
        }

    }
}
