//! Authenticated loopback bridge. Remote tools edit a detached song; the native
//! editor alone can accept a revision-bound draft. Credentials live in a private
//! temporary directory and disappear when the editor closes the bridge.
use super::agent::Draft;
use super::*;
use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
    time::Duration,
};
pub struct Bridge {
    _home: tempfile::TempDir,
    registry: PathBuf,
    cancelled: Arc<AtomicBool>,
    drafts: Receiver<Result<Draft, String>>,
}
impl Bridge {
    pub fn start(workspace: &Workspace) -> Result<Self, Error> {
        let token = workspace.file().session.revision_token();
        let doc = workspace
            .file()
            .session
            .document()
            .reparse(workspace.file().session.serialize()?)?;
        let file = crate::storage::FileSession::from_new(crate::file_origin::NewSong {
            origin: crate::file_origin::Origin::Untitled {
                resource_base: workspace.file().resource_base().to_owned(),
            },
            session: crate::history::Session::new(doc),
        })?;
        let mut shadow = if let Some(chart) = workspace.active_chart() {
            Workspace::from_file(file, chart)?
        } else {
            Workspace::from_file_unfocused(file)
        };
        let home = tempfile::Builder::new().prefix("deadsync-mcp-").tempdir()?;
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let secret = uuid::Uuid::new_v4().to_string();
        let discovery =
            json!({"address":listener.local_addr()?.to_string(),"token":secret,"version":1});
        let path = home.path().join("connection.json");
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options
            .open(path)?
            .write_all(discovery.to_string().as_bytes())?;
        let registry = default_connection_path()?;
        let parent = registry
            .parent()
            .ok_or_else(|| Error::InvalidEdit("Invalid bridge registry path".into()))?;
        std::fs::create_dir_all(parent)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
        }
        let mut publication = tempfile::NamedTempFile::new_in(parent)?;
        publication.write_all(discovery.to_string().as_bytes())?;
        publication.persist(&registry).map_err(|e| e.error)?;
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancel = Arc::clone(&cancelled);
        let (sender, drafts) = mpsc::sync_channel(8);
        std::thread::Builder::new()
            .name("deadsync-mcp-bridge".into())
            .spawn(move || {
                let mut toolbox = Toolbox::default();
                while !cancel.load(Ordering::Acquire) {
                    let (mut stream, _) = match listener.accept() {
                        Ok(v) => v,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(25));
                            continue;
                        }
                        Err(_) => break,
                    };
                    let result = (|| -> Result<(), String> {
                        stream
                            .set_read_timeout(Some(Duration::from_secs(3)))
                            .map_err(|e| e.to_string())?;
                        stream
                            .set_write_timeout(Some(Duration::from_secs(3)))
                            .map_err(|e| e.to_string())?;
                        let envelope = read_frame(&mut BufReader::new(&stream))?;
                        if envelope["token"].as_str() != Some(secret.as_str()) {
                            return Err("Invalid bridge token".into());
                        }
                        let request = envelope["request"].clone();
                        let mutation = request["method"] == "tools/call"
                            && matches!(
                                request["params"]["name"].as_str(),
                                Some("apply_preview" | "undo" | "redo")
                            );
                        let response = protocol::response_cancellable(
                            request,
                            &mut shadow,
                            &mut toolbox,
                            false,
                            &cancel,
                        );
                        if mutation
                            && response
                                .as_ref()
                                .is_some_and(|v| v["result"]["isError"] == false)
                        {
                            let draft = shadow
                                .file()
                                .session
                                .serialize()
                                .map(|bytes| Draft {
                                    token: token.clone(),
                                    bytes,
                                    message: "MCP proposal".into(),
                                })
                                .map_err(|e| e.to_string());
                            sender.try_send(draft).map_err(|_| {
                                "Editor proposal queue is full; review pending proposals first"
                                    .to_owned()
                            })?;
                        }
                        if let Some(response) = response {
                            write_frame(&mut stream, &response)?;
                        }
                        Ok(())
                    })();
                    if let Err(error) = result {
                        let _ = write_frame(&mut stream, &json!({"bridge_error":error}));
                    }
                }
            })?;
        Ok(Self {
            _home: home,
            registry,
            cancelled,
            drafts,
        })
    }
    pub fn discovery_path(&self) -> PathBuf {
        self.registry.clone()
    }
    pub fn poll(&self) -> Option<Result<Draft, String>> {
        self.drafts.try_recv().ok()
    }
}
impl Drop for Bridge {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        // Keep stable discovery as an expired pointer. Removing it after a
        // token check could race another editor publishing a new connection.
        // The private per-instance credential is removed with _home.
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
    let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if metadata.len() > 4096 {
        return Err("Invalid discovery file".into());
    }
    let discovery: Value = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let address: std::net::SocketAddr = discovery["address"]
        .as_str()
        .ok_or("Missing address")?
        .parse()
        .map_err(|_| "Invalid address")?;
    if !address.ip().is_loopback() {
        return Err("Bridge address must be loopback".into());
    }
    loop {
        if input.fill_buf().map_err(|e| e.to_string())?.is_empty() {
            return Ok(());
        }
        let request = read_frame(&mut input)?;
        if request.get("id").is_none() {
            continue;
        }
        let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(3))
            .map_err(|e| e.to_string())?;
        stream
            .set_read_timeout(Some(Duration::from_secs(180)))
            .map_err(|e| e.to_string())?;
        write_frame(
            &mut stream,
            &json!({"token":discovery["token"],"request":request}),
        )?;
        let response = read_frame(&mut BufReader::new(stream))?;
        if let Some(error) = response.get("bridge_error") {
            return Err(error.to_string());
        }
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
