//! Keyboard adapter for the source's two-endpoint region selection. Chart
//! geometry and Misc rectangles are shared with the actual renderer below.
use super::{Panel, State, spacing_view::Projection, view_layout::Geometry, view_render::Viewport};
use crate::{act, i18n};
use deadlib_platform::input::KeyCode;
use deadlib_present::{actors::Actor, space::screen_width};
use deadsync_editor::{
    Error,
    chart_focus::Position,
    event_clipboard::{Event, WidgetKind, region::Modifiers},
    playback::Clock,
    region_selection::{
        geometry::{self, Axis, Range},
        inventory::Entry,
        misc_layout::{Layout, Rect},
    },
    selection_preferences::{Overlap, Preferences, RegionMode},
    spacing_index::Index,
    spacing_search::Settings as SpacingSettings,
    zoom::SpacingMode,
};
pub(super) struct Settings {
    points: [Position; 2],
    x: [f64; 2],
    index: usize,
    preferences: Preferences,
    misc: bool,
    add: bool,
    toggle: bool,
}
pub(super) struct DraftWidgetCache {
    binding: deadsync_editor::chart_order::SessionBinding,
    charts: Vec<usize>,
    values: [u64; 5],
    preview: String,
    entries: std::collections::BTreeMap<usize, Vec<deadsync_editor::event_clipboard::SongWidget>>,
}
pub(super) struct RenderPlan {
    source: usize,
    pub(super) admission: deadsync_editor::region_selection::visible_events::Plan,
    rectangles: Vec<(Event, Rect)>,
    widget_metrics: deadsync_editor::options_preferences::dpi::WidgetMetrics,
}
/// Folded/zero-speed projections can place different logical rows on one
/// screen coordinate. Keep the final rectangles shared by drawing and picking.
fn separate_projected_widgets(rectangles: &mut Vec<(Event, Rect)>, padding: f64, bounds: [f64; 4]) {
    let [left, top, right, bottom] = bounds;
    // A stable screen-order sweep retains logical insertion order at equal Y.
    // Retain only visible hit targets, exactly as the renderer clips them.
    rectangles.retain(|(_, r)| {
        r.x < right && r.x + r.width > left && r.y < bottom && r.y + r.height > top
    });
    rectangles.sort_by(|a, b| a.1.y.total_cmp(&b.1.y));
    let mut active = Vec::<Rect>::new();
    rectangles.retain_mut(|(_, rectangle)| {
        active.retain(|previous| previous.y + previous.height + padding > rectangle.y);
        active.sort_by(|a, b| (a.y + a.height).total_cmp(&(b.y + b.height)));
        for previous in &active {
            if rectangle.x < previous.x + previous.width
                && previous.x < rectangle.x + rectangle.width
                && rectangle.y < previous.y + previous.height + padding
                && previous.y < rectangle.y + rectangle.height + padding
            {
                rectangle.y = previous.y + previous.height + padding;
            }
        }
        if rectangle.y >= bottom {
            return false;
        }
        // Non-overlapping positive-size visible rectangles bound this active
        // set by physical viewport capacity, independently of chart row count.
        active.push(*rectangle);
        true
    });
}
impl State {
    pub(super) fn refresh_misc_draft_widgets(&mut self) {
        let Some(draft) = self.song_properties.as_ref() else {
            self.misc_draft_widgets = None;
            return;
        };
        let session = &self.workspace.file().session;
        let charts: Vec<_> = self
            .visible_view_geometry()
            .iter()
            .map(|g| g.source)
            .collect();
        let values = [
            draft.sample_start(),
            draft.sample_length(),
            draft.last_second_hint(),
            draft.music_offset(),
            draft.sync_offset(),
        ]
        .map(f64::to_bits);
        let preview = draft.text(deadsync_editor::song_text::Field::PreviewMusic);
        if self.misc_draft_widgets.as_ref().is_some_and(|c| {
            c.binding.is_current(session)
                && c.charts == charts
                && c.values == values
                && c.preview == preview
        }) {
            return;
        }
        let entries = charts
            .iter()
            .map(|&chart| {
                draft
                    .widgets(session, chart)
                    .map(|widgets| (chart, widgets))
            })
            .collect::<Result<_, _>>();
        match entries {
            Ok(entries) => {
                self.misc_draft_widgets = Some(DraftWidgetCache {
                    binding: deadsync_editor::chart_order::SessionBinding::capture(session),
                    charts,
                    values,
                    preview: preview.to_owned(),
                    entries,
                })
            }
            Err(error) => {
                self.misc_draft_widgets = None;
                self.status = i18n::tr_fmt(
                    "Edition",
                    "Error",
                    &[("message", &crate::i18n::editor_error(&error))],
                );
            }
        }
    }
    pub(super) fn sync_selection_preferences_history(&mut self) {
        if let Some(update) = self.workspace.take_selection_preferences_update() {
            self.selection_preferences = update.preferences;
            if let Some(file) = update.file {
                self.publish_application_preferences_owner(file);
            }
        }
    }
    pub(super) fn refresh_misc_inventory(&mut self) {
        if !self.workspace.has_chart() {
            self.misc_inventories.clear();
            self.region_selection = None;
            return;
        }
        let mut charts: Vec<_> = self
            .visible_view_geometry()
            .iter()
            .map(|g| g.source)
            .collect();
        if let Some(source) = self.preview_source_chart() {
            if !charts.contains(&source) {
                charts.push(source);
            }
        }
        self.misc_inventories
            .retain(|chart, _| charts.contains(chart));
        for chart in charts {
            if self
                .misc_inventories
                .get(&chart)
                .is_some_and(|i| i.is_current(&self.workspace.file().session, chart))
            {
                continue;
            }
            match self.workspace.misc_inventory(chart) {
                Ok(inventory) => {
                    self.misc_inventories.insert(chart, inventory);
                }
                Err(error) => {
                    self.misc_inventories.remove(&chart);
                    self.status = i18n::tr_fmt(
                        "Edition",
                        "Error",
                        &[("message", &crate::i18n::editor_error(&error))],
                    );
                }
            }
        }
    }
    pub(super) fn open_region_selection(&mut self) {
        let result = (|| -> Result<_, Error> {
            let current = self.capture_song_property_presentation()?.focus().clone();
            let mut selection = if let Some(settings) = &self.event_selection {
                self.workspace.expand_event_selection(&settings.selection)?
            } else {
                self.workspace
                    .region_inventory(self.workspace.chart())?
                    .choices()
            };
            selection.include_song_widgets(&self.workspace.file().session)?;
            Ok((current, selection))
        })();
        match result {
            Ok((point, selection)) => {
                self.replace_region_event_selection(selection);
                self.lane_entry_release_input();
                self.region_selection = Some(Settings {
                    points: [point.clone(), point],
                    x: [0.0, f64::from(self.workspace.grid().columns())],
                    index: 0,
                    preferences: self.selection_preferences,
                    misc: false,
                    add: false,
                    toggle: false,
                });
                self.panel = Some(Panel::EventSelection);
                self.status = i18n::tr("Edition", "RegionSelectionHelp");
            }
            Err(error) => {
                self.status = i18n::tr_fmt(
                    "Edition",
                    "Error",
                    &[("message", &crate::i18n::editor_error(&error))],
                )
            }
        }
    }
    pub(super) fn region_selection_key(&mut self, code: KeyCode) {
        let Some(mut settings) = self.region_selection.take() else {
            return;
        };
        if code == KeyCode::Escape {
            self.status = i18n::tr("Edition", "EventSelectionTitle");
            return;
        }
        let result = (|| -> Result<bool, Error> {
            if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
                settings.index =
                    (settings.index + if code == KeyCode::ArrowUp { 9 } else { 1 }) % 10;
            }
            let direction = if code == KeyCode::ArrowLeft {
                -1.0
            } else {
                1.0
            };
            if matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight) {
                match settings.index {
                    0 | 2 => {
                        let index = settings.index / 2;
                        let session = &self.workspace.file().session;
                        let step = if self.shift {
                            0.001
                        } else {
                            if self.spacing_mode == SpacingMode::ConstantTime {
                                0.01
                            } else {
                                deadsync_editor::workspace::snap_edit_rows(self.workspace.snap())
                                    as f64
                            }
                        };
                        settings.points[index] = if self.spacing_mode == SpacingMode::ConstantTime {
                            let clock = Clock::capture(session, self.workspace.chart())?;
                            Position::at_time(
                                session,
                                self.workspace.chart(),
                                clock.chart_time_to_music(
                                    settings.points[index].chart_time() + direction * step,
                                )?,
                            )?
                        } else {
                            Position::at_fractional_row(
                                session,
                                self.workspace.chart(),
                                settings.points[index].row() + direction * step,
                            )?
                        };
                    }
                    1 | 3 => {
                        let i = (settings.index - 1) / 2;
                        settings.x[i] = (settings.x[i]
                            + direction * if self.shift { 0.01 } else { 0.5 })
                        .clamp(-64.0, 128.0);
                    }
                    4 => {
                        settings.preferences.overlap =
                            Overlap::ALL[(settings.preferences.overlap.index()
                                + if direction < 0.0 { 2 } else { 1 })
                                % 3]
                    }
                    5 => {
                        settings.preferences.region =
                            RegionMode::ALL[(settings.preferences.region.index() + 1) % 2]
                    }
                    6 => settings.misc = !settings.misc,
                    7 => settings.add = !settings.add,
                    8 => settings.toggle = !settings.toggle,
                    _ => {}
                }
            }
            if code == KeyCode::F2 && matches!(settings.index, 0 | 2) {
                settings.points[settings.index / 2] =
                    self.capture_song_property_presentation()?.focus().clone();
            }
            if code == KeyCode::Home && matches!(settings.index, 0 | 2) {
                settings.points[settings.index / 2] = Position::at_fractional_row(
                    &self.workspace.file().session,
                    self.workspace.chart(),
                    0.0,
                )?;
            }
            if code == KeyCode::F3 {
                settings.preferences = Preferences::default();
            }
            if code == KeyCode::Enter {
                if settings.index < 9 {
                    settings.index = 9;
                } else {
                    self.apply_region_selection(&settings)?;
                    return Ok(true);
                }
            }
            Ok(false)
        })();
        match result {
            Ok(true) => {}
            Ok(false) => {
                self.region_selection = Some(settings);
                self.status = i18n::tr("Edition", "RegionSelectionHelp");
            }
            Err(error) => {
                self.region_selection = Some(settings);
                self.status = i18n::tr_fmt(
                    "Edition",
                    "Error",
                    &[("message", &crate::i18n::editor_error(&error))],
                );
            }
        }
    }
    fn region_geometry(&self) -> Result<(Geometry, Viewport), Error> {
        let g = self
            .visible_view_geometry()
            .into_iter()
            .find(|g| g.source == self.workspace.chart())
            .ok_or_else(|| Error::InvalidEdit("Region chart is not visible".into()))?;
        Ok((g, self.chart_viewport()))
    }
    fn apply_region_selection(&mut self, settings: &Settings) -> Result<(), Error> {
        let chart = self.workspace.chart();
        let session = &self.workspace.file().session;
        if settings
            .points
            .iter()
            .any(|p| !p.is_bound_to(session, chart))
        {
            return Err(Error::InvalidEdit(
                "Region endpoints belong to an older chart clock".into(),
            ));
        }
        let inventory = self.workspace.region_inventory(chart)?;
        let session = &self.workspace.file().session;
        let (geometry, viewport) = self.region_geometry()?;
        let projection = self.display_projection(viewport.top, viewport.bottom, viewport.cursor)?;
        let width = f64::from(geometry.lane_width);
        let x = Range::normalized(settings.x[0] * width, settings.x[1] * width)?;
        let lanes = geometry::selected_lanes(
            x,
            width,
            width * self.zoom.size_zoom(),
            settings.preferences.overlap,
        )?;
        let by_time = self.spacing_mode == SpacingMode::ConstantTime;
        let bounds = Range::normalized(
            if by_time {
                settings.points[0].chart_time()
            } else {
                settings.points[0].row()
            },
            if by_time {
                settings.points[1].chart_time()
            } else {
                settings.points[1].row()
            },
        )?;
        let clock = Clock::capture(session, chart)?;
        let index = Index::new(clock.spacing_sections().collect())?;
        let current = self.capture_song_property_presentation()?;
        let scroll = if self.spacing_mode == SpacingMode::Variable {
            deadsync_editor::scroll_interpolation::Timeline::capture(session, chart)?.rate_at(
                current.row(),
                clock.music_to_chart_time(current.music_time())?,
            )?
        } else {
            1.0
        };
        let spacing = SpacingSettings {
            mode: self.spacing_mode,
            reverse: self.reverse_display,
            speeds: self.current_spacing_speeds(),
            interpolated_scroll: scroll,
            zoom: self.zoom.spacing_zoom(),
        };
        let bounds = geometry::adjusted_range(
            &index,
            spacing,
            if by_time { Axis::Time } else { Axis::Position },
            bounds,
            width * self.zoom.size_zoom() * 0.5,
            settings.preferences.overlap,
        )?;
        let ys = self.region_endpoint_y(settings, &projection)?;
        let y = Range::normalized(ys[0], ys[1])?;
        let (misc, source_y) = self.source_region_misc(
            chart,
            geometry,
            &index,
            spacing,
            deadsync_editor::spacing::Anchor {
                y: f64::from(viewport.cursor),
                time: clock.music_to_chart_time(current.music_time())?,
                row: current.row(),
            },
            settings,
        )?;
        let y = source_y.unwrap_or(y);
        let screen = Rect {
            x: f64::from(geometry.left) + x.min,
            y: y.min,
            width: x.max - x.min,
            height: y.max - y.min,
        };
        use deadsync_editor::key_binding_preferences::scroll_modifiers::Kind as Held;
        let mods = Modifiers {
            alt: settings.misc || self.owned_gesture_query(Held::SelectionMisc),
            control: settings.toggle || self.owned_gesture_query(Held::SelectionToggle),
            shift: settings.add || self.owned_gesture_query(Held::SelectionExtend),
        };
        let candidates = inventory.collect(
            bounds,
            by_time,
            lanes,
            self.workspace.grid().columns(),
            self.render_policy(),
            mods.alt,
            &misc,
            screen,
            settings.preferences.overlap,
        );
        let mut selection = self
            .event_selection
            .as_ref()
            .ok_or_else(|| Error::InvalidEdit("Region selection is unavailable".into()))?
            .selection
            .clone();
        selection.finish_region(session, &candidates, mods, settings.preferences.region)?;
        let file = self.capture_application_preferences_file()?;
        self.workspace.set_selection_preferences(
            self.selection_preferences,
            settings.preferences,
            Some(&file),
        )?;
        self.sync_selection_preferences_history();
        self.replace_region_event_selection(selection);
        self.panel = Some(Panel::EventSelection);
        self.status = i18n::tr_fmt(
            "Edition",
            "RegionSelectionApplied",
            &[("count", &candidates.len().to_string())],
        );
        Ok(())
    }
    fn region_endpoint_y(
        &self,
        settings: &Settings,
        projection: &Projection<'_>,
    ) -> Result<[f64; 2], Error> {
        if settings
            .points
            .iter()
            .any(|p| !p.is_bound_to(&self.workspace.file().session, self.workspace.chart()))
        {
            return Err(Error::InvalidEdit(
                "Region endpoints belong to an older chart clock".into(),
            ));
        }
        let mut ys = [0.0; 2];
        for (i, p) in settings.points.iter().enumerate() {
            ys[i] = projection
                .sample_y(p.row(), p.chart_time())
                .ok_or_else(|| Error::InvalidEdit("Region endpoint projection failed".into()))?;
        }
        Ok(ys)
    }
    pub(super) fn region_selection_overlay(
        &self,
        actors: &mut Vec<Actor>,
        geometry: Geometry,
        viewport: Viewport,
        plan: Option<&RenderPlan>,
    ) {
        let Some(settings) = &self.region_selection else {
            return;
        };
        let Some(completion) = plan.and_then(|p| p.admission.completion()) else {
            return;
        };
        let Some(start_y) = completion.start_y() else {
            return;
        };
        let ys = [start_y, completion.current_y()];
        if completion.used_fallback() {
            actors.push(act!(text:font("miso"):settext(i18n::tr("Edition","RegionPreviewApproximate")):align(0.0,0.0):xy(geometry.left.max(geometry.clip_left)+8.0,viewport.top+8.0):zoom(0.35):maxwidth((geometry.field_width-16.0).max(1.0)):diffuse(1.0,0.75,0.3,1.0):z(8006)));
        }
        if let Some(Err(error)) = completion.current() {
            actors.push(act!(text:font("miso"):settext(i18n::tr_fmt("Edition","Error",&[("message",&crate::i18n::editor_error(error))])):align(0.0,0.0):xy(geometry.left.max(geometry.clip_left)+8.0,viewport.top+24.0):zoom(0.35):maxwidth((geometry.field_width-16.0).max(1.0)):diffuse(1.0,0.5,0.3,1.0):z(8006)));
        }
        let w = f64::from(geometry.lane_width);
        let a = (f64::from(geometry.left) + settings.x[0] * w)
            .min(f64::from(geometry.left) + settings.x[1] * w);
        let b = (f64::from(geometry.left) + settings.x[0] * w)
            .max(f64::from(geometry.left) + settings.x[1] * w);
        let top = ys[0]
            .min(ys[1])
            .clamp(f64::from(viewport.top), f64::from(viewport.bottom));
        let bottom = ys[0]
            .max(ys[1])
            .clamp(f64::from(viewport.top), f64::from(viewport.bottom));
        if b > a && bottom > top {
            actors.push(act!(quad: xy(((a+b)*0.5)as f32,((top+bottom)*0.5)as f32): zoomto((b-a)as f32,(bottom-top)as f32): diffuse(0.3,0.7,1.0,0.2): z(8005)));
        }
    }
    pub(super) fn region_selection_actors(&self, actors: &mut Vec<Actor>) {
        let Some(s) = &self.region_selection else {
            return;
        };
        let width = screen_width();
        let x = (width - 330.0).max(5.0);
        let y = 105.0;
        actors.push(act!(quad: xy(x+160.0,y+150.0): zoomto(320.0,300.0): diffuse(0.045,0.065,0.095,0.95): z(8010)));
        let names = [
            "RegionStart",
            "RegionStartLane",
            "RegionEnd",
            "RegionEndLane",
            "RegionOverlap",
            "RegionInterval",
            "RegionMisc",
            "RegionAdd",
            "RegionToggle",
            "RegionApply",
        ];
        let flags = |v| i18n::tr("Edition", if v { "RegionYes" } else { "RegionNo" });
        let values = [
            format!(
                "{:.6}",
                if self.spacing_mode == SpacingMode::ConstantTime {
                    s.points[0].chart_time()
                } else {
                    s.points[0].row()
                }
            ),
            format!("{:.2}", s.x[0]),
            format!(
                "{:.6}",
                if self.spacing_mode == SpacingMode::ConstantTime {
                    s.points[1].chart_time()
                } else {
                    s.points[1].row()
                }
            ),
            format!("{:.2}", s.x[1]),
            i18n::tr(
                "Edition",
                ["RegionAny", "RegionCenter", "RegionAll"][s.preferences.overlap.index()],
            )
            .to_string(),
            i18n::tr(
                "Edition",
                if s.preferences.region.uses_lanes() {
                    "RegionTimeAndLanes"
                } else {
                    "RegionTimeOnly"
                },
            )
            .to_string(),
            flags(s.misc).to_string(),
            flags(s.add).to_string(),
            flags(s.toggle).to_string(),
            String::new(),
        ];
        for (i, key) in names.iter().enumerate() {
            let ui_row_start = actors.len();
            let value = format!(
                "{}{}: {}",
                if i == s.index { "> " } else { "  " },
                i18n::tr("Edition", key),
                values[i]
            );
            actors.push(act!(text: font("miso"): settext(value): align(0.0,0.0): xy(x+8.0,y+8.0+i as f32*26.0): zoom(0.42): maxwidth(305.0): diffuse(0.85,0.9,1.0,1.0): z(8011)));

            self.ui_row(actors, ui_row_start, i == s.index, 26.);
        }
    }
}
impl State {
    /// Source selection has its own layout frame and directional scans. The
    /// renderer shares Layout's mechanics, but not this region-dependent frame.
    fn source_region_misc(
        &self,
        chart: usize,
        geometry: Geometry,
        index: &Index,
        spacing: SpacingSettings,
        focal: deadsync_editor::spacing::Anchor,
        settings: &Settings,
    ) -> Result<(Vec<(Event, Rect)>, Option<Range>), Error> {
        use deadsync_editor::region_selection::misc_scan::{self, Bounds, Item};
        let session = &self.workspace.file().session;
        let inventory = self
            .misc_inventories
            .get(&chart)
            .filter(|i| i.is_current(session, chart))
            .ok_or_else(|| Error::InvalidEdit("Misc inventory is stale".into()))?;
        let entries = inventory
            .miscellaneous_indices()
            .iter()
            .map(|&i| inventory.entries()[i])
            .collect::<Vec<_>>();
        let items = entries
            .iter()
            .map(|e| Item {
                row: e.event.chart_position(),
                time: e.chart_time,
                phase: e.phase,
            })
            .collect::<Vec<_>>();
        let identities = entries
            .iter()
            .enumerate()
            .map(|(i, e)| (e.event, i))
            .collect::<std::collections::BTreeMap<_, _>>();
        let rate_items = index
            .sections()
            .iter()
            .map(|p| {
                let kind = match p.order {
                    0 => deadsync_editor::event_clipboard::TimingKind::Signature,
                    1 => deadsync_editor::event_clipboard::TimingKind::Tempo,
                    7 => deadsync_editor::event_clipboard::TimingKind::Delay,
                    13 => deadsync_editor::event_clipboard::TimingKind::Scroll,
                    15 | 16 => deadsync_editor::event_clipboard::TimingKind::Stop,
                    17 => deadsync_editor::event_clipboard::TimingKind::Warp,
                    _ => return None,
                };
                {
                    let row = p.mapping.row();
                    if row < 0.0 || row.fract() != 0.0 {
                        return None;
                    }
                    identities
                        .get(&Event::Timing {
                            row: row as u64,
                            kind,
                        })
                        .copied()
                }
            })
            .collect::<Vec<_>>();
        let times = Range::normalized(
            settings.points[0].chart_time(),
            settings.points[1].chart_time(),
        )?;
        let rows = Range::normalized(settings.points[0].row(), settings.points[1].row())?;
        let bounds = Bounds {
            start_time: times.min,
            end_time: times.max,
            start_row: rows.min,
            end_row: rows.max,
        };
        let metrics = self.misc_widget_dpi_metrics()?;
        let mut layout = Layout::new(
            f64::from(geometry.left) - 4.0,
            f64::from(geometry.right()) + 4.0,
            self.misc_event_area_width() as i32,
            metrics.padding,
        )
        .map_err(|_| Error::InvalidEdit("Invalid miscellaneous widget geometry".into()))?;
        let result = misc_scan::scan(
            index,
            focal,
            spacing,
            bounds,
            &items,
            &rate_items,
            |i, y| {
                let entry = entries[i];
                let key = if spacing.mode == SpacingMode::ConstantTime {
                    entry.chart_time
                } else {
                    entry.event.chart_position()
                };
                layout
                    .position(
                        i as u64,
                        entry.widget.expect("misc inventory"),
                        key,
                        y,
                        metrics.width,
                        metrics.height,
                    )
                    .map_err(|_| {
                        Error::InvalidEdit("Invalid miscellaneous widget geometry".into())
                    })?;
                layout
                    .maximum_single_row_height(metrics.height)
                    .map_err(|_| Error::InvalidEdit("Invalid miscellaneous widget geometry".into()))
            },
        )?;
        let range = match (result.start_y, result.end_y) {
            (Some(a), Some(b)) => Some(Range::normalized(a, b)?),
            _ => None,
        };
        let rectangles = if range.is_some() {
            result
                .order
                .into_iter()
                .filter_map(|i| layout.rectangle(i as u64).map(|r| (entries[i].event, r)))
                .collect()
        } else {
            Vec::new()
        };
        Ok((rectangles, range))
    }
    fn misc_budget(&self, chart: usize) -> deadsync_editor::options_preferences::DrawBudget {
        let visible = self.visible_view_geometry();
        // Original chart tree already contains held raw events and initial
        // deletions when proportional budgets are computed.
        let counts = |source: usize| {
            let context = if source == self.workspace.chart() {
                self.preview_source_chart().unwrap_or(source)
            } else {
                source
            };
            let Some(inventory) = self.misc_inventories.get(&context) else {
                return (0, 0);
            };
            let mut count = inventory.event_count();
            if source == self.workspace.chart() {
                if let Some(preview) = &self.preview {
                    let committed = self
                        .workspace
                        .file()
                        .session
                        .loaded_grid(context)
                        .map_or(0, |grid| grid.head_count());
                    count = count
                        .saturating_sub(committed)
                        .saturating_add(preview.grid().head_count());
                } else if let Some(input) = self.lane_entry_model() {
                    count = count
                        .saturating_sub(input.deleted_heads().count())
                        .saturating_add(input.drafts().count());
                }
            }
            (
                count.min(i32::MAX as usize) as i32,
                inventory.rate_count().min(i32::MAX as usize) as i32,
            )
        };
        let mut events = 0i32;
        let mut rates = 0i32;
        for geometry in &visible {
            let (own, rate) = counts(geometry.source);
            events = events.saturating_add(own);
            rates = rates.saturating_add(rate);
        }
        let (own_events, own_rates) = counts(chart);
        self.render_limits().per_chart(
            visible.len().min(i32::MAX as usize) as i32,
            own_events,
            events,
            own_rates,
            rates,
        )
    }
    pub(super) fn source_viewport_plan(
        &self,
        geometry: Geometry,
        viewport: Viewport,
    ) -> Result<RenderPlan, Error> {
        use deadsync_editor::region_selection::visible_events;
        let session = &self.workspace.file().session;
        let preview_position = if geometry.focused {
            self.preview_position()?
        } else {
            None
        };
        let chart = preview_position
            .as_ref()
            .map_or(geometry.source, |position| position.chart());
        let inventory = self
            .misc_inventories
            .get(&chart)
            .filter(|i| i.is_current(session, chart))
            .ok_or_else(|| Error::InvalidEdit("Misc inventory is stale".into()))?;
        let clock = inventory.clock();
        let index = inventory.spacing_index();
        let current = self.capture_song_property_presentation()?;
        let (time, row) = if let Some(position) = &preview_position {
            (position.chart_time(), position.row())
        } else if chart == self.workspace.chart() {
            (
                clock.music_to_chart_time(current.music_time())?,
                current.row(),
            )
        } else {
            (
                clock.music_to_chart_time(current.music_time())?,
                clock.position(current.music_time())?,
            )
        };
        let scroll = if self.spacing_mode == SpacingMode::Variable {
            deadsync_editor::scroll_interpolation::Timeline::capture(session, chart)?
                .rate_at(row, time)?
        } else {
            1.0
        };
        let spacing = SpacingSettings {
            mode: self.spacing_mode,
            reverse: self.reverse_display,
            speeds: self.current_spacing_speeds(),
            interpolated_scroll: scroll,
            zoom: self.zoom.spacing_zoom(),
        };
        let mut entries = std::borrow::Cow::Borrowed(inventory.entries());
        if self.song_properties.is_some() {
            let entries = entries.to_mut();
            let cache = self
                .misc_draft_widgets
                .as_ref()
                .filter(|c| c.binding.is_current(session))
                .ok_or_else(|| Error::InvalidEdit("Song widget preview is unavailable".into()))?;
            entries.retain(|e| !matches!(e.event, Event::SongWidget(_)));
            for &widget in cache.entries.get(&chart).ok_or_else(|| {
                Error::InvalidEdit("Song widget preview chart is unavailable".into())
            })? {
                entries.push(Entry {
                    event: Event::SongWidget(widget),
                    chart_time: widget.chart_time(),
                    phase: 0,
                    widget: Some(if widget.kind() == WidgetKind::Preview {
                        deadsync_editor::region_selection::misc_layout::Kind::Preview
                    } else {
                        deadsync_editor::region_selection::misc_layout::Kind::EndHint
                    }),
                });
            }
            entries.sort_by(|a, b| {
                a.event
                    .chart_position()
                    .total_cmp(&b.event.chart_position())
                    .then_with(|| {
                        let rank = |e: Entry| match e.event {
                            Event::SongWidget(w) => {
                                if w.kind() == WidgetKind::Preview {
                                    0
                                } else {
                                    1
                                }
                            }
                            _ => 2,
                        };
                        rank(*a).cmp(&rank(*b))
                    })
                    .then(a.phase.cmp(&b.phase))
            });
        }
        if geometry.focused
            && self.preview.is_some()
            && self.receptor_policy(geometry.source).is_none()
        {
            return Err(Error::InvalidEdit(
                "Generated preview classification is unavailable".into(),
            ));
        }
        // Admission and rendering must address the same immutable note tree.
        // Only the focused view displays a generated preview; neighboring open
        // views keep their committed notes and the source timing/misc inventory.
        let grid = if geometry.focused {
            self.preview.as_ref().map(super::GenerationPreview::grid)
        } else {
            None
        }
        .or_else(|| session.loaded_grid(chart))
        .ok_or_else(|| Error::InvalidEdit("View grid is unavailable".into()))?;
        let half = f64::from(geometry.lane_width) * self.zoom.size_zoom() * 0.5;
        let active_region = if geometry.focused && self.preview.is_none() {
            self.region_selection
                .as_ref()
                .map(|settings| -> Result<visible_events::Region, Error> {
                    let projection =
                        self.display_projection(viewport.top, viewport.bottom, viewport.cursor)?;
                    let ys = self.region_endpoint_y(settings, &projection)?;
                    Ok(visible_events::Region {
                        start_time: settings.points[0].chart_time(),
                        start_row: settings.points[0].row(),
                        current_y: ys[1],
                    })
                })
                .transpose()?
        } else {
            None
        };
        let admission = visible_events::prepare_with_input(
            grid,
            clock,
            index,
            &entries,
            deadsync_editor::spacing::Anchor {
                y: f64::from(viewport.cursor),
                time,
                row,
            },
            spacing,
            visible_events::Viewport {
                top: f64::from(viewport.top),
                bottom: f64::from(viewport.bottom),
                half_arrow: half,
                hold_cap: half,
            },
            self.source_event_budget(geometry.source),
            self.render_policy().misc_events,
            active_region,
            self.misc_budget(geometry.source).rate_events,
            if self.preview.is_none() && chart == self.workspace.chart() {
                self.lane_entry_model()
            } else {
                None
            },
        )?;
        let metrics = self.misc_widget_dpi_metrics()?;
        let mut layout = Layout::new(
            f64::from(geometry.left) - 4.0,
            f64::from(geometry.right()) + 4.0,
            self.misc_event_area_width() as i32,
            metrics.padding,
        )
        .map_err(|_| Error::InvalidEdit("Invalid miscellaneous widget geometry".into()))?;
        for &(i, y) in admission.misc_calls() {
            let e = entries[i];
            layout
                .position(
                    i as u64,
                    e.widget.expect("misc inventory"),
                    if self.spacing_mode == SpacingMode::ConstantTime {
                        e.chart_time
                    } else {
                        e.event.chart_position()
                    },
                    y,
                    metrics.width,
                    metrics.height,
                )
                .map_err(|_| Error::InvalidEdit("Invalid miscellaneous widget geometry".into()))?;
        }
        let mut rectangles = admission
            .misc_calls()
            .iter()
            .filter_map(|&(i, _)| layout.rectangle(i as u64).map(|r| (entries[i].event, r)))
            .collect::<Vec<_>>();
        separate_projected_widgets(
            &mut rectangles,
            metrics.padding,
            [
                f64::from(geometry.clip_left),
                f64::from(viewport.top),
                f64::from(geometry.clip_right),
                f64::from(viewport.bottom),
            ],
        );
        Ok(RenderPlan {
            source: chart,
            admission,
            rectangles,
            widget_metrics: metrics,
        })
    }
    pub(super) fn source_event_budget(&self, chart: usize) -> i32 {
        self.misc_budget(chart).events
    }
    pub(super) fn selection_marker_budget(&self, chart: usize) -> usize {
        self.misc_budget(chart).markers.max(0) as usize
    }
    #[cfg(test)]
    pub(super) fn misc_event_actors(
        &self,
        actors: &mut Vec<Actor>,
        _projection: &Projection<'_>,
        geometry: Geometry,
        viewport: Viewport,
    ) -> usize {
        let Ok(plan) = self.source_viewport_plan(geometry, viewport) else {
            return 0;
        };
        self.misc_event_actors_from_plan(actors, geometry, viewport, &plan)
    }
    pub(super) fn misc_event_actors_from_plan(
        &self,
        actors: &mut Vec<Actor>,
        geometry: Geometry,
        viewport: Viewport,
        plan: &RenderPlan,
    ) -> usize {
        if !self.render_policy().misc_events {
            return 0;
        }
        let rects = plan.rectangles.iter().copied();
        let metrics = plan.widget_metrics;
        let Some(inventory) = self.misc_inventories.get(&plan.source) else {
            return 0;
        };
        let mut events = 0usize;
        let partial = plan.admission.stopped_by_quota;
        for (event, rect) in rects {
            if rect.y + rect.height < f64::from(viewport.top)
                || rect.y > f64::from(viewport.bottom)
                || rect.x + rect.width < f64::from(geometry.clip_left)
                || rect.x > f64::from(geometry.clip_right)
            {
                continue;
            }
            events += 1;
            let x = (rect.x as f32).max(geometry.clip_left);
            let right = ((rect.x + rect.width) as f32).min(geometry.clip_right);
            let y = (rect.y as f32).max(viewport.top);
            let bottom = ((rect.y + rect.height) as f32).min(viewport.bottom);
            if x >= right || y >= bottom {
                continue;
            }
            let chosen = geometry.focused
                && self.preview.is_none()
                && self
                    .event_selection
                    .as_ref()
                    .is_some_and(|s| s.selection.selected_events().any(|e| e == event));
            let color = if chosen {
                [0.2, 0.6, 0.85, 0.85]
            } else {
                [0.12, 0.22, 0.32, 0.85]
            };
            let name = match event {
                Event::Timing { kind, .. } => {
                    i18n::tr("Edition", super::event_selection::timing_key(kind))
                }
                Event::Attack { .. } => i18n::tr("Edition", "EventSelectionAttack"),
                Event::Pattern { .. } => i18n::tr("Edition", "PatternMarker"),
                Event::SongWidget(w) => {
                    if w.kind() == WidgetKind::Preview {
                        i18n::tr("Edition", "SongSampleRegion")
                    } else {
                        i18n::tr_fmt(
                            "Edition",
                            "ChartEndHint",
                            &[(
                                "seconds",
                                &format!(
                                    "{:.3}",
                                    match inventory.clock().chart_time_to_music(w.chart_time()) {
                                        Ok(seconds) => seconds,
                                        Err(_) => continue,
                                    }
                                ),
                            )],
                        )
                    }
                }
                Event::Note(_) => continue,
            };
            let label = if let Some(value) = inventory.display_value(event) {
                format!("{name} {value}")
            } else {
                name.to_string()
            };
            actors.push(act!(quad: xy((x+right)*0.5,(y+bottom)*0.5): zoomto(right-x,bottom-y): diffuse(color[0],color[1],color[2],color[3]): z(8003)));
            actors.push(misc_widget_text(
                label,
                [rect.x as f32, rect.y as f32],
                [x, y, right - x, bottom - y],
                metrics,
            ));
        }
        if partial {
            actors.push(act!(text: font("miso"): settext(i18n::tr("Edition","RegionMiscPartial")): align(0.0,0.0): xy(geometry.left.max(geometry.clip_left),viewport.bottom-48.0): zoom(0.35): maxwidth(geometry.field_width): diffuse(1.0,0.75,0.3,1.0): z(8006)));
        }
        events.max(0) as usize
    }
}
// Miso's retained line box is 24 px. Match the source ImGui 15 px font
// through the existing DPI-to-canvas mapping, then clip without changing glyph
// size or moving the original widget origin when only part of it is visible.
fn misc_widget_text(
    label: String,
    origin: [f32; 2],
    visible: [f32; 4],
    metrics: deadsync_editor::options_preferences::dpi::WidgetMetrics,
) -> Actor {
    let mut actor = act!(text: font("miso"): settext(label): align(0.0,0.0):
        xy(origin[0]+metrics.inset_x as f32,origin[1]+metrics.inset_y as f32):
        zoom((15.0/24.0)*metrics.font_scale as f32):
        diffuse(0.9,0.95,1.0,1.0): z(8004));
    if let Actor::Text { clip, .. } = &mut actor {
        *clip = Some(visible);
    }
    actor
}
#[cfg(test)]
mod tests {
    #[test]
    fn folded_projection_rectangles_keep_individual_hit_targets_and_leave_other_columns() {
        use super::*;
        let event = |row| Event::Timing {
            row,
            kind: deadsync_editor::event_clipboard::TimingKind::Label,
        };
        let rect = |x| Rect {
            x,
            y: 211.,
            width: 80.,
            height: 18.,
        };
        let mut rectangles = vec![
            (event(48), rect(249.)),
            (event(192), rect(249.)),
            (event(240), rect(500.)),
        ];
        separate_projected_widgets(&mut rectangles, 2., [0., 100., 854., 395.]);
        assert_eq!(rectangles[0], (event(48), rect(249.)));
        assert_eq!(rectangles[1].0, event(192));
        assert_eq!(rectangles[1].1.y, 231.);
        assert_eq!(rectangles[2], (event(240), rect(500.)));
        for (event, rectangle) in &rectangles {
            let point = [
                rectangle.x + rectangle.width * 0.5,
                rectangle.y + rectangle.height * 0.5,
            ];
            let hits = rectangles
                .iter()
                .filter(|(_, candidate)| {
                    point[0] >= candidate.x
                        && point[0] <= candidate.x + candidate.width
                        && point[1] >= candidate.y
                        && point[1] <= candidate.y + candidate.height
                })
                .map(|(event, _)| *event)
                .collect::<Vec<_>>();
            assert_eq!(hits, [*event]);
        }
    }
    #[test]
    fn dense_folded_widgets_are_bounded_to_viewport_and_deterministic() {
        use super::*;
        let input = (0..10_000)
            .map(|row| {
                (
                    Event::Timing {
                        row,
                        kind: deadsync_editor::event_clipboard::TimingKind::Label,
                    },
                    Rect {
                        x: 249.,
                        y: 211.,
                        width: 80.,
                        height: 18.,
                    },
                )
            })
            .collect::<Vec<_>>();
        let mut first = input.clone();
        let mut second = input;
        for rectangles in [&mut first, &mut second] {
            separate_projected_widgets(rectangles, 2., [0., 105., 854., 395.]);
            assert_eq!(rectangles.len(), 10);
            assert!(rectangles.iter().all(|(_, r)| r.y >= 105. && r.y < 395.));
            assert!(
                rectangles
                    .windows(2)
                    .all(|pair| pair[0].1.y + pair[0].1.height + 2. <= pair[1].1.y)
            );
        }
        assert_eq!(first, second);
    }
    #[test]
    fn misc_widget_overflow_clips_without_resizing_or_reanchoring_glyphs() {
        let metrics = deadsync_editor::options_preferences::dpi::Capture::default()
            .native_widget_metrics(80.0, 20.0)
            .unwrap();
        for visible in [
            [120., 90., 30., 20.],
            [100., 100., 80., 4.],
            [120., 100., 30., 4.],
        ] {
            let actor = misc_widget_text(
                "BPM 120.000 very long label".into(),
                [100., 90.],
                visible,
                metrics,
            );
            let Actor::Text {
                offset,
                scale,
                clip,
                max_width,
                max_height,
                ..
            } = actor
            else {
                panic!("text");
            };
            assert_eq!(
                offset,
                [100. + metrics.inset_x as f32, 90. + metrics.inset_y as f32]
            );
            assert_eq!(scale, [15. / 24.; 2]);
            assert_eq!(scale[1] * 24., 15.);
            assert_eq!(clip, Some(visible));
            assert_eq!((max_width, max_height), (None, None));
        }
    }

