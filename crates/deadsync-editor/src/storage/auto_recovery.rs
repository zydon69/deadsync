//! Rate-limited recovery writes. Disk work owns an immutable snapshot and never
//! borrows the live editor; polling does not wait for the worker.
mod poll;
use super::FileSession;
use crate::{Error, history::RevisionToken};
use std::{
    sync::{
        Arc,
        mpsc::{self, Receiver, TryRecvError},
    },
    time::{Duration, Instant},
};

pub struct AutoRecovery {
    interval: Duration,
    last_attempt: Option<Instant>,
    saved: Option<Receipt>,
    worker: Option<Receiver<Result<Receipt, Error>>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    fn file() -> (tempfile::TempDir, FileSession) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("song.sm");
        fs::write(
            &path,
            b"#TITLE:Original;#NOTES:dance-single::Hard:9:0,0,0,0,0:1000;",
        )
        .unwrap();
        (dir, FileSession::open(&path).unwrap())
    }
    #[test]
    fn snapshot_captured_before_normal_save_cannot_publish_after_it() {
        let (_dir, mut file) = file();
        file.session.set_text(None, "TITLE", "First").unwrap();
        let pending = file.recovery_snapshot().unwrap().unwrap();
        file.session.set_text(None, "TITLE", "Second").unwrap();
        file.save().unwrap();
        assert!(matches!(pending.write(true), Err(Error::ExternalChange)));
        assert!(!file.recovery_path().unwrap().exists());
    }
    #[test]
    fn older_snapshot_cannot_replace_a_new_explicit_draft() {
        let (_dir, mut file) = file();
        file.session.set_text(None, "TITLE", "First").unwrap();
        file.save_recovery().unwrap();
        let pending = file.recovery_snapshot().unwrap().unwrap();
        file.session.set_text(None, "TITLE", "Second").unwrap();
        file.save_recovery().unwrap();
        let expected = fs::read(file.recovery_path().unwrap()).unwrap();
        assert!(matches!(pending.write(true), Err(Error::RecoveryConflict)));
        assert_eq!(fs::read(file.recovery_path().unwrap()).unwrap(), expected);
    }
}
struct Receipt {
    policy_revision: u64,
    publication_revision: u64,
    token: RevisionToken,
    baseline: Option<Arc<[u8]>>,
}
impl Default for AutoRecovery {
    fn default() -> Self {
        Self::new(Duration::from_secs(30))
    }
}
impl AutoRecovery {
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            last_attempt: None,
            saved: None,
            worker: None,
        }
    }
    pub fn is_pending(&self) -> bool {
        self.worker.is_some()
    }

    /// Returns true when a recovery publication completed. Check FileSession
    /// for a retained durability warning; a published file is never a refusal.
    /// A clean or unchanged
    /// session does not create a draft, and failures wait for the next interval.
    pub fn update(&mut self, file: &FileSession, now: Instant) -> Result<bool, Error> {
        let completed = self.poll_pending()?;
        let last = *self.last_attempt.get_or_insert(now);
        if now.saturating_duration_since(last) >= self.interval {
            self.request(file, now)?;
        }
        Ok(completed)
    }

    /// Request an immediate snapshot; at most one write can be in flight.
    pub fn request(&mut self, file: &FileSession, now: Instant) -> Result<(), Error> {
        self.request_with_sync(file, now, super::sync_directory)
    }
    pub(super) fn request_with_sync(
        &mut self,
        file: &FileSession,
        now: Instant,
        sync: impl FnMut(&std::path::Path) -> Result<(), Error> + Send + 'static,
    ) -> Result<(), Error> {
        if self.worker.is_some() {
            return Ok(());
        }
        self.last_attempt = Some(now);
        if !file.has_pending_recovery_durability()
            && (!file.session.is_dirty()
                || self.saved.as_ref().is_some_and(|receipt| {
                    file.session.accepts_token(&receipt.token)
                        && receipt.baseline == file.expected
                        && receipt.policy_revision == file.output_policy_revision()
                        && receipt.publication_revision == file.publication_revision
                }))
        {
            return Ok(());
        }
        let Some(snapshot) = file.recovery_snapshot()? else {
            return Ok(());
        };
        let receipt = Receipt {
            policy_revision: file.output_policy_revision(),
            publication_revision: file.publication_revision,
            token: file.session.revision_token(),
            baseline: file.expected.clone(),
        };
        let (sender, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("editor-recovery".into())
            .spawn(move || {
                let result = snapshot.write_with_sync(true, sync).map(|_| receipt);
                let _ = sender.send(result);
            })?;
        self.worker = Some(receiver);
        Ok(())
    }
}
