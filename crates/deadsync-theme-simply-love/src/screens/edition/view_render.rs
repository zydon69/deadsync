//! Per-chart immutable timing inputs and shared lane-note actor construction.
//! The Session remains the owner of every live note grid, including unsaved edits.
use super::{player_color, spacing_view::Projection, text, view_layout::Geometry};
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
    pub waveform_music: String,
    pub waveform_offset: f64,
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
        let waveform_music = deadsync_editor::chart_text::local_value(
            document,
            chart,
            deadsync_editor::chart_text::Field::Music,
        )?
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| document.text(None, "MUSIC").unwrap_or_default());
        let waveform_offset = deadsync_editor::chart_properties::Draft::capture(session, chart)?
            .effective_music_offset();
        Ok(Self {
            waveform_music,
            waveform_offset,
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
        speeds: Speeds,
    ) -> Result<Projection<'_>, Error> {
        let time = self.clock.music_to_chart_time(music_time)?;
        let row = self.clock.position(music_time)?;
        if mode == SpacingMode::ConstantRow {
            let anchor = row.max(0.0).min(i32::MAX as f64).floor() as u64;
            let axis = RowProjection::new(
                anchor,
                f64::from(viewport.cursor),
                speeds.row * zoom,
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
            speeds,
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
    selection: Option<(u64, u64)>,
    palette: Option<(
        &deadsync_editor::note_color_palette::Palette,
        Option<&Signatures>,
    )>,
    pending: Option<&deadsync_editor::lane_entry::Inputs>,
    admitted: Option<&[deadsync_editor::notes::Position]>,
    explicit_selection: Option<&BTreeSet<deadsync_editor::notes::Position>>,
) -> NoteRenderStats {
    let mut stats = NoteRenderStats::default();
    if note_limit == 0 {
        stats.partial = admitted.is_none() && !ranges.is_empty();
        return stats;
    }
    let needs_classification = palette.is_some();
    stats.partial = needs_classification && policy.is_none();
    let default_palette = palette.is_none().then(|| {
        deadsync_editor::note_color_palette::Palette::capture(Default::default())
            .expect("valid default note colors")
    });
    let (note_palette, signatures) =
        palette.unwrap_or_else(|| (default_palette.as_ref().unwrap(), None));
    // Callers query beyond both viewport edges for crossing sprites. Allocate
    // bins for that same padded window, still under the existing hard caps.
    let padding = geometry.lane_width * size_zoom;
    let bins = ((viewport.bottom - viewport.top + 2.0 * padding).ceil() as usize)
        .clamp(1, 4096)
        .min(note_limit);
    let bins_per_range = (bins / ranges.len().max(1)).max(1);
    let mut shown = BTreeSet::new();
    let mut queried = 0usize;
    // Admission counts musical notes, not the atlas tiles composing a hold.
    // Even one admitted hold may need hundreds of visible body strips.
    let hold_body_budget = super::hold_render::viewport_body_budget(
        geometry.lane_width * size_zoom,
        viewport.bottom - viewport.top,
    );
    let mut tree_queries = 0usize;
    let tree_limit = note_limit.saturating_mul(4);
    stats.partial |= admitted.is_none() && ranges.len() > bins;
    let exact_range = [(0, 0)];
    let draw_ranges = if admitted.is_some() {
        &exact_range[..]
    } else {
        ranges
    };
    for &(start, end) in draw_ranges.iter().take(bins) {
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
        let overview = if let Some(admitted) = admitted {
            deadsync_editor::notes::Overview {
                notes: admitted
                    .iter()
                    .filter_map(|&position| grid.get(position).map(|note| (position, note)))
                    .collect(),
                queries: 0,
                budget_exhausted: false,
            }
        } else {
            grid.visible_overview_budgeted(start, end, bins_per_range, remaining, remaining_queries)
                .expect("ordered bounded note viewport")
        };
        queried += overview.notes.len();
        tree_queries += overview.queries;
        stats.partial |= overview.budget_exhausted
            || admitted.is_none()
                && end.saturating_sub(start).saturating_add(1) > bins_per_range as u64;
        for (position, note) in overview.notes {
            if !shown.insert(position) && admitted.is_none() {
                continue;
            }
            if pending.is_some_and(|input| input.deleted_heads().any(|deleted| deleted == position))
            {
                continue;
            }
            let presentation = policy.and_then(|policy| policy.presentation(position));
            stats.partial |= needs_classification && presentation.is_none();
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
                let Some(tail_y) = projection.note_y(tail) else {
                    continue;
                };
                let selected = explicit_selection.map_or_else(
                    || selection.is_some_and(|(a, b)| a <= position.row && position.row <= b),
                    |selected| selected.contains(&position),
                );
                let request = super::hold_render::ChartRequest {
                    layout,
                    row: position.row,
                    lane: position.lane,
                    player: position.player as usize,
                    selected,
                    roll: note.kind == Kind::Roll,
                    active: presentation.is_some_and(|p| p.raw_active_hold),
                    center_x: x,
                    head_center: y,
                    tail_center: tail_y,
                    width: geometry.lane_width * size_zoom,
                    reverse,
                    viewport: [
                        geometry.clip_left,
                        viewport.top,
                        geometry.clip_right,
                        viewport.bottom,
                    ],
                    cutoff: presentation
                        .filter(|p| p.clip_hold)
                        .map(|_| f64::from(viewport.cursor)),
                    size_zoom: f64::from(size_zoom),
                    body_budget: hold_body_budget,
                };
                let result = match palette {
                    Some((palette, signatures)) => {
                        super::hold_render::chart_render_cached_with_options(
                            request,
                            palette,
                            signatures,
                            policy.is_none_or(|p| p.light_holds()),
                        )
                    }
                    None => super::hold_render::chart_render(request),
                };
                match result {
                    Ok(output) => {
                        stats.partial |= output.partial;
                        actors.extend(output.actors);
                        if presentation.is_some_and(|p| p.fake) {
                            let marker = deadsync_editor::receptors::alpha(f64::from(size_zoom))
                                .map_err(|_| "Invalid note alpha")
                                .and_then(|alpha| {
                                    super::short_note_render::fake_marker(
                                        [x, y as f32],
                                        geometry.lane_width * size_zoom,
                                        alpha,
                                    )
                                });
                            match marker {
                                Ok(mut actor) => match super::hold_render::clip_world_sprite(
                                    &mut actor,
                                    [
                                        geometry.clip_left,
                                        viewport.top,
                                        geometry.clip_right,
                                        viewport.bottom,
                                    ],
                                ) {
                                    Ok(true) => actors.push(actor),
                                    Ok(false) => {}
                                    Err(_) => stats.partial = true,
                                },
                                Err(_) => stats.partial = true,
                            }
                        }
                    }
                    Err(_) => {
                        stats.partial = true;
                    }
                }
                continue;
            }
            let selected = explicit_selection.map_or_else(
                || selection.is_some_and(|(a, b)| a <= position.row && position.row <= b),
                |selected| selected.contains(&position),
            );
            let kind = match note.kind {
                Kind::Mine => super::short_note_render::Kind::Mine,
                Kind::Lift => super::short_note_render::Kind::Lift,
                Kind::Keysound | Kind::Attack => {
                    if y >= f64::from(viewport.top) && y <= f64::from(viewport.bottom) {
                        text(
                            actors,
                            if note.kind == Kind::Keysound {
                                "K"
                            } else {
                                "A"
                            },
                            x - 6.0 * size_zoom,
                            y as f32 - 9.0 * size_zoom,
                            0.65 * size_zoom,
                            player_color(position.player),
                        );
                    }
                    continue;
                }
                _ => super::short_note_render::Kind::Tap,
            };
            let color = if note.kind == Kind::Mine {
                note_palette.mine(layout, position.player, selected)
            } else {
                note_palette
                    .color_row(position.row, signatures)
                    .and_then(|row| {
                        note_palette.tap(layout, row, position.lane, position.player, selected)
                    })
            };
            let result = color
                .map_err(|_| "Invalid short-note color")
                .and_then(|fill| {
                    super::short_note_render::render(super::short_note_render::Request {
                        layout,
                        lane: position.lane,
                        kind,
                        selected,
                        fake: note.kind == Kind::Fake || presentation.is_some_and(|p| p.fake),
                        center: [x, y as f32],
                        width: geometry.lane_width * size_zoom,
                        fill,
                        alpha: deadsync_editor::receptors::alpha(f64::from(size_zoom))
                            .map_err(|_| "Invalid note alpha")?,
                    })
                });
            match result {
                Ok(output) => {
                    for mut actor in output {
                        match super::hold_render::clip_world_sprite(
                            &mut actor,
                            [
                                geometry.clip_left,
                                viewport.top,
                                geometry.clip_right,
                                viewport.bottom,
                            ],
                        ) {
                            Ok(true) => actors.push(actor),
                            Ok(false) => {}
                            Err(_) => stats.partial = true,
                        }
                    }
                }
                Err(_) => stats.partial = true,
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
    marker_limit: usize,
    render: deadsync_editor::options_preferences::RenderPolicy,
) {
    let bins = budget.clamp(1, 4096);
    let per_range = (bins / ranges.len().max(1)).max(1);
    let mut marker_count = 0usize;
    let mut regions = BTreeSet::new();
    for &(start, end) in ranges.iter().take(bins) {
        let bands = deadsync_editor::projection::RowBands::new(start, end, per_range)
            .expect("bounded visible bands");
        for (a, b) in bands {
            if render.markers && marker_count < marker_limit {
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
                            marker_count += 1;
                        }
                    }
                }
                if let Some(signatures) = &data.signatures {
                    if marker_count >= marker_limit {
                        continue;
                    }
                    if a <= i32::MAX as u64 {
                        if let Ok(mut starts) = signatures.measure_starts(a, b.min(i32::MAX as u64))
                        {
                            if let Some((row, measure)) = starts.next() {
                                if let Some(y) = projection.y(row).filter(|y| {
                                    *y >= f64::from(viewport.top)
                                        && *y <= f64::from(viewport.bottom)
                                }) {
                                    horizontal(actors, geometry, y as f32, 0.4);
                                    marker_count += 1;
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
            if render.regions && geometry.visible_x(geometry.left - 9.0) {
                actors.push(act!(quad: xy(geometry.left-9.0,((y1+y2)*0.5) as f32): zoomto(4.0,(y2-y1).abs().max(2.0) as f32): diffuse(0.7,0.4,1.0,0.8): z(8002)));
            }
        }
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
        draw_at(grid, ranges, limit, 0, 635.0)
    }
    fn draw_at(
        grid: &Grid,
        ranges: &[(u64, u64)],
        limit: usize,
        anchor: u64,
        bottom: f32,
    ) -> NoteRenderStats {
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
            bottom,
            cursor: 160.0,
        };
        let projection = Projection::Rows(
            RowProjection::new(anchor, 160.0, 1.0, 105.0, f64::from(bottom)).unwrap(),
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
            None,
            None,
        )
    }
    #[test]
    fn padded_sparse_query_keeps_crossing_sprites_without_claiming_partial_output() {
        let mut grid = Grid::new(4, 1).unwrap();
        for row in [0, 240] {
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
        let stats = draw_at(&grid, &[(0, 355)], 1000, 72, 395.0);
        assert_eq!(stats.candidates, 2);
        assert!(!stats.partial);
        assert!(stats.tree_queries <= 4000);
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

#[cfg(test)]
mod admitted_hold_body_tests {
    use super::*;
    use deadsync_editor::notes::{Note, Position};

    #[test]
    fn admitted_long_hold_does_not_starve_adjacent_roll_body() {
        let positions = [
            Position {
                player: 0,
                lane: 0,
                row: 0,
            },
            Position {
                player: 0,
                lane: 1,
                row: 48,
            },
        ];
        let mut grid = Grid::new(4, 1).unwrap();
        for (position, kind, tail) in [
            (positions[0], Kind::Hold, 1_000_000_000),
            (positions[1], Kind::Roll, 192),
        ] {
            let mut note = Note::tap();
            note.kind = kind;
            note.end = Some(tail);
            grid.set(position, Some(note)).unwrap();
        }
        for bottom in [395.0, 635.0, 1000.0] {
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
                bottom,
                cursor: 160.0,
            };
            let projection = Projection::Rows(
                RowProjection::new(0, 160.0, 1.0, 105.0, f64::from(bottom)).unwrap(),
                None,
            );
            let mut actors = Vec::new();
            let stats = notes(
                &mut actors,
                &grid,
                "dance-single",
                &projection,
                geometry,
                viewport,
                &[(0, 1000)],
                1.0,
                positions.len(),
                None,
                false,
                None,
                None,
                None,
                Some(&positions),
                None,
            );
            assert_eq!(stats.candidates, 2);
            assert!(
                !stats.partial,
                "visible bodies must not consume note admission: {bottom}"
            );
            for lane in [0, 1] {
                let x = geometry.left + (lane as f32 + 0.5) * geometry.lane_width;
                assert!(
                    actors
                        .iter()
                        .filter(|actor| matches!(actor,
                            Actor::Sprite { offset, .. } if (offset[0] - x).abs() < 0.001
                        ))
                        .count()
                        > 4,
                    "lane{lane} needs body strips as well as head/cap"
                );
            }
        }
    }
}

#[cfg(test)]
mod explicit_note_selection_tests {
    use super::*;
    use deadsync_editor::notes::{Note, Position};

    #[test]
    fn explicit_tap_hold_and_roll_selection_matches_range_selected_graphics() {
        let positions = (0..3)
            .map(|lane| Position {
                player: 0,
                lane,
                row: 0,
            })
            .collect::<Vec<_>>();
        let mut grid = Grid::new(4, 1).unwrap();
        for (position, kind) in positions
            .iter()
            .copied()
            .zip([Kind::Tap, Kind::Hold, Kind::Roll])
        {
            let mut note = Note::tap();
            note.kind = kind;
            note.end = (kind != Kind::Tap).then_some(96);
            grid.set(position, Some(note)).unwrap();
        }
        let selected = positions.iter().copied().collect::<BTreeSet<_>>();
        let render = |range, explicit| {
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
            let mut actors = Vec::new();
            let stats = notes(
                &mut actors,
                &grid,
                "dance-single",
                &projection,
                geometry,
                viewport,
                &[(0, 192)],
                1.0,
                positions.len(),
                None,
                false,
                range,
                None,
                None,
                Some(&positions),
                explicit,
            );
            assert!(!stats.partial);
            actors
                .into_iter()
                .filter_map(|actor| match actor {
                    Actor::Sprite { tint, uv_rect, .. } => Some((tint, uv_rect)),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let ordinary = render(None, None);
        let ranged = render(Some((0, 0)), None);
        assert_ne!(
            ordinary, ranged,
            "selection must change the actual note graphics"
        );
        assert_eq!(render(None, Some(&selected)), ranged);
        assert_eq!(render(Some((0, 0)), Some(&selected)), ranged);
        assert_eq!(
            render(Some((0, 0)), Some(&BTreeSet::new())),
            ordinary,
            "explicit deselection must override an older range highlight"
        );
    }
}
