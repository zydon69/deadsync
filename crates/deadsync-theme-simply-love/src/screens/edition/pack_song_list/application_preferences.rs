//! Typed application settings survive document replacement; transport, source
//! bindings, chart caches, cursor and undo stacks belong to the new document.
use super::State;
use deadsync_editor::{
    Error, audio_preferences::Audio, minimap_content_preferences, minimap_preferences,
    note_color_preferences, scroll_preferences, waveform_preferences, zoom, zoom_preferences,
};
use std::{path::PathBuf, time::Instant};
pub(super) struct Snapshot {
    audio: Audio,
    receptors: deadsync_editor::receptor_preferences::Preferences,
    snap: deadsync_editor::snap::Preferences,
    audio_path: Option<PathBuf>,
    zoom: zoom::Manager,
    zoom_epoch: Instant,
    zoom_preferences: zoom_preferences::Preferences,
    spacing: zoom::SpacingMode,
    reverse: bool,
    scroll: scroll_preferences::Preferences,
    minimap: minimap_preferences::Preferences,
    minimap_position: deadsync_editor::minimap_position_preferences::Preferences,
    content: minimap_content_preferences::Preferences,
    colors: std::sync::Arc<note_color_preferences::Preferences>,
    waveform: waveform_preferences::Preferences,
    profiles: Option<PathBuf>,
}
impl Snapshot {
    pub(super) fn capture(state: &State) -> Self {
        Self {
            audio: state.audio_preferences,
            receptors: state.receptor_preferences(),
            snap: deadsync_editor::snap::Preferences {
                level: deadsync_editor::snap::Level::native(state.workspace.snap())
                    .expect("valid native snap"),
                lock: state
                    .audio_preferences_file
                    .as_ref()
                    .map(|file| file.snap().lock)
                    .unwrap_or_default(),
            },
            audio_path: state.audio_preferences_path.clone(),
            zoom: state.zoom.clone(),
            zoom_epoch: state.zoom_epoch,
            zoom_preferences: state.zoom_preferences,
            spacing: state.spacing_mode,
            reverse: state.reverse_display,
            scroll: state.scroll_preferences,
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
        }
    }
    pub(super) fn install(self, target: &mut State) -> Result<(), Error> {
        // Read/validate the explicit custom path before any old-state ownership
        // is taken. A malformed/deleted-parent/directory read must not publish.
        if let Some(path) = self.audio_path.as_deref() {
            target.configure_audio_preferences(path)?;
        }
        target.configure_note_color_arc(std::sync::Arc::clone(&self.colors))?;
        if let Some(file) = target.audio_preferences_file.as_mut() {
            file.set_note_colors(self.colors)?;
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
        target.audio_preferences = self.audio;
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
        target.spacing_mode = self.spacing;
        target.reverse_display = self.reverse;
        target.scroll_preferences = self.scroll;
        target.waveform_preferences = self.waveform;
        target.apply_waveform_preferences();
        target.profile_directory = self.profiles;
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
        let mut position = source.minimap_position_preferences();
        position.mount = deadsync_editor::minimap_position_preferences::Mount::RightSideOfWindow;
        position.select_mode =
            deadsync_editor::minimap_position_preferences::SelectMode::MoveToSelectedArea;
        source.configure_minimap_position_preferences(position);
        source.scroll_preferences.set_stop_playback(true);
        source.zoom.set_zoom(2.0).unwrap();
        source.zoom.update(1.0).unwrap();
        source.spacing_mode = zoom::SpacingMode::ConstantTime;
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
        assert_eq!(target.minimap_position_preferences(), position);
        assert!(target.scroll_preferences.stop_playback());
        assert_eq!(target.note_colors.colors.subdivisions[0], [0.1, 0.2, 0.3]);
        assert_eq!(target.zoom, source.zoom);
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
}
