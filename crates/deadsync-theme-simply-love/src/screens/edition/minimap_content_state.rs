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
    preview: Option<(Point, Point)>,
    labels: Vec<Point>,
}
struct MetadataEntry {
    binding: Binding,
    data: Result<Metadata, Arc<str>>,
}
pub(super) struct Content {
    preview_owner: Option<Arc<()>>,
    pub(super) options: ContentPreferences,
    palette: Palette,
    revision: u64,
    metadata: RefCell<Option<MetadataEntry>>,
    frames: minimap_content_cache::Cache,
    error: RefCell<Option<Arc<str>>>,
}
impl Content {
    pub(super) fn palette(&self) -> &Palette {
        &self.palette
    }
}
impl Default for Content {
    fn default() -> Self {
        Self {
            preview_owner: None,
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
        let chart = state
            .preview_source_chart()
            .unwrap_or(state.workspace.chart());
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
        let preview = if song
            .text(deadsync_editor::song_text::Field::PreviewMusic)
            .is_empty()
        {
            Some((
                Point {
                    row: clock.position(start)?,
                    chart_time: clock.music_to_chart_time(start)?,
                },
                Point {
                    row: clock.position(end)?,
                    chart_time: clock.music_to_chart_time(end)?,
                },
            ))
        } else {
            None
        };
        Ok(Self {
            signatures,
            patterns,
            preview,
            labels,
        })
    }
}
impl State {
    pub(super) fn refresh_minimap_preview(&mut self) {
        let owner = self
            .preview
            .as_ref()
            .map(|_| self.receptor_views.preview_owner());
        let same = match (&owner, &self.minimap.content.preview_owner) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        };
        if !same {
            self.minimap.content.preview_owner = owner;
            self.minimap.content.frames.clear();
            self.minimap.content.metadata.get_mut().take();
        }
    }

