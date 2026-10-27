//! Read-only, bounded pack discovery. Cultural ordering is supplied by the shell.
use crate::document::chart_type::canonical as canonical_layout;
use crate::{
    Error,
    document::{
        Format,
        metadata::{self, Metadata as Headers},
    },
    generation::job::{Job, Phase, Progress},
};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Metadata {
    pub title: Option<String>,
    pub title_transliteration: Option<String>,
    pub artist: Option<String>,
    pub credit: Option<String>,
    pub ratings: String,
}
impl Metadata {
    pub fn sort_title(&self) -> &str {
        self.title_transliteration
            .as_deref()
            .filter(|v| !v.is_empty())
            .or(self.title.as_deref())
            .unwrap_or("")
    }
}
pub trait Reader {
    fn read(
        &self,
        bytes: &[u8],
        format: Format,
        limits: metadata::Limits,
    ) -> Result<Metadata, Error>;
}
#[derive(Clone, Copy, Default)]
pub struct HeaderReader;
impl Reader for HeaderReader {
    fn read(
        &self,
        bytes: &[u8],
        format: Format,
        limits: metadata::Limits,
    ) -> Result<Metadata, Error> {
        let headers = Headers::parse_with_limits(Arc::<[u8]>::from(bytes), format, limits)?;
        let mut charts: Vec<_> = headers
            .charts()
            .iter()
            .filter_map(|chart| {
                canonical_layout(&chart.steps_type).map(|kind| {
                    let rating = parse_meter(&chart.meter, format);
                    (kind, rating)
                })
            })
            .collect();
        // The original's difficulty branch queries Type, not DifficultyType.
        // Canonical format layout names never match its difficulty-name table.
        // total_cmp supplies a valid total order even for NaN; source's numeric
        // subtraction comparer is not a valid ordering on mixed NaN cases.
        charts.sort_by(compare_charts);
        let mut ratings = String::new();
        let mut previous = None;
        for (kind, rating) in charts {
            if let Some(previous) = previous {
                ratings.push(if previous == kind { ',' } else { '|' });
            }
            ratings.push_str(&(rating as i32).to_string());
            previous = Some(kind);
        }
        Ok(Metadata {
            title: headers.text("TITLE"),
            title_transliteration: headers.text("TITLETRANSLIT"),
            artist: headers.text("ARTIST"),
            credit: headers.text("CREDIT"),
            ratings,
        })
    }
}
fn parse_meter(text: &str, format: Format) -> f64 {
    let text = text.trim();
    if format == Format::Sm {
        return text.parse::<i32>().map(f64::from).unwrap_or(0.0);
    }
    let unsigned = text
        .strip_prefix('+')
        .or_else(|| text.strip_prefix('-'))
        .unwrap_or(text);
    if unsigned
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphabetic)
    {
        if unsigned.eq_ignore_ascii_case("nan") {
            return f64::NAN;
        }
        if unsigned.eq_ignore_ascii_case("infinity") {
            return if text.starts_with('-') {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            };
        }
        return 0.0;
    }
    if !text.contains(',') {
        return text.parse::<f64>().unwrap_or(0.0);
    }
    // Fumen's invariant Convert.ChangeType<double> uses Float|AllowThousands.
    // Commas are permitted only in the integer portion, after its first digit;
    // grouping widths are not validated by the original numeric parser.
    let end = text.find(['.', 'e', 'E']).unwrap_or(text.len());
    if text[end..].contains(',') {
        return 0.0;
    }
    let prefix = &text[..end];
    let integer = prefix
        .strip_prefix('+')
        .or_else(|| prefix.strip_prefix('-'))
        .unwrap_or(prefix);
    if !integer.as_bytes().first().is_some_and(u8::is_ascii_digit)
        || !integer
            .bytes()
            .all(|value| value.is_ascii_digit() || value == b',')
    {
        return 0.0;
    }
    let mut normalized: String = text[..end].chars().filter(|&value| value != ',').collect();
    normalized.push_str(&text[end..]);
    normalized.parse::<f64>().unwrap_or(0.0)
}

fn compare_charts(a: &(&str, f64), b: &(&str, f64)) -> std::cmp::Ordering {
    layout_rank(a.0)
        .cmp(&layout_rank(b.0))
        .then_with(|| a.1.total_cmp(&b.1))
}

