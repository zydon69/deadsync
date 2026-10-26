//! Native minimap content integration; hooks remain in the review patch.
use super::*;
use super::{
    minimap_content_cache::{self, Frame},
    minimap_content_render::Mesh,
    minimap_content_scene::{self, Limits, Overlays, Point},
};
use deadsync_editor::{
    chart_order::Binding,
    generation::metadata::ChartData,
    minimap_background,
    minimap_content_preferences::Preferences as ContentPreferences,
    minimap_event_policy::Pattern,
    minimap_palette::{ColorMethod, Family, Palette},
    minimap_viewport::Geometry,
    note_color_preferences::Preferences as NotePreferences,
    playback::Clock,
    song_properties::Draft,
    timing_edit::{label::Labels, signature::Signatures},
};
use std::cell::RefCell;
struct Metadata {
    signatures: Signatures,
    patterns: Vec<Pattern>,
    preview: (Point, Point),
    labels: Vec<Point>,
}
struct MetadataEntry {
    binding: Binding,
    data: Result<Metadata, Arc<str>>,
}
pub(super) struct Content {
    pub(super) options: ContentPreferences,
    palette: Palette,
    revision: u64,
    metadata: RefCell<Option<MetadataEntry>>,
    frames: minimap_content_cache::Cache,
    error: RefCell<Option<Arc<str>>>,
}
impl Default for Content {
    fn default() -> Self {
        Self {
            options: ContentPreferences::default(),
            palette: Palette::new(&NotePreferences::default().colors)
                .expect("valid source default palette"),
            revision: 0,
            metadata: RefCell::new(None),
            frames: Default::default(),
            error: RefCell::new(None),
        }
    }
}
impl Metadata {
    fn capture(state: &State, clock: &Clock) -> Result<Self, deadsync_editor::Error> {
        let session = &state.workspace.file().session;
        let chart = state.workspace.chart();
        let document = session.document();
        let signatures = Signatures::from_document(document, chart)?;
        let patterns = ChartData::read(document, chart)?
            .and_then(|metadata| metadata.patterns)
            .unwrap_or_default()
            .into_iter()
            .map(|(start, definition)| {
                deadsync_editor::minimap_pattern_region::capture(clock, start, definition.length)
            })
            .collect::<Result<_, deadsync_editor::Error>>()?;
        let labels = Labels::from_document(document, chart)?
            .values()
            .keys()
            .map(|&row| {
                Ok(Point {
                    row: row as f64,
                    chart_time: clock.chart_time(row)?,
                })
            })
            .collect::<Result<_, deadsync_editor::Error>>()?;
        let song = Draft::capture(session)?;
        let start = song.sample_start();
        let end = song.sample_end()?;
        let preview = (
            Point {
                row: clock.position(start)?,
                chart_time: clock.music_to_chart_time(start)?,
            },
            Point {
                row: clock.position(end)?,
                chart_time: clock.music_to_chart_time(end)?,
            },
        );
        Ok(Self {
            signatures,
            patterns,
            preview,
            labels,
        })
    }
}
impl State {
    pub fn note_color_preferences(&self) -> &NotePreferences {
        &self.note_colors
    }
    pub(super) fn configure_note_color_preferences(
        &mut self,
        preferences: NotePreferences,
    ) -> Result<(), deadsync_editor::Error> {
        preferences.validate()?;
        let palette = Palette::new(&preferences.colors)?;
        self.note_colors = Arc::new(preferences);
        self.minimap.content.palette = palette;
        self.minimap.content.revision = self.minimap.content.revision.wrapping_add(1);
        self.minimap.content.frames.clear();
        Ok(())
    }
    pub(super) fn configure_minimap_content_preferences(
        &mut self,
        preferences: ContentPreferences,
    ) {
        self.minimap.content.options = preferences;
        self.minimap.content.revision = self.minimap.content.revision.wrapping_add(1);
        self.minimap.content.frames.clear();
        if !preferences.visible {
            self.minimap_release_input();
        }
    }
    /// Only certify a retained frame matching all current inputs and visibility.
    pub fn minimap_content_progress(&self) -> Option<(usize, bool, bool, bool)> {
        if self.panel.is_some()
            || !self.workspace.has_chart()
            || !self.minimap.content.options.visible
        {
            return None;
        }
        let geometry = self.minimap_viewport_geometry().ok()?;
        if (geometry.projection.editor_end - geometry.projection.editor_start)
            / geometry.projection.mini_range
            >= 1.0
        {
            return None;
        }
        let key = self.minimap_content_key(&geometry);
        let frame = self
            .minimap
            .content
            .frames
            .current(&self.workspace.file().session, self.workspace.chart(), &key)?
            .ok()?;
        Some((
            frame.note_count,
            frame.notes_complete,
            frame.markers_complete,
            frame.mesh.complete,
        ))
    }
    fn minimap_content_key(&self, geometry: &Geometry) -> minimap_content_cache::Key {
        let projection = &geometry.projection;
        minimap_content_cache::Key {
            axis: geometry.axis,
            projection: minimap_content_cache::Key::projection_values(projection),
            width: self.minimap.content.options.width,
            height: projection.visible_height as u32,
            focused: self.minimap_focused(),
            captured: self.minimap_capture_active(),
            selection: self.workspace.selection(),
            player: self.workspace.player(),
            palette_revision: self.minimap.content.revision,
            options_revision: self.minimap.content.revision,
        }
    }
    fn minimap_content_error(&self, actors: &mut Vec<Actor>, detail: Arc<str>, x: f32) {
        let mut previous = self.minimap.content.error.borrow_mut();
        if previous.as_ref() != Some(&detail) {
            log::warn!("Editor minimap: {detail}");
            *previous = Some(detail);
        }
        actors.push(act!(text: font("miso"): settext(i18n::tr("Edition","MinimapFailed")): xy(x,110.0): zoom(0.45): maxwidth(95.0): z(8013)));
    }
    pub(super) fn minimap_content_actors(
        &self,
        actors: &mut Vec<Actor>,
        geometry: &Geometry,
        clock: &Clock,
    ) {
        let options = self.minimap.content.options;
        if !options.visible {
            return;
        }
        let projection = &geometry.projection;
        let fraction = (projection.editor_end - projection.editor_start) / projection.mini_range;
        if fraction >= 1.0 {
            return;
        }
        let width = options.width;
        let x = (screen_width() - width as f32 - 8.0).max(0.0);
        let session = &self.workspace.file().session;
        let chart = self.workspace.chart();
        let mut metadata = self.minimap.content.metadata.borrow_mut();
        if metadata
            .as_ref()
            .is_none_or(|entry| !entry.binding.is_current(session, chart))
        {
            let binding = match Binding::capture(session, chart) {
                Ok(binding) => binding,
                Err(error) => {
                    self.minimap_content_error(
                        actors,
                        error.to_string().into(),
                        x + width as f32 * 0.5,
                    );
                    return;
                }
            };
            *metadata = Some(MetadataEntry {
                binding,
                data: Metadata::capture(self, clock).map_err(|error| error.to_string().into()),
            });
        }
        let data = match &metadata.as_ref().unwrap().data {
            Ok(data) => data,
            Err(detail) => {
                self.minimap_content_error(actors, Arc::clone(detail), x + width as f32 * 0.5);
                return;
            }
        };
        let selection = self.workspace.selection();
        let key = self.minimap_content_key(geometry);
        let frame = self.minimap.content.frames.frame(session, chart, key, || {
            let range = projection.editor_end - projection.editor_start;
            let background = minimap_background::build(
                projection,
                width,
                (projection.full_start + range, projection.full_end - range),
                geometry.focused_coordinate(),
                self.minimap_capture_active(),
                self.minimap_focused(),
            )?;
            let layout = &session.document().charts()[chart].steps_type;
            let routine = matches!(
                layout.as_str(),
                "dance-routine" | "smx-team" | "pump-routine"
            );
            let family = if layout.starts_with("pump-") && !self.note_colors.piu_row_based {
                Family::PumpLane {
                    start_index: if layout == "pump-halfdouble" { 2 } else { 0 },
                    routine,
                }
            } else if routine {
                Family::RoutineRow
            } else {
                Family::Row
            };
            let overlays = Overlays {
                patterns: &data.patterns,
                preview: Some(data.preview),
                labels: &data.labels,
                show_patterns: options.patterns,
                show_preview: options.preview,
                show_labels: options.labels,
                pattern_width: options.patterns_width,
                preview_width: options.preview_width,
                pattern_color: 0xff6a7a29,
                preview_color: 0xff7a7a7a,
                label_color: 0xff9c3db7,
            };
            let mut scene = minimap_content_scene::build_on_background(
                self.workspace.grid(),
                clock,
                projection,
                geometry.axis,
                width,
                options.note_width,
                options.note_spacing,
                0xff1e1e1e,
                overlays,
                Limits {
                    source_notes: options.max_notes,
                    query_heads: options.max_notes.saturating_add(1).min(65536),
                    tracks: 4096,
                    markers: 4096,
                },
                background,
                |position, _| {
                    let method = self.note_colors.step_color_method;
                    let row = if method == ColorMethod::Stepmania {
                        (position.row % 48) as u8
                    } else {
                        let measure = data.signatures.measure_position(position.row)?;
                        method.color_row(
                            position.row,
                            measure.row_in_measure,
                            data.signatures.at(position.row).denominator,
                        )?
                    };
                    let selected = selection
                        .is_some_and(|(start, end)| position.row >= start && position.row <= end);
                    self.minimap.content.palette.colors(
                        family,
                        row,
                        position.lane,
                        position.player,
                        selected,
                    )
                },
            )?;
            minimap_background::fade(scene.raster.pixels_mut(), fraction, 0.9)?;
            let mesh = Mesh::build(
                width,
                projection.visible_height as u32,
                scene.raster.pixels(),
                16384,
            )?;
            Ok(Frame {
                mesh,
                notes_complete: scene.notes_complete,
                markers_complete: scene.markers_complete,
                note_count: scene.note_count,
                tree_queries: scene.tree_queries,
            })
        });
        match frame {
            Ok(frame) => {
                *self.minimap.content.error.borrow_mut() = None;
                actors.push(frame.mesh.actor(
                    x,
                    projection.bounds_y as f32,
                    self.reverse_display,
                    8011,
                ));
                if !frame.complete() {
                    actors.push(act!(text: font("miso"): settext(i18n::tr("Edition","MinimapPartial")): xy(x+width as f32*0.5,82.0): zoom(0.45): maxwidth(width as f32): z(8013)));
                }
            }
            Err(detail) => self.minimap_content_error(actors, detail, x + width as f32 * 0.5),
        }
    }
}