    use super::*;
    use deadlib_platform::input::RawKeyboardEvent;
    use deadsync_editor::workspace::Command;
    use std::time::Instant;
    #[test]
    fn physical_f11_prepares_song_widgets_before_first_render_and_cancel_restores_cache_en_fr() {
        for language in ["en", "fr"] {
            let (_dir, mut state) = fixture(language);
            let bytes = state.workspace.file().session.serialize().unwrap();
            let revision = state.workspace.file().session.revision();
            let (geometry, viewport) = state.region_geometry().unwrap();
            let before = state.source_viewport_plan(geometry, viewport).unwrap();
            let notes = before.admission.notes().collect::<Vec<_>>();
            let rectangles = before.rectangles.clone();
            assert!(!notes.is_empty());
            assert!(!rectangles.is_empty());
            assert!(state.misc_draft_widgets.is_none());
            press(&mut state, KeyCode::F11);
            assert!(
                matches!(state.panel, Some(Panel::SongProperties(0))),
                "{}",
                state.status
            );
            assert!(
                state.misc_draft_widgets.is_some(),
                "opener must prepare before actors, without a later refresh tick"
            );
            let mut frame = Vec::new();
            state.actors(&mut frame);
            assert!(
                !frame
                    .iter()
                    .any(|actor| matches!(actor, Actor::Text { content, .. }
                if content.as_ref().contains("Song widget preview is unavailable")))
            );
            let plan = state.source_viewport_plan(geometry, viewport).unwrap();
            let admitted = plan.admission.notes().collect::<Vec<_>>();
            assert_eq!(admitted, notes);
            assert_eq!(plan.rectangles, rectangles);
            let projection = state
                .display_projection(viewport.top, viewport.bottom, viewport.cursor)
                .unwrap();
            let policy = state.receptor_policy(0);
            let mut note_actors = Vec::new();
            let stats = super::super::view_render::notes(
                &mut note_actors,
                state.workspace.grid(),
                &state.workspace.file().session.document().charts()[0].steps_type,
                &projection,
                geometry,
                viewport,
                &[(0, 192)],
                state.zoom.size_zoom() as f32,
                admitted.len(),
                policy.as_ref(),
                state.reverse_display,
                None,
                Some((&state.hold_colors, state.signatures.as_ref())),
                None,
                Some(&admitted),
                None,
            );
            assert_eq!(stats.candidates, admitted.len());
            assert!(!note_actors.is_empty());
            let mut widget_actors = Vec::new();
            state.misc_event_actors_from_plan(&mut widget_actors, geometry, viewport, &plan);
            assert!(!widget_actors.is_empty());
            drop(projection);
            assert_eq!(state.workspace.file().session.serialize().unwrap(), bytes);
            assert_eq!(state.workspace.file().session.revision(), revision);
            // Actual inline numeric acceptance refreshes the draft cache; the
            // inspector's Cancel must then remove it and restore resident widgets.
            state.panel = Some(Panel::SongSample(0));
            press(&mut state, KeyCode::Enter);
            assert!(state.text_input("0.3456789123"));
            press(&mut state, KeyCode::Enter);
            assert!(state.misc_draft_widgets.is_some());
            state.panel = Some(Panel::SongProperties(2));
            press(&mut state, KeyCode::Enter);
            assert!(state.song_properties.is_none());
            assert!(state.misc_draft_widgets.is_none());
            let restored = state.source_viewport_plan(geometry, viewport).unwrap();
            assert_eq!(restored.admission.notes().collect::<Vec<_>>(), notes);
            assert_eq!(restored.rectangles, rectangles);
            assert_eq!(state.workspace.file().session.serialize().unwrap(), bytes);
            assert_eq!(state.workspace.file().session.revision(), revision);
        }
    }