fn layout_rank(kind: &str) -> usize {
    crate::chart_order::TYPE_ORDER
        .iter()
        .position(|v| *v == kind)
        .unwrap_or(usize::MAX)
}
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub directories: usize,
    pub entries: usize,
    pub files_per_directory: usize,
    pub file_bytes: usize,
    pub total_bytes: usize,
    pub metadata_bytes: usize,
    pub charts_per_song: usize,
    pub tags_per_song: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            directories: 16384,
            entries: 262144,
            files_per_directory: 16384,
            file_bytes: 16 * 1024 * 1024,
            total_bytes: 512 * 1024 * 1024,
            metadata_bytes: 32 * 1024 * 1024,
            charts_per_song: 4096,
            tags_per_song: 65536,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IssueKind {
    ReadFailed(String),
    MetadataFailed(String),
    FileTooLarge,
    Ambiguous { candidates: usize },
    DuplicateTarget,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    pub path: PathBuf,
    pub kind: IssueKind,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Identity {
    #[cfg(unix)]
    dev: u64,
    #[cfg(unix)]
    ino: u64,
    #[cfg(not(unix))]
    created: Option<std::time::SystemTime>,
}
impl Identity {
    fn read(metadata: &fs::Metadata) -> Self {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Self {
                dev: metadata.dev(),
                ino: metadata.ino(),
            }
        }
        #[cfg(not(unix))]
        {
            Self {
                created: metadata.created().ok(),
            }
        }
    }
}
#[derive(Clone, Debug)]
pub struct Row {
    pub path: PathBuf,
    pub logical_path: PathBuf,
    pub folder: String,
    pub metadata: Metadata,
    identity: Identity,
    length: u64,
    modified: Option<std::time::SystemTime>,
}
impl Row {
    /// Read-only guard before a later explicit Open operation. Open still uses
    /// the normal strict editor loader, never the header-only metadata object.
    pub fn is_current(&self) -> Result<bool, Error> {
        let path = match self.logical_path.canonicalize() {
            Ok(path) => path,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(e.into()),
        };
        if path != self.path {
            return Ok(false);
        }
        let metadata = fs::metadata(path)?;
        Ok(metadata.is_file()
            && Identity::read(&metadata) == self.identity
            && metadata.len() == self.length
            && metadata.modified().ok() == self.modified)
    }
}
#[derive(Debug)]
pub struct Catalog {
    pub rows: Vec<Row>,
    pub issues: Vec<Issue>,
    pub source: PathBuf,
    pub root: PathBuf,
}
pub trait Orderer {
    fn order(&mut self, keys: &[OrderKey]) -> Result<Vec<usize>, Error>;
}
#[derive(Clone, Debug)]
pub struct OrderKey {
    pub title: String,
    pub folder: String,
    pub path: PathBuf,
}
impl Catalog {
    pub fn ordered(&self, orderer: &mut impl Orderer) -> Result<Vec<usize>, Error> {
        let keys: Vec<_> = self
            .rows
            .iter()
            .map(|row| OrderKey {
                title: row.metadata.sort_title().into(),
                folder: row.folder.clone(),
                path: row.path.clone(),
            })
            .collect();
        let order = orderer.order(&keys)?;
        if order.len() != keys.len() {
            return Err(Error::InvalidEdit(
                "Pack order is not a complete permutation".into(),
            ));
        }
        let mut seen = vec![false; keys.len()];
        for &index in &order {
            let Some(value) = seen.get_mut(index) else {
                return Err(Error::InvalidEdit(
                    "Pack order contains an invalid source index".into(),
                ));
            };
            if std::mem::replace(value, true) {
                return Err(Error::InvalidEdit(
                    "Pack order repeats a source index".into(),
                ));
            }
        }
        Ok(order)
    }
}
#[derive(Default)]
pub struct Selection {
    source: Option<PathBuf>,
    position: usize,
}
impl Selection {
    pub fn reconcile(&mut self, catalog: &Catalog, order: &[usize]) {
        if let Some(index) = self.source.as_ref().and_then(|source| {
            order.iter().position(|&index| {
                catalog
                    .rows
                    .get(index)
                    .is_some_and(|row| &row.path == source)
            })
        }) {
            self.position = index;
        } else {
            self.position = self.position.min(order.len().saturating_sub(1));
        }
        self.source = order
            .get(self.position)
            .and_then(|&index| catalog.rows.get(index))
            .map(|row| row.path.clone());
    }
    pub fn move_by(&mut self, delta: isize, catalog: &Catalog, order: &[usize]) {
        self.position = self
            .position
            .saturating_add_signed(delta)
            .min(order.len().saturating_sub(1));
        self.source = order
            .get(self.position)
            .and_then(|&index| catalog.rows.get(index))
            .map(|row| row.path.clone());
    }
    pub fn position(&self) -> usize {
        self.position
    }
    pub fn selected(&self) -> Option<&Path> {
        self.source.as_deref()
    }
}
pub struct Task {
    source: PathBuf,
    root: PathBuf,
    root_identity: Identity,
    limits: Limits,
}
impl Task {
    pub fn from_song_file(path: &Path, limits: Limits) -> Result<Self, Error> {
        let source = path.canonicalize()?;
        if !fs::metadata(&source)?.is_file() {
            return Err(Error::InvalidEdit("Source song is not a file".into()));
        }
        let root = source
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| Error::InvalidEdit("Source song has no pack parent".into()))?
            .to_path_buf();
        let root_identity = Identity::read(&fs::metadata(&root)?);
        Ok(Self {
            source,
            root,
            root_identity,
            limits,
        })
    }
    pub fn spawn(self) -> Result<Job<Catalog>, Error> {
        let reader = HeaderReader;
        Job::spawn_with("editor-pack-song-list", move |cancelled, progress| {
            self.run(&reader, cancelled, progress)
        })
    }
    pub fn run(
        self,
        reader: &impl Reader,
        cancelled: &AtomicBool,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<Catalog, Error> {
        check(cancelled)?;
        let mut directories = Vec::new();
        let mut entries = 0usize;
        for entry in fs::read_dir(&self.root)? {
            check(cancelled)?;
            let entry = entry?;
            bump(
                &mut entries,
                self.limits.entries,
                "Pack entry limit exceeded",
            )?;
            if entry.path().is_dir() {
                if directories.len() >= self.limits.directories {
                    return limit("Pack directory limit exceeded");
                };
                directories.push(entry.path());
            }
        }
        directories.sort();
        let total = directories.len();
        let mut rows = Vec::new();
        let mut issues = Vec::new();
        let mut targets = BTreeSet::new();
        let mut total_bytes = 0usize;
        let mut metadata_bytes = 0usize;
        for (position, directory) in directories.into_iter().enumerate() {
            check(cancelled)?;
            let mut candidates = Vec::new();
            let files = match fs::read_dir(&directory) {
                Ok(files) => files,
                Err(e) => {
                    issues.push(Issue {
                        path: directory,
                        kind: IssueKind::ReadFailed(e.to_string()),
                    });
                    continue;
                }
            };
            let mut file_count = 0usize;
            for entry in files {
                check(cancelled)?;
                bump(
                    &mut entries,
                    self.limits.entries,
                    "Pack entry limit exceeded",
                )?;
                bump(
                    &mut file_count,
                    self.limits.files_per_directory,
                    "Song directory entry limit exceeded",
                )?;
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(e) => {
                        issues.push(Issue {
                            path: directory.clone(),
                            kind: IssueKind::ReadFailed(e.to_string()),
                        });
                        continue;
                    }
                };
                let path = entry.path();
                if path.is_file()
                    && matches!(
                        path.extension().and_then(|v| v.to_str()),
                        Some("sm" | "ssc")
                    )
                {
                    candidates.push(path);
                }
            }
            candidates.sort();
            let ssc: Vec<_> = candidates
                .iter()
                .filter(|p| p.extension().is_some_and(|v| v == "ssc"))
                .collect();
            let same = if ssc.is_empty() {
                candidates
                    .iter()
                    .filter(|p| p.extension().is_some_and(|v| v == "sm"))
                    .collect::<Vec<_>>()
            } else {
                ssc
            };
            let Some(selected) = same.last() else {
                continue;
            };
            let logical_path = (*selected).clone();
            if same.len() > 1 {
                issues.push(Issue {
                    path: directory.clone(),
                    kind: IssueKind::Ambiguous {
                        candidates: same.len(),
                    },
                });
            }
            let path = match logical_path.canonicalize() {
                Ok(path) => path,
                Err(e) => {
                    issues.push(Issue {
                        path: logical_path,
                        kind: IssueKind::ReadFailed(e.to_string()),
                    });
                    continue;
                }
            };
            if !targets.insert(path.clone()) {
                issues.push(Issue {
                    path: logical_path,
                    kind: IssueKind::DuplicateTarget,
                });
                continue;
            }
            let mut file = match File::open(&path) {
                Ok(file) => file,
                Err(e) => {
                    issues.push(Issue {
                        path,
                        kind: IssueKind::ReadFailed(e.to_string()),
                    });
                    continue;
                }
            };
            let before = file.metadata()?;
            if before.len() > self.limits.file_bytes as u64 {
                issues.push(Issue {
                    path,
                    kind: IssueKind::FileTooLarge,
                });
                continue;
            }
            let bytes = match read_chunks(&mut file, &mut total_bytes, self.limits, cancelled) {
                Ok(Some(bytes)) => bytes,
                Ok(None) => {
                    issues.push(Issue {
                        path,
                        kind: IssueKind::FileTooLarge,
                    });
                    continue;
                }
                Err(Error::Io(error)) => {
                    issues.push(Issue {
                        path,
                        kind: IssueKind::ReadFailed(error.to_string()),
                    });
                    continue;
                }
                Err(error) => return Err(error),
            };
            check(cancelled)?;
            let format = if logical_path.extension().is_some_and(|v| v == "ssc") {
                Format::Ssc
            } else {
                Format::Sm
            };
            let metadata = match reader.read(
                &bytes,
                format,
                metadata::Limits {
                    source_bytes: self.limits.file_bytes,
                    charts: self.limits.charts_per_song,
                    tags: self.limits.tags_per_song,
                },
            ) {
                Ok(metadata) => metadata,
                Err(e) => {
                    issues.push(Issue {
                        path,
                        kind: IssueKind::MetadataFailed(e.to_string()),
                    });
                    continue;
                }
            };
            check(cancelled)?;
            let after = fs::metadata(&path)?;
            if Identity::read(&before) != Identity::read(&after)
                || before.len() != after.len()
                || before.modified().ok() != after.modified().ok()
            {
                return Err(Error::ExternalChange);
            }
            for value in [
                &metadata.title,
                &metadata.title_transliteration,
                &metadata.artist,
                &metadata.credit,
            ]
            .into_iter()
            .flatten()
            {
                metadata_bytes = metadata_bytes
                    .checked_add(value.len())
                    .ok_or_else(|| Error::Unsupported("Pack metadata size overflow".into()))?;
            }
            metadata_bytes = metadata_bytes
                .checked_add(metadata.ratings.len())
                .ok_or_else(|| Error::Unsupported("Pack metadata size overflow".into()))?;
            if metadata_bytes > self.limits.metadata_bytes {
                return limit("Pack metadata size limit exceeded");
            };
            rows.push(Row {
                path,
                logical_path,
                folder: directory
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                metadata,
                identity: Identity::read(&after),
                length: after.len(),
                modified: after.modified().ok(),
            });
            progress(Progress {
                phase: Phase::Preparing,
                completed: position + 1,
                total,
            });
        }
        check(cancelled)?;
        let root = fs::metadata(&self.root)?;
        if !root.is_dir() || Identity::read(&root) != self.root_identity || !self.source.is_file() {
            return Err(Error::ExternalChange);
        }
        progress(Progress {
            phase: Phase::Ready,
            completed: total,
            total,
        });
        check(cancelled)?;
        Ok(Catalog {
            rows,
            issues,
            source: self.source,
            root: self.root,
        })
    }
}
fn read_chunks(
    reader: &mut impl Read,
    total_bytes: &mut usize,
    limits: Limits,
    cancelled: &AtomicBool,
) -> Result<Option<Vec<u8>>, Error> {
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 32768];
    loop {
        check(cancelled)?;
        let length = reader.read(&mut chunk)?;
        if length == 0 {
            break;
        }
        *total_bytes = total_bytes
            .checked_add(length)
            .ok_or_else(|| Error::Unsupported("Pack byte count overflow".into()))?;
        if *total_bytes > limits.total_bytes {
            return limit("Pack total byte limit exceeded");
        }
        if bytes.len().saturating_add(length) > limits.file_bytes {
            return Ok(None);
        }
        bytes.extend_from_slice(&chunk[..length]);
    }
    check(cancelled)?;
    Ok(Some(bytes))
}

