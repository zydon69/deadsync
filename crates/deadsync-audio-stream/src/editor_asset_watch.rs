//! LastWrite observers owned by a bounded worker, never by DSP or the UI thread.
use notify::{
    EventKind, RecommendedWatcher, RecursiveMode, Watcher,
    event::{DataChange, MetadataKind, ModifyKind},
};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex, OnceLock,
        atomic::{AtomicU64, AtomicUsize, Ordering},
        mpsc::{self, SyncSender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorAssetKind {
    Main,
    DedicatedPreview,
}
impl EditorAssetKind {
    const fn index(self) -> usize {
        match self {
            Self::Main => 0,
            Self::DedicatedPreview => 1,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssetRegistration {
    pub owner: u64,
    pub kind: EditorAssetKind,
    pub registration: u64,
}
#[derive(Clone, Debug)]
pub struct AssetWatchUpdate {
    pub token: AssetRegistration,
    pub changed: bool,
    pub error: Option<String>,
}
struct Target {
    token: AssetRegistration,
    path: PathBuf,
}
#[derive(Default)]
struct Inbox {
    current: [Option<AssetRegistration>; 2],
    updates: [Option<AssetWatchUpdate>; 2],
    watched: [Option<Target>; 2],
    ready: [bool; 2],
}
struct Request {
    token: AssetRegistration,
    path: Option<PathBuf>,
}
#[derive(Default)]
struct Requests {
    pending: [Option<Request>; 2],
    closed: bool,
}
#[derive(Debug)]
struct Permit;
const MAX_WATCH_WORKERS: usize = 2;
static NEXT_REGISTRATION: AtomicU64 = AtomicU64::new(0);
static WATCH_WORKERS: AtomicUsize = AtomicUsize::new(0);
static JOINED: OnceLock<(Mutex<()>, Condvar)> = OnceLock::new();
impl Permit {
    fn acquire() -> Option<Self> {
        WATCH_WORKERS
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < MAX_WATCH_WORKERS).then_some(count + 1)
            })
            .ok()
            .map(|_| Self)
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        let (lock, wake) = JOINED.get_or_init(|| (Mutex::new(()), Condvar::new()));
        let _guard = lock.lock().unwrap_or_else(|e| e.into_inner());
        WATCH_WORKERS.fetch_sub(1, Ordering::AcqRel);
        wake.notify_all();
    }
}
#[derive(Debug)]
struct Retired {
    thread: JoinHandle<()>,
    _permit: Permit,
    joined: Arc<std::sync::atomic::AtomicBool>,
}
static REAPER: OnceLock<Result<SyncSender<Retired>, String>> = OnceLock::new();
fn reaper() -> Result<&'static SyncSender<Retired>, notify::Error> {
    REAPER
        .get_or_init(|| {
            let (tx, rx) = mpsc::sync_channel::<Retired>(MAX_WATCH_WORKERS);
            thread::Builder::new()
                .name("editor-watch-join".into())
                .spawn(move || {
                    while let Ok(retired) = rx.recv() {
                        let Retired {
                            thread,
                            _permit,
                            joined,
                        } = retired;
                        let _ = thread.join();
                        joined.store(true, Ordering::Release);
                        drop(_permit);
                    }
                })
                .map_err(|e| e.to_string())?;
            Ok(tx)
        })
        .as_ref()
        .map_err(|e| notify::Error::generic(e))
}
/// Explicit final-shutdown seam. No caller in an audio/frame callback should
/// wait here. Timeout keeps the bounded join quarantine alive; it never detaches.
pub fn wait_for_editor_asset_watch_shutdown(timeout: Duration) -> bool {
    let (lock, wake) = JOINED.get_or_init(|| (Mutex::new(()), Condvar::new()));
    let guard = lock.lock().unwrap_or_else(|e| e.into_inner());
    let _ = wake
        .wait_timeout_while(guard, timeout, |_| {
            WATCH_WORKERS.load(Ordering::Acquire) != 0
        })
        .unwrap_or_else(|e| e.into_inner());
    WATCH_WORKERS.load(Ordering::Acquire) == 0
}
/// Two coalesced registrations/updates. OS lifecycle and canonicalization run
/// on one worker. At most two workers may be active OR awaiting actual join.
pub struct EditorAssetWatch {
    owner: u64,
    serial: u64,
    inbox: Arc<Mutex<Inbox>>,
    paths: [Option<PathBuf>; 2],
    requests: Arc<(Mutex<Requests>, Condvar)>,
    lifecycle: Mutex<Option<Retired>>,
    joined: Arc<std::sync::atomic::AtomicBool>,
}
impl EditorAssetWatch {
    pub fn new(owner: u64) -> Result<Self, &'static str> {
        if owner == 0 {
            return Err("Audio asset watcher requires an owner");
        }
        Ok(Self {
            owner,
            serial: 0,
            inbox: Arc::default(),
            paths: [None, None],
            requests: Arc::new((Mutex::new(Requests::default()), Condvar::new())),
            lifecycle: Mutex::new(None),
            joined: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        })
    }
    pub fn register(
        &mut self,
        kind: EditorAssetKind,
        path: Option<PathBuf>,
    ) -> Result<AssetRegistration, notify::Error> {
        if self
            .requests
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .closed
        {
            return Err(notify::Error::generic("Editor asset watch is shut down"));
        }
        self.serial = NEXT_REGISTRATION
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |serial| {
                serial.checked_add(1)
            })
            .expect("Audio asset registration exhausted")
            + 1;
        let token = AssetRegistration {
            owner: self.owner,
            kind,
            registration: self.serial,
        };
        let index = kind.index();
        {
            let mut inbox = self.inbox.lock().unwrap_or_else(|e| e.into_inner());
            inbox.current[index] = Some(token);
            inbox.updates[index] = None;
            inbox.watched[index] = None;
            inbox.ready[index] = false;
        }
        self.paths[index] = path.clone();
        let (lock, wake) = &*self.requests;
        {
            let mut requests = lock.lock().unwrap_or_else(|e| e.into_inner());
            if requests.closed {
                return Err(notify::Error::generic("Editor asset watch is shut down"));
            }
            requests.pending[index] = Some(Request { token, path });
        }
        wake.notify_one();
        self.ensure_worker()?;
        Ok(token)
    }
    fn ensure_worker(&self) -> Result<(), notify::Error> {
        let mut lifecycle = self.lifecycle.lock().unwrap_or_else(|e| e.into_inner());
        if lifecycle.is_some() {
            return Ok(());
        }
        {
            let mut requests = self.requests.0.lock().unwrap_or_else(|e| e.into_inner());
            if requests.closed || requests.pending.iter().all(Option::is_none) {
                return Ok(());
            }
            if requests
                .pending
                .iter()
                .flatten()
                .all(|request| request.path.is_none())
            {
                requests.pending = [None, None];
                return Ok(());
            }
        }
        let _ = reaper()?;
        let Some(permit) = Permit::acquire() else {
            return Ok(());
        };
        let inbox = Arc::clone(&self.inbox);
        let requests = Arc::clone(&self.requests);
        let thread = thread::Builder::new()
            .name("editor-asset-watch".into())
            .spawn(move || watch_loop(requests, inbox))
            .map_err(notify::Error::io)?;
        self.joined.store(false, Ordering::Release);
        *lifecycle = Some(Retired {
            thread,
            _permit: permit,
            joined: Arc::clone(&self.joined),
        });
        Ok(())
    }
    pub fn shutdown_complete(&self) -> bool {
        self.joined.load(Ordering::Acquire)
    }
    pub fn current_registration(&self, kind: EditorAssetKind) -> Option<AssetRegistration> {
        self.inbox.lock().unwrap_or_else(|e| e.into_inner()).current[kind.index()]
    }
    pub fn path(&self, kind: EditorAssetKind) -> Option<&Path> {
        self.paths[kind.index()].as_deref()
    }
    pub fn ready(&self, kind: EditorAssetKind) -> bool {
        let inbox = self.inbox.lock().unwrap_or_else(|e| e.into_inner());
        inbox.ready[kind.index()]
            && inbox.watched[kind.index()]
                .as_ref()
                .is_some_and(|target| Some(target.token) == inbox.current[kind.index()])
    }
    pub fn drain(&self) -> [Option<AssetWatchUpdate>; 2] {
        if let Err(error) = self.ensure_worker() {
            let tokens = self.inbox.lock().unwrap_or_else(|e| e.into_inner()).current;
            for token in tokens.into_iter().flatten() {
                publish(&self.inbox, token, false, Some(error.to_string()));
            }
        }
        let mut inbox = self.inbox.lock().unwrap_or_else(|e| e.into_inner());
        [inbox.updates[0].take(), inbox.updates[1].take()]
    }
    pub fn shutdown(&mut self) {
        {
            let mut inbox = self.inbox.lock().unwrap_or_else(|e| e.into_inner());
            inbox.current = [None, None];
            inbox.updates = [None, None];
            inbox.watched = [None, None];
            inbox.ready = [false, false];
        }
        let (lock, wake) = &*self.requests;
        {
            let mut requests = lock.lock().unwrap_or_else(|e| e.into_inner());
            requests.closed = true;
            requests.pending = [None, None];
        }
        wake.notify_one();
        self.paths = [None, None];
        if let Some(retired) = self
            .lifecycle
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            // Permits are retained until join, so <=2 retired/active workers
            // exist in total. The capacity-2 supervisor queue cannot overflow.
            reaper()
                .expect("Watcher join supervisor exists")
                .try_send(retired)
                .expect("Bounded watcher join supervisor invariant");
        }
    }
}
impl Drop for EditorAssetWatch {
    fn drop(&mut self) {
        self.shutdown();
    }
}
fn watch_loop(requests: Arc<(Mutex<Requests>, Condvar)>, inbox: Arc<Mutex<Inbox>>) {
    let mut watcher: Option<RecommendedWatcher> = None;
    let mut subscribed: Vec<PathBuf> = Vec::with_capacity(2);
    let mut parents: [Option<PathBuf>; 2] = [None, None];
    loop {
        let batch = {
            let (lock, wake) = &*requests;
            let mut requests = lock.lock().unwrap_or_else(|e| e.into_inner());
            while !requests.closed && requests.pending.iter().all(Option::is_none) {
                requests = wake.wait(requests).unwrap_or_else(|e| e.into_inner());
            }
            if requests.closed {
                break;
            }
            [requests.pending[0].take(), requests.pending[1].take()]
        };
        let mut reconfigured = false;
        for request in batch.into_iter().flatten() {
            let index = request.token.kind.index();
            if inbox.lock().unwrap_or_else(|e| e.into_inner()).current[index] != Some(request.token)
            {
                continue;
            }
            #[cfg(test)]
            wait_setup_gate(request.token.owner);
            if inbox.lock().unwrap_or_else(|e| e.into_inner()).current[index] != Some(request.token)
            {
                continue;
            }
            let result = (|| -> Result<Option<PathBuf>, notify::Error> {
                let Some(path) = request.path else {
                    return Ok(None);
                };
                if !path.is_absolute() {
                    return Err(notify::Error::generic(
                        "Editor asset watch requires an absolute path",
                    ));
                }
                let parent = path
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or(Path::new("."));
                // Preserve original source spelling; normalize only the parent
                // used by OS callbacks (/var versus /private/var on macOS).
                let watched_parent = parent.canonicalize().map_err(notify::Error::io)?;
                let watched_path =
                    watched_parent.join(path.file_name().ok_or_else(|| {
                        notify::Error::generic("Editor asset requires a filename")
                    })?);
                if watcher.is_none() {
                    let callback = Arc::clone(&inbox);
                    watcher = Some(notify::recommended_watcher(
                        move |result: notify::Result<notify::Event>| {
                            handle_event(&callback, result)
                        },
                    )?);
                }
                if !subscribed.contains(&watched_parent) {
                    reconfigured = true;
                    inbox.lock().unwrap_or_else(|e| e.into_inner()).ready = [false, false];
                    watcher
                        .as_mut()
                        .unwrap()
                        .watch(&watched_parent, RecursiveMode::NonRecursive)?;
                    subscribed.push(watched_parent.clone());
                }
                let mut state = inbox.lock().unwrap_or_else(|e| e.into_inner());
                if state.current[index] == Some(request.token) {
                    state.watched[index] = Some(Target {
                        token: request.token,
                        path: watched_path,
                    });
                }
                drop(state);
                // Installation closes the async-registration race: reload once
                // after the watch is active, covering writes during OS setup.
                if !reconfigured {
                    acknowledge_targets(&inbox, Some(request.token));
                }
                Ok(Some(watched_parent))
            })();
            match result {
                Ok(parent) => {
                    if inbox.lock().unwrap_or_else(|e| e.into_inner()).current[index]
                        == Some(request.token)
                    {
                        parents[index] = parent;
                    }
                }
                Err(error) => publish(
                    &inbox,
                    request.token,
                    false,
                    Some(error.to_string().chars().take(1024).collect()),
                ),
            }
        }
        // Reuse one native observer for both source slots and for filename
        // changes in the same directory. Cleanup remains on this worker.
        subscribed.retain(|parent| {
            if parents.iter().any(|used| used.as_ref() == Some(parent)) {
                true
            } else {
                if let Some(watcher) = watcher.as_mut() {
                    reconfigured = true;
                    inbox.lock().unwrap_or_else(|e| e.into_inner()).ready = [false, false];
                    let _ = watcher.unwatch(parent);
                }
                false
            }
        });
        // FSEvents may restart its shared stream for any directory change.
        // Revalidate ALL current registrations after that gap, not just the
        // source whose path caused the native reconfiguration.
        if reconfigured {
            acknowledge_targets(&inbox, None);
        }
    }
    // RecommendedWatcher destruction (including native joins) occurs here.
}
fn acknowledge_targets(inbox: &Mutex<Inbox>, only: Option<AssetRegistration>) {
    let tokens = {
        let mut state = inbox.lock().unwrap_or_else(|e| e.into_inner());
        std::array::from_fn::<_, 2, _>(|index| {
            let token = state.watched[index]
                .as_ref()
                .filter(|target| {
                    Some(target.token) == state.current[index]
                        && only.is_none_or(|only| only == target.token)
                })
                .map(|target| target.token);
            if token.is_some() {
                state.ready[index] = true;
            }
            token
        })
    };
    for token in tokens.into_iter().flatten() {
        publish(inbox, token, true, None);
    }
}
fn is_last_write(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Modify(ModifyKind::Data(
            DataChange::Any | DataChange::Size | DataChange::Content
        )) | EventKind::Modify(ModifyKind::Metadata(MetadataKind::WriteTime))
    )
}
fn handle_event(inbox: &Mutex<Inbox>, result: notify::Result<notify::Event>) {
    match result {
        Ok(event) if is_last_write(&event.kind) => {
            let tokens = {
                let inbox = inbox.lock().unwrap_or_else(|e| e.into_inner());
                std::array::from_fn::<_, 2, _>(|index| {
                    inbox.watched[index]
                        .as_ref()
                        .filter(|target| {
                            Some(target.token) == inbox.current[index]
                                && event
                                    .paths
                                    .iter()
                                    .any(|path| path.as_os_str() == target.path.as_os_str())
                        })
                        .map(|target| target.token)
                })
            };
            for token in tokens.into_iter().flatten() {
                publish(inbox, token, true, None);
            }
        }
        Err(error) => {
            let tokens = inbox.lock().unwrap_or_else(|e| e.into_inner()).current;
            let message = error.to_string().chars().take(1024).collect::<String>();
            for token in tokens.into_iter().flatten() {
                publish(inbox, token, false, Some(message.clone()));
            }
        }
        _ => {}
    }
}
fn publish(inbox: &Mutex<Inbox>, token: AssetRegistration, changed: bool, error: Option<String>) {
    let mut inbox = inbox.lock().unwrap_or_else(|e| e.into_inner());
    let index = token.kind.index();
    if inbox.current[index] != Some(token) {
        return;
    }
    let update = inbox.updates[index].get_or_insert(AssetWatchUpdate {
        token,
        changed: false,
        error: None,
    });
    update.changed |= changed;
    if error.is_some() {
        update.error = error;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn event_classes_match_source_last_write_only() {
        assert!(is_last_write(&EventKind::Modify(ModifyKind::Data(
            DataChange::Content
        ))));
        assert!(is_last_write(&EventKind::Modify(ModifyKind::Metadata(
            MetadataKind::WriteTime
        ))));
        for kind in [
            EventKind::Any,
            EventKind::Create(notify::event::CreateKind::File),
            EventKind::Remove(notify::event::RemoveKind::File),
            EventKind::Modify(ModifyKind::Name(notify::event::RenameMode::Both)),
            EventKind::Modify(ModifyKind::Metadata(MetadataKind::Permissions)),
        ] {
            assert!(!is_last_write(&kind));
        }
    }
    #[test]
    fn bounded_coalescing_and_stale_owner_registration() {
        let old = AssetRegistration {
            owner: 1,
            kind: EditorAssetKind::Main,
            registration: 1,
        };
        let current = AssetRegistration {
            owner: 2,
            registration: 2,
            ..old
        };
        let inbox = Mutex::new(Inbox {
            current: [Some(current), None],
            ..Default::default()
        });
        publish(&inbox, old, true, None);
        assert!(inbox.lock().unwrap().updates[0].is_none());
        for _ in 0..10_000 {
            publish(&inbox, current, true, None);
        }
        publish(&inbox, current, false, Some("backend error".into()));
        let mut guard = inbox.lock().unwrap();
        let update = guard.updates[0].take().unwrap();
        assert!(update.changed);
        assert!(update.error.is_some());
        assert!(guard.updates[1].is_none());
        guard.current = [None, None];
        drop(guard);
        publish(&inbox, current, true, None);
    }
}

#[cfg(test)]
struct SetupGate {
    owner: u64,
    released: Arc<std::sync::atomic::AtomicBool>,
    entered: Option<mpsc::Sender<()>>,
}
#[cfg(test)]
static SETUP_GATES: Mutex<Vec<SetupGate>> = Mutex::new(Vec::new());
#[cfg(test)]
pub(crate) struct SetupGateRelease(Arc<std::sync::atomic::AtomicBool>);
#[cfg(test)]
impl SetupGateRelease {
    pub(crate) fn release(&self) {
        self.0.store(true, Ordering::Release);
    }
}
#[cfg(test)]
impl Drop for SetupGateRelease {
    fn drop(&mut self) {
        self.release();
    }
}
#[cfg(test)]
pub(crate) fn gate_next_setup(owner: u64) -> (mpsc::Receiver<()>, SetupGateRelease) {
    let (tx, rx) = mpsc::channel();
    let released = Arc::new(std::sync::atomic::AtomicBool::new(false));
    SETUP_GATES.lock().unwrap().push(SetupGate {
        owner,
        released: Arc::clone(&released),
        entered: Some(tx),
    });
    (rx, SetupGateRelease(released))
}
#[cfg(test)]
fn wait_setup_gate(owner: u64) {
    let gate = {
        let mut gates = SETUP_GATES.lock().unwrap();
        if let Some(i) = gates.iter().position(|gate| gate.owner == owner) {
            if gates[i].released.load(Ordering::Acquire) {
                gates.remove(i);
                None
            } else {
                if let Some(tx) = gates[i].entered.take() {
                    let _ = tx.send(());
                }
                Some(Arc::clone(&gates[i].released))
            }
        } else {
            None
        }
    };
    if let Some(released) = gate {
        let deadline = std::time::Instant::now() + Duration::from_secs(60);
        while !released.load(Ordering::Acquire) && std::time::Instant::now() < deadline {
            thread::sleep(Duration::from_millis(2));
        }
    }
}
#[cfg(test)]
mod lifecycle_barrier_tests {
    use super::*;
    #[test]
    fn registration_replacement_and_shutdown_return_while_os_setup_is_blocked() {
        let owner = 51878;
        let (entered, release) = gate_next_setup(owner);
        let mut watch = EditorAssetWatch::new(owner).unwrap();
        let directory = std::env::temp_dir();
        let first = watch
            .register(EditorAssetKind::Main, Some(directory.join("first.wav")))
            .unwrap();
        entered.recv_timeout(Duration::from_secs(60)).unwrap();
        assert!(!watch.ready(EditorAssetKind::Main));
        let latest = watch.register(EditorAssetKind::Main, None).unwrap();
        assert!(latest.registration > first.registration);
        assert_eq!(
            watch.current_registration(EditorAssetKind::Main),
            Some(latest)
        );
        watch.shutdown();
        assert!(watch.current_registration(EditorAssetKind::Main).is_none());
        assert!(watch.drain().iter().all(Option::is_none));
        release.release();
        // This exact worker must be joined, independent of other owners.
        let deadline = std::time::Instant::now() + Duration::from_secs(60);
        while !watch.shutdown_complete() {
            assert!(std::time::Instant::now() < deadline);
            thread::sleep(Duration::from_millis(2));
        }
    }
}

#[cfg(test)]
mod registration_identity_tests {
    use super::*;
    #[test]
    fn same_ui_owner_recreation_never_reuses_a_source_registration() {
        let mut old = EditorAssetWatch::new(9991).unwrap();
        let old_token = old.register(EditorAssetKind::Main, None).unwrap();
        old.shutdown();
        let mut replacement = EditorAssetWatch::new(9991).unwrap();
        let token = replacement.register(EditorAssetKind::Main, None).unwrap();
        assert_eq!(token.owner, old_token.owner);
        assert!(token.registration > old_token.registration);
    }
}
