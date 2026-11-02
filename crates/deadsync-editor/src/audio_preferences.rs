//! Shared editor application preferences, stored independently of simfiles.
//! Audio, waveform and zoom groups share conflict-safe atomic persistence.
pub mod durability;
pub mod fields;
pub(crate) mod history;
pub mod publication;
pub use fields::{EffectiveAudio, Field, FieldValue, MixGains};
mod file_startup;
use crate::{Error, waveform_preferences::Preferences as Waveform};
pub use file_startup::StartupTarget;
pub use history::Update;
use serde_json::{Map, Value};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Audio {
    fade_in: f64,
    fade_out: f64,
    music_rate: f64,
    offset: f64,
    main_volume: f32,
    music_volume: f32,
    assist_volume: f32,
    assist_attack: f32,
    assist_enabled: bool,
    assist_skip_beat: bool,
    beat_volume: f32,
    beat_attack: f32,
    beat_enabled: bool,
    beat_skip_assist: bool,
    dsp_size: i32,
    dsp_buffers: i32,
}
impl Default for Audio {
    fn default() -> Self {
        Self {
            fade_in: 0.0,
            fade_out: 1.5,
            music_rate: 1.0,
            offset: 0.0,
            main_volume: 1.0,
            music_volume: 0.5,
            assist_volume: 0.7,
            assist_attack: 0.0,
            assist_enabled: false,
            assist_skip_beat: false,
            beat_volume: 0.75,
            beat_attack: 0.0,
            beat_enabled: false,
            beat_skip_assist: false,
            dsp_size: 512,
            dsp_buffers: 4,
        }
    }
}
impl Audio {
    pub fn music_rate(&self) -> f64 {
        self.music_rate
    }
    pub fn set_music_rate(&mut self, rate: f64) -> Result<(), Error> {
        if !rate.is_finite() {
            return Err(Error::InvalidEdit("Music rate must be finite".into()));
        }
        self.music_rate = rate.clamp(0.25, 4.0);
        Ok(())
    }
    /// Original editor shortcuts advance to the next/previous tenth, using
    /// Fumen's one-ULP equality when the current rate is already on a tenth.
    pub fn step_music_rate(&mut self, increase: bool) {
        let scaled = self.music_rate * 10.0;
        let mut tenths = scaled as i32;
        if increase {
            // The original C# shortcuts use unchecked int32 arithmetic. Raw
            // imported rates can reach these boundaries before UI clamping.
            tenths = tenths.wrapping_add(1);
        } else if (tenths as f64).to_bits().abs_diff(scaled.to_bits()) <= 1 {
            tenths = tenths.wrapping_sub(1);
        }
        let next = (tenths as f64 / 10.0).clamp(0.25, 4.0);
        if next.to_bits().abs_diff(self.music_rate.to_bits()) > 1 {
            self.music_rate = next;
        }
    }
    pub fn fade_in(&self) -> f64 {
        self.fade_in
    }
    pub fn fade_out(&self) -> f64 {
        self.fade_out
    }
    pub fn set_fade_in(&mut self, seconds: f64) -> Result<(), Error> {
        self.fade_in = valid(seconds)?;
        Ok(())
    }
    pub fn set_fade_out(&mut self, seconds: f64) -> Result<(), Error> {
        self.fade_out = valid(seconds)?;
        Ok(())
    }
}
fn valid(seconds: f64) -> Result<f64, Error> {
    if seconds.is_finite() && seconds >= 0.0 {
        Ok(seconds)
    } else {
        Err(Error::InvalidEdit(
            "Preview fade must be finite and nonnegative".into(),
        ))
    }
}
fn read_bytes(path: &Path) -> Result<Option<Vec<u8>>, Error> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}
#[derive(Clone, PartialEq)]
struct Settings {
    initial_file_directory: Option<Value>,
    ftue: crate::ftue::Preferences,
    log_preferences: crate::log_preferences::Preferences,
    source_archive: Option<std::sync::Arc<Value>>,
    player: i32,
    general_options: crate::options_preferences::General,
    autogen: crate::autogen_preferences::Preferences,
    display_options: crate::options_preferences::Display,
    note_entry: crate::lane_entry::Mode,
    spacing: crate::scroll_preferences::spacing::Preference,
    suppress_external_song: bool,
    recent_songs: crate::recent_song_preferences::Preferences,
    advanced_save: crate::advanced_export::Options,
    key_bindings: std::sync::Arc<crate::key_binding_preferences::Preferences>,
    note_colors: std::sync::Arc<crate::note_color_preferences::Preferences>,
    receptors: crate::receptor_preferences::Preferences,
    dark: crate::dark_preferences::Preferences,
    streams: crate::stream_preferences::Preferences,
    selection: crate::selection_preferences::Preferences,
    density: crate::density_preferences::Preferences,
    performance: crate::performance::preferences::Preferences,
    snap: crate::snap::Preferences,
    audio: Audio,
    waveform: Waveform,
    zoom: crate::zoom_preferences::Preferences,
    scroll: crate::scroll_preferences::Preferences,
    minimap: crate::minimap_preferences::Preferences,
    minimap_position: crate::minimap_position_preferences::Preferences,
    minimap_content: crate::minimap_content_preferences::Preferences,
}

