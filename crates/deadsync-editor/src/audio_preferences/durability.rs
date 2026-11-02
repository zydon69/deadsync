//! Published preference receipts outlive File clones and bounded undo history.
use crate::Error;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};
#[derive(Clone, Debug)]
pub struct Receipt {
    path: PathBuf,
    expected: Arc<Vec<u8>>,
}
#[derive(Clone, Debug)]
pub struct PendingPublication {
    pub receipt: Receipt,
    pub warning: Arc<str>,
}
static PENDING: OnceLock<Mutex<BTreeMap<PathBuf, PendingPublication>>> = OnceLock::new();
fn pending() -> &'static Mutex<BTreeMap<PathBuf, PendingPublication>> {
    PENDING.get_or_init(Default::default)
}
pub(super) fn key(path: &Path) -> PathBuf {
    if path.as_os_str().is_empty() {
        return PathBuf::new();
    }
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    let Some(parent) = absolute.parent() else {
        return absolute;
    };
    parent
        .canonicalize()
        .unwrap_or_else(|_| parent.to_owned())
        .join(absolute.file_name().unwrap_or_default())
}
pub(super) fn retain(path: &Path, expected: Arc<Vec<u8>>, error: &Error) {
    let path = key(path);
    let receipt = Receipt {
        path: path.clone(),
        expected,
    };
    pending().lock().unwrap().insert(
        path,
        PendingPublication {
            receipt,
            warning: error.to_string().into(),
        },
    );
}
pub(super) fn for_key(key: &Path) -> Option<PendingPublication> {
    pending().lock().unwrap().get(key).cloned()
}
pub fn pending_publications() -> Vec<PendingPublication> {
    pending().lock().unwrap().values().cloned().collect()
}
pub fn has_pending_publications() -> bool {
    !pending().lock().unwrap().is_empty()
}
pub fn retry_all() -> Result<(), Error> {
    for publication in pending_publications() {
        publication.receipt.retry()?;
    }
    Ok(())
}
pub(super) fn retry_path(path: &Path) -> Result<(), Error> {
    if let Some(publication) = for_key(&key(path)) {
        publication.receipt.retry()?;
    }
    Ok(())
}
impl Receipt {
    pub fn path(&self) -> &Path {
        &self.path
    }
    /// Synchronize only; never serialize, rename, or republish preferences.
    pub fn retry(&self) -> Result<(), Error> {
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.path.with_extension("json.lock"))?;
        lock.try_lock().map_err(std::io::Error::other)?;
        self.retry_locked()
    }
    pub(super) fn retry_locked(&self) -> Result<(), Error> {
        if super::read_bytes(&self.path)?.as_deref() != Some(self.expected.as_slice()) {
            return Err(Error::ExternalChange);
        }
        let parent = self
            .path
            .parent()
            .ok_or_else(|| Error::InvalidEdit("Preferences path has no parent".into()))?;
        sync(parent)?;
        let mut pending = pending().lock().unwrap();
        if pending
            .get(&self.path)
            .is_some_and(|p| Arc::ptr_eq(&p.receipt.expected, &self.expected))
        {
            pending.remove(&self.path);
        }
        Ok(())
    }
}
pub(super) fn sync(parent: &Path) -> Result<(), Error> {
    #[cfg(test)]
    if SYNC_FAILURES.with(|count| {
        let n = count.get();
        if n > 0 {
            count.set(n - 1);
            true
        } else {
            false
        }
    }) {
        return Err(std::io::Error::other("Injected preference directory sync failure").into());
    }
    crate::storage::sync_directory(parent)
}
#[cfg(test)]
thread_local! {static SYNC_FAILURES:std::cell::Cell<usize>=const{std::cell::Cell::new(0)};}
#[cfg(test)]
pub(crate) fn fail_next_syncs(count: usize) {
    SYNC_FAILURES.with(|n| n.set(count));
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        audio_preferences::File,
        document::{Document, Format},
        history::Session,
        visual_preferences::Snapshot,
    };
    #[cfg(unix)]
    fn inode(path: &Path) -> u64 {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(path).unwrap().ino()
    }
    #[test]
    fn real_publication_sync_failure_advances_save_undo_redo_and_receipts_survive_owner_drop() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("prefs.json");
        std::fs::write(&path, b"{\"Version\":1}").unwrap();
        let mut file = File::open(&path).unwrap();
        let mut session =
            Session::new(Document::parse(b"#TITLE:Durability;".as_slice(), Format::Sm).unwrap());
        let song = session.serialize().unwrap();
        let before = file.waveform();
        let mut after = before;
        after.set_show(false);
        fail_next_syncs(2);
        assert!(
            session
                .set_visual_preferences(
                    Snapshot::Waveform(before),
                    Snapshot::Waveform(after),
                    Some(&file)
                )
                .unwrap()
        );
        let update = session.take_visual_preferences_update().unwrap();
        assert_eq!(update.preferences, Snapshot::Waveform(after));
        file = update.file.unwrap();
        assert!(!serde_json::from_slice::<serde_json::Value>(&std::fs::read(&path).unwrap()).unwrap()["Waveform"]["ShowWaveForm"].as_bool().unwrap());
        let saved = file
            .pending_publication()
            .expect("rename was published despite sync failure");
        assert!(!saved.warning.is_empty());
        assert!(!file.has_changes());
        assert!(session.undo_label().is_some());
        assert!(
            File::open(&path).is_err(),
            "replacement File must pass the retained receipt barrier"
        );
        let bytes = std::fs::read(&path).unwrap();
        #[cfg(unix)]
        let identity = inode(&path);
        saved.receipt.retry().unwrap();
        assert!(file.pending_publication().is_none());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        #[cfg(unix)]
        assert_eq!(inode(&path), identity);
        for (undo, expected) in [(true, before), (false, after)] {
            fail_next_syncs(1);
            session.capture_source_preferences_context(&file);
            assert!(if undo {
                session.undo().unwrap()
            } else {
                session.redo().unwrap()
            });
            let update = session.take_visual_preferences_update().unwrap();
            assert_eq!(update.preferences, Snapshot::Waveform(expected));
            file = update.file.unwrap();
            let bytes = std::fs::read(&path).unwrap();
            assert_eq!(serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["Waveform"]["ShowWaveForm"].as_bool().unwrap(),expected.show());
            let publication = file.pending_publication().unwrap();
            assert!(!file.has_changes());
            assert!(!session.is_dirty());
            assert_eq!(session.serialize().unwrap(), song);
            if undo {
                assert!(
                    saved.receipt.retry().is_err(),
                    "old receipt cannot synchronize a different publication"
                );
            }
            #[cfg(unix)]
            let identity = inode(&path);
            publication.receipt.retry().unwrap();
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
            #[cfg(unix)]
            assert_eq!(inode(&path), identity);
        }
        // A subsequent real legacy save must also report publication success,
        // regardless of whether the history entry or File remain alive.
        let mut p = file.waveform();
        p.set_show(true);
        file.set_waveform(p);
        fail_next_syncs(1);
        assert!(file.save().unwrap());
        let receipt = file.pending_publication().unwrap().receipt;
        drop(file);
        drop(session);
        assert!(for_key(&key(&path)).is_some());
        fail_next_syncs(1);
        assert!(File::open(&path).is_err());
        assert!(for_key(&key(&path)).is_some());
        let bytes = std::fs::read(&path).unwrap();
        #[cfg(unix)]
        let identity = inode(&path);
        receipt.retry().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        #[cfg(unix)]
        assert_eq!(inode(&path), identity);
        let mut reopened = File::open(&path).unwrap();
        assert!(reopened.waveform().show());
        assert!(!reopened.save().unwrap());
        assert!(reopened.pending_publication().is_none());
    }
    #[test]
    fn retry_refuses_external_bytes_without_republication_and_keeps_receipt() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("prefs.json");
        std::fs::write(&path, b"{\"Version\":1}").unwrap();
        let mut file = File::open(&path).unwrap();
        let mut p = file.waveform();
        p.set_show(false);
        file.set_waveform(p);
        fail_next_syncs(1);
        assert!(file.save().unwrap());
        let publication = file.pending_publication().unwrap();
        let published = std::fs::read(&path).unwrap();
        std::fs::write(&path, b"{\"Version\":1,\"External\":true}").unwrap();
        let external = std::fs::read(&path).unwrap();
        assert!(matches!(
            publication.receipt.retry(),
            Err(Error::ExternalChange)
        ));
        assert_eq!(std::fs::read(&path).unwrap(), external);
        assert!(file.pending_publication().is_some());
        // Explicitly restore the exact reviewed publication, then retry only sync.
        std::fs::write(&path, published).unwrap();
        publication.receipt.retry().unwrap();
        assert!(file.pending_publication().is_none());
    }
}
