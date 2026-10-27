//! Shared editor application preferences, stored independently of simfiles.
//! Audio, waveform and zoom groups share conflict-safe atomic persistence.
pub(crate) mod history;
use crate::{Error, waveform_preferences::Preferences as Waveform};
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
}
impl Default for Audio {
    fn default() -> Self {
        Self {
            fade_in: 0.0,
            fade_out: 1.5,
            music_rate: 1.0,
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
            tenths += 1;
        } else if (tenths as f64).to_bits().abs_diff(scaled.to_bits()) <= 1 {
            tenths -= 1;
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
    note_colors: std::sync::Arc<crate::note_color_preferences::Preferences>,
    receptors: crate::receptor_preferences::Preferences,
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
    expected: Option<Vec<u8>>,
    root: Map<String, Value>,
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
    pub fn open(path: &Path) -> Result<Self, Error> {
        let expected = read_bytes(path)?;
        let mut root = if let Some(bytes) = &expected {
            serde_json::from_slice::<Value>(bytes)
                .map_err(|error| Error::InvalidFormat(error.to_string()))?
                .as_object()
                .cloned()
                .ok_or_else(|| {
                    Error::InvalidFormat("Editor preferences must be an object".into())
                })?
        } else {
            Map::new()
        };
        if expected.is_some() && root.get("Version").and_then(Value::as_u64) != Some(1) {
            return Err(Error::Unsupported(
                "Unknown editor preferences version".into(),
            ));
        }
        root.insert("Version".into(), Value::from(1));
        let audio_fields = match root.get("Audio") {
            None => Map::new(),
            Some(Value::Object(fields)) => fields.clone(),
            Some(_) => {
                return Err(Error::InvalidFormat(
                    "Editor audio preferences must be an object".into(),
                ));
            }
        };
        let mut audio = Audio::default();
        if let Some(value) = audio_fields.get("MusicRate") {
            audio.set_music_rate(
                value
                    .as_f64()
                    .ok_or_else(|| Error::InvalidFormat("Invalid MusicRate".into()))?,
            )?;
        }
        for (name, target) in [
            ("PreviewFadeInTime", &mut audio.fade_in),
            ("PreviewFadeOutTime", &mut audio.fade_out),
        ] {
            if let Some(value) = audio_fields.get(name) {
                *target = valid(
                    value
                        .as_f64()
                        .ok_or_else(|| Error::InvalidFormat(format!("Invalid {name}")))?,
                )?;
            }
        }
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
        let note_colors = std::sync::Arc::new(crate::note_color_preferences::Preferences::read(
            &colors_group("NoteColor")?,
            &options_fields,
        )?);
        let settings = Settings {
            note_colors,
            receptors: crate::receptor_preferences::Preferences::read(&receptor_fields)?,
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
            expected,
            root,
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
    pub fn snap(&self) -> crate::snap::Preferences {
        self.settings.snap
    }
    pub fn has_snap_preferences(&self) -> bool {
        self.root.contains_key("SnapIndex") || self.root.contains_key("SnapLockIndex")
    }
    pub fn set_snap(&mut self, snap: crate::snap::Preferences) {
        self.settings.snap = snap;
    }
    pub fn receptors(&self) -> crate::receptor_preferences::Preferences {
        self.settings.receptors
    }
    pub fn set_receptors(&mut self, p: crate::receptor_preferences::Preferences) {
        self.settings.receptors = p;
    }
    /// Shared Options group; no independent preference file or implicit write.
    pub fn opening(&self) -> Result<crate::open_preferences::Preferences, Error> {
        crate::open_preferences::Preferences::read(&self.group_fields("Options")?)
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
    pub fn has_changes(&self) -> bool {
        self.settings != self.original || self.settings != self.written
    }
    pub fn save(&mut self) -> Result<bool, Error> {
        if !self.has_changes() {
            if read_bytes(&self.path)? != self.expected {
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
        if read_bytes(&self.path)? != self.expected {
            return Err(Error::ExternalChange);
        }
        let mut fields = self.audio_fields.clone();
        fields.insert(
            "MusicRate".into(),
            Value::from(self.settings.audio.music_rate),
        );
        fields.insert(
            "PreviewFadeInTime".into(),
            Value::from(self.settings.audio.fade_in),
        );
        fields.insert(
            "PreviewFadeOutTime".into(),
            Value::from(self.settings.audio.fade_out),
        );
        let mut waveform_fields = self.waveform_fields.clone();
        self.settings.waveform.write(&mut waveform_fields);
        let mut zoom_fields = self.zoom_fields.clone();
        self.settings.zoom.write(&mut zoom_fields);
        let mut scroll_fields = self.scroll_fields.clone();
        self.settings.scroll.write(&mut scroll_fields);
        let mut minimap_fields = self.minimap_fields.clone();
        self.settings.minimap.write(&mut minimap_fields);
        self.settings.minimap_position.write(&mut minimap_fields);
        let mut options_fields = self.group_fields("Options")?;
        self.settings
            .minimap_content
            .write(&mut minimap_fields, &mut options_fields)?;
        let mut root = self.root.clone();
        let mut note_fields = self.group_fields("NoteColor")?;
        self.settings
            .note_colors
            .write(&mut note_fields, &mut options_fields)?;
        root.insert("NoteColor".into(), Value::Object(note_fields));
        root.insert("Options".into(), Value::Object(options_fields));
        let mut receptor_fields = self.group_fields("Receptors")?;
        self.settings.receptors.write(&mut receptor_fields);
        root.insert("Receptors".into(), Value::Object(receptor_fields));
        self.settings.snap.write(&mut root);
        root.insert("MiniMap".into(), Value::Object(minimap_fields.clone()));
        root.insert("Scroll".into(), Value::Object(scroll_fields.clone()));
        root.insert("Zoom".into(), Value::Object(zoom_fields.clone()));
        root.insert("Waveform".into(), Value::Object(waveform_fields.clone()));
        root.insert("Audio".into(), Value::Object(fields.clone()));
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
        if read_bytes(&self.path)? != self.expected {
            return Err(Error::ExternalChange);
        }
        pending
            .persist(&self.path)
            .map_err(|error| Error::Io(error.error))?;
        self.expected = Some(bytes);
        self.written = self.settings.clone();
        self.root = root;
        self.audio_fields = fields;
        self.waveform_fields = waveform_fields;
        self.zoom_fields = zoom_fields;
        self.scroll_fields = scroll_fields;
        self.minimap_fields = minimap_fields;
        crate::storage::sync_directory(parent)?;
        self.original = self.settings.clone();
        Ok(true)
    }
}
