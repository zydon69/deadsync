//! Authenticated, optional loopback bridge with live host snapshots. Each MCP
//! client owns an isolated draft; only the native editor publishes proposals.
use super::agent::Draft;
use super::*;
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{Condvar, Mutex, OnceLock, atomic::AtomicUsize},
    time::Duration,
};
type Materializer = Box<dyn FnOnce() -> Result<crate::document::Document, Error> + Send>;
struct DeferredDocument {
    source: Mutex<Option<Materializer>>,
    document: OnceLock<Result<crate::document::Document, String>>,
}
impl DeferredDocument {
    fn resolve(&self) -> Result<&crate::document::Document, String> {
        self.document
            .get_or_init(|| {
                let materialize = self
                    .source
                    .lock()
                    .map_err(|_| "Snapshot unavailable")?
                    .take()
                    .ok_or("Snapshot already consumed")?;
                materialize().map_err(|error| error.to_string())
            })
            .as_ref()
            .map_err(Clone::clone)
    }
}
#[derive(Clone)]
struct Snapshot {
    epoch: Arc<()>,
    token: RevisionToken,
    document: Option<Arc<DeferredDocument>>,
    base: PathBuf,
    chart: Option<usize>,
}
impl Snapshot {
    fn capture(workspace: &Workspace) -> Result<Self, Error> {
        let session = &workspace.file().session;
        Ok(Self {
            epoch: Arc::new(()),
            token: session.revision_token(),
            document: None,
            base: workspace.file().resource_base().to_owned(),
            chart: workspace.active_chart(),
        })
    }
    fn workspace(&self) -> Result<Workspace, Error> {
        let file = crate::storage::FileSession::from_new(crate::file_origin::NewSong {
            origin: crate::file_origin::Origin::Untitled {
                resource_base: self.base.clone(),
            },
            session: crate::history::Session::new(
                self.document
                    .as_ref()
                    .ok_or_else(|| Error::InvalidEdit("Snapshot not requested".into()))?
                    .resolve()
                    .map_err(Error::InvalidEdit)?
                    .clone(),
            ),
        })?;
        match self.chart {
            Some(chart) => Workspace::from_file(file, chart),
            None => Ok(Workspace::from_file_unfocused(file)),
        }
    }
}
/// Metadata is cheap to refresh on every edit. An authenticated tool request
/// raises demand; the next UI synchronize copies editable state exactly once.
struct Host {
    state: Mutex<Snapshot>,
    requested: AtomicBool,
    invalidated: AtomicBool,
    discard: AtomicBool,
    ready: Condvar,
}
impl Host {
    fn lock(&self) -> std::sync::LockResult<std::sync::MutexGuard<'_, Snapshot>> {
        self.state.lock()
    }
    fn try_lock(&self) -> std::sync::TryLockResult<std::sync::MutexGuard<'_, Snapshot>> {
        self.state.try_lock()
    }
    fn invalidate(&self, host: &mut Snapshot) {
        host.epoch = Arc::new(());
        host.document = None;
    }
    fn snapshot(&self, cancelled: &AtomicBool) -> Result<Snapshot, String> {
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let mut host = self.lock().map_err(|_| "Host snapshot unavailable")?;
        loop {
            if cancelled.load(Ordering::Acquire) {
                return Err("Assistant bridge stopped".into());
            }
            if !self.invalidated.load(Ordering::Acquire) && host.document.is_some() {
                return Ok(host.clone());
            }
            self.requested.store(true, Ordering::Release);
            self.ready.notify_all();
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return Err(
                    "Editor did not provide a snapshot; keep the editor running and retry".into(),
                );
            }
            host = self
                .ready
                // Shutdown must not rely on winning the narrow notify/wait
                // race: periodically recheck the cancellation flag as well.
                .wait_timeout(host, remaining.min(Duration::from_millis(100)))
                .map_err(|_| "Host snapshot unavailable")?
                .0;
        }
    }
    fn refreshed_for_publication(
        &self,
        snapshot: &Snapshot,
        cancelled: &AtomicBool,
    ) -> Result<std::sync::MutexGuard<'_, Snapshot>, String> {
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let mut host = self.lock().map_err(|_| "Host snapshot unavailable")?;
        loop {
            if cancelled.load(Ordering::Acquire) {
                return Err("Assistant bridge stopped".into());
            }
            if !Arc::ptr_eq(&host.epoch, &snapshot.epoch) {
                return Err(
                    "Host changed or proposal was discarded; retry against the current song".into(),
                );
            }
            if !self.invalidated.load(Ordering::Acquire) {
                return Ok(host);
            }
            self.requested.store(true, Ordering::Release);
            self.ready.notify_all();
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return Err(
                    "Editor did not refresh the host; retry against the current song".into(),
                );
            }
            host = self
                .ready
                .wait_timeout(host, remaining.min(Duration::from_millis(100)))
                .map_err(|_| "Host snapshot unavailable")?
                .0;
        }
    }
    fn is_current(&self, host: &Snapshot, snapshot: &Snapshot) -> bool {
        !self.invalidated.load(Ordering::Acquire) && Arc::ptr_eq(&host.epoch, &snapshot.epoch)
    }
}
struct Proposal {
    epoch: Arc<()>,
    draft: Draft,
}
struct Client {
    epoch: Arc<()>,
    workspace: Workspace,
    toolbox: Toolbox,
}
pub struct Bridge {
    registry: PathBuf,
    secret: String,
    cancelled: Arc<AtomicBool>,
    host: Arc<Host>,
    draft: Arc<Mutex<Option<Proposal>>>,
    failure: Arc<Mutex<Option<String>>>,
}
fn registry_lock(path: &Path) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path.with_extension("lock"))?;
    file.lock()?;
    Ok(file)
}
impl Bridge {
    pub fn start(workspace: &Workspace) -> Result<Self, Error> {
        Self::start_at(workspace, default_connection_path()?)
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn start_for_test(workspace: &Workspace, registry: PathBuf) -> Result<Self, Error> {
        Self::start_at(workspace, registry)
    }
    fn start_at(workspace: &Workspace, registry: PathBuf) -> Result<Self, Error> {
        let host = Arc::new(Host {
            state: Mutex::new(Snapshot::capture(workspace)?),
            requested: AtomicBool::new(false),
            invalidated: AtomicBool::new(false),
            discard: AtomicBool::new(false),
            ready: Condvar::new(),
        });
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let secret = uuid::Uuid::new_v4().to_string();
        let discovery =
            json!({"address":listener.local_addr()?.to_string(),"token":secret,"version":1});
        let parent = registry
            .parent()
            .ok_or_else(|| Error::InvalidEdit("Invalid registry path".into()))?;
        std::fs::create_dir_all(parent)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
        }
        let lock = registry_lock(&registry)?;
        let mut publication = tempfile::NamedTempFile::new_in(parent)?;
        publication.write_all(discovery.to_string().as_bytes())?;
        publication.persist(&registry).map_err(|e| e.error)?;
        drop(lock);
        let cancelled = Arc::new(AtomicBool::new(false));
        let draft = Arc::new(Mutex::new(None));
        let failure = Arc::new(Mutex::new(None));
        let bridge = Self {
            registry,
            secret: secret.clone(),
            cancelled: cancelled.clone(),
            host: host.clone(),
            draft: draft.clone(),
            failure: failure.clone(),
        };
        let server = Server {
            registry: bridge.registry.clone(),
            failure,
            host,
            draft,
            cancelled,
            secret,
            clients: Mutex::new(HashMap::new()),
            active: Arc::new(AtomicUsize::new(0)),
            #[cfg(test)]
            before_publish: Mutex::new(None),
        };
        std::thread::Builder::new()
            .name("deadsync-mcp-bridge".into())
            .spawn(move || server.listen(listener))?;
        Ok(bridge)
    }
    /// Refresh on host revision/focus changes without changing address or token.
    pub fn synchronize(&self, workspace: &Workspace) -> Result<(), Error> {
        if let Ok(failure) = self.failure.try_lock() {
            if let Some(error) = failure.as_ref() {
                return Err(Error::InvalidEdit(error.clone()));
            }
        }
        let mut host = match self.host.try_lock() {
            Ok(host) => host,
            Err(std::sync::TryLockError::WouldBlock) => {
                // Block stale publication until a later frame can refresh
                // metadata. The UI never waits behind assistant workers.
                self.host.invalidated.store(true, Ordering::Release);
                return Ok(());
            }
            Err(_) => return Err(Error::InvalidEdit("Host snapshot unavailable".into())),
        };
        if !workspace.file().session.accepts_token(&host.token)
            || workspace.active_chart() != host.chart
            || workspace.file().resource_base() != host.base
        {
            *host = Snapshot::capture(workspace)?;
        }
        if self.host.discard.swap(false, Ordering::AcqRel) {
            self.host.invalidate(&mut host);
        }
        if self.host.requested.swap(false, Ordering::AcqRel) && host.document.is_none() {
            host.document = Some(Arc::new(DeferredDocument {
                source: Mutex::new(Some(Box::new(workspace.file().session.deferred_snapshot()))),
                document: OnceLock::new(),
            }));
        }
        self.host.invalidated.store(false, Ordering::Release);
        self.host.ready.notify_all();
        Ok(())
    }
    /// Reject every detached client draft, including an in-flight publication.
    /// Advancing the epoch prevents the rejected edit from resurfacing later.
    pub fn discard_proposals(&self) -> Result<(), Error> {
        self.host.invalidated.store(true, Ordering::Release);
        self.host.discard.store(true, Ordering::Release);
        if let Ok(mut host) = self.host.try_lock() {
            self.host.invalidate(&mut host);
            self.host.discard.store(false, Ordering::Release);
            // Keep publication blocked until synchronize refreshes metadata.
        }
        if let Ok(mut draft) = self.draft.try_lock() {
            *draft = None;
        }
        self.host.ready.notify_all();
        Ok(())
    }
    pub fn discovery_path(&self) -> PathBuf {
        self.registry.clone()
    }
    pub fn poll(&self) -> Option<Result<Draft, String>> {
        if let Some(error) = self.failure.try_lock().ok()?.take() {
            return Some(Err(error));
        }
        let host = self.host.try_lock().ok()?;
        if self.host.invalidated.load(Ordering::Acquire) {
            return None;
        }
        let proposal = self.draft.try_lock().ok()?.take()?;
        Arc::ptr_eq(&proposal.epoch, &host.epoch).then_some(Ok(proposal.draft))
    }
}
struct Server {
    registry: PathBuf,
    failure: Arc<Mutex<Option<String>>>,
    host: Arc<Host>,
    draft: Arc<Mutex<Option<Proposal>>>,
    cancelled: Arc<AtomicBool>,
    secret: String,
    clients: Mutex<HashMap<String, Arc<Mutex<Client>>>>,
    active: Arc<AtomicUsize>,
    #[cfg(test)]
    before_publish: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}
