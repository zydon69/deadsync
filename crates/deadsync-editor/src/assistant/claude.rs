//! Claude Code adapter. The CLI runs headless with every built-in tool,
//! setting source and customization disabled; the DeadSync tools are served
//! in-process through the SDK control protocol on the detached draft.
use super::agent::{
    ASSISTANT_INSTRUCTIONS, Diagnostics, Draft, Event, VERSION_TIMEOUT, diagnostic_error,
    drain_diagnostics, on_path, probe_version, send, stop,
};
use super::*;
use std::time::Duration;
#[cfg(test)]
use std::time::Instant;
use std::{
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, Command as Process, Stdio},
    sync::{Mutex, mpsc::SyncSender},
};

const SERVER: &str = "deadsync";
/// Oldest CLI whose isolation flags and SDK MCP protocol were verified.
const MINIMUM_VERSION: (u32, u32, u32) = (2, 1, 280);
/// DeadSync tools run synchronously; audio analysis of a 30-minute song can
/// take minutes. Match the external helper's one-hour response bound.
const TOOL_TIMEOUT_MS: &str = "3600000";

pub(super) fn run(
    workspace: Workspace,
    token: RevisionToken,
    prompt: String,
    cancelled: &AtomicBool,
    holder: &Arc<Mutex<Option<Child>>>,
    events: &SyncSender<Event>,
) -> Result<Draft, String> {
    let executable = std::env::var_os("DEADSYNC_CLAUDE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(default_executable);
    let diagnostics = Arc::new(Diagnostics::default());
    let result = verify_version(&executable, cancelled, holder, VERSION_TIMEOUT).and_then(|()| {
        let home = tempfile::tempdir().map_err(|e| e.to_string())?;
        run_process(
            workspace,
            token,
            prompt,
            cancelled,
            holder,
            events,
            home.path(),
            &executable,
            &diagnostics,
        )
    });
    stop(holder);
    result.map_err(|error| {
        if cancelled.load(Ordering::Acquire) {
            return "Assistant cancelled".into();
        }
        diagnostic_error(error, &diagnostics, "Claude")
    })
}

fn arguments() -> Vec<String> {
    let mut args: Vec<String> = [
        "-p",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        // An empty built-in set is an allowlist: tools added by future CLI
        // versions stay disabled. The init inventory is still verified.
        "--tools",
        "",
        "--safe-mode",
        "--setting-sources",
        "",
        "--strict-mcp-config",
        "--disable-slash-commands",
        "--no-session-persistence",
        "--permission-mode",
        "dontAsk",
        "--permission-prompts",
        "none",
    ]
    .map(String::from)
    .to_vec();
    args.push("--mcp-config".into());
    args.push(json!({"mcpServers":{SERVER:{"type":"sdk","name":SERVER}}}).to_string());
    args.push("--allowedTools".into());
    args.push(allowed_tools().join(","));
    args.push("--system-prompt".into());
    args.push(ASSISTANT_INSTRUCTIONS.into());
    args
}

fn tool_names() -> Vec<String> {
    protocol::tools()
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|tool| tool["name"].as_str().map(String::from))
        .collect()
}
/// Names under which Claude Code exposes the DeadSync tools.
fn allowed_tools() -> Vec<String> {
    tool_names()
        .iter()
        .map(|name| format!("mcp__{SERVER}__{name}"))
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn run_process(
    mut workspace: Workspace,
    token: RevisionToken,
    prompt: String,
    cancelled: &AtomicBool,
    holder: &Arc<Mutex<Option<Child>>>,
    events: &SyncSender<Event>,
    home: &Path,
    executable: &Path,
    diagnostics: &Arc<Diagnostics>,
) -> Result<Draft, String> {
    let mut child = Process::new(executable)
        .args(arguments())
        .env("MCP_TOOL_TIMEOUT", TOOL_TIMEOUT_MS)
        .current_dir(home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            format!(
                "Cannot start Claude Code: {error}. Set DEADSYNC_CLAUDE_PATH to the installed claude executable."
            )
        })?;
    let stderr = child.stderr.take();
    let mut input = child.stdin.take().ok_or("Missing Claude Code stdin")?;
    let output = child.stdout.take().ok_or("Missing Claude Code stdout")?;
    *holder
        .lock()
        .map_err(|_| "Assistant process lock poisoned")? = Some(child);
    if let Some(stderr) = stderr {
        let diagnostics = diagnostics.clone();
        std::thread::Builder::new()
            .name("deadsync-claude-diagnostics".into())
            .spawn(move || drain_diagnostics(stderr, &diagnostics))
            .map_err(|e| e.to_string())?;
    }
    if cancelled.load(Ordering::Acquire) {
        return Err("Assistant cancelled".into());
    }
    send(
        &mut input,
        json!({"type":"control_request","request_id":"deadsync-initialize","request":{"subtype":"initialize"}}),
    )?;
    send(
        &mut input,
        json!({"type":"user","message":{"role":"user","content":prompt},"parent_tool_use_id":null,"session_id":""}),
    )?;
    let mut session = Session::default();
    let mut reader = BufReader::new(output);
    loop {
        if cancelled.load(Ordering::Acquire) {
            return Err("Assistant cancelled".into());
        }
        let mut bytes = Vec::new();
        let read = std::io::Read::take(&mut reader, (protocol::MAX_FRAME + 1) as u64)
            .read_until(b'\n', &mut bytes)
            .map_err(|e| e.to_string())?;
        if read == 0 {
            return Err("Claude Code closed before completing the turn".into());
        }
        if bytes.len() > protocol::MAX_FRAME {
            return Err("Claude Code frame exceeds 1 MiB".into());
        }
        if bytes.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let frame: Value = serde_json::from_slice(&bytes)
            .map_err(|e| format!("Invalid Claude Code frame: {e}"))?;
        match session.handle(&frame, &mut workspace, cancelled)? {
            Step::Continue => {}
            Step::Text(delta) => events
                .send(Event::Text(delta))
                .map_err(|_| "Assistant view closed")?,
            Step::Reply(reply, tool) => {
                if let Some(name) = tool {
                    events
                        .send(Event::Tool(name))
                        .map_err(|_| "Assistant view closed")?;
                }
                send(&mut input, reply)?;
            }
            Step::Finished => {
                return Ok(Draft {
                    token,
                    bytes: workspace
                        .file()
                        .session
                        .serialize()
                        .map_err(|e| e.to_string())?,
                    message: session.message,
                });
            }
        }
    }
}