fn check(cancelled: &AtomicBool) -> Result<(), Error> {
    if cancelled.load(Ordering::Relaxed) {
        Err(Error::InvalidEdit("Pack list discovery cancelled".into()))
    } else {
        Ok(())
    }
}
fn limit<T>(message: &str) -> Result<T, Error> {
    Err(Error::Unsupported(message.into()))
}
fn bump(value: &mut usize, maximum: usize, message: &str) -> Result<(), Error> {
    *value = value
        .checked_add(1)
        .ok_or_else(|| Error::Unsupported(message.into()))?;
    if *value > maximum {
        limit(message)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chunked_read_checks_cancellation_after_each_chunk_and_refuses_growth() {
        struct Cancelling<'a>(&'a AtomicBool);
        impl Read for Cancelling<'_> {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                buffer[..3].copy_from_slice(b"abc");
                self.0.store(true, Ordering::Relaxed);
                Ok(3)
            }
        }
        let cancelled = AtomicBool::new(false);
        assert!(
            read_chunks(
                &mut Cancelling(&cancelled),
                &mut 0,
                Limits::default(),
                &cancelled
            )
            .unwrap_err()
            .to_string()
            .contains("cancelled")
        );
        let cancelled = AtomicBool::new(false);
        let limits = Limits {
            file_bytes: 2,
            ..Limits::default()
        };
        assert!(
            read_chunks(&mut &b"abc"[..], &mut 0, limits, &cancelled)
                .unwrap()
                .is_none()
        );
        let limits = Limits {
            total_bytes: 2,
            ..Limits::default()
        };
        assert!(matches!(
            read_chunks(&mut &b"abc"[..], &mut 0, limits, &cancelled),
            Err(Error::Unsupported(_))
        ));
    }
    #[test]
    fn numeric_meter_grammar_matches_actual_original_sm_ssc_reader_values() {
        fn decode(s: &str) -> Vec<u8> {
            let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let (mut bits, mut count, mut out) = (0u32, 0, Vec::new());
            for byte in s.bytes().take_while(|value| *value != b'=') {
                bits =
                    (bits << 6) | alphabet.iter().position(|value| *value == byte).unwrap() as u32;
                count += 6;
                if count >= 8 {
                    count -= 8;
                    out.push((bits >> count) as u8);
                    bits &= (1 << count) - 1;
                }
            }
            out
        }
        let request: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/grooveauthor/pack-songs.request.json"
        ))
        .unwrap();
        let expected: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/grooveauthor/pack-songs.expected.json"
        ))
        .unwrap();
        let mut compared = 0;
        for (input, output) in request["Songs"]
            .as_array()
            .unwrap()
            .iter()
            .zip(expected["Songs"].as_array().unwrap())
        {
            let name = input["Name"].as_str().unwrap();
            if !name.starts_with("grammar-") && !name.starts_with("rating-") {
                continue;
            }
            let file = &input["Files"][0];
            let format = if file["Name"].as_str().unwrap().ends_with(".ssc") {
                Format::Ssc
            } else {
                Format::Sm
            };
            let headers = Headers::parse(decode(file["Bytes"].as_str().unwrap()), format).unwrap();
            let actual = parse_meter(&headers.charts()[0].meter, format);
            let source = &output["Charts"][0]["Rating"];
            let expected = source
                .as_f64()
                .unwrap_or_else(|| source.as_str().unwrap().parse::<f64>().unwrap());
            assert!(
                actual == expected || (actual.is_nan() && expected.is_nan()),
                "{name}: {actual} != {expected}"
            );
            compared += 1;
        }
        assert_eq!(compared, 96);
    }
    #[test]
    fn original_finite_order_matches_and_pathological_comparer_is_explicitly_adapted() {
        let expected: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/grooveauthor/pack-songs.expected.json"
        ))
        .unwrap();
        for (name, values, native_indices) in [
            (
                "mixed-decimal-finite",
                vec![9.9, 9.1, 9.2, 9.05, 9.95],
                vec![3, 1, 2, 0, 4],
            ),
            (
                "mixed-nan-infinity-zero",
                vec![
                    f64::NAN,
                    0.0,
                    f64::INFINITY,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    -0.0,
                    1.25,
                    1.5,
                ],
                vec![4, 5, 1, 6, 7, 2, 3, 0],
            ),
        ] {
            let source = expected["Songs"]
                .as_array()
                .unwrap()
                .iter()
                .find(|song| song["Name"] == name)
                .unwrap();
            let mut indices: Vec<_> = (0..values.len()).collect();
            indices.sort_by(|&a, &b| {
                compare_charts(&("dance-single", values[a]), &("dance-single", values[b]))
            });
            assert_eq!(indices, native_indices);
            if name == "mixed-decimal-finite" {
                assert_eq!(serde_json::json!(indices), source["SortedIndices"]);
            } else {
                // The original returns -1 for NaN/NaN and infinity/infinity,
                // and regards signed zeros as equal. It cannot back sort_by.
                assert_eq!(source["Comparisons"][0][0], -1);
                assert_eq!(source["Comparisons"][2][2], -1);
                assert_eq!(source["Comparisons"][1][5], 0);
                assert_ne!(serde_json::json!(indices), source["SortedIndices"]);
                for value in &values {
                    assert_eq!(
                        compare_charts(&("dance-single", *value), &("dance-single", *value)),
                        std::cmp::Ordering::Equal
                    );
                }
                assert_eq!(
                    compare_charts(&("dance-single", -0.0), &("dance-single", 0.0)),
                    std::cmp::Ordering::Less
                );
            }
        }
    }
}
