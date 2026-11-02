//! Newline-delimited JSON-RPC transport. stdout contains protocol frames only.
use super::Toolbox;
use crate::workspace::Workspace;
use serde_json::{Value, json};
use std::{
    io::{BufRead, Write},
    sync::atomic::AtomicBool,
};

pub const MAX_FRAME: usize = 1024 * 1024;

pub fn tools() -> Value {
    let empty = json!({"type":"object","properties":{},"additionalProperties":false});
    let chart = json!({"type":"object","properties":{"chart":{"type":"integer","minimum":0}},"additionalProperties":false});
    let mut tools = Vec::new();
    for (name, description, schema, read_only) in [
        (
            "inspect_song",
            "Read current song metadata, chart list and revision",
            empty.clone(),
            true,
        ),
        (
            "inspect_chart",
            "Read a bounded page of note heads with attributes",
            json!({"type":"object","properties":{"chart":{"type":"integer","minimum":0},"start_row":{"type":"integer","minimum":0},"cursor":{"type":"object","properties":{"row":{"type":"integer","minimum":0},"lane":{"type":"integer","minimum":0,"maximum":65535},"player":{"type":"integer","minimum":0,"maximum":4294967295u64}},"required":["row","lane","player"],"additionalProperties":false},"limit":{"type":"integer","minimum":1,"maximum":1000}},"additionalProperties":false}),
            true,
        ),
        (
            "analyze_chart",
            "Analyze current unsaved chart with RSSP",
            chart.clone(),
            true,
        ),
        (
            "generate_chart",
            "Prepare GrooveAuthor generation from an existing chart; does not apply or save",
            json!({"type":"object","properties":{"chart":{"type":"integer","minimum":0},"target_layout":{"type":"string"},"seed":{"type":"integer"}},"additionalProperties":false}),
            false,
        ),
        (
            "preview_note_edits",
            "Validate and preview an atomic list of note edits; kind:null deletes a note",
            json!({"type":"object","properties":{"chart":{"type":"integer","minimum":0},"edits":{"type":"array","minItems":1,"maxItems":10000,"items":{"type":"object","properties":{"row":{"type":"integer","minimum":0},"lane":{"type":"integer","minimum":0},"player":{"type":"integer","minimum":0},"kind":{"enum":["tap","hold","roll","mine","lift","fake",null]},"end_row":{"type":["integer","null"],"minimum":0},"attributes":{"type":"string"},"tail_attributes":{"type":"string"}},"required":["row","lane","kind"],"additionalProperties":false}}},"required":["edits"],"additionalProperties":false}),
            false,
        ),
        (
            "preview_changes",
            "Read the current detached proposal",
            empty.clone(),
            true,
        ),
        (
            "apply_preview",
            "Apply a current preview by its exact id to undoable memory; does not save",
            json!({"type":"object","properties":{"preview_id":{"type":"string"}},"required":["preview_id"],"additionalProperties":false}),
            false,
        ),
        (
            "discard_preview",
            "Discard a detached proposal",
            empty.clone(),
            false,
        ),
        (
            "undo",
            "Undo the most recent editor transaction",
            empty.clone(),
            false,
        ),
        (
            "redo",
            "Redo the last undone editor transaction",
            empty.clone(),
            false,
        ),
        (
            "save_chart",
            "Save current song only when the host explicitly enables disk writes",
            empty,
            false,
        ),
    ] {
        tools.push(json!({"name":name,"description":description,"inputSchema":schema,"annotations":{"readOnlyHint":read_only,"destructiveHint":name=="save_chart","openWorldHint":false}}));
    }
    let scope = json!({"enum":["song","chart"]});
    tools.push(json!({"name":"inspect_timing","description":"Inspect scoped tempo nodes and inherited timing","inputSchema":{"type":"object","properties":{"chart":{"type":"integer","minimum":0},"scope":scope},"additionalProperties":false},"annotations":{"readOnlyHint":true,"openWorldHint":false}}));
    tools.push(json!({"name":"preview_tempo_edits","description":"Prepare scoped BPM node edits; null removes a node. Song scope may affect multiple charts.","inputSchema":{"type":"object","properties":{"chart":{"type":"integer","minimum":0},"scope":scope,"nodes":{"type":"array","minItems":1,"maxItems":10000,"items":{"type":"object","properties":{"row":{"type":"integer","minimum":0,"maximum":2147483647},"bpm":{"type":["number","null"],"minimum":0.000001,"maximum":10000}},"required":["row","bpm"],"additionalProperties":false}}},"required":["scope","nodes"],"additionalProperties":false},"annotations":{"readOnlyHint":false,"destructiveHint":false,"openWorldHint":false}}));
    #[cfg(feature = "assistant-audio")]
    {
        tools.push(json!({"name":"analyze_audio","description":"Estimate constant tempo, phase and onset count from the song audio; local signal processing, no audio upload","inputSchema":empty_schema(),"annotations":{"readOnlyHint":true,"openWorldHint":false}}));
        tools.push(json!({"name":"generate_audio_chart","description":"Prepare a new SSC chart from audio onsets and GrooveAuthor footwork; inspect tempo confidence before applying","inputSchema":{"type":"object","properties":{"bpm":{"type":"number","minimum":40,"maximum":300},"beat_offset_seconds":{"type":"number","minimum":0},"max_nps":{"type":"number","minimum":0.5,"maximum":30},"subdivision":{"enum":[1,2,3,4,6,8,12,16]},"target_layout":{"type":"string"},"seed":{"type":"integer"},"meter":{"type":"integer","minimum":1,"maximum":99}},"additionalProperties":false},"annotations":{"readOnlyHint":false,"destructiveHint":false,"openWorldHint":false}}));
    }
    for tool in &mut tools {
        if matches!(
            tool["name"].as_str(),
            Some("generate_chart" | "generate_audio_chart")
        ) {
            tool["inputSchema"]["properties"]["profile"] = json!({"enum":["balanced","stamina_single","stamina_double","no_candles","no_inward","no_transition_limit"]});
            tool["inputSchema"]["properties"]["brackets"] = json!({"enum":["balanced","none"]});
        }
    }
    if let Some(tool) = tools
        .iter_mut()
        .find(|t| t["name"] == "generate_audio_chart")
    {
        tool["inputSchema"]["properties"]["minimum_relative_energy"] =
            json!({"type":"number","minimum":0,"maximum":1});
    }
    json!(tools)
}
#[cfg(feature = "assistant-audio")]
fn empty_schema() -> Value {
    json!({"type":"object","properties":{},"additionalProperties":false})
}