#[derive(Clone)]
pub struct File {
    path: PathBuf,
    durability_key: PathBuf,
    expected: Option<std::sync::Arc<Vec<u8>>>,
    root: std::sync::Arc<Map<String, Value>>,
    audio_fields: Map<String, Value>,
    waveform_fields: Map<String, Value>,
    zoom_fields: Map<String, Value>,
    scroll_fields: Map<String, Value>,
    minimap_fields: Map<String, Value>,
    original: Settings,
    written: Settings,
    settings: Settings,
}
impl File {
    pub fn memory() -> Result<Self, Error> {
        Self::from_root(
            Path::new(""),
            None,
            Map::from_iter([("Version".into(), Value::from(1))]),
        )
    }
    pub fn open(path: &Path) -> Result<Self, Error> {
        durability::retry_path(path)?;
        let expected = read_bytes(path)?.map(std::sync::Arc::new);
        let root = if let Some(bytes) = &expected {
            serde_json::from_slice::<Value>(bytes.as_slice())
                .map_err(|error| Error::InvalidFormat(error.to_string()))?
                .as_object()
                .cloned()
                .ok_or_else(|| {
                    Error::InvalidFormat("Editor preferences must be an object".into())
                })?
        } else {
            Map::new()
        };
        Self::from_root(path, expected, root)
    }
    fn from_root(
        path: &Path,
        expected: Option<std::sync::Arc<Vec<u8>>>,
        mut root: Map<String, Value>,
    ) -> Result<Self, Error> {
        if expected.is_some() && root.get("Version").and_then(Value::as_u64) != Some(1) {
            return Err(Error::Unsupported(
                "Unknown editor preferences version".into(),
            ));
        }
        root.insert("Version".into(), Value::from(1));
        let source_archive = root
            .remove("GrooveAuthorSourceArchive")
            .map(std::sync::Arc::new);
        let audio_fields = match root.get("Audio") {
            None => Map::new(),
            Some(Value::Object(fields)) => fields.clone(),
            Some(_) => {
                return Err(Error::InvalidFormat(
                    "Editor audio preferences must be an object".into(),
                ));
            }
        };
        let audio = Audio::read(&audio_fields)?;
        let waveform_fields = match root.get("Waveform") {
            None => Map::new(),
            Some(Value::Object(fields)) => fields.clone(),
            Some(_) => {
                return Err(Error::InvalidFormat(
                    "Editor waveform preferences must be an object".into(),
                ));
            }
        };
        let zoom_fields = match root.get("Zoom") {
            None => Map::new(),
            Some(Value::Object(fields)) => fields.clone(),
            Some(_) => {
                return Err(Error::InvalidFormat(
                    "Editor zoom preferences must be an object".into(),
                ));
            }
        };
        let scroll_fields = match root.get("Scroll") {
            None => Map::new(),
            Some(Value::Object(fields)) => fields.clone(),
            Some(_) => {
                return Err(Error::InvalidFormat(
                    "Editor scroll preferences must be an object".into(),
                ));
            }
        };
        let minimap_fields = match root.get("MiniMap") {
            None => Map::new(),
            Some(Value::Object(fields)) => fields.clone(),
            Some(_) => {
                return Err(Error::InvalidFormat(
                    "Editor minimap preferences must be an object".into(),
                ));
            }
        };
        let receptor_fields = match root.get("Receptors") {
            None => Map::new(),
            Some(Value::Object(fields)) => fields.clone(),
            Some(_) => {
                return Err(Error::InvalidFormat(
                    "Editor Receptors preferences must be an object".into(),
                ));
            }
        };
        let colors_group = |name: &str| -> Result<Map<String, Value>, Error> {
            match root.get(name) {
                None => Ok(Map::new()),
                Some(Value::Object(fields)) => Ok(fields.clone()),
                Some(_) => Err(Error::InvalidFormat(format!(
                    "Editor {name} preferences must be an object"
                ))),
            }
        };
        let options_fields = colors_group("Options")?;
        let key_bindings = std::sync::Arc::new(crate::key_binding_preferences::Preferences::read(
            &colors_group("KeyBinds")?,
        )?);
        let note_colors = std::sync::Arc::new(crate::note_color_preferences::Preferences::read(
            &colors_group("NoteColor")?,
            &options_fields,
        )?);
        let settings = Settings {
            initial_file_directory: root.get("OpenFileDialogInitialDirectory").cloned(),
            ftue: crate::ftue::Preferences::read(&root)?,
            log_preferences: crate::log_preferences::Preferences::read_source(&root)?,
            source_archive,
            player: root
                .get("Player")
                .map(|v| {
                    v.as_i64()
                        .and_then(|n| i32::try_from(n).ok())
                        .ok_or_else(|| {
                            Error::InvalidFormat("Invalid source player preference".into())
                        })
                })
                .transpose()?
                .unwrap_or(0),
            general_options: crate::options_preferences::General::read(&options_fields)?,
            autogen: crate::autogen_preferences::Preferences::read(&root)?,
            display_options: crate::options_preferences::Display::read(&options_fields)?,
            note_entry: crate::lane_entry::Mode::read(root.get("NoteEntryMode")),
            spacing: crate::scroll_preferences::spacing::Preference::read_native(
                scroll_fields.get("SpacingMode"),
            )?,
            suppress_external_song: match options_fields
                .get("SuppressExternalSongModificationNotification")
            {
                None => false,
                Some(Value::Bool(value)) => *value,
                _ => {
                    return Err(Error::InvalidFormat(
                        "Invalid SuppressExternalSongModificationNotification".into(),
                    ));
                }
            },
            recent_songs: crate::recent_song_preferences::Preferences::read(&root)?,
            advanced_save: crate::advanced_export::Options::read(&root)?,
            note_colors,
            key_bindings,
            receptors: crate::receptor_preferences::Preferences::read(&receptor_fields)?,
            dark: crate::dark_preferences::Preferences::read(&colors_group("Dark")?)?,
            streams: crate::stream_preferences::Preferences::read(&colors_group("Stream")?)?,
            selection: crate::selection_preferences::Preferences::read(&colors_group(
                "Selection",
            )?)?,
            density: crate::density_preferences::Preferences::read(&colors_group("DensityGraph")?)?,
            performance: crate::performance::preferences::Preferences::read(&colors_group(
                "Performance",
            )?)?,
            snap: if root.contains_key("SnapIndex") || root.contains_key("SnapLockIndex") {
                crate::snap::Preferences::read(&root)
            } else {
                crate::snap::Preferences {
                    level: crate::snap::Level::source(1)?,
                    ..Default::default()
                }
            },
            audio,
            waveform: Waveform::read(&waveform_fields)?,
            zoom: crate::zoom_preferences::Preferences::read(&zoom_fields)?,
            scroll: crate::scroll_preferences::Preferences::read(&scroll_fields)?,
            minimap: crate::minimap_preferences::Preferences::read(&minimap_fields)?,
            minimap_position: crate::minimap_position_preferences::Preferences::read(
                &minimap_fields,
            )?,
            minimap_content: crate::minimap_content_preferences::Preferences::read(
                &minimap_fields,
                &options_fields,
            )?,
        };
        Ok(Self {
            path: path.to_owned(),
            durability_key: durability::key(path),
            expected,
            root: std::sync::Arc::new(root),
            audio_fields,
            waveform_fields,
            zoom_fields,
            scroll_fields,
            minimap_fields,
            original: settings.clone(),
            written: settings.clone(),
            settings,
        })
    }
    pub fn preferred_player(&self) -> i32 {
        self.settings.player
    }
    pub fn set_preferred_player(&mut self, player: i32) {
        self.settings.player = player;
    }
    pub fn source_archive(&self) -> Option<&Value> {
        self.settings.source_archive.as_deref()
    }
    pub fn autogen(&self) -> crate::autogen_preferences::Preferences {
        self.settings.autogen.clone()
    }
    pub fn set_autogen(&mut self, value: crate::autogen_preferences::Preferences) {
        self.settings.autogen = value;
    }
    pub fn note_entry_mode(&self) -> crate::lane_entry::Mode {
        self.settings.note_entry
    }
    pub fn set_note_entry_mode(&mut self, mode: crate::lane_entry::Mode) {
        self.settings.note_entry = mode;
    }
    pub fn spacing_mode(&self) -> crate::zoom::SpacingMode {
        self.settings.spacing.effective()
    }
    pub fn spacing_preference(&self) -> crate::scroll_preferences::spacing::Preference {
        self.settings.spacing
    }
    pub fn set_spacing_preference(
        &mut self,
        value: crate::scroll_preferences::spacing::Preference,
    ) {
        self.settings.spacing = value;
    }
    pub fn has_spacing_mode(&self) -> bool {
        self.scroll_fields.contains_key("SpacingMode")
    }
    pub fn set_spacing_mode(&mut self, mode: crate::zoom::SpacingMode) {
        self.settings.spacing =
            crate::scroll_preferences::spacing::Preference::from_effective(mode);
    }
    pub fn audio(&self) -> Audio {
        self.settings.audio
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn set_audio(&mut self, audio: Audio) {
        self.settings.audio = audio;
    }
    pub fn waveform(&self) -> Waveform {
        self.settings.waveform
    }
    pub fn set_waveform(&mut self, waveform: Waveform) {
        self.settings.waveform = waveform;
    }
    pub fn zoom(&self) -> crate::zoom_preferences::Preferences {
        self.settings.zoom
    }
    pub fn set_zoom(&mut self, zoom: crate::zoom_preferences::Preferences) {
        self.settings.zoom = zoom;
    }
    pub fn scroll(&self) -> crate::scroll_preferences::Preferences {
        self.settings.scroll
    }
    pub fn performance(&self) -> crate::performance::preferences::Preferences {
        self.settings.performance
    }
    pub fn set_performance(
        &mut self,
        preferences: crate::performance::preferences::Preferences,
    ) -> Result<(), Error> {
        preferences.write(&mut Map::new())?;
        self.settings.performance = preferences;
        Ok(())
    }
    pub fn set_scroll(&mut self, scroll: crate::scroll_preferences::Preferences) {
        self.settings.scroll = scroll;
    }
    pub fn minimap(&self) -> crate::minimap_preferences::Preferences {
        self.settings.minimap
    }
    pub fn set_minimap(&mut self, minimap: crate::minimap_preferences::Preferences) {
        self.settings.minimap = minimap;
    }
    pub fn minimap_position(&self) -> crate::minimap_position_preferences::Preferences {
        self.settings.minimap_position
    }
    pub fn set_minimap_position(
        &mut self,
        preferences: crate::minimap_position_preferences::Preferences,
    ) {
        self.settings.minimap_position = preferences;
    }
    pub fn set_minimap_content(
        &mut self,
        preferences: crate::minimap_content_preferences::Preferences,
    ) -> Result<(), Error> {
        let mut minimap = Map::new();
        let mut options = Map::new();
        preferences.write(&mut minimap, &mut options)?;
        self.settings.minimap_content = preferences;
        Ok(())
    }
    fn group_fields(
        &self,
        name: &str,
    ) -> Result<serde_json::Map<String, serde_json::Value>, Error> {
        match self.root.get(name) {
            None => Ok(Default::default()),
            Some(serde_json::Value::Object(fields)) => Ok(fields.clone()),
            Some(_) => Err(Error::InvalidFormat(format!(
                "Editor {name} preferences must be an object"
            ))),
        }
    }
    pub fn suppress_external_song_notifications(&self) -> bool {
        self.settings.suppress_external_song
    }
    pub fn set_suppress_external_song_notifications(&mut self, value: bool) {
        self.settings.suppress_external_song = value;
    }
    pub fn advanced_save(&self) -> crate::advanced_export::Options {
        self.settings.advanced_save
    }
    pub fn set_advanced_save(&mut self, options: crate::advanced_export::Options) {
        self.settings.advanced_save = options;
    }
    pub fn snap(&self) -> crate::snap::Preferences {
        self.settings.snap
    }
    pub fn has_snap_preferences(&self) -> bool {
        self.root.contains_key("SnapIndex") || self.root.contains_key("SnapLockIndex")
    }
    pub fn set_snap(&mut self, snap: crate::snap::Preferences) {
        self.settings.snap = snap;
    }
    pub fn density(&self) -> crate::density_preferences::Preferences {
        self.settings.density
    }
    pub fn set_density(&mut self, p: crate::density_preferences::Preferences) -> Result<(), Error> {
        p.validate()?;
        self.settings.density = p;
        Ok(())
    }
    pub fn density_stream(&self) -> Result<crate::density::StreamSettings, Error> {
        Ok(self.settings.streams.density_settings())
    }
    pub fn selection(&self) -> crate::selection_preferences::Preferences {
        self.settings.selection
    }
    pub fn set_selection(
        &mut self,
        value: crate::selection_preferences::Preferences,
    ) -> Result<(), Error> {
        value.validate()?;
        self.settings.selection = value;
        Ok(())
    }
    pub fn streams(&self) -> crate::stream_preferences::Preferences {
        self.settings.streams
    }
    pub fn set_streams(&mut self, p: crate::stream_preferences::Preferences) -> Result<(), Error> {
        p.validate()?;
        self.settings.streams = p;
        Ok(())
    }
    pub fn dark(&self) -> crate::dark_preferences::Preferences {
        self.settings.dark
    }
    pub fn set_dark(&mut self, value: crate::dark_preferences::Preferences) -> Result<(), Error> {
        value.validate()?;
        self.settings.dark = value;
        Ok(())
    }
    pub fn receptors(&self) -> crate::receptor_preferences::Preferences {
        self.settings.receptors
    }
    pub fn set_receptors(&mut self, p: crate::receptor_preferences::Preferences) {
        self.settings.receptors = p;
    }
    /// Shared Options group; no independent preference file or implicit write.
    pub fn general_options(&self) -> &crate::options_preferences::General {
        &self.settings.general_options
    }
    pub fn set_general_options(
        &mut self,
        value: crate::options_preferences::General,
    ) -> Result<(), Error> {
        value.validate()?;
        self.settings.general_options = value;
        Ok(())
    }
    pub fn new_song_sync_offset(&self) -> Result<f64, Error> {
        Ok(self.settings.general_options.new_sync)
    }
    pub fn opening(&self) -> Result<crate::open_preferences::Preferences, Error> {
        let mut fields = self.group_fields("Options")?;
        self.settings.general_options.write(&mut fields)?;
        crate::open_preferences::Preferences::read(&fields)
    }
    pub fn key_bindings(&self) -> std::sync::Arc<crate::key_binding_preferences::Preferences> {
        std::sync::Arc::clone(&self.settings.key_bindings)
    }
    pub fn set_key_bindings(
        &mut self,
        p: std::sync::Arc<crate::key_binding_preferences::Preferences>,
    ) -> Result<(), Error> {
        p.validate()?;
        self.settings.key_bindings = p;
        Ok(())
    }
    pub fn note_colors(&self) -> Result<crate::note_color_preferences::Preferences, Error> {
        Ok(self.settings.note_colors.as_ref().clone())
    }
    pub fn note_colors_arc(&self) -> std::sync::Arc<crate::note_color_preferences::Preferences> {
        std::sync::Arc::clone(&self.settings.note_colors)
    }
    pub fn set_note_colors(
        &mut self,
        p: std::sync::Arc<crate::note_color_preferences::Preferences>,
    ) -> Result<(), Error> {
        p.validate()?;
        self.settings.note_colors = p;
        Ok(())
    }
    pub fn minimap_content(
        &self,
    ) -> Result<crate::minimap_content_preferences::Preferences, Error> {
        Ok(self.settings.minimap_content)
    }
    pub fn recent_songs(&self) -> &crate::recent_song_preferences::Preferences {
        &self.settings.recent_songs
    }
    pub fn set_recent_songs(
        &mut self,
        preferences: crate::recent_song_preferences::Preferences,
    ) -> Result<(), Error> {
        preferences.validate()?;
        self.settings.recent_songs = preferences;
        Ok(())
    }
    pub fn record_recent_song(&mut self, song: &crate::recent_songs::Song) -> Result<(), Error> {
        self.settings.recent_songs.record(song)
    }
    pub fn has_changes(&self) -> bool {
        self.settings != self.original || self.settings != self.written
    }
    fn projected_root(&self) -> Result<Map<String, Value>, Error> {
        let mut fields = self.audio_fields.clone();
        self.settings.audio.write(&mut fields);
        let mut waveform_fields = self.waveform_fields.clone();
        self.settings.waveform.write(&mut waveform_fields);
        let mut zoom_fields = self.zoom_fields.clone();
        self.settings.zoom.write(&mut zoom_fields);
        let mut scroll_fields = self.scroll_fields.clone();
        self.settings.scroll.write(&mut scroll_fields);
        if self.settings.spacing != self.written.spacing {
            scroll_fields.insert(
                "SpacingMode".into(),
                self.settings.spacing.source_name().into(),
            );
        }
        let mut minimap_fields = self.minimap_fields.clone();
        self.settings.minimap.write(&mut minimap_fields);
        self.settings.minimap_position.write(&mut minimap_fields);
        let mut options_fields = self.group_fields("Options")?;
        self.settings.display_options.write(&mut options_fields);
        self.settings.general_options.write(&mut options_fields)?;
        if self.settings.suppress_external_song != self.written.suppress_external_song {
            options_fields.insert(
                "SuppressExternalSongModificationNotification".into(),
                Value::Bool(self.settings.suppress_external_song),
            );
        }
        self.settings
            .minimap_content
            .write(&mut minimap_fields, &mut options_fields)?;
        self.settings
            .recent_songs
            .write_options(&mut options_fields)?;
        let mut root = self.root.as_ref().clone();
        match &self.settings.initial_file_directory {
            Some(value) => {
                root.insert("OpenFileDialogInitialDirectory".into(), value.clone());
            }
            None => {
                root.remove("OpenFileDialogInitialDirectory");
            }
        }
        if let Some(archive) = &self.settings.source_archive {
            root.insert("GrooveAuthorSourceArchive".into(), archive.as_ref().clone());
        }

        if self.settings.player != self.written.player {
            root.insert("Player".into(), self.settings.player.into());
        }

        self.settings
            .ftue
            .write_changed(&self.written.ftue, &mut root);
        self.settings
            .log_preferences
            .write_changed(self.written.log_preferences, &mut root);
        if self.settings.performance != self.written.performance {
            let mut fields = self.group_fields("Performance")?;
            self.settings.performance.write(&mut fields)?;
            root.insert("Performance".into(), Value::Object(fields));
        }
        self.settings
            .autogen
            .write_changed(&self.written.autogen, &mut root);
        if self.settings.note_entry != self.written.note_entry {
            root.insert(
                "NoteEntryMode".into(),
                self.settings.note_entry.name().into(),
            );
        }
        self.settings.recent_songs.write(&mut root);
        if self.settings.advanced_save != self.written.advanced_save {
            self.settings.advanced_save.write(&mut root);
        }
        let mut key_fields = self.group_fields("KeyBinds")?;
        self.settings.key_bindings.write(&mut key_fields)?;
        root.insert("KeyBinds".into(), Value::Object(key_fields));
        let mut note_fields = self.group_fields("NoteColor")?;
        self.settings
            .note_colors
            .write(&mut note_fields, &mut options_fields)?;
        root.insert("NoteColor".into(), Value::Object(note_fields));
        root.insert("Options".into(), Value::Object(options_fields));
        let mut dark_fields = self.group_fields("Dark")?;
        self.settings.dark.write(&mut dark_fields);
        root.insert("Dark".into(), Value::Object(dark_fields));
        let mut selection_fields = self.group_fields("Selection")?;
        self.settings.selection.write(&mut selection_fields)?;
        root.insert("Selection".into(), Value::Object(selection_fields));
        let mut stream_fields = self.group_fields("Stream")?;
        self.settings.streams.write(&mut stream_fields)?;
        root.insert("Stream".into(), Value::Object(stream_fields));
        let mut receptor_fields = self.group_fields("Receptors")?;
        self.settings.receptors.write(&mut receptor_fields);
        root.insert("Receptors".into(), Value::Object(receptor_fields));
        let mut density_fields = self.group_fields("DensityGraph")?;
        self.settings.density.write(&mut density_fields)?;
        root.insert("DensityGraph".into(), Value::Object(density_fields));
        self.settings.snap.write(&mut root);
        root.insert("MiniMap".into(), Value::Object(minimap_fields.clone()));
        root.insert("Scroll".into(), Value::Object(scroll_fields.clone()));
        root.insert("Zoom".into(), Value::Object(zoom_fields.clone()));
        root.insert("Waveform".into(), Value::Object(waveform_fields.clone()));
        root.insert("Audio".into(), Value::Object(fields.clone()));
        Ok(root)
    }
    pub fn pending_publication(&self) -> Option<durability::PendingPublication> {
        durability::for_key(&self.durability_key)
    }
    pub fn retry_pending_durability(&self) -> Result<(), Error> {
        durability::retry_path(&self.path)
    }
    /// Success means publication succeeded. Directory-sync failures retain an
    /// inspectable receipt/warning and must never be treated as a rollback.
    pub fn save(&mut self) -> Result<bool, Error> {
        durability::retry_path(&self.path)?;
        if !self.has_changes() {
            if read_bytes(&self.path)?.as_deref() != self.expected.as_ref().map(|b| b.as_slice()) {
                return Err(Error::ExternalChange);
            }
            return Ok(false);
        }
        let parent = self
            .path
            .parent()
            .ok_or_else(|| Error::InvalidEdit("Preferences path has no parent".into()))?;
        fs::create_dir_all(parent)?;
        let lock_path = self.path.with_extension("json.lock");
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)?;
        lock.try_lock().map_err(std::io::Error::other)?;
        // Recheck after acquiring the file lock, closing the race with another
        // File publishing between the initial barrier and this lock acquisition.
        self.durability_key = durability::key(&self.path);
        if let Some(publication) = durability::for_key(&self.durability_key) {
            publication.receipt.retry_locked()?;
        }
        if read_bytes(&self.path)?.as_deref() != self.expected.as_ref().map(|b| b.as_slice()) {
            return Err(Error::ExternalChange);
        }
        let root = self.projected_root()?;
        let mut bytes = serde_json::to_vec_pretty(&root)
            .map_err(|error| Error::InvalidFormat(error.to_string()))?;
        bytes.push(b'\n');
        let mut pending = tempfile::NamedTempFile::new_in(parent)?;
        if self.expected.is_some() {
            pending
                .as_file()
                .set_permissions(fs::metadata(&self.path)?.permissions())?;
        }
        pending.write_all(&bytes)?;
        pending.as_file().sync_all()?;
        if read_bytes(&self.path)?.as_deref() != self.expected.as_ref().map(|b| b.as_slice()) {
            return Err(Error::ExternalChange);
        }
        pending
            .persist(&self.path)
            .map_err(|error| Error::Io(error.error))?;
        self.expected = Some(std::sync::Arc::new(bytes));
        self.written = self.settings.clone();
        self.audio_fields = root["Audio"].as_object().expect("projected group").clone();
        self.waveform_fields = root["Waveform"]
            .as_object()
            .expect("projected group")
            .clone();
        self.zoom_fields = root["Zoom"].as_object().expect("projected group").clone();
        self.scroll_fields = root["Scroll"].as_object().expect("projected group").clone();
        self.minimap_fields = root["MiniMap"]
            .as_object()
            .expect("projected group")
            .clone();
        let mut root = root;
        root.remove("GrooveAuthorSourceArchive");
        self.root = std::sync::Arc::new(root);
        self.original = self.settings.clone();
        self.durability_key = durability::key(&self.path);
        if let Err(error) = durability::sync(parent) {
            durability::retain(
                &self.path,
                self.expected.as_ref().expect("published bytes").clone(),
                &error,
            );
        }
        Ok(true)
    }
}

impl File {
    pub fn display_options(&self) -> crate::options_preferences::Display {
        self.settings.display_options
    }
    pub fn set_display_options(&mut self, value: crate::options_preferences::Display) {
        self.settings.display_options = value;
    }
}

impl File {
    pub fn log_preferences(&self) -> crate::log_preferences::Preferences {
        self.settings.log_preferences
    }
    pub fn set_log_preferences(&mut self, value: crate::log_preferences::Preferences) {
        self.settings.log_preferences = value;
    }
}

impl File {
    pub fn ftue(&self) -> &crate::ftue::Preferences {
        &self.settings.ftue
    }
    pub fn set_ftue(&mut self, value: crate::ftue::Preferences) {
        self.settings.ftue = value;
    }
}
pub mod source_import;

impl File {
    /// Publish the full projected root as a memory baseline. No filesystem path
    /// is invented, and explicit/default/unknown fields survive later history.
    pub(crate) fn commit_memory_preferences(&mut self) -> Result<(), Error> {
        debug_assert!(self.path.as_os_str().is_empty());
        *self = Self::from_root(&self.path, None, self.projected_root()?)?;
        Ok(())
    }
}
