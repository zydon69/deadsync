//! Editor audio preferences, stored independently of simfiles.
use crate::Error;
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
}
impl Default for Audio {
    fn default() -> Self {
        Self {
            fade_in: 0.0,
            fade_out: 1.5,
        }
    }
}
impl Audio {
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
pub struct File {
    path: PathBuf,
    expected: Option<Vec<u8>>,
    root: Map<String, Value>,
    audio_fields: Map<String, Value>,
    original: Audio,
    written: Audio,
    audio: Audio,
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
        Ok(Self {
            path: path.to_owned(),
            expected,
            root,
            audio_fields,
            original: audio,
            written: audio,
            audio,
        })
    }
    pub fn audio(&self) -> Audio {
        self.audio
    }
    pub fn set_audio(&mut self, audio: Audio) {
        self.audio = audio;
    }
    pub fn has_changes(&self) -> bool {
        self.audio != self.original || self.audio != self.written
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
        fields.insert("PreviewFadeInTime".into(), Value::from(self.audio.fade_in));
        fields.insert(
            "PreviewFadeOutTime".into(),
            Value::from(self.audio.fade_out),
        );
        let mut root = self.root.clone();
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
        self.written = self.audio;
        self.root = root;
        self.audio_fields = fields;
        crate::storage::sync_directory(parent)?;
        self.original = self.audio;
        Ok(true)
    }
}