pub fn response(
    request: Value,
    workspace: &mut Workspace,
    toolbox: &mut Toolbox,
    allow_save: bool,
) -> Option<Value> {
    response_cancellable(
        request,
        workspace,
        toolbox,
        allow_save,
        &AtomicBool::new(false),
    )
}
pub(super) fn response_cancellable(
    request: Value,
    workspace: &mut Workspace,
    toolbox: &mut Toolbox,
    allow_save: bool,
    cancelled: &AtomicBool,
) -> Option<Value> {
    let id = request.get("id")?.clone();
    if request.get("jsonrpc") != Some(&json!("2.0")) {
        return Some(error(id, -32600, "Invalid JSON-RPC version"));
    }
    let result = match request.get("method").and_then(Value::as_str) {
        Some("initialize") => Ok(
            json!({"protocolVersion":"2025-06-18","capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"deadsync-editor","version":env!("CARGO_PKG_VERSION")}}),
        ),
        Some("ping") => Ok(json!({})),
        Some("tools/list") => Ok(json!({"tools":tools()})),
        Some("tools/call") => {
            let p = &request["params"];
            match p["name"].as_str() {
                Some(name) => {
                    let result = toolbox.call(
                        workspace,
                        name,
                        p.get("arguments").cloned().unwrap_or(json!({})),
                        allow_save,
                        cancelled,
                    );
                    Ok(match result {
                        Ok(value) => {
                            json!({"content":[{"type":"text","text":value.to_string()}],"structuredContent":value,"isError":false})
                        }
                        Err(e) => {
                            json!({"content":[{"type":"text","text":e.to_string()}],"isError":true})
                        }
                    })
                }
                None => Err((-32602, "Missing tool name")),
            }
        }
        _ => Err((-32601, "Method not found")),
    };
    Some(match result {
        Ok(result) => json!({"jsonrpc":"2.0","id":id,"result":result}),
        Err((code, message)) => error(id, code, message),
    })
}

pub fn serve(
    mut input: impl BufRead,
    mut output: impl Write,
    workspace: &mut Workspace,
    allow_save: bool,
) -> std::io::Result<()> {
    let mut toolbox = Toolbox::default();
    loop {
        let mut frame = Vec::new();
        let read = std::io::Read::take(&mut input, (MAX_FRAME + 1) as u64)
            .read_until(b'\n', &mut frame)?;
        if read == 0 {
            return Ok(());
        }
        if frame.len() > MAX_FRAME {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "MCP frame exceeds 1 MiB",
            ));
        }
        if frame.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let result = match serde_json::from_slice(&frame) {
            Ok(value) => response(value, workspace, &mut toolbox, allow_save),
            Err(_) => Some(error(Value::Null, -32700, "Invalid JSON")),
        };
        if let Some(result) = result {
            serde_json::to_writer(&mut output, &result)?;
            output.write_all(b"\n")?;
            output.flush()?;
        }
    }
}
fn error(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}