struct RequestLease(Arc<AtomicUsize>);
impl Drop for RequestLease {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
impl Server {
    fn listen(self, listener: TcpListener) {
        let server = Arc::new(self);
        let (rejected, rejections) = std::sync::mpsc::sync_channel::<TcpStream>(8);
        let stopped = server.cancelled.clone();
        if let Err(error) = std::thread::Builder::new()
            .name("deadsync-mcp-busy".into())
            .spawn(move || {
                while !stopped.load(Ordering::Acquire) {
                    match rejections.recv_timeout(Duration::from_millis(25)) {
                        Ok(mut stream) => {
                            let _ = reject_busy(&mut stream);
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            })
        {
            server.listener_failed(error);
            return;
        }
        while !server.cancelled.load(Ordering::Acquire) {
            let (stream, _) = match listener.accept() {
                Ok(pair) => pair,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(25));
                    continue;
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => {
                    server.listener_failed(error);
                    break;
                }
            };
            if server.active.load(Ordering::Acquire) >= 8 {
                // Admission never waits for a slow rejected peer. Both the
                // rejection queue and its worker count are bounded; overflow
                // closes excess connections rather than allocating threads.
                let _ = rejected.try_send(stream);
                continue;
            }
            // Keep a descriptor for an allocation/thread-pressure fallback.
            // The worker receives its own descriptor; closing the listener's
            // copy never aborts a successfully dispatched connection.
            let worker_stream = match stream.try_clone() {
                Ok(stream) => stream,
                Err(_) => {
                    server.serve(stream);
                    continue;
                }
            };
            server.active.fetch_add(1, Ordering::AcqRel);
            let lease = RequestLease(server.active.clone());
            let worker = server.clone();
            let spawned = std::thread::Builder::new()
                .name("deadsync-mcp-request".into())
                .spawn(move || {
                    let _lease = lease;
                    worker.serve(worker_stream);
                });
            if spawned.is_err() {
                // Degrade to the listener only when the OS cannot create a
                // worker. Preserve the request and its MCP response in full.
                server.serve(stream);
            }
        }
    }
    fn listener_failed(&self, error: std::io::Error) {
        self.cancelled.store(true, Ordering::Release);
        self.host.ready.notify_all();
        if let Ok(mut failure) = self.failure.lock() {
            *failure = Some(format!(
                "MCP listener stopped: {error}. Toggle MCP off and on to restart."
            ));
        }
        remove_owned_registry(&self.registry, &self.secret);
    }
    fn serve(&self, mut stream: TcpStream) {
        let result = (|| -> Result<(), String> {
            // BSD/macOS accepts inherit the listener's nonblocking flag. A
            // request worker must wait for the complete bounded frame instead
            // of treating ordinary packet arrival latency as a bridge failure.
            stream.set_nonblocking(false).map_err(|e| e.to_string())?;
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .map_err(|e| e.to_string())?;
            stream
                .set_write_timeout(Some(Duration::from_secs(3)))
                .map_err(|e| e.to_string())?;
            let envelope = read_frame(&mut BufReader::new(&stream))?;
            if !valid_token(envelope["token"].as_str(), &self.secret) {
                return Err("Invalid bridge token".into());
            }
            let request = &envelope["request"];
            let response = match self.respond(&envelope) {
                Ok(value) => value,
                Err(error) => Some(tool_error(request, &error)),
            };
            if let Some(response) = response {
                write_frame(&mut stream, &response)?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            let _ = write_frame(&mut stream, &json!({"bridge_error":error}));
        }
    }
    fn client(&self, key: &str, snapshot: &Snapshot) -> Result<Arc<Mutex<Client>>, String> {
        if key.len() > 128 {
            return Err("Invalid client identifier".into());
        }
        let mut clients = self
            .clients
            .lock()
            .map_err(|_| "Client registry unavailable")?;
        if !clients.contains_key(key) {
            if clients.len() >= 8 {
                let idle = clients
                    .iter()
                    .find(|(_, v)| Arc::strong_count(v) == 1)
                    .map(|(key, _)| key.clone())
                    .ok_or("Too many active MCP clients")?;
                clients.remove(&idle);
            }
            clients.insert(key.to_owned(), Arc::new(Mutex::new(Client::new(snapshot)?)));
        }
        Ok(clients[key].clone())
    }
    fn respond(&self, envelope: &Value) -> Result<Option<Value>, String> {
        let request = &envelope["request"];
        if let Some(response) = protocol::response_without_workspace(request) {
            return Ok(response);
        }
        let snapshot = self.host.snapshot(&self.cancelled)?;
        let client = self.client(envelope["client"].as_str().unwrap_or("legacy"), &snapshot)?;
        let mut client = client.lock().map_err(|_| "Client state unavailable")?;
        let current = self.host.lock().map_err(|_| "Host snapshot unavailable")?;
        let is_current = self.host.is_current(&current, &snapshot);
        drop(current);
        if !is_current {
            return Err(
                "Host changed or proposal was discarded; retry against the current song".into(),
            );
        }
        if !Arc::ptr_eq(&client.epoch, &snapshot.epoch) {
            *client = Client::new(&snapshot)?;
        }
        let mutation = request["method"] == "tools/call"
            && matches!(
                request["params"]["name"].as_str(),
                Some("apply_preview" | "undo" | "redo")
            );
        // Reserve before mutation: backpressure leaves preview/history intact.
        let mut publication = if mutation {
            Some(
                self.draft
                    .lock()
                    .map_err(|_| "Proposal mailbox unavailable")?,
            )
        } else {
            None
        };
        if let Some(slot) = publication.as_mut() {
            if slot
                .as_ref()
                .is_some_and(|proposal| !Arc::ptr_eq(&proposal.epoch, &snapshot.epoch))
            {
                **slot = None;
            }
        }
        if publication.as_ref().is_some_and(|slot| slot.is_some()) {
            return Err(
                "Review or discard the pending MCP proposal before publishing another".into(),
            );
        }
        let Client {
            workspace,
            toolbox,
            epoch,
        } = &mut *client;
        let before = workspace.file().session.revision();
        let response = protocol::response_cancellable(
            request.clone(),
            workspace,
            toolbox,
            false,
            &self.cancelled,
        );
        if before != workspace.file().session.revision() {
            // Any failure after a detached mutation must force a fresh client
            // next time, even when the native revision itself did not change.
            *epoch = Arc::new(());
            if response
                .as_ref()
                .is_some_and(|value| value["result"]["isError"] == false)
            {
                let proposal = Draft {
                    token: snapshot.token.clone(),
                    bytes: workspace
                        .file()
                        .session
                        .serialize()
                        .map_err(|error| error.to_string())?,
                    message: "MCP proposal".into(),
                };
                #[cfg(test)]
                if let Some(hook) = self.before_publish.lock().unwrap().take() {
                    hook();
                }
                if let Some(slot) = publication.as_mut() {
                    // A contended UI frame temporarily blocks publication. Wait
                    // on the worker, releasing the host lock so UI can refresh.
                    let unchanged = proposal.bytes.as_slice()
                        == snapshot
                            .document
                            .as_ref()
                            .ok_or("Snapshot not requested")?
                            .resolve()?
                            .source()
                            .as_ref();
                    let _host = self
                        .host
                        .refreshed_for_publication(&snapshot, &self.cancelled)?;
                    **slot = (!unchanged).then_some(Proposal {
                        epoch: snapshot.epoch.clone(),
                        draft: proposal,
                    });
                    *epoch = snapshot.epoch.clone();
                }
            }
        }
        Ok(response)
    }
}
impl Client {
    fn new(snapshot: &Snapshot) -> Result<Self, String> {
        Ok(Self {
            epoch: snapshot.epoch.clone(),
            workspace: snapshot.workspace().map_err(|e| e.to_string())?,
            toolbox: Toolbox::default(),
        })
    }
}
fn tool_error(request: &Value, message: &str) -> Value {
    if request["method"] != "tools/call" {
        return json!({"jsonrpc":"2.0","id":request["id"],"error":{"code":-32000,"message":message}});
    }
    json!({"jsonrpc":"2.0","id":request["id"],"result":{"isError":true,"content":[{"type":"text","text":message}]}})
}
impl Drop for Bridge {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        self.host.ready.notify_all();
        remove_owned_registry(&self.registry, &self.secret);
    }
}
fn remove_owned_registry(registry: &Path, secret: &str) {
    if let Ok(_lock) = registry_lock(registry) {
        let owned = std::fs::read(registry)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .is_some_and(|value| valid_token(value["token"].as_str(), secret));
        if owned {
            let _ = std::fs::remove_file(registry);
        }
    }
}
fn valid_token(candidate: Option<&str>, secret: &str) -> bool {
    // Length is public (UUID). subtle prevents optimizer-induced short-circuit
    // comparisons of the fixed-length secret bytes.
    use subtle::ConstantTimeEq;
    candidate.is_some_and(|candidate| bool::from(candidate.as_bytes().ct_eq(secret.as_bytes())))
}
fn read_frame(input: &mut impl BufRead) -> Result<Value, String> {
    let mut bytes = Vec::new();
    std::io::Read::take(input, (protocol::MAX_FRAME + 1) as u64)
        .read_until(b'\n', &mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.is_empty() || bytes.len() > protocol::MAX_FRAME {
        return Err("Invalid bridge frame size".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
fn reject_busy(stream: &mut TcpStream) -> Result<(), String> {
    // Consume the bounded request before closing: dropping a TCP socket with
    // unread request bytes can reset it and discard the useful busy response.
    // Admission itself remains bounded even for a peer sending no data.
    stream.set_nonblocking(false).map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_millis(100)))
        .map_err(|e| e.to_string())?;
    let deadline = std::time::Instant::now() + Duration::from_millis(100);
    let mut remaining_bytes = protocol::MAX_FRAME + 1;
    let mut chunk = [0u8; 2048];
    while remaining_bytes > 0 {
        let remaining_time = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining_time.is_zero() {
            break;
        }
        stream
            .set_read_timeout(Some(remaining_time))
            .map_err(|e| e.to_string())?;
        let bound = chunk.len().min(remaining_bytes);
        match std::io::Read::read(stream, &mut chunk[..bound]) {
            Ok(0) => break,
            Ok(count) => {
                remaining_bytes -= count;
                if chunk[..count].contains(&b'\n') {
                    break;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    write_frame(
        stream,
        &json!({"bridge_error":"MCP bridge is busy (8 active requests); retry shortly"}),
    )
}
fn write_frame(output: &mut impl Write, value: &Value) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    if bytes.len() > protocol::MAX_FRAME {
        return Err("Bridge response exceeds 1 MiB; request a smaller page".into());
    }
    output
        .write_all(&bytes)
        .and_then(|_| output.write_all(b"\n"))
        .and_then(|_| output.flush())
        .map_err(|e| e.to_string())
}
pub fn connect(path: &Path, mut input: impl BufRead, mut output: impl Write) -> Result<(), String> {
    let client = uuid::Uuid::new_v4().to_string();
    loop {
        if input.fill_buf().map_err(|e| e.to_string())?.is_empty() {
            return Ok(());
        }
        let mut frame = Vec::new();
        std::io::Read::take(&mut input, (protocol::MAX_FRAME + 1) as u64)
            .read_until(b'\n', &mut frame)
            .map_err(|e| e.to_string())?;
        if frame.len() > protocol::MAX_FRAME {
            // Discard the rest of this line without retaining unbounded input.
            // A newline already read belongs to this frame, not the next one.
            if frame.last() != Some(&b'\n') {
                input.skip_until(b'\n').map_err(|e| e.to_string())?;
            }
            write_frame(
                &mut output,
                &json!({"jsonrpc":"2.0","id":null,"error":{
                    "code":-32700,"message":"MCP frame exceeds 1 MiB"
                }}),
            )?;
            continue;
        }
        if frame.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let request: Value = match serde_json::from_slice(&frame) {
            Ok(request) => request,
            Err(_) => {
                write_frame(
                    &mut output,
                    &json!({
                        "jsonrpc": "2.0", "id": null,
                        "error": { "code": -32700, "message": "Invalid JSON" }
                    }),
                )?;
                continue;
            }
        };
        if let Some(response) = protocol::response_without_workspace(&request) {
            if let Some(response) = response {
                write_frame(&mut output, &response)?;
            }
            continue;
        }
        let result = (|| -> Result<Value, String> {
            let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
            if metadata.len() > 4096 {
                return Err("Invalid discovery file".into());
            }
            let discovery: Value =
                serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            let address: std::net::SocketAddr = discovery["address"]
                .as_str()
                .ok_or("Missing address")?
                .parse()
                .map_err(|_| "Invalid address")?;
            if !address.ip().is_loopback() {
                return Err("Bridge address must be loopback".into());
            }
            let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(3))
                .map_err(|e| e.to_string())?;
            stream
                .set_read_timeout(Some(Duration::from_secs(3600)))
                .map_err(|e| e.to_string())?;
            write_frame(
                &mut stream,
                &json!({"token":discovery["token"],"client":client,"request":request}),
            )?;
            let response = read_frame(&mut BufReader::new(stream))?;
            if let Some(error) = response.get("bridge_error") {
                return Err(error.to_string());
            }
            Ok(response)
        })();
        let response = match result {
            Ok(v) => v,
            Err(e) => tool_error(&request, &e),
        };
        write_frame(&mut output, &response)?;
    }
}

/// Stable per-user discovery; the most recently opened editor bridge owns it.
pub fn default_connection_path() -> Result<PathBuf, Error> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .ok_or_else(|| {
            Error::InvalidEdit("Cannot locate user home for the editor connection".into())
        })?;
    let home = PathBuf::from(home);
    #[cfg(target_os = "macos")]
    let directory = home.join("Library/Caches/DeadSync/assistant");
    #[cfg(not(target_os = "macos"))]
    let directory = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .map(|p| p.join("deadsync-assistant"))
        .unwrap_or_else(|| home.join(".cache/deadsync/assistant"));
    Ok(directory.join("connection.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn connected_responses(
        path: &Path,
        input: Vec<u8>,
        after_response: impl FnMut(usize),
    ) -> Vec<Value> {
        struct Output<F> {
            bytes: Vec<u8>,
            responses: usize,
            after_response: F,
        }
        impl<F: FnMut(usize)> Write for Output<F> {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.bytes.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                self.responses += 1;
                (self.after_response)(self.responses);
                Ok(())
            }
        }
        let mut output = Output {
            bytes: Vec::new(),
            responses: 0,
            after_response,
        };
        connect(path, std::io::Cursor::new(input), &mut output).unwrap();
        output
            .bytes
            .split(|b| *b == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_slice(line).unwrap())
            .collect()
    }
    #[test]
    fn connected_helper_initializes_before_f4_and_recovers_without_restarting() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("connection.json");
        let host = workspace();
        let mut bridge = None;
        let requests = [
            json!({"jsonrpc":"2.0","id":1,"method":"initialize"}),
            json!({"jsonrpc":"2.0","id":2,"method":"ping"}),
            json!({"jsonrpc":"2.0","id":3,"method":"tools/list"}),
            json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"inspect_song"}}),
            json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"inspect_song"}}),
        ];
        let mut input = Vec::new();
        for request in requests {
            write_frame(&mut input, &request).unwrap();
        }
        let responses = connected_responses(&path, input, |count| {
            if count == 4 {
                // Simulate enabling F4 after the helper was already initialized.
                let enabled = Bridge::start_at(&host, path.clone()).unwrap();
                enabled.host.requested.store(true, Ordering::Release);
                enabled.synchronize(&host).unwrap();
                bridge = Some(enabled);
            }
        });
        assert_eq!(responses.len(), 5);
        assert_eq!(
            responses[0]["result"]["serverInfo"]["name"],
            "deadsync-editor"
        );
        assert_eq!(responses[1]["result"], json!({}));
        assert!(responses[2]["result"]["tools"].as_array().unwrap().len() > 1);
        assert_eq!(responses[3]["result"]["isError"], true);
        assert_eq!(responses[4]["id"], 5);
        assert_eq!(responses[4]["result"]["isError"], false);
        drop(bridge);
    }
    #[test]
    fn oversized_connected_input_is_drained_and_following_requests_survive() {
        let home = tempfile::tempdir().unwrap();
        for excess in [1, 8192] {
            let mut input = vec![b'x'; protocol::MAX_FRAME + excess - 1];
            input.push(b'\n');
            write_frame(&mut input, &json!({"jsonrpc":"2.0","id":9,"method":"ping"})).unwrap();
            let responses = connected_responses(&home.path().join("absent.json"), input, |_| {});
            assert_eq!(responses.len(), 2);
            assert_eq!(responses[0]["error"]["code"], -32700);
            assert!(responses[0]["id"].is_null());
            assert_eq!(responses[1]["id"], 9);
            assert_eq!(responses[1]["result"], json!({}));
        }
    }
    #[test]
    fn saturated_bridge_returns_busy_and_the_same_helper_can_retry() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("connection.json");
        let host = workspace();
        let mut snapshot = Snapshot::capture(&host).unwrap();
        snapshot.document = Some(Arc::new(DeferredDocument {
            source: Mutex::new(Some(Box::new(host.file().session.deferred_snapshot()))),
            document: OnceLock::new(),
        }));
        let active = Arc::new(AtomicUsize::new(8));
        let cancelled = Arc::new(AtomicBool::new(false));
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let secret = "busy-test-fixture".to_string();
        std::fs::write(
            &path,
            json!({"address":listener.local_addr().unwrap().to_string(),"token":secret})
                .to_string(),
        )
        .unwrap();
        let server = Server {
            registry: path.clone(),
            secret,
            host: Arc::new(Host {
                state: Mutex::new(snapshot),
                requested: AtomicBool::new(false),
                invalidated: AtomicBool::new(false),
                discard: AtomicBool::new(false),
                ready: Condvar::new(),
            }),
            draft: Arc::new(Mutex::new(None)),
            failure: Arc::new(Mutex::new(None)),
            cancelled: cancelled.clone(),
            clients: Mutex::new(HashMap::new()),
            active: active.clone(),
            before_publish: Mutex::new(None),
        };
        let worker = std::thread::spawn(move || server.listen(listener));
        let mut input = Vec::new();
        for id in [1, 2] {
            write_frame(&mut input, &json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"inspect_song"}})).unwrap();
        }
        let responses = connected_responses(&path, input, |count| {
            if count == 1 {
                active.store(0, Ordering::Release);
            }
        });
        cancelled.store(true, Ordering::Release);
        worker.join().unwrap();
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0]["result"]["isError"], true);
        assert!(
            responses[0]["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("busy"),
            "{}",
            responses[0]
        );
        assert_eq!(responses[1]["id"], 2);
        assert_eq!(responses[1]["result"]["isError"], false);
    }
    fn workspace() -> Workspace {
        let document=crate::document::Document::parse(Arc::<[u8]>::from(b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Edit;#METER:1;#NOTES:1000;".as_slice()),crate::document::Format::Ssc).unwrap();
        let file = crate::storage::FileSession::from_new(crate::file_origin::NewSong {
            origin: crate::file_origin::Origin::Untitled {
                resource_base: std::env::temp_dir(),
            },
            session: crate::history::Session::new(document),
        })
        .unwrap();
        Workspace::from_file(file, 0).unwrap()
    }
    fn wire_request(path: &Path, tool: &str, arguments: Value) -> Value {
        wire_request_with_client(path, "regression", tool, arguments)
    }
    fn wire_request_with_client(path: &Path, client: &str, tool: &str, arguments: Value) -> Value {
        let connection: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let mut stream = TcpStream::connect(connection["address"].as_str().unwrap()).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        write_frame(&mut stream,&json!({"token":connection["token"],"client":client,"request":{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":tool,"arguments":arguments}}})).unwrap();
        let response = read_frame(&mut BufReader::new(stream)).unwrap();
        if response["result"]["isError"] == true || response.get("error").is_some() {
            eprintln!("Bridge fixture request {tool}: {response}");
        }
        response
    }
    fn request(
        bridge: &Bridge,
        host: &Workspace,
        path: &Path,
        tool: &str,
        arguments: Value,
    ) -> Value {
        std::thread::scope(|scope| {
            let (send, receive) = std::sync::mpsc::channel();
            scope.spawn(move || {
                send.send(wire_request(path, tool, arguments)).unwrap();
            });
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            loop {
                bridge.synchronize(host).unwrap();
                match receive.recv_timeout(Duration::from_millis(1)) {
                    Ok(response) => return response,
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        assert!(
                            std::time::Instant::now() < deadline,
                            "request did not finish while UI was pumping"
                        );
                    }
                    Err(error) => panic!("request worker stopped: {error}"),
                }
            }
        })
    }
    #[test]
    fn host_changes_refresh_without_restarting_bridge_and_registry_is_removed() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("connection.json");
        let mut host = workspace();
        let bridge = Bridge::start_at(&host, path.clone()).unwrap();
        let initial = std::fs::read(&path).unwrap();
        assert_eq!(
            request(&bridge, &host, &path, "inspect_song", json!({}))["result"]["structuredContent"]["charts"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        host.execute(Command::DuplicateChart).unwrap();
        bridge.synchronize(&host).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), initial);
        assert_eq!(
            request(&bridge, &host, &path, "inspect_song", json!({}))["result"]["structuredContent"]["charts"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        drop(bridge);
        assert!(!path.exists());
    }
    #[test]
    fn ui_poll_does_not_wait_for_a_worker_publication() {
        let home = tempfile::tempdir().unwrap();
        let bridge = Bridge::start_at(&workspace(), home.path().join("connection.json")).unwrap();
        let publication = bridge.draft.lock().unwrap();
        std::thread::scope(|scope| {
            let (send, receive) = std::sync::mpsc::channel();
            let bridge = &bridge;
            scope.spawn(move || {
                send.send(bridge.poll()).unwrap();
            });
            let response = receive.recv_timeout(Duration::from_secs(2));
            drop(publication);
            assert!(
                response
                    .expect("UI poll waited for the worker permit")
                    .is_none()
            );
        });
    }

    #[test]
    fn full_mailbox_rejects_before_mutation_and_client_can_retry() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("connection.json");
        let host = workspace();
        let bridge = Bridge::start_at(&host, path.clone()).unwrap();
        let prepare = |row| {
            request(
                &bridge,
                &host,
                &path,
                "preview_note_edits",
                json!({"chart":0,"edits":[{"row":row,"lane":1,"kind":"tap"}]}),
            )["result"]["structuredContent"]["preview_id"]
                .clone()
        };
        let preview = prepare(48);
        assert_eq!(
            request(
                &bridge,
                &host,
                &path,
                "apply_preview",
                json!({"preview_id":preview})
            )["result"]["isError"],
            false
        );
        let second = prepare(96);
        assert_eq!(
            request(
                &bridge,
                &host,
                &path,
                "apply_preview",
                json!({"preview_id":second})
            )["result"]["isError"],
            true
        );
        assert_eq!(
            request(&bridge, &host, &path, "inspect_chart", json!({"chart":0}))["result"]["structuredContent"]
                ["total_notes"],
            2
        );
        assert!(bridge.poll().is_some());
        assert_eq!(
            request(
                &bridge,
                &host,
                &path,
                "apply_preview",
                json!({"preview_id":second})
            )["result"]["isError"],
            false
        );
        assert_eq!(
            request(&bridge, &host, &path, "inspect_chart", json!({"chart":0}))["result"]["structuredContent"]
                ["total_notes"],
            3
        );
    }
    #[test]
    fn closing_old_bridge_does_not_remove_new_owner_registry() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("connection.json");
        let host = workspace();
        let old = Bridge::start_at(&host, path.clone()).unwrap();
        let new = Bridge::start_at(&host, path.clone()).unwrap();
        drop(old);
        assert_eq!(
            request(&new, &host, &path, "inspect_song", json!({}))["result"]["isError"],
            false
        );
        drop(new);
        assert!(!path.exists());
    }
    #[test]
    fn slow_unauthenticated_client_does_not_block_other_requests() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("connection.json");
        let host = workspace();
        let bridge = Bridge::start_at(&host, path.clone()).unwrap();
        let info: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let slow = TcpStream::connect(info["address"].as_str().unwrap()).unwrap();
        slow.set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
        let error = slow.peek(&mut [0u8; 1]).expect_err(
            "the bridge must wait for a frame instead of immediately rejecting a delayed client",
        );
        assert!(matches!(
            error.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
        ));
        let start = std::time::Instant::now();
        assert_eq!(
            request(&bridge, &host, &path, "inspect_song", json!({}))["result"]["isError"],
            false
        );
        assert!(start.elapsed() < Duration::from_secs(2));
    }
    #[test]
    fn rejected_proposal_is_not_reintroduced_by_the_next_edit() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("connection.json");
        let mut host = workspace();
        let bridge = Bridge::start_at(&host, path.clone()).unwrap();
        let publish = |row| {
            let preview = request(
                &bridge,
                &host,
                &path,
                "preview_note_edits",
                json!({
                    "chart":0, "edits":[{"row":row,"lane":1,"kind":"tap"}]
                }),
            )["result"]["structuredContent"]["preview_id"]
                .clone();
            assert_eq!(
                request(
                    &bridge,
                    &host,
                    &path,
                    "apply_preview",
                    json!({"preview_id":preview})
                )["result"]["isError"],
                false
            );
        };
        publish(48);
        assert!(bridge.poll().is_some());
        bridge.discard_proposals().unwrap();
        publish(96);
        assert_eq!(
            request(&bridge, &host, &path, "inspect_chart", json!({"chart":0}))["result"]["structuredContent"]
                ["total_notes"],
            2
        );
        let draft = bridge.poll().unwrap().unwrap();
        draft.apply(&mut host).unwrap();
        let rows: Vec<_> = host
            .grid()
            .iter()
            .map(|(position, _)| position.row)
            .collect();
        assert_eq!(rows, vec![0, 96]);
        assert_eq!(host.active_chart(), Some(0));
        host.execute(Command::Undo).unwrap();
        assert_eq!(host.grid().iter().count(), 1);
        assert_eq!(host.active_chart(), Some(0));
        host.execute(Command::Redo).unwrap();
        let rows: Vec<_> = host
            .grid()
            .iter()
            .map(|(position, _)| position.row)
            .collect();
        assert_eq!(rows, vec![0, 96]);
        assert_eq!(host.active_chart(), Some(0));
    }
    #[test]
    fn same_chart_count_proposal_keeps_a_closed_chart_unfocused_through_history() {
        let mut host = workspace();
        host.clear_active_chart();
        // Global SSC metadata must precede the first #NOTEDATA block.
        let mut bytes = b"#TITLE:Assistant metadata edit;".to_vec();
        bytes.extend(host.file().session.serialize().unwrap());
        let draft = Draft {
            token: host.file().session.revision_token(),
            bytes,
            message: String::new(),
        };
        draft.apply(&mut host).unwrap();
        assert_eq!(host.active_chart(), None);
        assert_eq!(
            host.file().session.document().text(None, "TITLE"),
            Some("Assistant metadata edit".into())
        );
        host.execute(Command::Undo).unwrap();
        assert_eq!(host.active_chart(), None);
        assert_eq!(host.file().session.document().text(None, "TITLE"), None);
        host.execute(Command::Redo).unwrap();
        assert_eq!(host.active_chart(), None);
        assert_eq!(
            host.file().session.document().text(None, "TITLE"),
            Some("Assistant metadata edit".into())
        );
    }
    #[test]
    fn rejecting_does_not_wait_for_an_inflight_worker_or_publish_its_old_epoch() {
        let home = tempfile::tempdir().unwrap();
        let bridge = Bridge::start_at(&workspace(), home.path().join("connection.json")).unwrap();
        let epoch = bridge.host.lock().unwrap().epoch.clone();
        let mut publication = bridge.draft.lock().unwrap();
        bridge.discard_proposals().unwrap();
        *publication = Some(Proposal {
            epoch,
            draft: Draft {
                token: workspace().file().session.revision_token(),
                bytes: vec![],
                message: String::new(),
            },
        });
        drop(publication);
        assert!(bridge.poll().is_none());
    }
    #[test]
    fn only_authenticated_tool_demand_captures_an_edited_song_and_clients_share_snapshot() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("connection.json");
        let mut host = workspace();
        let bridge = Bridge::start_at(&host, path.clone()).unwrap();
        assert!(bridge.host.lock().unwrap().document.is_none());
        for row in 1..20 {
            host.execute(Command::Seek(row * 48)).unwrap();
            host.execute(Command::Place(1)).unwrap();
            bridge.synchronize(&host).unwrap();
            assert!(bridge.host.lock().unwrap().document.is_none());
        }
        assert!(!bridge.host.requested.load(Ordering::Acquire));
        assert_eq!(
            request(&bridge, &host, &path, "inspect_song", json!({}))["result"]["isError"],
            false
        );
        let snapshot = bridge.host.lock().unwrap().clone();
        assert_eq!(
            wire_request_with_client(&path, "second", "inspect_song", json!({}))["result"]["isError"],
            false
        );
        let document = snapshot.document.as_ref().unwrap();
        assert!(document.document.get().is_some());
        assert!(Arc::ptr_eq(
            document,
            bridge.host.lock().unwrap().document.as_ref().unwrap()
        ));
        assert_eq!(snapshot.workspace().unwrap().grid().iter().count(), 20);
        host.execute(Command::Place(2)).unwrap();
        bridge.synchronize(&host).unwrap();
        assert!(bridge.host.lock().unwrap().document.is_none());
    }
    #[test]
    fn neutral_requests_and_unauthenticated_tools_never_request_a_snapshot() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("connection.json");
        let host = workspace();
        let bridge = Bridge::start_at(&host, path.clone()).unwrap();
        let connection: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        for (method, authenticated) in [
            ("initialize", true),
            ("ping", true),
            ("tools/list", true),
            ("tools/call", false),
        ] {
            let mut stream = TcpStream::connect(connection["address"].as_str().unwrap()).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let token = if authenticated {
                connection["token"].clone()
            } else {
                json!("incorrect")
            };
            write_frame(
                &mut stream,
                &json!({"token":token,"client":"neutral", "request":{
                    "jsonrpc":"2.0","id":1,"method":method,
                    "params":{"name":"inspect_song","arguments":{}}
                }}),
            )
            .unwrap();
            let response = read_frame(&mut BufReader::new(stream)).unwrap();
            if authenticated {
                assert!(response.get("result").is_some());
            } else {
                assert!(response.get("bridge_error").is_some());
            }
            assert!(!bridge.host.requested.load(Ordering::Acquire));
            bridge.synchronize(&host).unwrap();
            assert!(bridge.host.lock().unwrap().document.is_none());
        }
    }
    #[test]
    fn ui_operations_do_not_wait_behind_the_host_lock_and_defer_invalidation_safely() {
        let home = tempfile::tempdir().unwrap();
        let host = workspace();
        let bridge = Bridge::start_at(&host, home.path().join("connection.json")).unwrap();
        let held = bridge.host.lock().unwrap();
        let epoch = held.epoch.clone();
        std::thread::scope(|scope| {
            let (send, receive) = std::sync::mpsc::channel();
            let bridge = &bridge;
            let host = &host;
            scope.spawn(move || {
                bridge.synchronize(host).unwrap();
                assert!(bridge.poll().is_none());
                bridge.discard_proposals().unwrap();
                send.send(()).unwrap();
            });
            let result = receive.recv_timeout(Duration::from_secs(2));
            drop(held);
            result.expect("UI operation waited for host lock");
        });
        assert!(bridge.host.invalidated.load(Ordering::Acquire));
        bridge.synchronize(&host).unwrap();
        let state = bridge.host.lock().unwrap();
        assert!(!Arc::ptr_eq(&epoch, &state.epoch));
        assert!(!bridge.host.invalidated.load(Ordering::Acquire));
        assert!(state.document.is_none());
    }
    fn publication_after_contended_frame(cancel: bool) {
        let home = tempfile::tempdir().unwrap();
        let host = workspace();
        let bridge = Bridge::start_at(&host, home.path().join("connection.json")).unwrap();
        bridge.host.requested.store(true, Ordering::Release);
        bridge.synchronize(&host).unwrap();
        let server = Server {
            registry: bridge.registry.clone(),
            secret: bridge.secret.clone(),
            host: bridge.host.clone(),
            draft: bridge.draft.clone(),
            cancelled: bridge.cancelled.clone(),
            failure: bridge.failure.clone(),
            clients: Mutex::new(HashMap::new()),
            active: Arc::new(AtomicUsize::new(0)),
            before_publish: Mutex::new(None),
        };
        let envelope = |tool: &str, arguments: Value| {
            json!({"client":"contended", "request":{
                "jsonrpc":"2.0", "id":1, "method":"tools/call", "params":{"name":tool,"arguments":arguments}
            }})
        };
        let preview = server
            .respond(&envelope(
                "preview_note_edits",
                json!({
                    "chart":0,"edits":[{"row":48,"lane":1,"kind":"tap"}]
                }),
            ))
            .unwrap()
            .unwrap()["result"]["structuredContent"]["preview_id"]
            .clone();
        let epoch = bridge.host.lock().unwrap().epoch.clone();
        let (mutated_send, mutated_receive) = std::sync::mpsc::channel();
        let (release_send, release_receive) = std::sync::mpsc::channel();
        *server.before_publish.lock().unwrap() = Some(Box::new(move || {
            mutated_send.send(()).unwrap();
            release_receive
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
        }));
        std::thread::scope(|scope| {
            let (done_send, done_receive) = std::sync::mpsc::channel();
            let server = &server;
            let apply = envelope("apply_preview", json!({"preview_id":preview}));
            scope.spawn(move || {
                done_send.send(server.respond(&apply)).unwrap();
            });
            mutated_receive
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
            // The native song is unchanged; only a worker lock makes this UI
            // frame defer its metadata refresh after the client already edited.
            let held = bridge.host.lock().unwrap();
            bridge.synchronize(&host).unwrap();
            drop(held);
            assert!(bridge.host.invalidated.load(Ordering::Acquire));
            if cancel {
                bridge.cancelled.store(true, Ordering::Release);
            }
            release_send.send(()).unwrap();
            if !cancel {
                let mut state = bridge.host.lock().unwrap();
                let deadline = std::time::Instant::now() + Duration::from_secs(5);
                while !bridge.host.requested.load(Ordering::Acquire) {
                    let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                    assert!(
                        !remaining.is_zero(),
                        "publisher did not wait for UI refresh"
                    );
                    state = bridge.host.ready.wait_timeout(state, remaining).unwrap().0;
                }
                drop(state);
                assert!(matches!(
                    done_receive.try_recv(),
                    Err(std::sync::mpsc::TryRecvError::Empty)
                ));
                bridge.synchronize(&host).unwrap();
            }
            let response = done_receive.recv_timeout(Duration::from_secs(5)).unwrap();
            if cancel {
                assert!(response.is_err());
            } else {
                assert_eq!(response.unwrap().unwrap()["result"]["isError"], false);
            }
        });
        // Pure contention does not advance the host epoch.
        assert!(Arc::ptr_eq(&epoch, &bridge.host.lock().unwrap().epoch));
        if cancel {
            bridge.cancelled.store(false, Ordering::Release);
            bridge.synchronize(&host).unwrap();
            assert!(bridge.poll().is_none());
            let page = server
                .respond(&envelope("inspect_chart", json!({"chart":0})))
                .unwrap()
                .unwrap();
            assert_eq!(
                page["result"]["structuredContent"]["total_notes"], 1,
                "an unpublished mutation leaked through the next request"
            );
        } else {
            assert!(bridge.poll().is_some());
            let page = server
                .respond(&envelope("inspect_chart", json!({"chart":0})))
                .unwrap()
                .unwrap();
            assert_eq!(page["result"]["structuredContent"]["total_notes"], 2);
        }
    }
    #[test]
    fn contended_unchanged_host_refresh_still_publishes_the_client_mutation() {
        publication_after_contended_frame(false);
    }
    #[test]
    fn failed_publication_resets_client_even_when_host_epoch_is_unchanged() {
        publication_after_contended_frame(true);
    }
    #[test]
    fn deferred_snapshot_keeps_edited_notes_isolated_from_later_host_changes() {
        let mut host = workspace();
        host.execute(Command::Seek(48)).unwrap();
        host.execute(Command::Place(1)).unwrap();
        let mut snapshot = Snapshot::capture(&host).unwrap();
        snapshot.document = Some(Arc::new(DeferredDocument {
            source: Mutex::new(Some(Box::new(host.file().session.deferred_snapshot()))),
            document: OnceLock::new(),
        }));
        host.execute(Command::Seek(96)).unwrap();
        host.execute(Command::Place(1)).unwrap();
        let detached = snapshot.workspace().unwrap();
        let rows: Vec<_> = detached
            .grid()
            .iter()
            .map(|(position, _)| position.row)
            .collect();
        assert_eq!(rows, vec![0, 48]);
        assert_eq!(host.grid().iter().count(), 3);
    }
    #[test]
    fn listener_failure_is_reported_and_removes_only_its_discovery() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("connection.json");
        let host = workspace();
        let bridge = Bridge::start_at(&host, path.clone()).unwrap();
        let server = Server {
            registry: path.clone(),
            secret: bridge.secret.clone(),
            host: bridge.host.clone(),
            draft: bridge.draft.clone(),
            cancelled: bridge.cancelled.clone(),
            failure: bridge.failure.clone(),
            clients: Mutex::new(HashMap::new()),
            active: Arc::new(AtomicUsize::new(0)),
            #[cfg(test)]
            before_publish: Mutex::new(None),
        };
        server.listener_failed(std::io::Error::other("injected accept failure"));
        assert!(!path.exists());
        assert!(
            bridge
                .synchronize(&host)
                .unwrap_err()
                .to_string()
                .contains("injected accept failure")
        );
        assert!(
            matches!(bridge.poll(), Some(Err(error)) if error.contains("injected accept failure"))
        );
    }
    #[test]
    fn token_verification_rejects_prefixes_and_equal_length_mismatches() {
        let token = "ab4a67f0-4c8c-47dd-a3f7-ebc125d503a9";
        assert!(valid_token(Some(token), token));
        for position in 0..token.len() {
            let mut changed = token.as_bytes().to_vec();
            changed[position] ^= 1;
            assert!(!valid_token(
                Some(std::str::from_utf8(&changed).unwrap()),
                token
            ));
        }
        assert!(!valid_token(None, token));
        assert!(!valid_token(Some(&token[..35]), token));
        assert!(!valid_token(Some(&format!("{token}x")), token));
    }
}
