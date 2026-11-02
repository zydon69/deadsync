//! Authenticated, optional loopback bridge with live host snapshots. Each MCP
//! client owns an isolated draft; only the native editor publishes proposals.
use super::agent::Draft;
use super::*;
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{Mutex, atomic::AtomicUsize},
    time::Duration,
};
#[derive(Clone)]
struct Snapshot {
    epoch: Arc<()>,
    token: RevisionToken,
    document: crate::document::Document,
    base: PathBuf,
    chart: Option<usize>,
}
impl Snapshot {
    fn capture(workspace: &Workspace) -> Result<Self, Error> {
        let session = &workspace.file().session;
        Ok(Self {
            epoch: Arc::new(()),
            token: session.revision_token(),
            document: session.document().reparse(session.serialize()?)?,
            base: workspace.file().resource_base().to_owned(),
            chart: workspace.active_chart(),
        })
    }
    fn workspace(&self) -> Result<Workspace, Error> {
        let file = crate::storage::FileSession::from_new(crate::file_origin::NewSong {
            origin: crate::file_origin::Origin::Untitled {
                resource_base: self.base.clone(),
            },
            session: crate::history::Session::new(self.document.clone()),
        })?;
        match self.chart {
            Some(chart) => Workspace::from_file(file, chart),
            None => Ok(Workspace::from_file_unfocused(file)),
        }
    }
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
    host: Arc<Mutex<Snapshot>>,
    draft: Arc<Mutex<Option<Draft>>>,
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
    fn start_at(workspace: &Workspace, registry: PathBuf) -> Result<Self, Error> {
        let host = Arc::new(Mutex::new(Snapshot::capture(workspace)?));
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
        let bridge = Self {
            registry,
            secret: secret.clone(),
            cancelled: cancelled.clone(),
            host: host.clone(),
            draft: draft.clone(),
        };
        let server = Server {
            host,
            draft,
            cancelled,
            secret,
            clients: Mutex::new(HashMap::new()),
            active: Arc::new(AtomicUsize::new(0)),
        };
        std::thread::Builder::new()
            .name("deadsync-mcp-bridge".into())
            .spawn(move || server.listen(listener))?;
        Ok(bridge)
    }
    /// Refresh on host revision/focus changes without changing address or token.
    pub fn synchronize(&self, workspace: &Workspace) -> Result<(), Error> {
        let mut host = self
            .host
            .lock()
            .map_err(|_| Error::InvalidEdit("Host snapshot unavailable".into()))?;
        if !workspace.file().session.accepts_token(&host.token)
            || workspace.active_chart() != host.chart
            || workspace.file().resource_base() != host.base
        {
            *host = Snapshot::capture(workspace)?;
        }
        Ok(())
    }
    pub fn discovery_path(&self) -> PathBuf {
        self.registry.clone()
    }
    pub fn poll(&self) -> Option<Result<Draft, String>> {
        self.draft.try_lock().ok()?.take().map(Ok)
    }
}
struct Server {
    host: Arc<Mutex<Snapshot>>,
    draft: Arc<Mutex<Option<Draft>>>,
    cancelled: Arc<AtomicBool>,
    secret: String,
    clients: Mutex<HashMap<String, Arc<Mutex<Client>>>>,
    active: Arc<AtomicUsize>,
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
        while !server.cancelled.load(Ordering::Acquire) {
            let (stream, _) = match listener.accept() {
                Ok(pair) => pair,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(25));
                    continue;
                }
                Err(_) => break,
            };
            if server.active.load(Ordering::Acquire) >= 8 {
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
            if envelope["token"].as_str() != Some(self.secret.as_str()) {
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
        let snapshot = self
            .host
            .lock()
            .map_err(|_| "Host snapshot unavailable")?
            .clone();
        let client = self.client(envelope["client"].as_str().unwrap_or("legacy"), &snapshot)?;
        let mut client = client.lock().map_err(|_| "Client state unavailable")?;
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
        if publication.as_ref().is_some_and(|slot| slot.is_some()) {
            return Err(
                "Review or discard the pending MCP proposal before publishing another".into(),
            );
        }
        let Client {
            workspace, toolbox, ..
        } = &mut *client;
        let before = workspace.file().session.revision();
        let response = protocol::response_cancellable(
            request.clone(),
            workspace,
            toolbox,
            false,
            &self.cancelled,
        );
        if before != workspace.file().session.revision()
            && response
                .as_ref()
                .is_some_and(|v| v["result"]["isError"] == false)
        {
            let proposal = Draft {
                token: snapshot.token,
                bytes: workspace
                    .file()
                    .session
                    .serialize()
                    .map_err(|e| e.to_string())?,
                message: "MCP proposal".into(),
            };
            if let Some(slot) = publication.as_mut() {
                **slot = Some(proposal);
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
        if let Ok(_lock) = registry_lock(&self.registry) {
            let owned = std::fs::read(&self.registry)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                .is_some_and(|v| v["token"].as_str() == Some(self.secret.as_str()));
            if owned {
                let _ = std::fs::remove_file(&self.registry);
            }
        }
    }
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
        let request = read_frame(&mut input)?;
        if request.get("id").is_none() {
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
    fn workspace() -> Workspace {
        let document=crate::document::Document::parse(Arc::<[u8]>::from(b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Edit;#METER:1;#NOTES:1000;".as_slice()),crate::document::Format::Ssc).unwrap();
        Snapshot {
            epoch: Arc::new(()),
            token: crate::history::Session::new(document.clone()).revision_token(),
            document,
            base: std::env::temp_dir(),
            chart: Some(0),
        }
        .workspace()
        .unwrap()
    }
    fn request(path: &Path, tool: &str, arguments: Value) -> Value {
        let connection: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let mut stream = TcpStream::connect(connection["address"].as_str().unwrap()).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        write_frame(&mut stream,&json!({"token":connection["token"],"client":"regression","request":{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":tool,"arguments":arguments}}})).unwrap();
        let response = read_frame(&mut BufReader::new(stream)).unwrap();
        if response["result"]["isError"] == true || response.get("error").is_some() {
            eprintln!("Bridge fixture request {tool}: {response}");
        }
        response
    }
    #[test]
    fn host_changes_refresh_without_restarting_bridge_and_registry_is_removed() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("connection.json");
        let mut host = workspace();
        let bridge = Bridge::start_at(&host, path.clone()).unwrap();
        let initial = std::fs::read(&path).unwrap();
        assert_eq!(
            request(&path, "inspect_song", json!({}))["result"]["structuredContent"]["charts"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        host.execute(Command::DuplicateChart).unwrap();
        bridge.synchronize(&host).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), initial);
        assert_eq!(
            request(&path, "inspect_song", json!({}))["result"]["structuredContent"]["charts"]
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
        let bridge = Bridge::start_at(&workspace(), path.clone()).unwrap();
        let prepare = |row| {
            request(
                &path,
                "preview_note_edits",
                json!({"chart":0,"edits":[{"row":row,"lane":1,"kind":"tap"}]}),
            )["result"]["structuredContent"]["preview_id"]
                .clone()
        };
        let preview = prepare(48);
        assert_eq!(
            request(&path, "apply_preview", json!({"preview_id":preview}))["result"]["isError"],
            false
        );
        let second = prepare(96);
        assert_eq!(
            request(&path, "apply_preview", json!({"preview_id":second}))["result"]["isError"],
            true
        );
        assert_eq!(
            request(&path, "inspect_chart", json!({"chart":0}))["result"]["structuredContent"]["total_notes"],
            2
        );
        assert!(bridge.poll().is_some());
        assert_eq!(
            request(&path, "apply_preview", json!({"preview_id":second}))["result"]["isError"],
            false
        );
        assert_eq!(
            request(&path, "inspect_chart", json!({"chart":0}))["result"]["structuredContent"]["total_notes"],
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
            request(&path, "inspect_song", json!({}))["result"]["isError"],
            false
        );
        drop(new);
        assert!(!path.exists());
    }
    #[test]
    fn slow_unauthenticated_client_does_not_block_other_requests() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("connection.json");
        let _bridge = Bridge::start_at(&workspace(), path.clone()).unwrap();
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
            request(&path, "inspect_song", json!({}))["result"]["isError"],
            false
        );
        assert!(start.elapsed() < Duration::from_secs(2));
    }
}
