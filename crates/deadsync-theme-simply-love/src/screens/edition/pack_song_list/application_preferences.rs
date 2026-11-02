//! Typed application settings survive document replacement; transport, source
//! bindings, chart caches, cursor and undo stacks belong to the new document.
use super::State;
use deadsync_editor::{
    Error, audio_preferences::Audio, minimap_content_preferences, minimap_preferences,
    note_color_preferences, scroll_preferences, waveform_preferences, zoom, zoom_preferences,
};
use std::{path::PathBuf, time::Instant};
pub(in crate::screens::edition) struct Snapshot {
    recent: deadsync_editor::recent_song_preferences::Preferences,
    keys: std::sync::Arc<deadsync_editor::key_binding_preferences::Preferences>,
    audio: Audio,
    display_options: deadsync_editor::options_preferences::Display,
    general_options: deadsync_editor::options_preferences::General,
    log_preferences: deadsync_editor::log_preferences::Preferences,
    performance: super::super::performance::Runtime,
    ftue: deadsync_editor::ftue::Preferences,
    receptors: deadsync_editor::receptor_preferences::Preferences,
    dark: deadsync_editor::dark_preferences::Preferences,
    streams: deadsync_editor::stream_preferences::Preferences,
    density: deadsync_editor::density_preferences::Preferences,
    snap: deadsync_editor::snap::Preferences,
    source_player: Option<i32>,
    source_memory: Option<deadsync_editor::audio_preferences::File>,
    audio_path: Option<PathBuf>,
    zoom: zoom::Manager,
    zoom_epoch: Instant,
    zoom_preferences: zoom_preferences::Preferences,
    spacing: deadsync_editor::scroll_preferences::spacing::Preference,
    advance_by_snap: bool,
    reverse: bool,
    scroll: scroll_preferences::Preferences,
    selection: deadsync_editor::selection_preferences::Preferences,
    clipboards: deadsync_editor::workspace::Clipboards,
    minimap: minimap_preferences::Preferences,
    minimap_position: deadsync_editor::minimap_position_preferences::Preferences,
    content: minimap_content_preferences::Preferences,
    colors: std::sync::Arc<note_color_preferences::Preferences>,
    waveform: waveform_preferences::Preferences,
    profiles: Option<PathBuf>,
    autogen: deadsync_editor::autogen_preferences::Preferences,
}
impl Snapshot {
    pub(in crate::screens::edition) fn capture(state: &State) -> Self {
        Self {
            recent: state.file_menu.recent_preferences().clone(),
            keys: std::sync::Arc::clone(&state.command_bindings.preferences),
            audio: state.audio_preferences,
            display_options: state.display_options,
            general_options: state.general_options.clone(),
            log_preferences: state.log_preferences(),
            performance: state.performance.clone(),
            ftue: state.ftue_preferences(),
            receptors: state.receptor_preferences(),
            dark: state.dark_preferences(),
            streams: state.stream_preferences(),
            density: state.density_preferences(),
            snap: deadsync_editor::snap::Preferences {
                level: deadsync_editor::snap::Level::native(state.workspace.snap())
                    .expect("valid native snap"),
                lock: state
                    .audio_preferences_file
                    .as_ref()
                    .or(state.source_preferences_import.memory.as_ref())
                    .map(|file| file.snap().lock)
                    .unwrap_or_default(),
            },
            source_player: state.source_player_preference(),
            source_memory: state.source_preferences_import.memory.clone(),
            audio_path: state.audio_preferences_path.clone(),
            zoom: state.zoom.clone(),
            zoom_epoch: state.zoom_epoch,
            zoom_preferences: state.zoom_preferences,
            spacing: state.spacing_preference,
            advance_by_snap: state.workspace.auto_advance(),
            reverse: state.reverse_display,
            scroll: state.scroll_preferences,
            selection: state.selection_preferences,
            clipboards: state.workspace.capture_clipboards(),
            minimap: state.minimap.preferences,
            minimap_position: state.minimap.position,
            content: state.minimap.content.options,
            colors: state
                .note_color_draft
                .as_ref()
                .map(|draft| draft.baseline())
                .unwrap_or_else(|| std::sync::Arc::clone(&state.note_colors)),
            waveform: state.waveform_preferences,
            profiles: state.profile_directory.clone(),
            autogen: state.autogen_preferences.clone(),
        }
    }
    pub(in crate::screens::edition) fn install(self, target: &mut State) -> Result<(), Error> {
        // Read/validate the explicit custom path before any old-state ownership
        // is taken. A malformed/deleted-parent/directory read must not publish.
        if let Some(path) = self.audio_path.as_deref() {
            target.configure_audio_preferences(path)?;
        }
        if self.source_memory.is_some() && self.audio_path.is_none() {
            target.audio_preferences_file = None;
            target.audio_preferences_path = None;
        }
        target.source_preferences_import.memory = self.source_memory;
        if let Some(player) = self.source_player {
            target.install_source_player_preference(player)?;
        }
        target.file_menu.install_recent(self.recent.clone());
        target.configure_note_color_arc(std::sync::Arc::clone(&self.colors))?;
        if let Some(file) = target.audio_preferences_file.as_mut() {
            file.set_note_colors(self.colors)?;
            file.set_recent_songs(self.recent)?;
        }
        target.configure_key_binding_preferences(std::sync::Arc::clone(&self.keys));
        if let Some(file) = target.audio_preferences_file.as_mut() {
            file.set_key_bindings(self.keys)?;
        }
        target.configure_density_preferences(self.density)?;
        if let Some(file) = target.audio_preferences_file.as_mut() {
            file.set_density(self.density)?;
        }
        target.configure_stream_preferences(self.streams)?;
        if let Some(file) = target.audio_preferences_file.as_mut() {
            file.set_streams(self.streams)?;
        }
        target.configure_dark_preferences(self.dark);
        if let Some(file) = target.audio_preferences_file.as_mut() {
            file.set_dark(self.dark)?;
        }
        target.configure_receptor_preferences(self.receptors);
        if let Some(file) = target.audio_preferences_file.as_mut() {
            file.set_receptors(self.receptors);
        }
        target.configure_minimap_preferences(self.minimap);
        target.configure_minimap_position_preferences(self.minimap_position);
        if let Some(file) = target.audio_preferences_file.as_mut() {
            file.set_minimap_position(self.minimap_position);
            file.set_minimap_content(self.content)?;
        }
        target.configure_minimap_content_preferences(self.content);
        target.workspace.set_auto_advance(self.advance_by_snap);
        target.publish_note_entry_mode();
        if let Some(file) = target.audio_preferences_file.as_mut() {
            file.set_spacing_preference(self.spacing);
        }
        target.audio_preferences = self.audio;
        target.configure_display_options(self.display_options);
        target.configure_general_options(self.general_options.clone())?;
        target.configure_log_preferences(self.log_preferences);
        target.performance = self.performance;
        target.ftue = self.ftue.clone();
        if let Some(file) = target.audio_preferences_file.as_mut() {
            file.set_ftue(self.ftue);
        }
        let performance = target.performance_preferences();
        if let Some(file) = target.audio_preferences_file.as_mut() {
            file.set_performance(performance)?;
        }
        if let Some(file) = target.audio_preferences_file.as_mut() {
            file.set_log_preferences(self.log_preferences);
        }
        if let Some(file) = target.audio_preferences_file.as_mut() {
            file.set_general_options(self.general_options)?;
        }
        if let Some(file) = target.audio_preferences_file.as_mut() {
            file.set_display_options(self.display_options);
        }
        target
            .workspace
            .execute(deadsync_editor::workspace::Command::SetSnap(
                self.snap.level.native_index(),
            ))?;
        if let Some(file) = target.audio_preferences_file.as_mut() {
            file.set_snap(self.snap);
        }
        target.audio_preferences_path = self.audio_path;
        target.zoom = self.zoom;
        target.zoom_epoch = self.zoom_epoch;
        target.zoom_preferences = self.zoom_preferences;
        target.configure_spacing_preference(self.spacing);
        target.reverse_display = self.reverse;
        target.scroll_preferences = self.scroll;
        target.selection_preferences = self.selection;
        target.workspace.install_clipboards(self.clipboards);
        if let Some(file) = target.audio_preferences_file.as_mut() {
            file.set_selection(self.selection)?;
        }
        target.waveform_preferences = self.waveform;
        target.apply_waveform_preferences();
        target.profile_directory = self.profiles;
        target.configure_autogen_preferences(self.autogen);
        target.publish_autogen_profile_preferences();
        // Rebuild derived presentation inputs for the new document after all
        // application values are installed; no former chart cache is reused.
        target.refresh();
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use deadsync_editor::workspace::Command;
    #[test]
    fn memory_only_application_settings_survive_but_cursor_and_chart_caches_do_not() {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.ssc");
        std::fs::write(
            &path,
            "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
        )
        .unwrap();
        let mut source = State::open(&path, 0).unwrap();
        source.audio_preferences_path = None;
        source.audio_preferences_file = None;
        source.audio_preferences.set_music_rate(1.7).unwrap();
        let mut receptors = source.receptor_preferences();
        receptors.hide_arrows = false;
        receptors.animation.pulse = false;
        source.configure_receptor_preferences(receptors);
        let mut density = source.density_preferences();
        density.visible = false;
        density.height = 123;
        source.configure_density_preferences(density).unwrap();
        let mut streams = source.stream_preferences();
        streams.note_type = deadsync_editor::stream_preferences::NoteType::EighthTriplet;
        source.configure_stream_preferences(streams).unwrap();
        let mut dark = source.dark_preferences();
        dark.set_show(true);
        source.configure_dark_preferences(dark);
        let mut position = source.minimap_position_preferences();
        position.mount = deadsync_editor::minimap_position_preferences::Mount::RightSideOfWindow;
        position.select_mode =
            deadsync_editor::minimap_position_preferences::SelectMode::MoveToSelectedArea;
        source.configure_minimap_position_preferences(position);
        source.scroll_preferences.set_stop_playback(true);
        source.zoom.set_zoom(2.0).unwrap();
        source.zoom.update(1.0).unwrap();
        source.workspace.set_auto_advance(true);
        source.configure_spacing_preference(
            deadsync_editor::scroll_preferences::spacing::Preference::ConstantTime,
        );
        source.reverse_display = true;
        let mut colors = source.note_colors.as_ref().clone();
        colors.colors.subdivisions[0] = [0.1, 0.2, 0.3];
        source.configure_note_color_preferences(colors).unwrap();
        source.workspace.execute(Command::Seek(192)).unwrap();
        source.workspace.execute(Command::SetSnap(9)).unwrap();
        let mut target = State::open(&path, 0).unwrap();
        Snapshot::capture(&source).install(&mut target).unwrap();
        assert_eq!(target.audio_preferences.music_rate(), 1.7);
        assert_eq!(target.receptor_preferences(), receptors);
        assert_eq!(target.density_preferences(), density);
        assert_eq!(target.stream_preferences(), streams);
        assert_eq!(target.source_density.stream.threshold, 12);
        assert_eq!(target.dark_preferences(), dark);
        assert_eq!(target.minimap_position_preferences(), position);
        assert!(target.scroll_preferences.stop_playback());
        assert_eq!(target.note_colors.colors.subdivisions[0], [0.1, 0.2, 0.3]);
        assert_eq!(target.zoom, source.zoom);
        assert!(target.workspace.auto_advance());
        assert_eq!(target.spacing_mode, zoom::SpacingMode::ConstantTime);
        assert!(target.reverse_display);
        assert!(target.audio_preferences_path.is_none());
        assert_eq!(source.workspace.row(), 192);
        assert_eq!(target.workspace.row(), 0);
        assert_eq!(target.workspace.snap(), 9);
        let views = target
            .active_views
            .as_ref()
            .expect("common constructor creates chart views");
        assert!(views.belongs_to(&target.workspace.file().session));
        assert!(!views.belongs_to(&source.workspace.file().session));
        assert!(!views.entries().is_empty());
        for view in views.entries() {
            assert_eq!(
                Some(view.identity()),
                target
                    .workspace
                    .file()
                    .session
                    .chart_ids()
                    .get(view.source_index())
            );
            assert!(view.clock(&target.workspace.file().session).is_ok());
            assert!(view.clock(&source.workspace.file().session).is_err());
        }
        let projection = target.display_projection(0.0, 1000.0, 500.0).unwrap();
        assert!(matches!(
            &projection,
            crate::screens::edition::spacing_view::Projection::Time { .. }
        ));
        let cursor = projection.note_y(0).unwrap();
        let next_beat = projection.note_y(48).unwrap();
        assert!((cursor - 500.0).abs() < 1e-9);
        let expected =
            0.5 * deadsync_editor::spacing::Speeds::default().time * target.zoom.spacing_zoom();
        assert!((cursor - next_beat - expected).abs() < 1e-9);
        assert!(
            next_beat < cursor,
            "reverse display applies to the new song projection"
        );
        if let Some(position) = &target.focus_position {
            assert!(position.is_current(&target.workspace));
            assert!(!position.is_current(&source.workspace));
            assert_eq!(position.edit_row(), 0);
        }
    }
    #[test]
    fn unsaved_note_color_preview_does_not_escape_into_replacement_song() {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("preview.ssc");
        std::fs::write(
            &path,
            "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
        )
        .unwrap();
        let mut source = State::open(&path, 0).unwrap();
        let baseline = std::sync::Arc::clone(&source.note_colors);
        let mut draft =
            deadsync_editor::note_color_inspector::Draft::capture(std::sync::Arc::clone(&baseline))
                .unwrap();
        draft.adjust(0, true, false).unwrap();
        source.configure_note_color_arc(draft.staged()).unwrap();
        source.note_color_draft = Some(draft);
        assert_ne!(source.note_colors, baseline);
        let mut target = State::open(&path, 0).unwrap();
        Snapshot::capture(&source).install(&mut target).unwrap();
        assert_eq!(target.note_colors, baseline);
        assert!(target.note_color_draft.is_none());
    }
    #[test]
    fn memory_key_bindings_survive_document_replacement_without_preview_or_runtime() {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("keys.ssc");
        std::fs::write(
            &path,
            "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;",
        )
        .unwrap();
        let mut source = State::open(&path, 0).unwrap();
        let mut preferences = deadsync_editor::key_binding_preferences::Preferences::default();
        preferences
            .commands
            .insert("Save".into(), vec![vec!["F12".into()]]);
        let preferences = std::sync::Arc::new(preferences);
        source.configure_key_binding_preferences(std::sync::Arc::clone(&preferences));
        let mut target = State::open(&path, 0).unwrap();
        Snapshot::capture(&source).install(&mut target).unwrap();
        assert_eq!(target.command_bindings.preferences, preferences);
        assert!(target.key_binding_draft.is_none());
    }

    #[test]
    fn detached_widget_clipboard_and_selection_policy_survive_song_swap_en_fr() {
        use deadsync_editor::{
            event_clipboard::{Event, WidgetKind},
            playback::Clock,
            selection_preferences::{Overlap, Preferences, RegionMode},
            song_properties::Draft,
        };
        for lang in ["en", "fr"] {
            crate::i18n::init(deadsync_assets::language::load_for_tests(lang));
            let dir = tempfile::tempdir().unwrap();
            let a = dir.path().join("a.ssc");
            let b = dir.path().join("b.ssc");
            std::fs::write(&a,"#VERSION:0.83;#BPMS:0=120;#SAMPLESTART:1;#SAMPLELENGTH:2;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;").unwrap();
            std::fs::write(&b,"#VERSION:0.83;#BPMS:0=240;#OFFSET:0.25;#SAMPLESTART:3;#SAMPLELENGTH:9;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;").unwrap();
            let mut source = State::open(&a, 0).unwrap();
            let mut selection = source.workspace.misc_inventory(0).unwrap().choices();
            let index = selection
                .events()
                .iter()
                .position(|e| matches!(e,Event::SongWidget(w)if w.kind()==WidgetKind::Preview))
                .unwrap();
            selection.set_selected(index, true).unwrap();
            source.workspace.copy_event_selection(&selection).unwrap();
            source.selection_preferences = Preferences {
                overlap: Overlap::All,
                region: RegionMode::TimeOrPositionAndLane,
            };
            let source_before = source.workspace.file().session.serialize().unwrap();
            let mut target = State::open(&b, 0).unwrap();
            let before = target.workspace.file().session.serialize().unwrap();
            Snapshot::capture(&source).install(&mut target).unwrap();
            assert_eq!(target.selection_preferences, source.selection_preferences);
            assert!(target.event_selection.is_none());
            assert!(target.focus_position.is_none());
            let expected = Clock::capture(&target.workspace.file().session, 0)
                .unwrap()
                .music_time(49)
                .unwrap();
            target
                .workspace
                .paste_events_at_fractional_row(48.75)
                .unwrap();
            let draft = Draft::capture(&target.workspace.file().session).unwrap();
            assert_eq!(draft.sample_start(), expected);
            assert_eq!(draft.sample_length(), 9.0);
            assert_eq!(
                source.workspace.file().session.serialize().unwrap(),
                source_before
            );
            target.workspace.execute(Command::Undo).unwrap();
            assert_eq!(target.workspace.file().session.serialize().unwrap(), before);
        }
    }
}

#[cfg(test)]
mod spacing_raw_snapshot_tests {
    use super::*;
    use deadsync_editor::scroll_preferences::spacing::Preference;
    #[test]
    fn raw_spacing_survives_document_and_no_song_application_transfer() {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.ssc");
        std::fs::write(&path,b"#VERSION:0.83;#TITLE:Source;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        let prefs = root.path().join("preferences.json");
        std::fs::write(
            &prefs,
            br#"{"Version":1,"Scroll":{"SpacingMode":"99","Future":17}}"#,
        )
        .unwrap();
        let mut source = State::open_with_preferences(&path, 0, &prefs).unwrap();
        source.configure_spacing_preference(Preference::Unknown(-2));
        source
            .audio_preferences_file
            .as_mut()
            .unwrap()
            .set_spacing_preference(Preference::Unknown(-2));
        let mut target = State::without_song(root.path()).unwrap();
        let bytes = std::fs::read(&prefs).unwrap();
        Snapshot::capture(&source).install(&mut target).unwrap();
        assert_eq!(target.spacing_preference(), Preference::Unknown(-2));
        assert_eq!(target.spacing_mode(), zoom::SpacingMode::ConstantTime);
        assert_eq!(
            target
                .audio_preferences_file
                .as_ref()
                .unwrap()
                .spacing_preference(),
            Preference::Unknown(-2)
        );
        assert_eq!(std::fs::read(&prefs).unwrap(), bytes);
        assert!(!target.has_song());
        assert_eq!(target.workspace.file().session.undo_label(), None);
    }
}