    fn fixture(lang: &str) -> (tempfile::TempDir, State) {
        crate::i18n::init(deadsync_assets::language::load_for_tests(lang));
        deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
            854.0, 480.0,
        ));
        let rows = (0..16)
            .map(|i| {
                let mut row = [b'0'; 4];
                if [0, 1, 2, 4, 8].contains(&i) {
                    row[i % 4] = b'1';
                }
                String::from_utf8(row.to_vec()).unwrap()
            })
            .collect::<Vec<_>>()
            .join("\n");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("region.ssc");
        std::fs::write(&path,format!("#VERSION:0.83;#BPMS:0=120;#SAMPLESTART:0.139;#SAMPLELENGTH:0;#LASTSECONDHINT:0.209;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:{rows};")).unwrap();
        let mut state = State::open(&path, 0).unwrap();
        state.configure_spacing_preference(
            deadsync_editor::scroll_preferences::spacing::Preference::from_effective(
                SpacingMode::ConstantRow,
            ),
        );
        state.command(Command::SetSnap(0));
        state.help = false;
        (dir, state)
    }
    fn key(state: &mut State, code: KeyCode, pressed: bool) {
        state.raw_key(&RawKeyboardEvent {
            code,
            pressed,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
    }
    fn press(state: &mut State, code: KeyCode) {
        key(state, code, true);
        key(state, code, false);
    }
    fn ctrl(state: &mut State, code: KeyCode) {
        key(state, KeyCode::SuperLeft, true);
        press(state, code);
        key(state, KeyCode::SuperLeft, false);
    }
    fn rows(state: &State) -> Vec<u64> {
        state
            .event_selection
            .as_ref()
            .unwrap()
            .selection
            .selected_events()
            .filter_map(|e| match e {
                Event::Note(p) => Some(p.row),
                _ => None,
            })
            .collect()
    }
    fn bytes(state: &State) -> Vec<u8> {
        state.workspace.file().session.serialize().unwrap()
    }
    #[test]
    fn keyboard_two_endpoints_apply_exact_notes_and_cancel_without_document_edits_en_fr() {
        for lang in ["en", "fr"] {
            let (_dir, mut state) = fixture(lang);
            let before = bytes(&state);
            state.open_event_selection();
            press(&mut state, KeyCode::F4);
            assert!(state.region_selection.is_some());
            press(&mut state, KeyCode::ArrowDown);
            press(&mut state, KeyCode::ArrowDown);
            press(&mut state, KeyCode::ArrowRight);
            assert_eq!(
                state.region_selection.as_ref().unwrap().points[1].row(),
                48.0
            );
            let mut actors = Vec::new();
            state.actors(&mut actors);
            assert!(
                actors
                    .iter()
                    .any(|a| matches!(a,Actor::Sprite{z:8005,tint,..}if *tint==[0.3,0.7,1.0,0.2]))
            );
            assert_ne!(
                i18n::tr("Edition", "RegionSelectionHelp").as_ref(),
                "RegionSelectionHelp"
            );
            press(&mut state, KeyCode::Enter);
            press(&mut state, KeyCode::Enter);
            assert!(state.region_selection.is_none());
            assert_eq!(rows(&state), vec![12, 24, 48]);
            assert_eq!(bytes(&state), before);
            assert_eq!(
                state
                    .event_selection
                    .as_ref()
                    .unwrap()
                    .selection
                    .last_selected_event()
                    .unwrap()
                    .row(),
                48
            );
            press(&mut state, KeyCode::F4);
            press(&mut state, KeyCode::ArrowRight);
            press(&mut state, KeyCode::Escape);
            assert!(state.region_selection.is_none());
            assert_eq!(rows(&state), vec![12, 24, 48]);
            assert_eq!(bytes(&state), before);
        }
    }
    #[test]
    fn keyboard_misc_selection_uses_source_scans_even_with_render_budget_zero_en_fr() {
        for lang in ["en", "fr"] {
            let (_dir, mut state) = fixture(lang);
            state.open_event_selection();
            state.open_region_selection();
            let session = &state.workspace.file().session;
            let points = [
                Position::at_fractional_row(session, 0, -200.0).unwrap(),
                Position::at_fractional_row(session, 0, 200.0).unwrap(),
            ];
            {
                let settings = state.region_selection.as_mut().unwrap();
                settings.points = points;
                settings.x = [-20.0, 20.0];
                settings.misc = true;
                settings.index = 9;
            }
            let (g, v) = state.region_geometry().unwrap();
            let p = state.display_projection(v.top, v.bottom, v.cursor).unwrap();
            let rectangles = state.source_viewport_plan(g, v).unwrap().rectangles;
            assert!(
                rectangles
                    .iter()
                    .any(|(e, _)| matches!(e,Event::SongWidget(w)if w.kind()==WidgetKind::Preview))
            );
            assert!(
                rectangles
                    .iter()
                    .any(|(e, _)| matches!(e,Event::SongWidget(w)if w.kind()==WidgetKind::EndHint))
            );
            let mut rendered = Vec::new();
            state.misc_event_actors(&mut rendered, &p, g, v);
            assert!(
                rendered
                    .iter()
                    .any(|a| matches!(a, Actor::Sprite { z: 8003, .. }))
            );
            drop(p);
            let before = bytes(&state);
            state.display_options.limits.max_events = 0;
            state.display_options.limits.max_rate_events = 0;
            press(&mut state, KeyCode::Enter);
            assert!(state.region_selection.is_none());
            let chosen: Vec<_> = state
                .event_selection
                .as_ref()
                .unwrap()
                .selection
                .selected_events()
                .collect();
            assert!(
                chosen
                    .iter()
                    .any(|e| matches!(e,Event::SongWidget(w)if w.kind()==WidgetKind::Preview))
            );
            assert!(
                chosen
                    .iter()
                    .any(|e| matches!(e,Event::SongWidget(w)if w.kind()==WidgetKind::EndHint))
            );
            assert!(chosen.iter().all(|e| !matches!(e, Event::Note(_))));
            assert_eq!(bytes(&state), before);
            let (g, v) = state.region_geometry().unwrap();
            let p = state.display_projection(v.top, v.bottom, v.cursor).unwrap();
            let mut rendered = Vec::new();
            state.misc_event_actors(&mut rendered, &p, g, v);
            let plan = state.source_viewport_plan(g, v).unwrap();
            assert_eq!(plan.admission.events().len(), 1);
            assert!(matches!(
                plan.admission.events()[0],
                Event::Timing {
                    kind: deadsync_editor::event_clipboard::TimingKind::Signature,
                    row: 0
                }
            ));
            assert_eq!(
                rendered
                    .iter()
                    .filter(|a| matches!(a, Actor::Sprite { z: 8003, .. }))
                    .count(),
                1
            );
            assert_eq!(state.misc_budget(0).events, 0);
        }
    }
    #[test]
    fn widget_paste_keeps_fractional_focus_target_length_and_one_reliable_undo_en_fr() {
        for lang in ["en", "fr"] {
            let (_dir, mut state) = fixture(lang);
            state.open_event_selection();
            state.open_region_selection();
            let mut choices = state.misc_inventories[&0].choices();
            let index = choices
                .events()
                .iter()
                .position(|e| matches!(e,Event::SongWidget(w)if w.kind()==WidgetKind::Preview))
                .unwrap();
            choices.set_selected(index, true).unwrap();
            state.replace_region_event_selection(choices);
            state.region_selection = None;
            ctrl(&mut state, KeyCode::KeyC);
            assert!(state.panel.is_none());
            state.navigate_exact_position(48.75, true);
            let before = bytes(&state);
            key(&mut state, KeyCode::ShiftLeft, true);
            ctrl(&mut state, KeyCode::KeyV);
            key(&mut state, KeyCode::ShiftLeft, false);
            let draft =
                deadsync_editor::song_properties::Draft::capture(&state.workspace.file().session)
                    .unwrap();
            assert_eq!(
                draft.sample_start(),
                Clock::capture(&state.workspace.file().session, 0)
                    .unwrap()
                    .music_time(49)
                    .unwrap()
            );
            assert_eq!(draft.sample_length(), 0.0);
            let focus = state.focus_position.as_ref().unwrap();
            assert!(focus.is_current(&state.workspace));
            assert_eq!(focus.row(), 48.75);
            ctrl(&mut state, KeyCode::KeyZ);
            assert_eq!(bytes(&state), before);
        }
    }
    #[test]
    fn selection_preferences_are_persistent_non_file_undo_and_modal_lane_ownership_cannot_commit_en_fr()
     {
        for lang in ["en", "fr"] {
            let (dir, mut state) = fixture(lang);
            let preferences = dir.path().join("prefs.json");
            std::fs::write(&preferences, "{\"Version\":1,\"Selection\":{\"Future\":7}}").unwrap();
            state.configure_audio_preferences(&preferences).unwrap();
            state.configure_spacing_preference(
                deadsync_editor::scroll_preferences::spacing::Preference::from_effective(
                    SpacingMode::ConstantRow,
                ),
            );
            let before = bytes(&state);
            state.open_event_selection();
            press(&mut state, KeyCode::F4);
            for _ in 0..4 {
                press(&mut state, KeyCode::ArrowDown);
            }
            press(&mut state, KeyCode::ArrowRight);
            let selected = state.region_selection.as_ref().unwrap().preferences;
            assert_ne!(selected, state.selection_preferences);
            press(&mut state, KeyCode::Enter);
            press(&mut state, KeyCode::Enter);
            assert_eq!(state.selection_preferences, selected);
            assert_eq!(bytes(&state), before);
            let file = deadsync_editor::audio_preferences::File::open(&preferences).unwrap();
            assert_eq!(file.selection(), selected);
            state.panel = None;
            state.command(Command::Undo);
            assert_eq!(state.selection_preferences, Preferences::default());
            assert_eq!(bytes(&state), before);
            state.command(Command::Redo);
            assert_eq!(state.selection_preferences, selected);
            let mut keys = deadsync_editor::key_binding_preferences::Preferences::default();
            keys.commands
                .insert("Arrow0".into(), vec![vec!["F13".into()]]);
            state.configure_key_binding_preferences(std::sync::Arc::new(keys));
            key(&mut state, KeyCode::F13, true);
            state.open_event_selection();
            state.open_region_selection();
            assert!(state.lane_entry_model().is_none());
            press(&mut state, KeyCode::ArrowRight);
            press(&mut state, KeyCode::Escape);
            key(&mut state, KeyCode::F13, false);
            assert_eq!(bytes(&state), before);
            assert!(state.lane_entry_model().is_none());
        }
    }
    #[test]
    fn staged_song_widgets_update_shared_geometry_without_publishing_document_en_fr() {
        for lang in ["en", "fr"] {
            let (_dir, mut state) = fixture(lang);
            let before = bytes(&state);
            let mut draft =
                deadsync_editor::song_properties::Draft::capture(&state.workspace.file().session)
                    .unwrap();
            draft.set_sample_start(0.3).unwrap();
            draft.set_sample_length(0.0).unwrap();
            draft.set_last_second_hint(0.4).unwrap();
            state.song_properties = Some(draft);
            state.refresh_misc_draft_widgets();
            let widgets = &state.misc_draft_widgets.as_ref().unwrap().entries[&0];
            assert_eq!(
                widgets
                    .iter()
                    .find(|w| w.kind() == WidgetKind::Preview)
                    .unwrap()
                    .chart_time(),
                Clock::capture(&state.workspace.file().session, 0)
                    .unwrap()
                    .music_to_chart_time(0.3)
                    .unwrap()
            );
            let (g, v) = state.region_geometry().unwrap();
            let projection = state.display_projection(v.top, v.bottom, v.cursor).unwrap();
            let rectangles = state.source_viewport_plan(g, v).unwrap().rectangles;
            assert_eq!(
                rectangles
                    .iter()
                    .filter(|(e, _)| matches!(e, Event::SongWidget(_)))
                    .count(),
                2
            );
            drop(projection);
            assert_eq!(bytes(&state), before);
            state.song_properties = None;
            state.refresh_misc_draft_widgets();
            assert!(state.misc_draft_widgets.is_none());
            assert_eq!(bytes(&state), before);
        }
    }

    #[test]
    fn source_copy_cut_bridge_native_row_ranges_without_overwriting_explicit_mixed_choices_en_fr() {
        for lang in ["en", "fr"] {
            let (_dir, mut state) = fixture(lang);
            let prefs = deadsync_editor::key_binding_preferences::source_profile::preset(
                &Default::default(),
                deadsync_editor::key_binding_preferences::source_profile::Platform::current(),
            )
            .unwrap()
            .preferences;
            state.configure_key_binding_preferences(std::sync::Arc::new(prefs));
            state.workspace.execute(Command::Seek(12)).unwrap();
            state.workspace.execute(Command::ToggleSelection).unwrap();
            state.workspace.execute(Command::Seek(24)).unwrap();
            assert_eq!(state.workspace.selection(), Some((12, 24)));
            ctrl(&mut state, KeyCode::KeyC);
            let selected = state
                .event_selection
                .as_ref()
                .unwrap()
                .selection
                .selected_events()
                .collect::<Vec<_>>();
            assert_eq!(selected.len(), 2);
            assert!(selected.iter().all(|e| matches!(e, Event::Note(_))));
            state.navigate_exact_position(144.0, true);
            ctrl(&mut state, KeyCode::KeyV);
            assert!(state.workspace.grid().iter().any(|(p, _)| p.row == 144));
            assert!(state.workspace.grid().iter().any(|(p, _)| p.row == 156));
            let mut mixed = state.workspace.misc_inventory(0).unwrap().choices();
            let i = mixed
                .events()
                .iter()
                .position(|e| matches!(e,Event::SongWidget(w)if w.kind()==WidgetKind::Preview))
                .unwrap();
            mixed.set_selected(i, true).unwrap();
            state.replace_region_event_selection(mixed);
            ctrl(&mut state, KeyCode::KeyC);
            assert_eq!(
                state
                    .event_selection
                    .as_ref()
                    .unwrap()
                    .selection
                    .selected_count(),
                1
            );
            assert!(
                state
                    .event_selection
                    .as_ref()
                    .unwrap()
                    .selection
                    .selected_events()
                    .all(|e| matches!(e, Event::SongWidget(_)))
            );
            state.event_selection = None;
            if state.workspace.selection().is_some() {
                state.workspace.execute(Command::ToggleSelection).unwrap();
            }
            let notes = state.workspace.grid().iter().count();
            state.navigate_exact_position(192.0, true);
            ctrl(&mut state, KeyCode::KeyC);
            assert!(state.event_selection.is_none());
            assert_eq!(state.status, i18n::tr("Edition", "EventSelectionNoChoice"));
            ctrl(&mut state, KeyCode::KeyV);
            assert_eq!(state.workspace.grid().iter().count(), notes);
            let draft =
                deadsync_editor::song_properties::Draft::capture(&state.workspace.file().session)
                    .unwrap();
            assert_eq!(
                draft.sample_start(),
                Clock::capture(&state.workspace.file().session, 0)
                    .unwrap()
                    .music_time(192)
                    .unwrap()
            );
            state.workspace.execute(Command::Seek(12)).unwrap();
            state.workspace.execute(Command::ToggleSelection).unwrap();
            state.workspace.execute(Command::Seek(24)).unwrap();
            let before = bytes(&state);
            ctrl(&mut state, KeyCode::KeyX);
            assert!(
                !state
                    .workspace
                    .grid()
                    .iter()
                    .any(|(p, _)| p.row == 12 || p.row == 24)
            );
            state.workspace.execute(Command::Undo).unwrap();
            assert_eq!(bytes(&state), before);
        }
    }

    #[test]
    fn source_quota_uses_chronological_combined_admission_and_real_note_renderer_en_fr() {
        for lang in ["en", "fr"] {
            let (_dir, mut state) = fixture(lang);
            state.display_options.limits.max_events = 1;
            let (g, v) = state.region_geometry().unwrap();
            let plan = state.source_viewport_plan(g, v).unwrap();
            let admitted = plan.admission.notes().collect::<Vec<_>>();
            assert_eq!(plan.admission.events().len(), 3);
            assert_eq!(admitted.len(), 1);
            assert_eq!(admitted[0].row, 0);
            let p = state.display_projection(v.top, v.bottom, v.cursor).unwrap();
            let grid = state.workspace.grid();
            let policy = state.receptor_policy(0);
            let mut actors = vec![];
            let stats = super::super::view_render::notes(
                &mut actors,
                grid,
                &state.workspace.file().session.document().charts()[0].steps_type,
                &p,
                g,
                v,
                &[(0, 192)],
                state.zoom.size_zoom() as f32,
                admitted.len(),
                policy.as_ref(),
                state.reverse_display,
                None,
                Some((&state.hold_colors, state.signatures.as_ref())),
                None,
                Some(&admitted),
                None,
            );
            assert_eq!(stats.candidates, 1);
            assert!(!actors.is_empty());
            drop(p);
            state.display_options.limits.max_events = 0;
            state.display_options.render.misc_events = false;
            let plan = state.source_viewport_plan(g, v).unwrap();
            assert_eq!(plan.admission.events().len(), 1);
            assert!(matches!(plan.admission.events()[0],Event::Note(p)if p.row==0));
        }
    }

    #[test]
    fn active_region_rate_budget_bounds_real_preview_work_without_replacing_keyboard_points_en_fr()
    {
        for lang in ["en", "fr"] {
            let (_dir, mut state) = fixture(lang);
            state.open_event_selection();
            state.open_region_selection();
            let session = &state.workspace.file().session;
            let points = [
                Position::at_fractional_row(session, 0, 96.0).unwrap(),
                Position::at_fractional_row(session, 0, 48.0).unwrap(),
            ];
            state.region_selection.as_mut().unwrap().points = points;
            let before = bytes(&state);
            state.display_options.limits.max_events = 0;
            state.display_options.limits.max_rate_events = 0;
            let (g, v) = state.region_geometry().unwrap();
            let low = state.source_viewport_plan(g, v).unwrap();
            let report = low.admission.completion().unwrap();
            assert!(report.used_fallback());
            assert_eq!(report.extra_steps(), 0);
            let low_processed = report.processed_rates();
            let mut actors = vec![];
            state.actors(&mut actors);
            let warning = i18n::tr("Edition", "RegionPreviewApproximate");
            assert!(
                actors.iter().any(
                    |a| matches!(a,Actor::Text{content,..}if content.as_str()==warning.as_ref())
                )
            );
            state.display_options.limits.max_rate_events = 100;
            let high = state.source_viewport_plan(g, v).unwrap();
            let report = high.admission.completion().unwrap();
            assert!(!report.used_fallback());
            assert!(report.extra_steps() > 0);
            assert!(report.processed_rates() > low_processed);
            let settings = state.region_selection.as_ref().unwrap();
            assert_eq!(
                (settings.points[0].row(), settings.points[1].row()),
                (96.0, 48.0)
            );
            assert!(
                settings
                    .points
                    .iter()
                    .all(|p| p.is_bound_to(&state.workspace.file().session, 0))
            );
            assert_eq!(bytes(&state), before);
        }
    }
    #[test]
    fn held_tree_admission_counts_raw_identities_and_cancellation_never_commits_en_fr() {
        for lang in ["en", "fr"] {
            let (_dir, mut state) = fixture(lang);
            let before = bytes(&state);
            let mut input = state.workspace.capture_lane_input().unwrap();
            state
                .workspace
                .press_lane_input(
                    &mut input,
                    deadsync_editor::lane_entry::Press {
                        lane: 0,
                        player: 0,
                        row: 24.0,
                        snap_rows: 0,
                        modified: false,
                        playing: false,
                        advance_by_snap: false,
                    },
                )
                .unwrap();
            input
                .move_to(&state.workspace.file().session, 0.0, 0, false)
                .unwrap();
            state.lane_input.model = Some(input);
            state.prepare_lane_draft_policy();
            state.display_options.render.misc_events = false;
            state.display_options.limits.max_events = 100;
            let (g, v) = state.region_geometry().unwrap();
            let plan = state.source_viewport_plan(g, v).unwrap();
            let same = plan
                .admission
                .heads()
                .iter()
                .filter(|h| h.position.row == 0 && h.position.lane == 0)
                .collect::<Vec<_>>();
            assert_eq!(same.len(), 2);
            assert!(!same[0].edited);
            assert!(same[1].edited);
            assert_eq!(same[1].note.end, Some(24));
            state.display_options.limits.max_events = 0;
            let low = state.source_viewport_plan(g, v).unwrap();
            assert!(low.admission.heads().iter().all(|h| !h.edited));
            let mut actors = vec![];
            state.actors(&mut actors);
            assert_eq!(bytes(&state), before);
            assert!(state.lane_entry_cancel());
            let clear = state.source_viewport_plan(g, v).unwrap();
            assert!(clear.admission.heads().iter().all(|h| !h.edited));
            assert_eq!(bytes(&state), before);
        }
    }

    #[test]
    fn misc_dpi_metrics_drive_real_layout_and_text_once_in_canvas_units_en_fr() {
        for lang in ["en", "fr"] {
            for (dpi, pixels) in [(1.0, 1u32), (2.0, 1), (2.0, 2)] {
                let (_dir, mut state) = fixture(lang);
                state.ui_dpi = Some(deadsync_editor::options_preferences::dpi::Capture::new(
                    &state.general_options,
                    dpi,
                ));
                state.ui_dpi_render_scale = dpi as f32;
                state.set_ui_dpi_viewport(854 * pixels, 480 * pixels);
                let before = bytes(&state);
                let metrics = state.misc_widget_dpi_metrics().unwrap();
                let (g, v) = state.region_geometry().unwrap();
                let plan = state.source_viewport_plan(g, v).unwrap();
                assert_eq!(plan.widget_metrics, metrics);
                assert!(!plan.rectangles.is_empty());
                assert!(
                    plan.rectangles
                        .iter()
                        .all(|(_, r)| (r.height - metrics.height).abs() < 1e-9)
                );
                let mut actors = vec![];
                assert!(state.misc_event_actors_from_plan(&mut actors, g, v, &plan) > 0);
                let scales = actors
                    .iter()
                    .filter_map(|a| match a {
                        Actor::Text { z: 8004, scale, .. } => Some(*scale),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assert!(!scales.is_empty());
                assert!(scales.iter().all(|s| {
                    (s[0] - (15.0 / 24.0) * metrics.font_scale as f32).abs() < 1e-6
                        && (s[1] - s[0]).abs() < 1e-6
                }));
                assert_eq!(bytes(&state), before);
            }
        }
    }
}

impl State {
    pub(super) fn copy_source_region_selection(&mut self, cut: bool) {
        // An explicit mixed selection always wins. Native Shift-navigation
        // creates a separate row range, which must also remain copyable under
        // source bindings and must feed the same mixed paste clipboard.
        if self.event_selection.is_none() {
            if self.workspace.selection().is_none() {
                self.status = i18n::tr("Edition", "EventSelectionNoChoice");
                return;
            }
            match self.workspace.note_range_event_selection() {
                Ok(selection) => self.replace_region_event_selection(selection),
                Err(error) => {
                    self.status = i18n::tr_fmt(
                        "Edition",
                        "Error",
                        &[("message", &crate::i18n::editor_error(&error))],
                    );
                    return;
                }
            }
        }
        let settings = self
            .event_selection
            .take()
            .expect("selection prepared above");
        let result = if cut {
            self.workspace.cut_event_selection(&settings.selection)
        } else {
            self.workspace.copy_event_selection(&settings.selection)
        };
        if settings
            .selection
            .is_current(&self.workspace.file().session)
        {
            self.event_selection = Some(settings);
        }
        match result {
            Ok(()) => {
                self.status = i18n::tr(
                    "Edition",
                    if cut {
                        "EventSelectionCut"
                    } else {
                        "EventSelectionCopied"
                    },
                )
            }
            Err(error) => {
                self.status = i18n::tr_fmt(
                    "Edition",
                    "Error",
                    &[("message", &crate::i18n::editor_error(&error))],
                )
            }
        }
        self.refresh();
    }
}

#[cfg(test)]
mod precise_misc_width_consumer_tests {
    use super::*;
    use deadlib_platform::input::RawKeyboardEvent;
    use deadsync_editor::workspace::Command;
    fn press(state: &mut State, code: KeyCode) {
        for pressed in [true, false] {
            state.raw_key(&RawKeyboardEvent {
                code,
                pressed,
                repeat: false,
                timestamp: std::time::Instant::now(),
                host_nanos: 0,
            });
        }
    }
    fn rectangles(state: &State) -> Vec<(Event, Rect)> {
        let (g, v) = state.region_geometry().unwrap();
        state.source_viewport_plan(g, v).unwrap().rectangles
    }
    #[test]
    fn precise_misc_width_changes_actual_shared_render_selection_rectangles_and_undo() {
        i18n::init_for_tests();
        deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
            854., 480.,
        ));
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("misc.ssc");
        std::fs::write(&path,"#VERSION:0.83;#BPMS:0=120;#TIMESIGNATURES:0=4=4;#LABELS:0=MARK;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100;").unwrap();
        let mut state = State::open(&path, 0).unwrap();
        state.refresh_misc_inventory();
        let before = rectangles(&state);
        assert!(!before.is_empty());
        let song = state.workspace.file().session.serialize().unwrap();
        state.open_display_options();
        state.options_field = 6;
        state.panel = Some(Panel::OptionsSettings(1));
        press(&mut state, KeyCode::Enter);
        assert!(state.wants_text_input());
        assert!(state.text_input("0"));
        press(&mut state, KeyCode::Enter);
        state.panel = Some(Panel::OptionsSettings(3));
        press(&mut state, KeyCode::Enter);
        let changed = rectangles(&state);
        assert_ne!(changed, before);
        assert_eq!(state.misc_event_area_width(), 0.);
        state.command(Command::Undo);
        assert_eq!(rectangles(&state), before);
        state.command(Command::Redo);
        assert_eq!(rectangles(&state), changed);
        assert_eq!(state.workspace.file().session.serialize().unwrap(), song);
    }
}

#[cfg(test)]
mod generated_preview_admission_tests {
    use super::super::{GenerationJob, GenerationPreview, view_render};
    use super::*;
    use deadsync_editor::generation::job::ChartOptions;
    use std::time::{Duration, Instant};
    fn fixture(language: &str) -> (tempfile::TempDir, State) {
        i18n::init(deadsync_assets::language::load_for_tests(language));
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("preview.ssc");
        let measure = "2000\n0000\n3000\n0001\n0L00\n00M0\nF000\n0100";
        let notes = std::iter::repeat_n(measure, 8)
            .collect::<Vec<_>>()
            .join(",\n");
        std::fs::write(&path,format!("#VERSION:0.83;#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0}};#BPMS:0=120;#FAKES:2=1;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Edit;#METER:9;#NOTES:{notes};#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Edit;#METER:5;#OFFSET:0.75;#BPMS:0=60;#TIMESIGNATURES:0=3=4;#LABELS:2=Source B;#NOTES:0001\n1000\n0100\n0010;")).unwrap();
        let mut state = State::open(&path, 0).unwrap();
        state.take_audio_commands();
        state.chart_views_apply(super::super::chart_views::ViewAction::Pin);
        assert!(state.focus_chart_at_time(1, 0.));
        state.chart_views_apply(super::super::chart_views::ViewAction::Pin);
        assert!(state.focus_chart_at_time(0, 0.));
        state.take_audio_commands();
        (root, state)
    }
    fn start_and_publish(state: &mut State) {
        start_and_publish_from(state, 0);
    }
    fn start_and_publish_from(state: &mut State, source: usize) {
        let task = state
            .workspace
            .generation_task_at_with_profiles(
                source,
                ChartOptions::balanced("dance-double"),
                None,
                Default::default(),
            )
            .unwrap();
        state.job = Some(GenerationJob::Chart(task.spawn().unwrap()));
        state.panel = Some(Panel::Generating);
        let deadline = Instant::now() + Duration::from_secs(20);
        while state.job.is_some() {
            state.update();
            assert!(Instant::now() < deadline, "{}", state.status);
            std::thread::yield_now();
        }
        assert!(
            matches!(state.preview, Some(GenerationPreview::Chart(_))),
            "{}",
            state.status
        );
        assert_eq!(state.preview.as_ref().unwrap().grid().columns(), 8);
    }
    fn check_render(state: &State) {
        let (g, v) = state.region_geometry().unwrap();
        let grid = state.preview.as_ref().unwrap().grid();
        let plan = state.source_viewport_plan(g, v).unwrap();
        let admitted = plan.admission.notes().collect::<Vec<_>>();
        assert!(!admitted.is_empty(), "actual preview viewport admits notes");
        assert!(plan.admission.heads().iter().all(|head| !head.edited));
        assert!(
            admitted
                .iter()
                .all(|&position| grid.get(position).is_some()),
            "same grid for admission/render"
        );
        let policy = state
            .receptor_policy(0)
            .expect("preview classification prepared at publication");
        for &position in &admitted {
            let presentation = policy
                .presentation(position)
                .expect("generated head absent from committed still classified");
            assert!(!presentation.hidden && !presentation.active_hold);
        }
        let projection = state.display_projection(v.top, v.bottom, v.cursor).unwrap();
        let mut actors = Vec::new();
        let stats = view_render::notes(
            &mut actors,
            grid,
            state.preview.as_ref().unwrap().layout(),
            &projection,
            g,
            v,
            &[(0, grid.last_row())],
            state.zoom.size_zoom() as f32,
            admitted.len(),
            Some(&policy),
            state.reverse_display,
            None,
            Some((&state.hold_colors, state.signatures.as_ref())),
            None,
            Some(&admitted),
            None,
        );
        assert_eq!(stats.candidates, admitted.len());
        assert!(
            !plan.admission.stopped_by_quota,
            "default event budget admits the viewport"
        );
        assert!(
            actors
                .iter()
                .any(|actor| matches!(actor, Actor::Sprite { z: 8004, .. })),
            "actual note sprites emitted"
        );
        let mut full = Vec::new();
        state.actors(&mut full);
        assert!(full.iter().any(|actor|matches!(actor,Actor::Sprite{z:8004,offset,..}if offset[0]>=g.left&&offset[0]<=g.right())),"full scene contains focused preview notes");
        for other in state
            .visible_view_geometry()
            .into_iter()
            .filter(|geometry| !geometry.focused)
        {
            let plan = state.source_viewport_plan(other, v).unwrap();
            let original = state
                .workspace
                .file()
                .session
                .loaded_grid(other.source)
                .unwrap();
            assert!(
                plan.admission
                    .notes()
                    .all(|position| original.get(position).is_some()),
                "inactive views retain committed admission"
            );
        }
    }
    #[test]
    fn real_generated_double_preview_renders_at_home_end_then_cancel_or_apply_without_source_mutation()
     {
        deadlib_present::space::with_metrics(
            deadlib_present::space::Metrics::centered(1100., 760.),
            || {
                for language in ["en", "fr"] {
                    for apply in [false, true] {
                        let (_root, mut state) = fixture(language);
                        let before = state.workspace.file().session.serialize().unwrap();
                        let count = state.workspace.file().session.document().charts().len();
                        start_and_publish(&mut state);
                        assert!(
                            state
                                .preview
                                .as_ref()
                                .unwrap()
                                .grid()
                                .iter()
                                .any(|(position, _)| state
                                    .workspace
                                    .grid()
                                    .get(position)
                                    .is_none())
                        );
                        for code in [KeyCode::Home, KeyCode::End] {
                            state.panel_key(code);
                            check_render(&state);
                            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
                        }
                        if apply {
                            state.panel_key(KeyCode::Enter);
                            assert!(state.preview.is_none(), "{}", state.status);
                            assert_eq!(
                                state.workspace.file().session.document().charts().len(),
                                count + 1
                            );
                            assert_eq!(state.workspace.grid().columns(), 8);
                            state.command(deadsync_editor::workspace::Command::Undo);
                            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
                        } else {
                            state.panel_key(KeyCode::Escape);
                            assert!(state.preview.is_none());
                            assert_eq!(state.status, i18n::tr("Edition", "GenerationDiscarded"));
                            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
                            assert_eq!(state.workspace.grid().columns(), 4);
                        }
                    }
                }
            },
        );
    }
    #[test]
    fn preview_quota_counts_preview_heads_instead_of_committed_heads_and_stays_bounded() {
        deadlib_present::space::with_metrics(
            deadlib_present::space::Metrics::centered(1100., 760.),
            || {
                let (_root, mut state) = fixture("en");
                start_and_publish(&mut state);
                state.display_options.limits.max_events = 12;
                let visible = state.visible_view_geometry();
                let chart = state.workspace.chart();
                let mut total = 0i32;
                let mut own = 0i32;
                let mut total_rates = 0i32;
                let mut own_rates = 0i32;
                for geometry in &visible {
                    let inventory = &state.misc_inventories[&geometry.source];
                    let source_grid = state
                        .workspace
                        .file()
                        .session
                        .loaded_grid(geometry.source)
                        .unwrap();
                    let count = if geometry.focused {
                        inventory.event_count() - source_grid.head_count()
                            + state.preview.as_ref().unwrap().grid().head_count()
                    } else {
                        inventory.event_count()
                    };
                    let count = count.min(i32::MAX as usize) as i32;
                    let rates = inventory.rate_count() as i32;
                    total += count;
                    total_rates += rates;
                    if geometry.source == chart {
                        own = count;
                        own_rates = rates;
                    }
                }
                assert_eq!(
                    state.source_event_budget(chart),
                    state
                        .render_limits()
                        .per_chart(visible.len() as i32, own, total, own_rates, total_rates)
                        .events
                );
                let (g, v) = state.region_geometry().unwrap();
                let plan = state.source_viewport_plan(g, v).unwrap();
                assert!(plan.admission.heads().iter().all(|head| !head.edited));
                assert!(plan.admission.heads().len() <= 12);
                state.display_options.limits.max_events = 0;
                let plan = state.source_viewport_plan(g, v).unwrap();
                assert!(plan.admission.heads().len() <= 1);
                assert!(plan.admission.stopped_by_quota);
            },
        );
    }
    #[test]
    fn source_b_preview_uses_b_clock_metadata_and_apply_focus_preserves_real_music_instant() {
        deadlib_present::space::with_metrics(
            deadlib_present::space::Metrics::centered(1100., 760.),
            || {
                for playing in [false, true] {
                    for apply in [false, true] {
                        let (_root, mut state) = fixture("en");
                        state.navigate_exact_position(96.5, true);
                        let owner = state.editor_asset_owner();
                        if playing {
                            let mut snapshot = deadsync_gameplay::GameplayAudioSnapshot::default();
                            snapshot.editor.owner = owner;
                            snapshot.editor.output.available = true;
                            snapshot.editor.output.leased = true;
                            snapshot.editor.output.period_control =
                                deadsync_gameplay::EditorBufferControl::Adjustable;
                            snapshot.editor.output.requested_period = Some(512);
                            snapshot.editor.output.negotiated_period = Some(512);
                            state.update_audio(snapshot);
                            state.take_audio_commands();
                            state.toggle_audio();
                            assert!(
                                state.chart_transport_is_playing(),
                                "no MUSIC still creates a main logical transport"
                            );
                            state.take_audio_commands();
                            state.bind_decoder_generation(Some(7));
                            state.bind_decoder_asset_epoch(Some(owner), Some(13));
                        }
                        let before = state.workspace.file().session.serialize().unwrap();
                        let original_row = state.workspace.row();
                        let original_time = state.chart_focus_time().unwrap();
                        start_and_publish_from(&mut state, 1);
                        assert_eq!(state.workspace.active_chart(), Some(0));
                        assert_eq!(state.workspace.row(), original_row);
                        assert_eq!(
                            state.chart_focus_time().unwrap().to_bits(),
                            original_time.to_bits()
                        );
                        let position = state.preview_position().unwrap().unwrap();
                        assert_eq!(position.chart(), 1);
                        if playing {
                            assert_eq!(position.music_time().to_bits(), original_time.to_bits());
                        } else {
                            assert_eq!(position.row().to_bits(), 96.5f64.to_bits());
                        }
                        let clock = Clock::capture(&state.workspace.file().session, 1).unwrap();
                        let (g, v) = state.region_geometry().unwrap();
                        let plan = state.source_viewport_plan(g, v).unwrap();
                        assert_eq!(plan.source, 1);
                        state.configure_spacing_preference(
                            deadsync_editor::scroll_preferences::spacing::Preference::ConstantTime,
                        );
                        let expected = f64::from(v.cursor)
                            + (clock.note_time(144).unwrap() - position.chart_time())
                                * state.current_spacing_speeds().time
                                * state.zoom.spacing_zoom();
                        let projection =
                            state.display_projection(v.top, v.bottom, v.cursor).unwrap();
                        assert!((projection.note_y(144).unwrap() - expected).abs() < 1e-9);
                        assert_eq!(
                            state
                                .view_inputs
                                .get(&state.workspace.file().session, 1)
                                .unwrap()
                                .signatures
                                .as_ref()
                                .unwrap()
                                .at(96)
                                .numerator,
                            3
                        );
                        assert!(
                            state
                                .receptor_policy(0)
                                .unwrap()
                                .presentation(plan.admission.notes().next().unwrap())
                                .is_some()
                        );
                        assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
                        drop(projection);
                        // Real background density task consumes the same immutable grid
                        // and source B metadata, not the committed focused chart A.
                        let mut density = state.density_preferences();
                        density.visible = true;
                        state.configure_density_preferences(density).unwrap();
                        let deadline = Instant::now() + Duration::from_secs(20);
                        while state.is_density_loading() {
                            state.update();
                            assert!(Instant::now() < deadline);
                            std::thread::yield_now();
                        }
                        let expected_density =
                            deadsync_editor::density::task::Task::capture_preview_grid(
                                &state.workspace.file().session,
                                1,
                                state.source_density.stream,
                                state.preview.as_ref().unwrap().grid(),
                            )
                            .unwrap()
                            .run(&std::sync::atomic::AtomicBool::new(false))
                            .unwrap();
                        let committed_a = deadsync_editor::density::task::Task::capture(
                            &state.workspace.file().session,
                            0,
                            state.source_density.stream,
                        )
                        .unwrap()
                        .run(&std::sync::atomic::AtomicBool::new(false))
                        .unwrap();
                        assert_ne!(
                            expected_density.series.measures, committed_a.series.measures,
                            "fixture distinguishes source B preview from focused A"
                        );
                        let actual_density = state
                            .source_density_report()
                            .expect("published generated-grid report");
                        assert_eq!(
                            actual_density.series.measures,
                            expected_density.series.measures
                        );
                        assert_eq!(
                            actual_density.series.final_time.to_bits(),
                            expected_density.series.final_time.to_bits()
                        );
                        // Actual minimap actor path is available behind the review panel;
                        // cached content is rebound when the immutable preview changes.
                        let mut mini = Vec::new();
                        state.minimap_actors(&mut mini);
                        let progress = state
                            .minimap_content_progress()
                            .expect("generated minimap frame");
                        assert!(
                            progress.0 > 0
                                && progress.0
                                    <= state.preview.as_ref().unwrap().grid().head_count()
                        );
                        assert!(!mini.is_empty());
                        let mut second = Vec::new();
                        state.minimap_actors(&mut second);
                        assert_eq!(state.minimap_content_progress(), Some(progress));
                        if apply {
                            state.panel_key(KeyCode::Enter);
                            assert!(state.preview.is_none());
                            assert_eq!(state.workspace.active_chart(), Some(2));
                            let focused = state.focus_position.as_ref().unwrap();
                            assert!(focused.is_current(&state.workspace));
                            assert!((focused.music_time() - original_time).abs() < 1e-9);
                            let expected = deadsync_editor::chart_focus::Position::at_time(
                                &state.workspace.file().session,
                                2,
                                original_time,
                            )
                            .unwrap();
                            assert_eq!(focused.row().to_bits(), expected.row().to_bits());
                            if playing {
                                assert!(state.chart_transport_is_playing());
                                assert_eq!(state.chart_audio_generation(), Some(7));
                                assert_eq!(state.playback_asset_epoch(), Some(13));
                            }
                        } else {
                            state.panel_key(KeyCode::Escape);
                            assert!(state.preview_source_chart().is_none());
                            assert_eq!(state.workspace.active_chart(), Some(0));
                            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
                            assert_eq!(
                                state.chart_focus_time().unwrap().to_bits(),
                                original_time.to_bits()
                            );
                        }
                        assert!(!state.take_audio_commands().iter().any(|command|matches!(command,deadsync_gameplay::GameplayAudioCommand::EditorStopMusic{..}|deadsync_gameplay::GameplayAudioCommand::EditorStartMusic{..}|deadsync_gameplay::GameplayAudioCommand::EditorSwapMusicAsset{..}|deadsync_gameplay::GameplayAudioCommand::EditorClearMusicAsset{..})),"same/no asset keeps transport ownership");
                    }
                }
            },
        );
    }
}
#[cfg(test)]
mod held_query_consumer_tests {
    use super::*;
    use deadsync_editor::key_binding_preferences::{
        Preferences as Keys,
        scroll_modifiers::{Kind, Query},
    };
    fn key(s: &mut State, code: KeyCode, pressed: bool) {
        s.raw_key(&deadlib_platform::input::RawKeyboardEvent {
            code,
            pressed,
            repeat: false,
            timestamp: std::time::Instant::now(),
            host_nanos: 0,
        });
    }
    fn fixture() -> (tempfile::TempDir, State) {
        crate::i18n::init_for_tests();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("select.ssc");
        std::fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#LABELS:1=label;#NOTES:1000\n0100\n0010\n0000;").unwrap();
        let mut s = State::open(&path, 0).unwrap();
        let mut prefs = Keys::default();
        for (kind, name) in [
            (Kind::SelectionToggle, "F13"),
            (Kind::SelectionExtend, "F14"),
            (Kind::SelectionMisc, "F15"),
        ] {
            prefs
                .scroll_modifiers
                .set(kind, Query::from_alternatives(vec![vec![name.into()]]));
        }
        s.configure_key_binding_preferences(std::sync::Arc::new(prefs));
        s.configure_spacing_preference(
            deadsync_editor::scroll_preferences::spacing::Preference::from_effective(
                SpacingMode::ConstantRow,
            ),
        );
        s.open_region_selection();
        (dir, s)
    }
    fn apply(s: &mut State, start: f64, end: f64, misc: bool) {
        let mut settings = s.region_selection.take().unwrap();
        settings.points = [
            Position::at_fractional_row(&s.workspace.file().session, 0, start).unwrap(),
            Position::at_fractional_row(&s.workspace.file().session, 0, end).unwrap(),
        ];
        if misc {
            settings.x = [-64., 128.];
        }
        s.apply_region_selection(&settings).unwrap();
        s.region_selection = Some(settings);
    }
    fn notes(s: &State) -> usize {
        s.event_selection
            .as_ref()
            .unwrap()
            .selection
            .selected_events()
            .filter(|event| matches!(event, Event::Note(_)))
            .count()
    }
    #[test]
    fn remapped_toggle_and_extend_queries_change_actual_region_membership_not_fixed_ctrl_shift() {
        for (toggle, physical, expected) in [
            (true, KeyCode::F13, 2),
            (true, KeyCode::ControlLeft, 1),
            (false, KeyCode::F14, 3),
            (false, KeyCode::ShiftLeft, 1),
        ] {
            let (_dir, mut s) = fixture();
            let bytes = s.workspace.file().session.serialize().unwrap();
            apply(&mut s, -0.25, 0., false);
            assert_eq!(notes(&s), 1);
            key(&mut s, physical, true);
            apply(
                &mut s,
                if toggle { 47.75 } else { 95.75 },
                if toggle { 48. } else { 96. },
                false,
            );
            assert_eq!(notes(&s), expected);
            key(&mut s, physical, false);
            assert_eq!(s.workspace.file().session.serialize().unwrap(), bytes);
        }
    }
    #[test]
    fn remapped_misc_query_selects_real_misc_rectangles_and_releases_cleanly() {
        let (_dir, mut s) = fixture();
        key(&mut s, KeyCode::AltRight, true);
        apply(&mut s, -0.25, 144., true);
        assert_eq!(notes(&s), 3);
        assert!(
            !s.event_selection
                .as_ref()
                .unwrap()
                .selection
                .selected_events()
                .any(|event| matches!(event, Event::Timing { .. }))
        );
        key(&mut s, KeyCode::AltRight, false);
        key(&mut s, KeyCode::F15, true);
        apply(&mut s, -0.25, 144., true);
        assert_eq!(notes(&s), 0);
        assert!(
            s.event_selection
                .as_ref()
                .unwrap()
                .selection
                .selected_events()
                .any(|event| matches!(
                    event,
                    Event::Timing {
                        kind: deadsync_editor::event_clipboard::TimingKind::Label,
                        ..
                    }
                ))
        );
        key(&mut s, KeyCode::F15, false);
        assert!(!s.owned_gesture_query(Kind::SelectionMisc));
    }
    #[test]
    fn held_query_aliases_modal_ownership_focus_release_and_callback_quarantine_are_distinct() {
        let (_dir, mut s) = fixture();
        let mut prefs = (*s.command_bindings.preferences).clone();
        prefs.scroll_modifiers.set(
            Kind::SelectionToggle,
            Query::from_alternatives(vec![vec!["RightAlt".into()]]),
        );
        s.configure_key_binding_preferences(std::sync::Arc::new(prefs));
        s.panel = Some(Panel::EventSelection);
        key(&mut s, KeyCode::AltLeft, true);
        assert!(s.owned_gesture_query(Kind::SelectionToggle));
        key(&mut s, KeyCode::AltRight, true);
        key(&mut s, KeyCode::AltLeft, false);
        assert!(s.owned_gesture_query(Kind::SelectionToggle));
        s.release_input();
        assert!(!s.owned_gesture_query(Kind::SelectionToggle));
        key(&mut s, KeyCode::AltRight, false);
        key(&mut s, KeyCode::AltRight, true);
        assert!(s.owned_gesture_query(Kind::SelectionToggle));
        s.panel = Some(Panel::KeyBindingSettings(0));
        assert!(!s.owned_gesture_query(Kind::SelectionToggle));
        key(&mut s, KeyCode::AltRight, false);
    }
}

#[cfg(test)]
mod held_click_query_tests {
    use super::*;
    use deadsync_editor::key_binding_preferences::{
        Preferences as Keys,
        scroll_modifiers::{Kind, Query},
    };
    fn key(s: &mut State, code: KeyCode, pressed: bool) {
        s.raw_key(&deadlib_platform::input::RawKeyboardEvent {
            code,
            pressed,
            repeat: false,
            timestamp: std::time::Instant::now(),
            host_nanos: 0,
        });
    }
    fn press(s: &mut State, code: KeyCode) {
        key(s, code, true);
        key(s, code, false);
    }
    #[test]
    fn actual_event_list_click_extension_uses_remapped_query_and_global_click_lane_policy() {
        for lanes in [false, true] {
            crate::i18n::init_for_tests();
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("click.ssc");
            std::fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0001\n0100\n0000;").unwrap();
            let mut s = State::open(&path, 0).unwrap();
            let mut prefs = Keys::default();
            prefs.scroll_modifiers.set(
                Kind::SelectionExtend,
                Query::from_alternatives(vec![vec!["F14".into()]]),
            );
            s.configure_key_binding_preferences(std::sync::Arc::new(prefs));
            if lanes {
                s.open_selection_settings();
                s.panel = Some(Panel::SelectionSettings(1));
                press(&mut s, KeyCode::ArrowRight);
                s.panel = Some(Panel::SelectionSettings(3));
                press(&mut s, KeyCode::Enter);
                assert_eq!(
                    s.selection_preferences.region,
                    RegionMode::TimeOrPositionAndLane
                );
            }
            // Opening the list normally captures only the current row. Supply
            // the real three-row inventory, initially unchecked, so the test
            // can exercise actual list clicks across distinct rows.
            s.open_event_selection();
            let mut choices = s.workspace.event_selection_between(0, 96).unwrap();
            choices.select_all(false);
            s.replace_region_event_selection(choices);
            let indices = s
                .event_selection
                .as_ref()
                .unwrap()
                .selection
                .events()
                .iter()
                .enumerate()
                .filter_map(|(i, event)| matches!(event, Event::Note(_)).then_some(i))
                .collect::<Vec<_>>();
            assert_eq!(indices.len(), 3);
            press(&mut s, KeyCode::Home);
            for _ in 0..indices[0] {
                press(&mut s, KeyCode::ArrowDown);
            }
            press(&mut s, KeyCode::Space);
            for _ in indices[0]..indices[2] {
                press(&mut s, KeyCode::ArrowDown);
            }
            key(&mut s, KeyCode::F14, true);
            press(&mut s, KeyCode::Space);
            key(&mut s, KeyCode::F14, false);
            let notes = s
                .event_selection
                .as_ref()
                .unwrap()
                .selection
                .selected_events()
                .filter(|event| matches!(event, Event::Note(_)))
                .count();
            assert_eq!(notes, if lanes { 2 } else { 3 });
        }
    }
}