#[derive(Default)]
struct Session {
    toolbox: Toolbox,
    message: String,
    tool_calls: usize,
    verified: bool,
    /// A new assistant message began after text was already shown.
    paragraph: bool,
}
#[derive(Debug)]
enum Step {
    Continue,
    Text(String),
    /// A control response, with the tool name to report when it ran one.
    Reply(Value, Option<String>),
    Finished,
}
impl Session {
    fn handle(
        &mut self,
        frame: &Value,
        workspace: &mut Workspace,
        cancelled: &AtomicBool,
    ) -> Result<Step, String> {
        match frame["type"].as_str() {
            Some("system") if frame["subtype"] == "init" => {
                verify_inventory(frame)?;
                self.verified = true;
                Ok(Step::Continue)
            }
            Some("stream_event") => {
                let event = &frame["event"];
                if frame["parent_tool_use_id"].is_null()
                    && event["type"] == "message_start"
                    && !self.message.is_empty()
                {
                    self.paragraph = true;
                }
                if frame["parent_tool_use_id"].is_null()
                    && event["type"] == "content_block_delta"
                    && event["delta"]["type"] == "text_delta"
                    && let Some(delta) = event["delta"]["text"].as_str()
                {
                    // Replies around tool calls are separate messages; keep
                    // them as separate paragraphs instead of gluing them.
                    let delta =
                        if std::mem::take(&mut self.paragraph) && !self.message.ends_with('\n') {
                            format!("\n\n{delta}")
                        } else {
                            delta.to_owned()
                        };
                    if self.message.len() + delta.len() > 128 * 1024 {
                        return Err("Assistant response exceeds 128 KiB".into());
                    }
                    self.message.push_str(&delta);
                    return Ok(Step::Text(delta));
                }
                Ok(Step::Continue)
            }
            Some("control_request") => {
                let id = frame["request_id"].clone();
                let request = &frame["request"];
                if request["subtype"] != "mcp_message" || request["server_name"] != SERVER {
                    return Ok(Step::Reply(
                        json!({"type":"control_response","response":{"subtype":"error","request_id":id,"error":"This editor host does not grant additional tools or permissions"}}),
                        None,
                    ));
                }
                let message = request["message"].clone();
                let tool = if message["method"] == "tools/call" {
                    // Tools must never run before the inventory was checked.
                    if !self.verified {
                        return Err(
                            "Claude Code requested a tool before its inventory was verified".into(),
                        );
                    }
                    self.tool_calls += 1;
                    if self.tool_calls > 128 {
                        return Err("Assistant reached the 128-tool-call limit; narrow the request and retry".into());
                    }
                    Some(
                        message["params"]["name"]
                            .as_str()
                            .ok_or("Tool call has no name")?
                            .to_owned(),
                    )
                } else {
                    None
                };
                let response = protocol::response_cancellable(
                    message,
                    workspace,
                    &mut self.toolbox,
                    false,
                    cancelled,
                )
                .unwrap_or(json!({}));
                Ok(Step::Reply(
                    json!({"type":"control_response","response":{"subtype":"success","request_id":id,"response":{"mcp_response":response}}}),
                    tool,
                ))
            }
            Some("result") => {
                if frame["is_error"] == true || frame["subtype"] != "success" {
                    // Execution errors carry their explanation in `errors`;
                    // API errors in `result`. The subtype is the last resort.
                    let errors: Vec<&str> = frame["errors"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .filter(|error| !error.trim().is_empty())
                        .collect();
                    let detail = if errors.is_empty() {
                        frame["result"]
                            .as_str()
                            .filter(|result| !result.trim().is_empty())
                            .or_else(|| frame["subtype"].as_str())
                            .unwrap_or("unknown error")
                            .to_owned()
                    } else {
                        errors.join("; ")
                    };
                    let detail: String = detail.chars().take(500).collect();
                    return Err(format!("Claude Code did not complete the turn: {detail}"));
                }
                Ok(Step::Finished)
            }
            _ => Ok(Step::Continue),
        }
    }
}

/// Require exactly the DeadSync tools from a connected DeadSync server:
/// refuse anything a future CLI version adds by default, and refuse a session
/// whose server failed or exposes none of the tools a request needs.
fn verify_inventory(init: &Value) -> Result<(), String> {
    let allowed = allowed_tools();
    let tools = init["tools"]
        .as_array()
        .ok_or("Claude Code reported no tool inventory")?;
    if let Some(extra) = tools.iter().find(|tool| {
        !tool
            .as_str()
            .is_some_and(|name| allowed.iter().any(|a| a == name))
    }) {
        return Err(format!(
            "Claude Code exposed an unexpected tool ({extra}); the assistant refuses to run"
        ));
    }
    let servers = init["mcp_servers"]
        .as_array()
        .ok_or("Claude Code reported no MCP servers")?;
    if servers.len() != 1 || servers[0]["name"] != SERVER {
        return Err(
            "Claude Code loaded MCP servers other than DeadSync; the assistant refuses to run"
                .into(),
        );
    }
    match servers[0]["status"].as_str() {
        Some("connected") => {}
        // Claude Code connects an SDK server before emitting `init` and never
        // repeats that frame, so there is no later inventory to wait for.
        // Measured on 2.1.280 with a 40 s `initialize`: it waited, then
        // reported `connected`. A slow `tools/list` instead yields
        // `connected` without tools, refused below as a missing tool.
        Some("pending") => {
            return Err(
                "The DeadSync tools were still connecting when Claude Code started; retry the request"
                    .into(),
            );
        }
        status => {
            return Err(format!(
                "The DeadSync tools failed to start in Claude Code (status: {}); the assistant refuses to run",
                status.unwrap_or("unknown")
            ));
        }
    }
    if let Some(missing) = allowed
        .iter()
        .find(|name| !tools.iter().any(|tool| tool == name.as_str()))
    {
        return Err(format!(
            "Claude Code did not expose the DeadSync tool {missing}; the assistant refuses to run"
        ));
    }
    Ok(())
}

fn parse_version(text: &str) -> Option<(u32, u32, u32)> {
    let mut parts = text.split_whitespace().next()?.split('.');
    let mut next = || parts.next()?.parse().ok();
    Some((next()?, next()?, next()?))
}
fn verify_version(
    executable: &Path,
    cancelled: &AtomicBool,
    holder: &Mutex<Option<Child>>,
    timeout: Duration,
) -> Result<(), String> {
    let (success, text) = probe_version(executable, cancelled, holder, timeout).map_err(|error| {
        if cancelled.load(Ordering::Acquire) {
            return error;
        }
        format!(
            "Claude Code CLI: {error}. Install it or set DEADSYNC_CLAUDE_PATH to the claude executable."
        )
    })?;
    if !success || parse_version(&text).is_none_or(|v| v < MINIMUM_VERSION) {
        let (major, minor, patch) = MINIMUM_VERSION;
        return Err(format!(
            "Unsupported Claude Code CLI. This assistant requires version {major}.{minor}.{patch} or later."
        ));
    }
    Ok(())
}

pub(super) fn installed() -> bool {
    std::env::var_os("DEADSYNC_CLAUDE_PATH").is_some() || {
        let executable = default_executable();
        executable.is_absolute() && executable.is_file() || on_path("claude")
    }
}

fn default_executable() -> PathBuf {
    // A game launched from the desktop does not inherit the shell PATH.
    let (home, name) = if cfg!(windows) {
        ("USERPROFILE", "claude.exe")
    } else {
        ("HOME", "claude")
    };
    if let Some(home) = std::env::var_os(home) {
        let installed = PathBuf::from(home).join(".local").join("bin").join(name);
        if installed.is_file() {
            return installed;
        }
    }
    for installed in ["/opt/homebrew/bin/claude", "/usr/local/bin/claude"] {
        if Path::new(installed).is_file() {
            return installed.into();
        }
    }
    PathBuf::from("claude")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn workspace() -> Workspace {
        let document=crate::document::Document::parse(Arc::<[u8]>::from(b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Edit;#METER:1;#NOTES:\n1000\n0000\n0000\n0000\n;\n".as_slice()),crate::document::Format::Ssc).unwrap();
        let file = crate::storage::FileSession::from_new(crate::file_origin::NewSong {
            origin: crate::file_origin::Origin::Untitled {
                resource_base: std::env::temp_dir(),
            },
            session: crate::history::Session::new(document),
        })
        .unwrap();
        Workspace::from_file(file, 0).unwrap()
    }
    fn init(tools: Value, servers: Value) -> Value {
        json!({"type":"system","subtype":"init","tools":tools,"mcp_servers":servers})
    }
    fn deadsync() -> Value {
        json!([{"name":"deadsync","status":"connected","source":"sdk"}])
    }
    fn all_tools() -> Vec<String> {
        allowed_tools()
    }

    #[test]
    fn isolation_arguments_disable_builtins_and_allow_only_deadsync_tools() {
        let args = arguments();
        let value = |flag: &str| &args[args.iter().position(|a| a == flag).unwrap() + 1];
        assert_eq!(value("--tools"), "");
        assert_eq!(value("--setting-sources"), "");
        assert_eq!(value("--permission-mode"), "dontAsk");
        for flag in [
            "--safe-mode",
            "--strict-mcp-config",
            "--no-session-persistence",
        ] {
            assert!(args.iter().any(|a| a == flag), "{flag}");
        }
        assert!(
            value("--allowedTools")
                .split(',')
                .all(|tool| tool.starts_with("mcp__deadsync__"))
        );
        let config: Value = serde_json::from_str(value("--mcp-config")).unwrap();
        assert_eq!(config["mcpServers"].as_object().unwrap().len(), 1);
        assert_eq!(config["mcpServers"]["deadsync"]["type"], "sdk");
    }

    #[test]
    fn inventory_with_builtin_tool_or_foreign_server_is_refused() {
        assert!(verify_inventory(&init(json!(all_tools()), deadsync())).is_ok());
        let mut extra = all_tools();
        extra.push("Bash".into());
        assert!(verify_inventory(&init(json!(extra), deadsync())).is_err());
        assert!(verify_inventory(&init(json!(["mcp__other__run"]), deadsync())).is_err());
        let foreign = json!([{"name":"deadsync"},{"name":"filesystem"}]);
        assert!(verify_inventory(&init(json!([]), foreign)).is_err());
        assert!(verify_inventory(&json!({"type":"system","subtype":"init"})).is_err());
    }

    #[test]
    fn inventory_without_a_connected_server_or_required_tools_is_refused() {
        for status in ["failed", "pending", "needs-auth"] {
            let server = json!([{"name":"deadsync","status":status,"source":"sdk"}]);
            let error = verify_inventory(&init(json!(all_tools()), server)).unwrap_err();
            assert!(error.contains(if status == "pending" {
                "connecting"
            } else {
                status
            }));
        }
        assert!(verify_inventory(&init(json!(all_tools()), json!([{"name":"deadsync"}]))).is_err());
        assert!(verify_inventory(&init(json!([]), deadsync())).is_err());
        let mut partial = all_tools();
        let missing = partial.pop().unwrap();
        let error = verify_inventory(&init(json!(partial), deadsync())).unwrap_err();
        assert!(error.contains(&missing));
    }

    #[test]
    fn tool_calls_run_on_the_draft_only_after_inventory_verification() {
        let mut draft = workspace();
        let mut session = Session::default();
        let call = json!({"type":"control_request","request_id":"r1","request":{"subtype":"mcp_message","server_name":"deadsync","message":{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"inspect_song","arguments":{}}}}});
        let cancelled = AtomicBool::new(false);
        assert!(session.handle(&call, &mut draft, &cancelled).is_err());
        let ready = init(json!(all_tools()), deadsync());
        session.handle(&ready, &mut draft, &cancelled).unwrap();
        let Step::Reply(reply, Some(tool)) = session.handle(&call, &mut draft, &cancelled).unwrap()
        else {
            panic!("tool call must reply");
        };
        assert_eq!(tool, "inspect_song");
        assert_eq!(reply["response"]["request_id"], "r1");
        let mcp = &reply["response"]["response"]["mcp_response"];
        assert_eq!(mcp["id"], 4);
        assert_eq!(mcp["result"]["isError"], false);
    }

    #[test]
    fn other_control_requests_and_servers_are_denied() {
        let mut draft = workspace();
        let mut session = Session::default();
        let cancelled = AtomicBool::new(false);
        for request in [
            json!({"subtype":"can_use_tool","tool_name":"Bash"}),
            json!({"subtype":"mcp_message","server_name":"other","message":{"jsonrpc":"2.0","id":1,"method":"tools/list"}}),
        ] {
            let frame = json!({"type":"control_request","request_id":"x","request":request});
            let Step::Reply(reply, None) = session.handle(&frame, &mut draft, &cancelled).unwrap()
            else {
                panic!("request must be answered");
            };
            assert_eq!(reply["response"]["subtype"], "error");
        }
    }

    #[test]
    fn streamed_text_accumulates_and_failed_results_are_reported() {
        let mut draft = workspace();
        let mut session = Session::default();
        let cancelled = AtomicBool::new(false);
        let delta = json!({"type":"stream_event","parent_tool_use_id":null,"event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Bonjour"}}});
        assert!(
            matches!(session.handle(&delta, &mut draft, &cancelled).unwrap(), Step::Text(t) if t == "Bonjour")
        );
        assert_eq!(session.message, "Bonjour");
        let done = json!({"type":"result","subtype":"success","is_error":false,"result":"Bonjour"});
        assert!(matches!(
            session.handle(&done, &mut draft, &cancelled).unwrap(),
            Step::Finished
        ));
        let failed = json!({"type":"result","subtype":"success","is_error":true,"result":"Failed to authenticate"});
        let error = session.handle(&failed, &mut draft, &cancelled).unwrap_err();
        assert!(error.contains("Failed to authenticate"));
        let mut session = Session::default();
        let start = json!({"type":"stream_event","parent_tool_use_id":null,"event":{"type":"message_start"}});
        for (text, expected) in [("Je regarde.", "Je regarde."), ("Voici.", "\n\nVoici.")] {
            session.handle(&start, &mut draft, &cancelled).unwrap();
            let delta = json!({"type":"stream_event","parent_tool_use_id":null,"event":{"type":"content_block_delta","delta":{"type":"text_delta","text":text}}});
            assert!(
                matches!(session.handle(&delta, &mut draft, &cancelled).unwrap(), Step::Text(t) if t == expected)
            );
        }
        assert_eq!(session.message, "Je regarde.\n\nVoici.");
        let execution = json!({"type":"result","subtype":"error_during_execution","is_error":true,"errors":["Tool stream interrupted",""],"result":""});
        let error = session
            .handle(&execution, &mut draft, &cancelled)
            .unwrap_err();
        assert!(error.contains("Tool stream interrupted"));
        let bare = json!({"type":"result","subtype":"error_max_turns","is_error":true,"errors":[]});
        assert!(
            session
                .handle(&bare, &mut draft, &cancelled)
                .unwrap_err()
                .contains("error_max_turns")
        );
        let long =
            json!({"type":"result","subtype":"error_during_execution","errors":["x".repeat(4000)]});
        assert!(
            session
                .handle(&long, &mut draft, &cancelled)
                .unwrap_err()
                .len()
                < 600
        );
    }