    pub fn note_color_preferences(&self) -> &NotePreferences {
        &self.note_colors
    }
    #[cfg(test)]
    pub(super) fn configure_note_color_preferences(
        &mut self,
        preferences: NotePreferences,
    ) -> Result<(), deadsync_editor::Error> {
        self.configure_note_color_arc(Arc::new(preferences))
    }
    pub(super) fn configure_note_color_arc(
        &mut self,
        preferences: Arc<NotePreferences>,
    ) -> Result<(), deadsync_editor::Error> {
        preferences.validate()?;
        let palette = Palette::new(&preferences.colors)?;
        let note_colors = preferences;
        let hold_colors =
            deadsync_editor::note_color_palette::Palette::capture(Arc::clone(&note_colors))?;
        self.note_colors = note_colors;
        self.hold_colors = hold_colors;
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
        if (self.panel.is_some() && !matches!(self.panel, Some(Panel::GenerationPreview(_))))
            || !self.workspace.has_chart()
            || !self.minimap.content.options.visible
        {
            return None;
        }
        let geometry = self.minimap_viewport_geometry().ok()?;
        let key = self.minimap_content_key(&geometry);
        let frame = self
            .minimap
            .content
            .frames
            .current(
                &self.workspace.file().session,
                self.preview_source_chart()
                    .unwrap_or(self.workspace.chart()),
                &key,
            )?
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
        let width = options.width;
        let x = match self.minimap_mount_bounds() {
            Ok(bounds) => bounds.x as f32,
            Err(error) => {
                self.minimap_content_error(
                    actors,
                    Arc::from(error.to_string()),
                    screen_width() * 0.5,
                );
                return;
            }
        };
        let session = &self.workspace.file().session;
        let chart = self
            .preview_source_chart()
            .unwrap_or(self.workspace.chart());
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
        let selection = if self.preview.is_some() {
            None
        } else {
            self.workspace.selection()
        };
        let key = self.minimap_content_key(geometry);
        let frame = self.minimap.content.frames.frame(session, chart, key, || {
            let grid = self
                .preview
                .as_ref()
                .map_or(self.workspace.grid(), |preview| preview.grid());
            let content = match geometry.axis {
                deadsync_editor::minimap_viewport::Axis::Time => {
                    (clock.chart_time(0)?, clock.note_time(grid.last_row())?)
                }
                deadsync_editor::minimap_viewport::Axis::Row => (0.0, grid.last_row() as f64),
            };
            let background = minimap_background::build_overview(
                projection,
                width,
                content,
                geometry.focused_coordinate(),
                self.minimap_capture_active(),
                self.minimap_focused(),
            )?;
            let layout = self.preview.as_ref().map_or(
                session.document().charts()[chart].steps_type.as_str(),
                |preview| preview.layout(),
            );
            let routine = matches!(layout, "dance-routine" | "smx-team" | "pump-routine");
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
                preview: data.preview,
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
            let scene = minimap_content_scene::build_on_background(
                grid,
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
                    let row = if method.effective() == ColorMethod::Stepmania {
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

#[cfg(test)]
mod source_raw_palette_consumer_workflows {
    use super::*;
    #[test]
    fn all_original_raw_cases_install_into_both_real_native_palette_caches() {
        i18n::init_for_tests();
        let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../deadsync-editor/tests/fixtures/note-color-raw-source.json"
        )))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let dir = tempfile::tempdir().unwrap();
            let mut state = State::without_song(dir.path()).unwrap();
            let file = state.capture_application_preferences_file().unwrap();
            let draft = state
                .workspace
                .prepare_source_preferences(
                    &file,
                    case["raw"].as_str().unwrap().as_bytes(),
                    deadsync_editor::key_binding_preferences::source_profile::Platform::Mac,
                )
                .unwrap();
            state
                .workspace
                .import_source_preferences(draft, &file, true)
                .unwrap();
            state.sync_source_preferences_history().unwrap();
            assert!(state.hold_colors.is_current(&state.note_colors));
            for r in case["lookups"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| !r["held"].as_bool().unwrap())
            {
                let player = r["player"].as_i64().unwrap();
                let ordinary = player < 0;
                let player = player.max(0) as u32;
                let selected = r["selected"].as_bool().unwrap();
                let row = r["row"].as_u64().unwrap() as u8;
                let actual = state
                    .hold_colors
                    .tap(
                        if ordinary {
                            "dance-single"
                        } else {
                            "dance-routine"
                        },
                        row,
                        0,
                        player,
                        selected,
                    )
                    .unwrap();
                assert_eq!(
                    actual,
                    (r["color"].as_u64().unwrap() as u32).to_le_bytes(),
                    "{}",
                    case["name"]
                );
                assert_eq!(
                    state
                        .minimap
                        .content
                        .palette
                        .colors(
                            if ordinary {
                                Family::Row
                            } else {
                                Family::RoutineRow
                            },
                            row,
                            0,
                            player,
                            selected
                        )
                        .unwrap()
                        .head,
                    r["ui"].as_u64().unwrap() as u32,
                    "{}",
                    case["name"]
                );
            }
            assert!(state.audio_preferences_file.is_none());
            assert!(!state.application_preferences_owner().unwrap().has_changes());
        }
    }
}

#[cfg(test)]
mod resident_preview_marker_tests {
    use super::*;
    #[test]
    fn minimap_marker_metadata_uses_effective_resident_preview_across_apply_undo_redo_en_fr() {
        for language in ["en", "fr"] {
            i18n::init(deadsync_assets::language::load_for_tests(language));
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("raw.sm");
            std::fs::write(&path,"#MUSIC:main.wav;#PREVIEW:opaque.wav;#SAMPLESTART:0.375;#SAMPLELENGTH:1.25;#BPMS:0=120;#NOTES:dance-single::Edit:9:0,0,0,0,0:1000;").unwrap();
            let mut state = State::open(&path, 0).unwrap();
            let marker = |state: &State| {
                let clock = Clock::capture(&state.workspace.file().session, 0).unwrap();
                Metadata::capture(state, &clock).unwrap().preview
            };
            assert!(
                marker(&state).is_some(),
                "ignored opaque raw SM PREVIEW uses main sample marker"
            );
            let mut draft = state.workspace.song_properties().unwrap();
            draft
                .set_text(
                    deadsync_editor::song_text::Field::PreviewMusic,
                    "authored.wav",
                )
                .unwrap();
            state.workspace.apply_song_properties(&draft).unwrap();
            state.refresh();
            assert!(marker(&state).is_none());
            state.command(deadsync_editor::workspace::Command::Undo);
            assert!(marker(&state).is_some());
            state.command(deadsync_editor::workspace::Command::Redo);
            assert!(marker(&state).is_none());
            let mut draft = state.workspace.song_properties().unwrap();
            draft
                .set_text(deadsync_editor::song_text::Field::PreviewMusic, "")
                .unwrap();
            state.workspace.apply_song_properties(&draft).unwrap();
            state.refresh();
            assert!(marker(&state).is_some());
        }
    }
}
