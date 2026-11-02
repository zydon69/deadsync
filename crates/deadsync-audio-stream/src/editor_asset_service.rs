//! Application-thread orchestration; two cancellable, joined decode workers.
use crate::{
    editor_asset_slots::{AssetLoadToken, EditorAssetSlots},
    editor_asset_watch::{AssetRegistration, EditorAssetKind, EditorAssetWatch},
};
use deadlib_audio_decode::{DecodeOptions, prepared::PreparedAudioSource};
use std::{
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

pub enum EditorAssetEvent {
    /// Accepted reset: caller invalidates active PCM before async publication.
    Reset { token: AssetLoadToken },
    Loaded {
        token: AssetLoadToken,
        source: Arc<PreparedAudioSource>,
    },
    Failed {
        token: AssetLoadToken,
        message: String,
    },
    WatchFailed {
        registration: AssetRegistration,
        message: String,
    },
}
struct Job {
    token: AssetLoadToken,
    path: PathBuf,
    cancel: Arc<AtomicBool>,
}
struct Completion {
    token: AssetLoadToken,
    result: Result<Arc<PreparedAudioSource>, String>,
}
#[derive(Default)]
struct Queue {
    pending: Option<Job>,
    closed: bool,
}
struct Worker {
    queue: Arc<(Mutex<Queue>, Condvar)>,
    current_cancel: Option<Arc<AtomicBool>>,
    thread: Option<JoinHandle<()>>,
}
impl Worker {
    fn new(
        index: usize,
        completions: Arc<Mutex<[Option<Completion>; 2]>>,
    ) -> std::io::Result<Self> {
        let queue = Arc::new((Mutex::new(Queue::default()), Condvar::new()));
        let shared = Arc::clone(&queue);
        let thread = thread::Builder::new()
            .name(format!("editor-asset-load-{index}"))
            .spawn(move || {
                loop {
                    let job = {
                        let (lock, wake) = &*shared;
                        let mut queue = lock.lock().unwrap_or_else(|e| e.into_inner());
                        while !queue.closed && queue.pending.is_none() {
                            queue = wake.wait(queue).unwrap_or_else(|e| e.into_inner());
                        }
                        if queue.closed {
                            break;
                        }
                        queue.pending.take().unwrap()
                    };
                    #[cfg(test)]
                    wait_load_gate(&job.path, &job.cancel);
                    let result = PreparedAudioSource::load_editor(
                        &job.path,
                        DecodeOptions::STEP_MANIA_TIMELINE,
                        &job.cancel,
                    )
                    .map_err(|e| e.to_string());
                    if !job.cancel.load(Ordering::Acquire) {
                        completions.lock().unwrap_or_else(|e| e.into_inner())[index] =
                            Some(Completion {
                                token: job.token,
                                result,
                            });
                    }
                }
            })?;
        Ok(Self {
            queue,
            current_cancel: None,
            thread: Some(thread),
        })
    }
    fn submit(&mut self, token: AssetLoadToken, path: PathBuf) {
        self.cancel();
        let cancel = Arc::new(AtomicBool::new(false));
        self.current_cancel = Some(Arc::clone(&cancel));
        let (queue, wake) = &*self.queue;
        queue.lock().unwrap_or_else(|e| e.into_inner()).pending = Some(Job {
            token,
            path,
            cancel,
        });
        wake.notify_one();
    }
    fn cancel(&mut self) {
        if let Some(cancel) = self.current_cancel.take() {
            cancel.store(true, Ordering::Release);
        }
    }
    fn shutdown(&mut self) {
        self.cancel();
        let (queue, wake) = &*self.queue;
        {
            let mut queue = queue.lock().unwrap_or_else(|e| e.into_inner());
            queue.closed = true;
            queue.pending = None;
        }
        wake.notify_one();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub struct EditorAssetService {
    watch: EditorAssetWatch,
    slots: EditorAssetSlots,
    paths: [Option<PathBuf>; 2],
    registrations: [Option<AssetRegistration>; 2],
    workers: [Worker; 2],
    completions: Arc<Mutex<[Option<Completion>; 2]>>,
    pending_events: [Option<EditorAssetEvent>; 2],
}
fn index(kind: EditorAssetKind) -> usize {
    match kind {
        EditorAssetKind::Main => 0,
        EditorAssetKind::DedicatedPreview => 1,
    }
}
impl EditorAssetService {
    pub fn new(owner: u64) -> Result<Self, String> {
        let watch = EditorAssetWatch::new(owner).map_err(str::to_owned)?;
        let completions = Arc::new(Mutex::new([None, None]));
        let main = Worker::new(0, Arc::clone(&completions)).map_err(|e| e.to_string())?;
        let preview = Worker::new(1, Arc::clone(&completions)).map_err(|e| e.to_string())?;
        Ok(Self {
            watch,
            slots: EditorAssetSlots::default(),
            paths: [None, None],
            registrations: [None, None],
            workers: [main, preview],
            completions,
            pending_events: [None, None],
        })
    }
    /// Idempotent source-path registration, including inactive/stopped assets.
    /// Watcher setup failure is diagnostic; it never prevents initial decoding.
    pub fn register(
        &mut self,
        kind: EditorAssetKind,
        path: Option<PathBuf>,
    ) -> Result<Option<AssetLoadToken>, String> {
        let i = index(kind);
        if self.registrations[i].is_some() && same_path(self.paths[i].as_ref(), path.as_ref()) {
            return Ok(None);
        }
        self.pending_events[i] = None;
        self.workers[i].cancel();
        self.completions.lock().unwrap_or_else(|e| e.into_inner())[i] = None;
        let registration_result = self.watch.register(kind, path.clone());
        let registration = self
            .watch
            .current_registration(kind)
            .ok_or("Asset registration missing")?;
        self.paths[i] = path.clone();
        self.registrations[i] = Some(registration);
        self.slots.register(registration);
        let token = self.slots.begin_load(registration).unwrap();
        self.pending_events[i] = Some(EditorAssetEvent::Reset { token });
        if let Some(path) = path {
            self.workers[i].submit(token, path);
        }
        let token = Some(token);
        if let Err(error) = registration_result {
            return Err(error.to_string());
        }
        Ok(token)
    }
    pub fn watch_ready(&self, kind: EditorAssetKind) -> bool {
        self.watch.ready(kind)
    }
    pub fn current_load_token(&self, kind: EditorAssetKind) -> Option<AssetLoadToken> {
        self.slots.current_load_token(kind)
    }
    pub fn source(&self, kind: EditorAssetKind) -> Option<&Arc<PreparedAudioSource>> {
        self.slots.source(kind)
    }
    pub fn is_current(&self, token: AssetLoadToken) -> bool {
        self.slots.is_current(token)
    }
    pub fn path(&self, kind: EditorAssetKind) -> Option<&std::path::Path> {
        self.paths[index(kind)].as_deref()
    }
    /// No file IO or joins here. Returned events are bounded by two source slots.
    pub fn poll(&mut self) -> Vec<EditorAssetEvent> {
        let mut events = Vec::with_capacity(6);
        for event in &mut self.pending_events {
            if let Some(event) = event.take() {
                events.push(event);
            }
        }
        // Notices reset cache before observing completed data from older loads.
        for update in self.watch.drain().into_iter().flatten() {
            let i = index(update.token.kind);
            if self.registrations[i] != Some(update.token) {
                continue;
            }
            if let Some(message) = update.error {
                events.push(EditorAssetEvent::WatchFailed {
                    registration: update.token,
                    message,
                });
            }
            if update.changed
                && let Some(path) = self.paths[i].clone()
                && let Some(token) = self.slots.begin_load(update.token)
            {
                self.workers[i].submit(token, path);
                events.push(EditorAssetEvent::Reset { token });
            }
        }
        let completions = {
            let mut inbox = self.completions.lock().unwrap_or_else(|e| e.into_inner());
            [inbox[0].take(), inbox[1].take()]
        };
        for completion in completions.into_iter().flatten() {
            if !self.slots.is_current(completion.token) {
                continue;
            }
            match completion.result {
                Ok(source) => {
                    self.slots.publish(completion.token, Arc::clone(&source));
                    events.push(EditorAssetEvent::Loaded {
                        token: completion.token,
                        source,
                    });
                }
                Err(message) => events.push(EditorAssetEvent::Failed {
                    token: completion.token,
                    message,
                }),
            }
        }
        // A ready acknowledgement may supersede an initial Reset within this
        // same batch. Only the final accepted revisions leave the service.
        events.retain(|event| match event {
            EditorAssetEvent::Reset { token }
            | EditorAssetEvent::Loaded { token, .. }
            | EditorAssetEvent::Failed { token, .. } => self.slots.is_current(*token),
            EditorAssetEvent::WatchFailed { registration, .. } => {
                self.registrations[index(registration.kind)] == Some(*registration)
            }
        });
        events
    }
    pub fn shutdown(&mut self) {
        self.watch.shutdown();
        for worker in &mut self.workers {
            worker.shutdown();
        }
        self.slots.clear();
        self.registrations = [None, None];
        self.paths = [None, None];
        self.pending_events = [None, None];
        *self.completions.lock().unwrap_or_else(|e| e.into_inner()) = [None, None];
    }
}
impl Drop for EditorAssetService {
    fn drop(&mut self) {
        self.shutdown();
    }
}
fn same_path(a: Option<&PathBuf>, b: Option<&PathBuf>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            if a.as_os_str() == b.as_os_str() {
                return true;
            }
            let a = a.to_string_lossy();
            let b = b.to_string_lossy();
            a.strip_prefix(r"\\?\").unwrap_or(&a).replace('\\', "/")
                == b.strip_prefix(r"\\?\").unwrap_or(&b).replace('\\', "/")
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    pub(super) fn wav(path: &std::path::Path, value: i16) {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&44u32.to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&48_000u32.to_le_bytes());
        bytes.extend_from_slice(&192_000u32.to_le_bytes());
        bytes.extend_from_slice(&4u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&8u32.to_le_bytes());
        for _ in 0..4 {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        std::fs::write(path, bytes).unwrap();
    }
    fn wait_loaded(
        service: &mut EditorAssetService,
        kind: EditorAssetKind,
    ) -> Arc<PreparedAudioSource> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            for event in service.poll() {
                if let EditorAssetEvent::Failed { message, .. } = event {
                    panic!("{message}");
                }
            }
            if let Some(source) = service.source(kind) {
                return Arc::clone(source);
            }
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(2));
        }
    }
    #[test]
    fn real_last_write_reloads_inactive_main_without_replacing_preview_cache() {
        let directory = std::env::temp_dir().join(format!(
            "editor-source-watch-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let main = directory.join("main.wav");
        let preview = directory.join("preview.wav");
        wav(&main, -1000);
        wav(&preview, 2000);
        let (entered, release) = crate::editor_asset_watch::gate_next_setup(10);
        let mut service = EditorAssetService::new(10).unwrap();
        service
            .register(EditorAssetKind::Main, Some(main.clone()))
            .unwrap();
        service
            .register(EditorAssetKind::DedicatedPreview, Some(preview))
            .unwrap();
        let old = wait_loaded(&mut service, EditorAssetKind::Main);
        wait_loaded(&mut service, EditorAssetKind::DedicatedPreview);
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            match entered.try_recv() {
                Ok(()) => break,
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    // The real host polls while previous watcher owners retire.
                    // Polling retries the bounded worker permit acquisition.
                    service.poll();
                    assert!(Instant::now() < deadline, "watcher setup gate not reached");
                    thread::sleep(Duration::from_millis(2));
                }
                Err(error) => panic!("watcher setup gate disconnected: {error}"),
            }
        }
        // Source PCM changes while the OS observer is deliberately unarmed.
        // The conservative ready acknowledgement must catch this lost window.
        wav(&main, 4000);
        release.release();
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            service.poll();
            let ready = [EditorAssetKind::Main, EditorAssetKind::DedicatedPreview]
                .into_iter()
                .all(|kind| {
                    service.watch_ready(kind)
                        && service
                            .current_load_token(kind)
                            .is_some_and(|token| token.revision >= 2)
                        && service.source(kind).is_some()
                });
            if ready {
                break;
            }
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(2));
        }
        let after_setup = Arc::clone(service.source(EditorAssetKind::Main).unwrap());
        let mut reader = after_setup.reader();
        let mut pcm = Vec::new();
        reader.read_dec_packet_into(&mut pcm);
        assert_eq!(pcm, [4000; 4]);
        let held_preview = Arc::clone(service.source(EditorAssetKind::DedicatedPreview).unwrap());
        // This second write occurs after readiness and exercises real LastWrite.
        wav(&main, 5000);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            service.poll();
            if let Some(new) = service.source(EditorAssetKind::Main) {
                if !Arc::ptr_eq(&after_setup, new) {
                    let mut reader = new.reader();
                    let mut samples = Vec::new();
                    assert!(reader.read_dec_packet_into(&mut samples));
                    assert_eq!(samples, [5000; 4]);
                    break;
                }
            }
            assert!(
                Instant::now() < deadline,
                "OS LastWrite did not reload source"
            );
            thread::sleep(Duration::from_millis(2));
        }
        assert!(Arc::ptr_eq(
            &held_preview,
            service.source(EditorAssetKind::DedicatedPreview).unwrap()
        ));
        let mut reader = old.reader();
        let mut samples = Vec::new();
        reader.read_dec_packet_into(&mut samples);
        assert_eq!(samples, [-1000; 4]);
        service.register(EditorAssetKind::Main, None).unwrap();
        service.poll();
        assert!(service.source(EditorAssetKind::Main).is_none());
        service.shutdown();
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn shutdown_invalidates_pending_load_and_releases_file_handles() {
        let directory =
            std::env::temp_dir().join(format!("editor-source-shutdown-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("sound.wav");
        wav(&path, 1);
        let mut service = EditorAssetService::new(20).unwrap();
        service.register(EditorAssetKind::Main, Some(path)).unwrap();
        service.shutdown();
        assert!(service.poll().is_empty());
        assert!(service.source(EditorAssetKind::Main).is_none());
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[cfg(test)]
mod lifecycle_timing_diagnostic {
    use super::*;
    #[test]
    fn report_editor_watcher_lifecycle_timings() {
        let directory =
            std::env::temp_dir().join(format!("editor-watch-lifecycle-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let measure = std::time::Instant::now();
        let mut service = EditorAssetService::new(912).unwrap();
        eprintln!("asset_watch_timing new={:?}", measure.elapsed());
        let measure = std::time::Instant::now();
        let _ = service.register(EditorAssetKind::Main, Some(directory.join("main.wav")));
        eprintln!("asset_watch_timing register_main={:?}", measure.elapsed());
        let measure = std::time::Instant::now();
        let _ = service.register(
            EditorAssetKind::DedicatedPreview,
            Some(directory.join("preview.wav")),
        );
        eprintln!(
            "asset_watch_timing register_preview={:?}",
            measure.elapsed()
        );
        let measure = std::time::Instant::now();
        service.poll();
        eprintln!("asset_watch_timing poll={:?}", measure.elapsed());
        let measure = std::time::Instant::now();
        service.shutdown();
        eprintln!("asset_watch_timing shutdown={:?}", measure.elapsed());
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[cfg(test)]
struct LoadGate {
    path: PathBuf,
    released: Arc<AtomicBool>,
    entered: Option<std::sync::mpsc::Sender<()>>,
}
#[cfg(test)]
static LOAD_GATES: Mutex<Vec<LoadGate>> = Mutex::new(Vec::new());
#[cfg(test)]
pub(crate) struct LoadGateRelease(Arc<AtomicBool>);
#[cfg(test)]
impl LoadGateRelease {
    pub(crate) fn send(&self, _: ()) -> Result<(), ()> {
        self.0.store(true, Ordering::Release);
        Ok(())
    }
}
#[cfg(test)]
impl Drop for LoadGateRelease {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}
#[cfg(test)]
pub(crate) fn gate_next_load(path: PathBuf) -> (std::sync::mpsc::Receiver<()>, LoadGateRelease) {
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let released = Arc::new(AtomicBool::new(false));
    LOAD_GATES.lock().unwrap().push(LoadGate {
        path,
        released: Arc::clone(&released),
        entered: Some(entered_tx),
    });
    (entered_rx, LoadGateRelease(released))
}
#[cfg(test)]
fn wait_load_gate(path: &std::path::Path, cancel: &AtomicBool) {
    let gate = {
        let mut gates = LOAD_GATES.lock().unwrap();
        if let Some(i) = gates.iter().position(|gate| gate.path == path) {
            if gates[i].released.load(Ordering::Acquire) {
                gates.remove(i);
                None
            } else {
                if let Some(entered) = gates[i].entered.take() {
                    let _ = entered.send(());
                }
                Some(Arc::clone(&gates[i].released))
            }
        } else {
            None
        }
    };
    if let Some(released) = gate {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !cancel.load(Ordering::Acquire)
            && !released.load(Ordering::Acquire)
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
}

#[cfg(test)]
mod orchestration_race_tests {
    use super::*;
    #[test]
    fn real_pending_a_b_a_rejects_old_load_and_failure_after_registration_changes() {
        let directory = std::env::temp_dir().join(format!(
            "editor-watch-aba-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let a = directory.join("a.wav");
        let b = directory.join("missing-b.wav");
        super::tests::wav(&a, -1000);
        let (entered, release) = gate_next_load(a.clone());
        let mut service = EditorAssetService::new(500).unwrap();
        let old = service
            .register(EditorAssetKind::Main, Some(a.clone()))
            .unwrap()
            .unwrap();
        entered
            .recv_timeout(std::time::Duration::from_secs(3))
            .unwrap();
        service.register(EditorAssetKind::Main, Some(b)).unwrap();
        let current = service
            .register(EditorAssetKind::Main, Some(a))
            .unwrap()
            .unwrap();
        assert!(!service.is_current(old));
        assert!(service.is_current(current));
        release.send(()).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            for event in service.poll() {
                match event {
                    EditorAssetEvent::Loaded { token, .. }
                    | EditorAssetEvent::Failed { token, .. }
                    | EditorAssetEvent::Reset { token } => {
                        assert_eq!(token.registration, current.registration);
                        assert!(token.revision >= current.revision);
                        assert!(service.is_current(token));
                    }
                    _ => {}
                }
            }
            if let Some(source) = service.source(EditorAssetKind::Main) {
                let mut reader = source.reader();
                let mut pcm = Vec::new();
                reader.read_dec_packet_into(&mut pcm);
                assert_eq!(pcm, [-1000; 4]);
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        service.shutdown();
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[cfg(test)]
mod directory_reconfiguration_tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn adding_another_directory_revalidates_unchanged_main_registration() {
        let directory = std::env::temp_dir().join(format!(
            "editor-two-directories-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let a = directory.join("a");
        let b = directory.join("b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        let main = a.join("main.wav");
        let preview = b.join("preview.wav");
        super::tests::wav(&main, 1000);
        super::tests::wav(&preview, 2000);
        let mut service = EditorAssetService::new(51990).unwrap();
        service.register(EditorAssetKind::Main, Some(main)).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(60);
        loop {
            service.poll();
            if service.watch_ready(EditorAssetKind::Main)
                && service
                    .current_load_token(EditorAssetKind::Main)
                    .is_some_and(|token| token.revision >= 2)
                && service.source(EditorAssetKind::Main).is_some()
            {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            thread::sleep(Duration::from_millis(2));
        }
        let before = service.current_load_token(EditorAssetKind::Main).unwrap();
        service
            .register(EditorAssetKind::DedicatedPreview, Some(preview))
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(60);
        loop {
            service.poll();
            if service.watch_ready(EditorAssetKind::DedicatedPreview)
                && service
                    .current_load_token(EditorAssetKind::Main)
                    .is_some_and(|token| {
                        token.registration == before.registration
                            && token.revision > before.revision
                    })
                && service.source(EditorAssetKind::Main).is_some()
            {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "unchanged main was not revalidated after shared stream reconfiguration"
            );
            thread::sleep(Duration::from_millis(2));
        }
        service.shutdown();
        std::fs::remove_dir_all(directory).unwrap();
    }
}
