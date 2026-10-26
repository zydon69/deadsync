//! Per-chart immutable timing inputs and shared lane-note actor construction.
//! The Session remains the owner of every live note grid, including unsaved edits.
use super::{lane_rotation, player_color, spacing_view::Projection, text, view_layout::Geometry};
use crate::{act, i18n};
use deadlib_present::actors::Actor;
use deadsync_editor::{
    Error,
    chart_order::Binding,
    generation::metadata::{ChartData, Guid},
    history::Session,
    notes::{Grid, Kind},
    playback::Clock,
    projection::{RowProjection, TimeProjection},
    region_index::RegionIndex,
    scroll_interpolation::Timeline,
    spacing::{Anchor, Speeds},
    spacing_geometry::{Cursor, Frame},
    spacing_index::Index,
    spacing_search::Settings,
    timing_edit::{label::Labels, signature::Signatures},
    zoom::SpacingMode,
};
use std::collections::{BTreeMap, BTreeSet};
pub(super) const NOTE_BUDGET: usize = 16_384;
#[derive(Clone, Copy)]
pub(super) struct Viewport {
    pub top: f32,
    pub bottom: f32,
    pub cursor: f32,
}
struct Entry {
    binding: Binding,
    data: Result<Data, String>,
}
#[derive(Default)]
pub(super) struct Cache {
    entries: BTreeMap<Guid, Entry>,
}
pub(super) struct Data {
    clock: std::sync::Arc<Clock>,
    index: Index,
    interpolation: Result<Timeline, String>,
    pub labels: Option<Labels>,
    pub signatures: Option<Signatures>,
    pub regions: RegionIndex,
}
impl Cache {
    /// Only viewport-visible identities need projection/metadata resources.
    /// Content tokens ignore application preference gestures and survive focus
    /// changes, but reject stale source indices and reloaded song sessions.
    pub fn prepare(
        &mut self,
        session: &Session,
        charts: &[usize],
        views: Option<&deadsync_editor::chart_views::ActiveChartViews>,
    ) {
        let keep: BTreeSet<_> = charts
            .iter()
            .filter_map(|chart| session.chart_ids().get(*chart))
            .collect();
        self.entries.retain(|id, _| keep.contains(id));
        for &chart in charts {
            let Some(id) = session.chart_ids().get(chart) else {
                continue;
            };
            if self
                .entries
                .get(id)
                .is_some_and(|entry| entry.binding.is_current(session, chart))
            {
                continue;
            }
            let Ok(binding) = Binding::capture(session, chart) else {
                continue;
            };
            self.entries.insert(
                id.clone(),
                Entry {
                    binding,
                    data: (|| {
                        let clock = views
                            .and_then(|views| {
                                views
                                    .entries()
                                    .iter()
                                    .find(|view| view.source_index() == chart)
                            })
                            .map(|view| view.clock(session))
                            .transpose()?;
                        let clock = match clock {
                            Some(clock) => clock,
                            None => std::sync::Arc::new(Clock::capture(session, chart)?),
                        };
                        Data::capture(session, chart, clock)
                    })()
                    .map_err(|error: Error| error.to_string()),
                },
            );
        }
    }
    pub fn get(&self, session: &Session, chart: usize) -> Result<&Data, Error> {
        let entry = session
            .chart_ids()
            .get(chart)
            .and_then(|id| self.entries.get(id))
            .filter(|entry| entry.binding.is_current(session, chart))
            .ok_or_else(|| Error::InvalidEdit("Chart view inputs are stale".into()))?;
        entry
            .data
            .as_ref()
            .map_err(|error| Error::InvalidEdit(error.clone()))
    }
}
impl Data {
    fn capture(
        session: &Session,
        chart: usize,
        clock: std::sync::Arc<Clock>,
    ) -> Result<Self, Error> {
        let index = Index::new(clock.spacing_sections().collect())?;
        let interpolation = Timeline::capture(session, chart).map_err(|error| error.to_string());
        let document = session.document();
        let labels = Labels::from_document(document, chart).ok();
        let signatures = Signatures::from_document(document, chart).ok();
        let regions = ChartData::read(document, chart)
            .ok()
            .flatten()
            .and_then(|data| data.patterns)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(row, definition)| {
                row.checked_add(definition.length).map(|end| (row, end))
            })
            .collect();
        Ok(Self {
            clock,
            index,
            interpolation,
            labels,
            signatures,
            regions: RegionIndex::new(regions)?,
        })
    }
    pub fn position_row(&self, music_time: f64) -> Result<f64, Error> {
        self.clock.position(music_time)
    }
    pub fn last_row(&self) -> u64 {
        self.regions.last_row().max(
            self.labels
                .as_ref()
                .and_then(|labels| labels.values().last_key_value().map(|(&row, _)| row))
                .unwrap_or(0),
        )
    }
    pub fn projection(
        &self,
        music_time: f64,
        mode: SpacingMode,
        reverse: bool,
        zoom: f64,
        viewport: Viewport,
    ) -> Result<Projection<'_>, Error> {
        let time = self.clock.music_to_chart_time(music_time)?;
        let row = self.clock.position(music_time)?;
        if mode == SpacingMode::ConstantRow {
            let anchor = row.max(0.0).min(i32::MAX as f64).floor() as u64;
            let axis = RowProjection::new(
                anchor,
                f64::from(viewport.cursor),
                zoom,
                f64::from(viewport.top),
                f64::from(viewport.bottom),
            )?
            .with_reverse(reverse)
            .with_row_offset(row - anchor as f64)?;
            return Ok(Projection::Rows(axis, None));
        }
        let focal = Anchor {
            y: f64::from(viewport.cursor),
            time,
            row,
        };
        let settings = Settings {
            mode,
            reverse,
            speeds: Speeds::default(),
            interpolated_scroll: if mode == SpacingMode::Variable {
                self.interpolation
                    .as_ref()
                    .map_err(|error| Error::InvalidEdit(error.clone()))?
                    .rate_at(row, time)?
            } else {
                1.0
            },
            zoom,
        };
        let cursor = Cursor::begin(&self.index, focal, settings)?
            .ok_or_else(|| Error::InvalidEdit("Missing view timing sections".into()))?;
        let axis = TimeProjection::new(
            time,
            focal.y,
            settings.speeds.time * zoom,
            f64::from(viewport.top),
            f64::from(viewport.bottom),
            reverse,
        )?;
        if mode == SpacingMode::Variable {
            Ok(Projection::Variable {
                geometry: Frame::capture(cursor)?,
                waveform: axis,
            })
        } else {
            Ok(Projection::Time {
                axis,
                clock: &self.clock,
                cursor,
            })
        }
    }
}

