//! Pack metadata model. Filesystem publication is deliberately owned by the caller.
pub mod storage;
pub mod song_list;
mod codec;
pub use codec::Document;
use std::sync::Arc;
pub const MAX_BYTES: usize = 4 * 1024 * 1024;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Data {
    pub title: Option<String>,
    pub title_transliteration: Option<String>,
    pub title_sort: Option<String>,
    pub series: Option<String>,
    pub year: i32,
    pub banner: Option<String>,
    /// Original enum values are not restricted to its two named variants.
    pub sync_offset: i32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(pub String);
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}
impl Data {
    pub fn new(
        folder: &str,
        banner: &str,
        utc_year: i32,
        active_sync: Option<f64>,
        preferred_sync: f64,
    ) -> Self {
        let known = |v: f64| {
            if crate::number::reference_equal(v, 0.0) {
                Some(0)
            } else if crate::number::reference_equal(v, 0.009) {
                Some(1)
            } else {
                None
            }
        };
        Self {
            title: Some(folder.into()),
            title_transliteration: Some(String::new()),
            title_sort: Some(folder.into()),
            series: Some(String::new()),
            year: utc_year,
            banner: Some(banner.into()),
            sync_offset: active_sync
                .and_then(known)
                .or_else(|| known(preferred_sync))
                .unwrap_or(1),
        }
    }
    pub(crate) fn values(&self) -> [Option<String>; 7] {
        [
            self.title.clone(),
            self.title_transliteration.clone(),
            self.title_sort.clone(),
            self.series.clone(),
            Some(self.year.to_string()),
            self.banner.clone(),
            Some(sync_name(self.sync_offset)),
        ]
    }
    pub fn canonical(&self) -> Result<Vec<u8>, Error> {
        validate(self)?;
        let mut s = String::from("[Group]\nVersion=1\n");
        for (key, value) in codec::KEYS.iter().zip(self.values()) {
            s.push_str(key);
            s.push('=');
            s.push_str(value.as_deref().unwrap_or(""));
            s.push('\n');
        }
        bounded(s.into_bytes())
    }
}
fn sync_name(value: i32) -> String {
    match value {
        0 => "NULL".into(),
        1 => "ITG".into(),
        v => v.to_string(),
    }
}
fn validate(data: &Data) -> Result<(), Error> {
    if data
        .values()
        .iter()
        .flatten()
        .any(|v| v.contains(['\r', '\n', '\0']))
    {
        return Err(Error("Pack values must occupy one line without NUL".into()));
    }
    Ok(())
}
fn bounded(bytes: Vec<u8>) -> Result<Vec<u8>, Error> {
    if bytes.len() > MAX_BYTES {
        Err(Error("Pack.ini exceeds the metadata size limit".into()))
    } else {
        Ok(bytes)
    }
}
/// A prepared snapshot does not certify that a file was written.
pub struct Prepared {
    identity: Arc<()>,
    generation: u64,
    bytes: Arc<[u8]>,
    baseline: Option<Arc<[u8]>>,
    snapshot: Data,
}
impl Prepared {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn baseline(&self) -> Option<&[u8]> {
        self.baseline.as_deref()
    }
}
pub struct Draft {
    document: Document,
    data: Data,
    exists: bool,
    identity: Arc<()>,
    generation: u64,
}
impl Draft {
    pub fn existing(document: Document) -> Self {
        Self {
            data: document.data().clone(),
            document,
            exists: true,
            identity: Arc::new(()),
            generation: 0,
        }
    }
    pub fn create(data: Data) -> Result<Self, Error> {
        let mut s = Self::existing(Document::parse(data.canonical()?)?);
        s.exists = false;
        Ok(s)
    }
    pub fn data(&self) -> &Data {
        &self.data
    }
    pub fn replace(&mut self, data: Data) -> Result<bool, Error> {
        validate(&data)?;
        let changed = self.data != data;
        self.data = data;
        Ok(changed)
    }
    /// Interactive year editing follows the original nonnegative property control.
    pub fn set_year(&mut self, year: i32) -> Result<bool, Error> {
        if year < 0 {
            return Err(Error("Pack year must be nonnegative".into()));
        }
        let mut next = self.data.clone();
        next.year = year;
        self.replace(next)
    }
    /// Unknown numeric enum values are retained on load, but are not UI choices.
    pub fn set_sync_offset(&mut self, value: i32) -> Result<bool, Error> {
        if value != 0 && value != 1 {
            return Err(Error("Unknown pack sync choice".into()));
        }
        let mut next = self.data.clone();
        next.sync_offset = value;
        self.replace(next)
    }
    pub fn has_unsaved_changes(&self) -> bool {
        !self.exists || self.data != *self.document.data()
    }
    pub fn prepare(&self) -> Result<Prepared, Error> {
        Ok(Prepared {
            identity: self.identity.clone(),
            generation: self.generation,
            bytes: self.document.render(&self.data)?.into(),
            baseline: self.exists.then(|| self.document.bytes.clone()),
            snapshot: self.data.clone(),
        })
    }
    /// Call only after publication of this exact prepared snapshot succeeds.
    pub fn acknowledge_saved(&mut self, prepared: &Prepared) -> Result<(), Error> {
        if !Arc::ptr_eq(&self.identity, &prepared.identity)
            || self.generation != prepared.generation
        {
            return Err(Error("Foreign or obsolete pack save receipt".into()));
        }
        let document = Document::parse(prepared.bytes.to_vec())?;
        if document.data() != &prepared.snapshot {
            return Err(Error("Saved pack snapshot has different metadata".into()));
        }
        self.document = document;
        self.exists = true;
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| Error("Pack save generation overflow".into()))?;
        Ok(())
    }
}