    #[test]
    fn version_gate_requires_the_verified_minimum() {
        assert_eq!(parse_version("2.1.280 (Claude Code)\n"), Some((2, 1, 280)));
        assert!(parse_version("2.1.281 (Claude Code)").unwrap() >= MINIMUM_VERSION);
        assert!(parse_version("2.1.9 (Claude Code)").unwrap() < MINIMUM_VERSION);
        assert_eq!(parse_version("garbage"), None);
        let holder = Mutex::new(None);
        let cancelled = AtomicBool::new(false);
        assert!(
            verify_version(
                Path::new("/nonexistent/deadsync-claude"),
                &cancelled,
                &holder,
                VERSION_TIMEOUT
            )
            .is_err()
        );
    }

    /// Runs the installed CLI with the full DeadSync tool set. Without a
    /// sign-in the turn fails at authentication, after the version and the
    /// inventory were accepted; with one, it must complete.
    #[test]
    #[ignore = "requires an installed Claude Code CLI"]
    fn installed_cli_accepts_the_full_deadsync_inventory() {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1024);
        let result = run(
            workspace(),
            workspace().file().session.revision_token(),
            "Reply with the word OK without using any tool.".into(),
            &AtomicBool::new(false),
            &Arc::new(Mutex::new(None)),
            &sender,
        );
        drop(receiver);
        match result {
            Ok(draft) => assert!(!draft.message.is_empty()),
            Err(error) => {
                eprintln!("installed CLI: {error}");
                for refused in [
                    "Unsupported",
                    "inventory",
                    "unexpected tool",
                    "DeadSync tool",
                    "MCP servers",
                ] {
                    assert!(!error.contains(refused), "{error}");
                }
                assert!(error.contains("authenticate"), "{error}");
            }
        }
    }

    #[cfg(unix)]
    fn stalled_cli() -> (tempfile::TempDir, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let script = directory.path().join("claude");
        std::fs::write(&script, "#!/bin/sh\nexec sleep 30\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        (directory, script)
    }

    #[cfg(unix)]
    #[test]
    fn stalled_version_probe_times_out_and_is_killed() {
        let (_directory, script) = stalled_cli();
        let holder = Mutex::new(None);
        let started = Instant::now();
        let error = verify_version(
            &script,
            &AtomicBool::new(false),
            &holder,
            Duration::from_millis(200),
        )
        .unwrap_err();
        assert!(error.contains("did not report its version"));
        assert!(started.elapsed() < Duration::from_secs(5));
        assert!(holder.lock().unwrap().is_none());
    }

    #[cfg(unix)]
    #[test]
    fn stalled_version_probe_is_cancellable_through_the_job_holder() {
        let (_directory, script) = stalled_cli();
        let holder = Arc::new(Mutex::new(None));
        let cancelled = Arc::new(AtomicBool::new(false));
        let (h, c) = (Arc::clone(&holder), Arc::clone(&cancelled));
        let started = Instant::now();
        let probe = std::thread::spawn(move || verify_version(&script, &c, &h, VERSION_TIMEOUT));
        while holder.lock().unwrap().is_none() {
            std::thread::sleep(Duration::from_millis(5));
        }
        // Same sequence as Job::cancel: flag, then kill the registered child.
        cancelled.store(true, Ordering::Release);
        if let Some(child) = holder.lock().unwrap().as_mut() {
            let _ = child.kill();
        }
        assert_eq!(probe.join().unwrap().unwrap_err(), "Assistant cancelled");
        assert!(started.elapsed() < Duration::from_secs(5));
        assert!(holder.lock().unwrap().is_none());
    }
}