pub(super) fn lanes(actors: &mut Vec<Actor>, grid: &Grid, geometry: Geometry, viewport: Viewport) {
    for lane in 0..=grid.columns() {
        let x = geometry.left + f32::from(lane) * geometry.lane_width;
        if geometry.visible_x(x) {
            actors.push(act!(quad: xy(x,(viewport.top+viewport.bottom)*0.5): zoomto(1.0,viewport.bottom-viewport.top): diffuse(0.12,0.16,0.22,1.0): z(8001)));
        }
    }
}
/// Shared by focused and inactive charts. Every note keeps its participant
/// color and its own layout's lane rotation. Selection/edit input is not part
/// of this renderer and therefore cannot accidentally target an inactive view.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct NoteRenderStats {
    pub candidates: usize,
    pub tree_queries: usize,
    pub partial: bool,
}
pub(super) fn partial_notice(
    actors: &mut Vec<Actor>,
    geometry: Geometry,
    viewport: Viewport,
    stats: NoteRenderStats,
) {
    let left = geometry.left.max(geometry.clip_left);
    let width = geometry.right().min(geometry.clip_right) - left;
    if stats.partial && width > 0.0 {
        actors.push(act!(text: font("miso"): settext(i18n::tr("Edition", "ChartViewPartial")): align(0.0,0.0): xy(left,viewport.bottom-32.0): zoom(0.4): wrapwidthpixels(width/0.4): maxwidth(width): maxheight(32.0): diffuse(1.0,0.8,0.3,1.0): z(8006)));
    }
}
pub(super) fn notes(
    actors: &mut Vec<Actor>,
    grid: &Grid,
    layout: &str,
    projection: &Projection<'_>,
    geometry: Geometry,
    viewport: Viewport,
    ranges: &[(u64, u64)],
    size_zoom: f32,
    note_limit: usize,
    policy: Option<&super::receptor_views::Policy<'_>>,
    reverse: bool,
    selection: Option<(u64,u64)>,
    palette: Option<(&deadsync_editor::note_color_palette::Palette,Option<&Signatures>)>,
    pending: Option<&deadsync_editor::lane_entry::Inputs>,
) -> NoteRenderStats {
    let mut stats = NoteRenderStats::default();
    if note_limit == 0 {
        stats.partial = !ranges.is_empty();
        return stats;
    }
    let bins = ((viewport.bottom - viewport.top).ceil() as usize)
        .clamp(1, 4096)
        .min(note_limit);
    let bins_per_range = (bins / ranges.len().max(1)).max(1);
    let mut shown = BTreeSet::new();
    let mut queried = 0usize;
    let mut hold_strips_remaining = note_limit;
    let mut tree_queries = 0usize;
    let tree_limit = note_limit.saturating_mul(4);
    stats.partial = ranges.len() > bins;
    for &(start, end) in ranges.iter().take(bins) {
        let remaining = note_limit.saturating_sub(queried);
        if remaining == 0 {
            stats.partial = true;
            break;
        }
        let remaining_queries = tree_limit.saturating_sub(tree_queries);
        if remaining_queries == 0 {
            stats.partial = true;
            break;
        }
        let overview = grid
            .visible_overview_budgeted(start, end, bins_per_range, remaining, remaining_queries)
            .expect("ordered bounded note viewport");
        queried += overview.notes.len();
        tree_queries += overview.queries;
        stats.partial |= overview.budget_exhausted
            || end.saturating_sub(start).saturating_add(1) > bins_per_range as u64;
        for (position, note) in overview.notes {
            if !shown.insert(position) {
                continue;
            }
            if pending.is_some_and(|input|input.deleted_heads().any(|deleted|deleted==position)){continue;}
            let presentation = policy.and_then(|policy| policy.presentation(position));
            if presentation.is_some_and(|presentation| presentation.hidden) {
                continue;
            }
            let x = geometry.left + (f32::from(position.lane) + 0.5) * geometry.lane_width;
            if !geometry.visible_x(x) {
                continue;
            }
            let Some(y) = projection.note_y(position.row) else {
                continue;
            };
            if let Some(tail) = note.end {
                let Some(tail_y) = projection.note_y(tail) else { continue; };
                let selected = selection.is_some_and(|(a,b)|a<=position.row&&position.row<=b);
                let request=super::hold_render::ChartRequest {
                    layout,row:position.row,lane:position.lane,player:position.player as usize,
                    selected,roll:note.kind==Kind::Roll,
                    active:presentation.is_some_and(|p|p.active_hold),
                    center_x:x,head_center:y,tail_center:tail_y,width:geometry.lane_width*size_zoom,
                    reverse,viewport:[geometry.clip_left,viewport.top,geometry.clip_right,viewport.bottom],
                    cutoff:presentation.filter(|p|p.clip_hold).map(|_|f64::from(viewport.cursor)),
                    size_zoom:f64::from(size_zoom),body_budget:hold_strips_remaining,
                };
                let result=match palette {
                    Some((palette,signatures))=>super::hold_render::chart_render_cached(request,palette,signatures),
                    None=>super::hold_render::chart_render(request),
                };
                match result {
                    Ok(output)=>{
                        hold_strips_remaining=hold_strips_remaining.saturating_sub(output.body_strips);
                        stats.partial|=output.partial;
                        actors.extend(output.actors);
                    }
                    Err(_)=>{stats.partial=true;}
                }
                continue;
            }
            let y = y as f32;
            let color = player_color(position.player);
            if y < viewport.top || y > viewport.bottom {
                continue;
            }
            let symbol = match note.kind {
                Kind::Mine => "M",
                Kind::Lift => "L",
                Kind::Fake => "F",
                Kind::Keysound => "K",
                Kind::Attack => "A",
                _ => "",
            };
            if symbol.is_empty() {
                let rotation = lane_rotation(layout, position.lane);
                actors.push(act!(sprite("test_input/highlightarrow.png"): xy(x,y): setsize(geometry.lane_width*0.7*size_zoom,geometry.lane_width*0.7*size_zoom): rotationz(rotation): diffuse(color[0],color[1],color[2],1.0): z(8004)));
            } else {
                text(
                    actors,
                    symbol,
                    x - 6.0 * size_zoom,
                    y - 9.0 * size_zoom,
                    0.65 * size_zoom,
                    color,
                );
            }
        }
    }
    stats.candidates = queried;
    stats.tree_queries = tree_queries;
    stats
}
pub(super) fn cursor(actors: &mut Vec<Actor>, geometry: Geometry, viewport: Viewport) {
    let left = geometry.left.max(geometry.clip_left);
    let right = geometry.right().min(geometry.clip_right);
    if right <= left {
        return;
    }
    let color = if geometry.focused {
        [1.0, 0.85, 0.25, 1.0]
    } else {
        [0.5, 0.6, 0.7, 0.7]
    };
    actors.push(act!(quad: xy((left+right)*0.5,viewport.cursor): zoomto(right-left,2.0): diffuse(color[0],color[1],color[2],color[3]): z(8005)));
}
/// Inactive views retain their own beat/signature/label and pattern markers.
/// Focused selection, waveform policy and interactive attack widgets remain
/// separate layers rather than being copied from a different chart.
pub(super) fn markers(
    actors: &mut Vec<Actor>,
    data: &Data,
    projection: &Projection<'_>,
    geometry: Geometry,
    viewport: Viewport,
    ranges: &[(u64, u64)],
    budget: usize,
) {
    let bins = budget.clamp(1, 4096);
    let per_range = (bins / ranges.len().max(1)).max(1);
    let mut labels = Vec::new();
    let mut regions = BTreeSet::new();
    for &(start, end) in ranges.iter().take(bins) {
        let bands = deadsync_editor::projection::RowBands::new(start, end, per_range)
            .expect("bounded visible bands");
        for (a, b) in bands {
            if let Some(row) = a.div_ceil(48).checked_mul(48).filter(|row| *row <= b) {
                let measure = data.signatures.as_ref().is_some_and(|signatures| {
                    signatures
                        .measure_position(row)
                        .is_ok_and(|position| position.row_in_measure == 0)
                });
                if !measure {
                    if let Some(y) = projection.y(row).filter(|y| {
                        *y >= f64::from(viewport.top) && *y <= f64::from(viewport.bottom)
                    }) {
                        horizontal(actors, geometry, y as f32, 0.15);
                    }
                }
            }
            if let Some(signatures) = &data.signatures {
                if a <= i32::MAX as u64 {
                    if let Ok(mut starts) = signatures.measure_starts(a, b.min(i32::MAX as u64)) {
                        if let Some((row, measure)) = starts.next() {
                            if let Some(y) = projection.y(row).filter(|y| {
                                *y >= f64::from(viewport.top) && *y <= f64::from(viewport.bottom)
                            }) {
                                horizontal(actors, geometry, y as f32, 0.4);
                                if geometry.visible_x(geometry.left - 30.0) {
                                    text(
                                        actors,
                                        &(measure + 1).to_string(),
                                        geometry.left - 30.0,
                                        y as f32,
                                        0.45,
                                        [0.6, 0.7, 0.8, 1.0],
                                    );
                                }
                            }
                        }
                    }
                }
            }
            if let Some(values) = &data.labels {
                if let Some((&row, value)) = values.values().range(a..=b).next() {
                    if let Some(y) = projection.y(row).filter(|y| {
                        *y >= f64::from(viewport.top) && *y <= f64::from(viewport.bottom)
                    }) {
                        labels.push((row, y, value.clone()));
                    }
                }
            }
        }
        for (a, b) in data
            .regions
            .visible(start, end, per_range)
            .expect("bounded region bands")
        {
            if !regions.insert((a, b)) {
                continue;
            }
            let (Some(y1), Some(y2)) = (
                projection.clipped_y(a, viewport.top, viewport.bottom),
                projection.clipped_y(b, viewport.top, viewport.bottom),
            ) else {
                continue;
            };
            if geometry.visible_x(geometry.left - 9.0) {
                actors.push(act!(quad: xy(geometry.left-9.0,((y1+y2)*0.5) as f32): zoomto(4.0,(y2-y1).abs().max(2.0) as f32): diffuse(0.7,0.4,1.0,0.8): z(8002)));
            }
            if a >= start && geometry.visible_x(geometry.left - 42.0) {
                text(
                    actors,
                    &i18n::tr("Edition", "PatternMarker"),
                    geometry.left - 42.0,
                    y1 as f32 - 9.0,
                    0.35,
                    [0.8, 0.6, 1.0, 1.0],
                );
            }
        }
    }
    if geometry.visible_x(geometry.right() + 12.0) {
        super::spacing_view::label_actors(
            actors,
            labels,
            geometry.right() + 12.0,
            geometry.clip_right.min(geometry.right() + 30.0),
        );
    }
}
fn horizontal(actors: &mut Vec<Actor>, geometry: Geometry, y: f32, alpha: f32) {
    let left = geometry.left.max(geometry.clip_left);
    let right = geometry.right().min(geometry.clip_right);
    if right > left {
        actors.push(act!(quad: xy((left+right)*0.5,y): zoomto(right-left,1.0): diffuse(0.6,0.7,0.8,alpha): z(8001)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadsync_editor::notes::{Note, Position};
    fn draw(grid: &Grid, ranges: &[(u64, u64)], limit: usize) -> NoteRenderStats {
        let geometry = Geometry {
            source: 0,
            order: 0,
            focused: true,
            left: 331.0,
            field_width: 192.0,
            lane_width: 48.0,
            clip_left: 0.0,
            clip_right: 854.0,
        };
        let viewport = Viewport {
            top: 105.0,
            bottom: 635.0,
            cursor: 160.0,
        };
        let projection = Projection::Rows(
            RowProjection::new(0, 160.0, 1.0, 105.0, 635.0).unwrap(),
            None,
        );
        notes(
            &mut Vec::new(),
            grid,
            "dance-single",
            &projection,
            geometry,
            viewport,
            ranges,
            1.0,
            limit,
            None,
            false,
            None,
            None,
            None,
        )
    }
    #[test]
    fn renderer_reports_candidate_limit_and_deterministic_partial_output() {
        let mut grid = Grid::new(4, 1).unwrap();
        for row in 0..100 {
            grid.set(
                Position {
                    player: 0,
                    lane: 0,
                    row,
                },
                Some(Note::tap()),
            )
            .unwrap();
        }
        let stats = draw(&grid, &[(0, 99)], 3);
        assert!(stats.partial);
        assert!(stats.candidates <= 3);
        assert!(stats.tree_queries <= 12);
        assert_eq!(stats, draw(&grid, &[(0, 99)], 3));
        let complete = draw(&grid, &[(0, 99)], 1000);
        assert!(!complete.partial);
        assert_eq!(complete.candidates, 100);
    }
    #[test]
    fn renderer_reports_query_limit_even_when_all_tracks_are_outside_view() {
        let mut grid = Grid::new(8, 128).unwrap();
        for player in 0..128 {
            for lane in 0..8 {
                grid.set(
                    Position {
                        player,
                        lane,
                        row: 10_000,
                    },
                    Some(Note::tap()),
                )
                .unwrap();
            }
        }
        let stats = draw(&grid, &[(0, 99)], 8);
        assert_eq!(stats.candidates, 0);
        assert_eq!(stats.tree_queries, 32);
        assert!(stats.partial);
    }
}
