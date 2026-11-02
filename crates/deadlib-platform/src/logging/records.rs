//! Additive bounded observation of records accepted by the application's logger.
//! Source GrooveAuthor has exactly Info/Warn/Error. Debug/Trace remain in the
//! existing sinks; this viewer never changes global emission/module filters.
use chrono::{DateTime, Local};
use std::{
    collections::VecDeque,
    fmt::{self, Write},
    path::Path,
    sync::{Arc, Mutex, OnceLock},
};
pub const MAX_RECORDS: usize = 1024;
/// Native safety adaptation: preserve a valid UTF-8 prefix and explicit marker.
pub const MAX_MESSAGE_BYTES: usize = 16 * 1024;
const TRUNCATED: &str = " […]";
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Info,
    Warn,
    Error,
}
pub type Time = DateTime<Local>;
#[derive(Clone, Debug)]
pub struct Record {
    pub time: Time,
    pub level: Level,
    pub message: Arc<str>,
}
#[derive(Clone, Debug)]
pub struct Revision(Arc<()>);
impl PartialEq for Revision {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for Revision {}
#[derive(Clone)]
pub struct Snapshot {
    pub revision: Revision,
    pub records: Vec<Record>,
}
struct Bounded {
    value: String,
    truncated: bool,
}
impl Bounded {
    fn new() -> Self {
        Self {
            value: String::new(),
            truncated: false,
        }
    }
    fn finish(mut self) -> Arc<str> {
        if self.truncated {
            self.value.push_str(TRUNCATED);
        }
        self.value.into()
    }
}
impl Bounded {
    fn append(&mut self, s: &str) {
        if self.truncated {
            return;
        }
        let remaining = (MAX_MESSAGE_BYTES - TRUNCATED.len()).saturating_sub(self.value.len());
        let mut end = remaining.min(s.len());
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        self.value.push_str(&s[..end]);
        self.truncated |= end < s.len();
    }
}
impl Write for Bounded {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for (index, part) in s.split('\0').enumerate() {
            if index > 0 {
                self.append("\\0");
            }
            self.append(part);
            if self.truncated {
                break;
            }
        }
        Ok(())
    }
}
struct Ring {
    revision: Revision,
    records: VecDeque<Record>,
}
impl Default for Ring {
    fn default() -> Self {
        Self {
            revision: Revision(Arc::new(())),
            records: VecDeque::new(),
        }
    }
}
impl Ring {
    fn push(&mut self, record: Record) {
        if self.records.len() == MAX_RECORDS {
            self.records.pop_back();
        }
        self.records.push_front(record);
        self.revision = Revision(Arc::new(()));
    }
    #[cfg(test)]
    fn append(&mut self, time: DateTime<Local>, record: &log::Record<'_>, path: Option<&Path>) {
        if let Some(record) = prepare(time, record, path) {
            self.push(record);
        }
    }
    fn clear(&mut self) {
        self.records.clear();
        self.revision = Revision(Arc::new(()));
    }
    fn snapshot(&self, previous: Option<&Revision>) -> Option<Snapshot> {
        if previous == Some(&self.revision) {
            None
        } else {
            Some(Snapshot {
                revision: self.revision.clone(),
                records: self.records.iter().cloned().collect(),
            })
        }
    }
}
fn ring() -> &'static Mutex<Ring> {
    static RING: OnceLock<Mutex<Ring>> = OnceLock::new();
    RING.get_or_init(|| Mutex::new(Ring::default()))
}
// Display implementations can themselves emit logs. Never invoke them while
// holding the memory-buffer mutex: nested accepted records remain observable.
fn prepare(time: DateTime<Local>, record: &log::Record<'_>, path: Option<&Path>) -> Option<Record> {
    let level = match record.level() {
        log::Level::Info => Level::Info,
        log::Level::Warn => Level::Warn,
        log::Level::Error => Level::Error,
        _ => return None,
    };
    let mut text = Bounded::new();
    let _ = write!(&mut text, "{}", record.args());
    if let Some(path) = path {
        let _ = write!(&mut text, " [file: {}]", path.display());
    }
    Some(Record {
        time,
        level,
        message: text.finish(),
    })
}
pub(super) fn capture(record: &log::Record<'_>, path: Option<&Path>) {
    if let Some(record) = prepare(Local::now(), record, path) {
        ring()
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(record);
    }
}
pub fn snapshot_if_changed(previous: Option<&Revision>) -> Option<Snapshot> {
    ring()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .snapshot(previous)
}
/// Clears only the observed memory buffer; never truncates or modifies log files.
pub fn clear() {
    ring().lock().unwrap_or_else(|p| p.into_inner()).clear();
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ring_retains_latest_source_levels_without_changing_global_filters() {
        let mut ring = Ring::default();
        for _ in 0..MAX_RECORDS + 2 {
            ring.append(
                Local::now(),
                &log::Record::builder()
                    .args(format_args!("record"))
                    .level(log::Level::Info)
                    .build(),
                None,
            );
        }
        assert_eq!(ring.records.len(), MAX_RECORDS);
        let revision = ring.revision.clone();
        ring.append(
            Local::now(),
            &log::Record::builder()
                .args(format_args!("debug file only"))
                .level(log::Level::Debug)
                .build(),
            None,
        );
        assert_eq!(ring.revision, revision);
        assert!(ring.snapshot(Some(&revision)).is_none());
        ring.clear();
        assert!(ring.records.is_empty());
        assert!(ring.snapshot(Some(&revision)).is_some());
    }
    #[test]
    fn utf8_messages_are_bounded_and_marked() {
        let mut text = Bounded::new();
        write!(&mut text, "{}", "é😀".repeat(MAX_MESSAGE_BYTES)).unwrap();
        let text = text.finish();
        assert!(text.len() <= MAX_MESSAGE_BYTES);
        assert!(text.ends_with(TRUNCATED));
        assert!(std::str::from_utf8(text.as_bytes()).is_ok());
        let mut small = Bounded::new();
        write!(&mut small, "a\nb").unwrap();
        assert_eq!(&*small.finish(), "a\nb");
    }
}
#[cfg(test)]
mod source_pipeline_tests {
    use super::*;
    #[test]
    fn real_original_queued_logger_order_and_eviction_are_preserved() {
        let mut ring = Ring::default();
        for index in 0..1026 {
            ring.append(
                Local::now(),
                &log::Record::builder()
                    .args(format_args!("i{index:04}"))
                    .level(log::Level::Info)
                    .build(),
                None,
            );
        }
        ring.append(
            Local::now(),
            &log::Record::builder()
                .args(format_args!("warn"))
                .level(log::Level::Warn)
                .build(),
            None,
        );
        ring.append(
            Local::now(),
            &log::Record::builder()
                .args(format_args!("error"))
                .level(log::Level::Error)
                .build(),
            None,
        );
        let actual: Vec<&str> = ring
            .records
            .iter()
            .map(|record| record.message.as_ref())
            .collect();
        let original: Vec<&str> =
            include_str!("../../tests/fixtures/editor-log-pipeline.expected.txt")
                .lines()
                .collect();
        assert_eq!(actual, original);
    }
}

#[cfg(test)]
mod reentrant_tests {
    use super::*;
    struct Nested;
    impl fmt::Display for Nested {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            // This is the actual observer entry point, without installing a logger.
            capture(
                &log::Record::builder()
                    .level(log::Level::Warn)
                    .args(format_args!("__observer_nested"))
                    .build(),
                None,
            );
            f.write_str("__observer_outer")
        }
    }
    #[test]
    fn display_can_capture_nested_records_without_holding_buffer_lock() {
        capture(
            &log::Record::builder()
                .level(log::Level::Info)
                .args(format_args!("{}", Nested))
                .build(),
            None,
        );
        let snapshot = snapshot_if_changed(None).unwrap();
        let retained: Vec<&str> = snapshot
            .records
            .iter()
            .map(|record| record.message.as_ref())
            .filter(|message| message.starts_with("__observer_"))
            .collect();
        assert_eq!(retained, ["__observer_outer", "__observer_nested"]);
    }
}
