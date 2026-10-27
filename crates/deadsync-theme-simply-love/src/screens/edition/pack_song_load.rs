//! Read-only strict song preparation. Dropping/cancelling never publishes a session.
use deadsync_editor::{
    Error, notes, pack::song_list::Row, storage::FileSession, workspace::Workspace,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, TryRecvError},
};

pub(super) struct Job {
    cancelled: Arc<AtomicBool>,
    receiver: Receiver<Result<Workspace, Error>>,
}
impl Job {
    pub(super) fn spawn(row: Row) -> Result<Self, Error> {
        let cancelled = Arc::new(AtomicBool::new(false));
        let signal = cancelled.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("editor-pack-song-open".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    prepare(&row, &signal)
                }))
                .unwrap_or_else(|_| Err(Error::InvalidEdit("Song loading worker failed".into())));
                let _ = sender.send(result);
            })?;
        Ok(Self {
            cancelled,
            receiver,
        })
    }
    pub(super) fn poll(&mut self) -> Option<Result<Workspace, Error>> {
        match self.receiver.try_recv() {
            Ok(value) if !self.cancelled.load(Ordering::Relaxed) => Some(value),
            Ok(_) | Err(TryRecvError::Disconnected) => {
                Some(Err(Error::InvalidEdit("Song loading cancelled".into())))
            }
            Err(TryRecvError::Empty) => None,
        }
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}
fn prepare(row: &Row, cancelled: &AtomicBool) -> Result<Workspace, Error> {
    let check = || -> Result<(), Error> {
        if cancelled.load(Ordering::Relaxed) {
            return Err(Error::InvalidEdit("Song loading cancelled".into()));
        }
        if !row.is_current()? {
            return Err(Error::ExternalChange);
        }
        Ok(())
    };
    check()?;
    // This is the strict existing editor loader, never Metadata -> Document.
    // Cancellation brackets its synchronous parser; it cannot interrupt parsing.
    let file = FileSession::open_bounded(&row.logical_path, 16 * 1024 * 1024, cancelled)?;
    check()?;
    if file.path() != row.path || file.has_external_change()? {
        return Err(Error::ExternalChange);
    }
    let chart = file
        .session
        .document()
        .charts()
        .iter()
        .position(|chart| notes::layout_columns(&chart.steps_type).is_some())
        .unwrap_or(0);
    let workspace = Workspace::from_file(file, chart)?;
    check()?;
    Ok(workspace)
}
