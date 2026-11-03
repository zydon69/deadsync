//! Structured agent tools shared by MCP and the native editor. Proposals are
//! detached, revision-bound and never publish a file implicitly.
pub mod agent;
#[cfg(feature = "assistant-audio")]
mod audio;
pub mod bridge;
mod claude;
pub mod protocol;

use crate::{
    Error,
    history::RevisionToken,
    notes::{Kind, Note, Position},
    workspace::{Command, Workspace},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

const MAX_EDITS: usize = 10_000;
const MAX_PAGE: usize = 1_000;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Edit {
    row: u32,
    lane: u16,
    #[serde(default)]
    player: u32,
    kind: Option<String>,
    end_row: Option<u32>,
    #[serde(default)]
    attributes: String,
    #[serde(default)]
    tail_attributes: String,
}

enum Change {
    Tempo {
        chart: usize,
        draft: crate::timing_edit::TempoDraft,
    },
    Notes {
        token: RevisionToken,
        chart: usize,
        edits: Vec<(Position, Option<Note>)>,
    },
    Generated(crate::generation::job::ChartPreview),
    #[cfg(feature = "assistant-audio")]
    Audio(audio::Preview),
}

struct Preview {
    id: String,
    change: Change,
    description: Value,
}

#[derive(Default)]
pub struct Toolbox {
    preview: Option<Preview>,
    #[cfg(feature = "assistant-audio")]
    audio_cache: Option<audio::AnalysisCache>,
}

impl Toolbox {
    pub fn preview(&self) -> Option<&Value> {
        self.preview.as_ref().map(|p| &p.description)
    }
    pub fn discard(&mut self) {
        self.preview = None;
    }

    /// The caller decides whether disk writes are permitted. This permission
    /// is deliberately not an argument that the model can grant to itself.
    pub fn call(
        &mut self,
        workspace: &mut Workspace,
        name: &str,
        arguments: Value,
        allow_save: bool,
        cancelled: &AtomicBool,
    ) -> Result<Value, Error> {
        if cancelled.load(Ordering::Acquire) {
            return invalid("Assistant request cancelled");
        }
        if !arguments.is_object() {
            return invalid("Tool arguments must be an object");
        }
        validate_arguments(name, &arguments)?;
        let chart = arguments
            .get("chart")
            .map(|v| {
                v.as_u64()
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or_else(|| Error::InvalidEdit("Invalid chart index".into()))
            })
            .transpose()?
            .unwrap_or(workspace.chart());
        match name {
            #[cfg(feature = "assistant-audio")]
            "analyze_audio" | "generate_audio_chart" => {
                self.audio(workspace, name, &arguments, cancelled)
            }
            "inspect_song" => {
                let session = &workspace.file().session;
                let charts: Vec<_> = session.document().charts().iter().enumerate().map(|(index,c)|
                    json!({"index":index,"layout":c.steps_type,"difficulty":c.difficulty,"meter":c.meter,"description":c.description})).collect();
                Ok(
                    json!({"title":session.document().text(None,"TITLE"),"charts":charts,
                    "revision":session.revision(),"dirty":session.is_dirty(),"active_chart":workspace.active_chart()}),
                )
            }
            "inspect_chart" => {
                let start = unsigned(&arguments, "start_row", 0)?;
                let end = unsigned(&arguments, "end_row", u64::MAX)?;
                if end < start {
                    return invalid("end_row must be at least start_row");
                }
                let limit = unsigned(&arguments, "limit", 100)?.min(MAX_PAGE as u64) as usize;
                let cursor = arguments
                    .get("cursor")
                    .map(|c| -> Result<Position, Error> {
                        Ok(Position {
                            row: unsigned(c, "row", 0)?,
                            lane: unsigned(c, "lane", 0)? as u16,
                            player: unsigned(c, "player", 0)? as u32,
                        })
                    })
                    .transpose()?;
                let grid = workspace.assistant_session().grid(chart)?;
                let page: Vec<_> = grid
                    .iter_heads_row_order(start, end, cursor)
                    .take(limit + 1)
                    .collect();
                let next = if page.len() > limit {
                    page.get(limit - 1)
                        .map(|(p, _)| json!({"row":p.row,"lane":p.lane,"player":p.player}))
                } else {
                    None
                };
                let notes: Vec<_> = page.into_iter().take(limit).map(|(p,n)|
                    json!({"row":p.row,"lane":p.lane,"player":p.player,"kind":note_kind(n.kind),"end_row":n.end,
                        "attributes":n.attributes.as_ref(),"tail_attributes":n.tail_attributes.as_ref()})).collect();
                Ok(
                    json!({"chart":chart,"columns":grid.columns(),"players":grid.players(),"notes":notes,"total_notes":grid.head_count(),"next_cursor":next,"revision":workspace.file().session.revision()}),
                )
            }
            "analyze_chart" => {
                let report =
                    crate::analysis::task::Task::capture(&workspace.file().session, chart)?
                        .run(cancelled)?;
                let s = report.summary();
                Ok(
                    json!({"chart":chart,"engine":"RSSP","max_nps":s.max_nps,"median_nps":s.median_nps,
                    "duration_seconds":s.duration_seconds,"matrix_rating":s.matrix_rating,"breakdown":s.detailed_breakdown,
                    "tech_notation":s.tech_notation_str,"total_measures":s.total_measures}),
                )
            }
            "preview_note_edits" => {
                let edits: Vec<Edit> = serde_json::from_value(
                    arguments
                        .get("edits")
                        .cloned()
                        .ok_or_else(|| Error::InvalidEdit("Missing edits".into()))?,
                )
                .map_err(|e| Error::InvalidEdit(e.to_string()))?;
                if edits.is_empty() || edits.len() > MAX_EDITS {
                    return invalid("Expected 1..10000 note edits");
                }
                let mut positions = BTreeSet::new();
                let mut changes = Vec::with_capacity(edits.len());
                for e in edits {
                    if e.row > i32::MAX as u32
                        || e.end_row.is_some_and(|r| r > i32::MAX as u32)
                        || e.attributes.len() > 4096
                        || e.tail_attributes.len() > 4096
                    {
                        return invalid("Note edit exceeds editor limits");
                    }
                    let position = Position {
                        row: u64::from(e.row),
                        lane: e.lane,
                        player: e.player,
                    };
                    if !positions.insert(position) {
                        return invalid("Duplicate edit position");
                    }
                    let note = match e.kind.as_deref() {
                        None => {
                            if e.end_row.is_some()
                                || !e.attributes.is_empty()
                                || !e.tail_attributes.is_empty()
                            {
                                return invalid(
                                    "Deleted notes cannot have attributes or endpoints",
                                );
                            }
                            None
                        }
                        Some(kind) => Some(Note {
                            kind: match kind.to_ascii_lowercase().as_str() {
                                "tap" => Kind::Tap,
                                "hold" => Kind::Hold,
                                "roll" => Kind::Roll,
                                "mine" => Kind::Mine,
                                "lift" => Kind::Lift,
                                "fake" => Kind::Fake,
                                "keysound" => Kind::Keysound,
                                "attack" => Kind::Attack,
                                _ => return invalid("Unsupported assistant note kind"),
                            },
                            end: e.end_row.map(u64::from),
                            attributes: Arc::from(e.attributes),
                            tail_attributes: Arc::from(e.tail_attributes),
                        }),
                    };
                    changes.push((position, note));
                }
                // Validate the whole transaction against a detached copy before
                // replacing the previous proposal. The real song is untouched.
                let session = &workspace.file().session;
                let document = session.document().reparse(session.serialize()?)?;
                let mut candidate = crate::history::Session::new(document);
                candidate.edit_notes(chart, "Assistant preview", changes.clone())?;
                let token = session.revision_token();
                let description = json!({"type":"note_edits","chart":chart,"edits":changes.len(),"revision":session.revision()});
                self.install(
                    Change::Notes {
                        token,
                        chart,
                        edits: changes,
                    },
                    description,
                )
            }
            "inspect_timing" => {
                let scope = if arguments["scope"] == "song" {
                    crate::timing_edit::Scope::Song
                } else {
                    crate::timing_edit::Scope::Chart
                };
                let draft = crate::timing_edit::TempoDraft::capture(
                    &workspace.file().session,
                    chart,
                    scope,
                )?;
                Ok(
                    json!({"chart":chart,"inherited":draft.inherited(),"tempos":draft.tempos().values(),"affected_charts":draft.affected_charts()}),
                )
            }
            "preview_tempo_edits" => {
                let scope = if arguments["scope"] == "song" {
                    crate::timing_edit::Scope::Song
                } else {
                    crate::timing_edit::Scope::Chart
                };
                let mut draft = crate::timing_edit::TempoDraft::capture(
                    &workspace.file().session,
                    chart,
                    scope,
                )?;
                let nodes = arguments["nodes"]
                    .as_array()
                    .ok_or_else(|| Error::InvalidEdit("Missing tempo nodes".into()))?;
                for node in nodes {
                    let row = node["row"]
                        .as_u64()
                        .ok_or_else(|| Error::InvalidEdit("Missing tempo row".into()))?;
                    if node["bpm"].is_null() {
                        draft.remove(row)?;
                    } else {
                        draft.set(
                            row,
                            node["bpm"]
                                .as_f64()
                                .ok_or_else(|| Error::InvalidEdit("Missing BPM".into()))?,
                        )?;
                    }
                }
                let description = json!({"type":"tempo_edits","chart":chart,"tempos":draft.tempos().values(),"affected_charts":draft.affected_charts()});
                self.install(Change::Tempo { chart, draft }, description)
            }
            "generate_chart" => {
                let layout = arguments
                    .get("target_layout")
                    .and_then(Value::as_str)
                    .unwrap_or("dance-single");
                let mut options = generation_options(&arguments, layout)?;
                if let Some(seed) = arguments.get("seed") {
                    options.seed = seed
                        .as_i64()
                        .and_then(|v| i32::try_from(v).ok())
                        .ok_or_else(|| Error::InvalidEdit("Invalid seed".into()))?;
                }
                let task = crate::generation::job::ChartTask::capture(
                    &workspace.file().session,
                    chart,
                    options,
                )?;
                let preview = task.run(cancelled, |_| {})?;
                let description = json!({"type":"generated_chart","source_chart":chart,"target_layout":preview.target_layout(),
                    "notes":preview.grid().head_count(),"seed":preview.seed(),"revision":workspace.file().session.revision()});
                self.install(Change::Generated(preview), description)
            }
            "preview_changes" => Ok(self.preview().cloned().unwrap_or(json!({"preview":null}))),
            "discard_preview" => {
                self.discard();
                Ok(json!({"discarded":true}))
            }
            "apply_preview" => {
                let requested = arguments
                    .get("preview_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| Error::InvalidEdit("Missing preview_id".into()))?;
                let preview = self
                    .preview
                    .as_ref()
                    .filter(|p| p.id == requested)
                    .ok_or_else(|| Error::InvalidEdit("Unknown preview_id".into()))?;
                let before_revision = workspace.file().session.revision();
                let added = match &preview.change {
                    Change::Tempo { chart, draft } => {
                        draft.apply(workspace.assistant_session(), *chart)?;
                        None
                    }
                    Change::Notes {
                        token,
                        chart,
                        edits,
                    } => {
                        let session = workspace.assistant_session();
                        if !session.accepts_token(token) {
                            return invalid("Preview is stale; prepare it again");
                        }
                        session.edit_notes(*chart, "Assistant note edits", edits.clone())?;
                        None
                    }
                    Change::Generated(chart) => Some(chart.apply(workspace.assistant_session())?),
                    #[cfg(feature = "assistant-audio")]
                    Change::Audio(chart) => Some(chart.apply(workspace.assistant_session())?),
                };
                if let Some(chart) = added {
                    workspace.execute(Command::SelectChart(chart))?;
                }
                self.discard();
                Ok(
                    json!({"applied":workspace.file().session.revision()!=before_revision,"revision":workspace.file().session.revision(),"saved":false}),
                )
            }
            "undo" | "redo" => {
                let before_revision = workspace.file().session.revision();
                workspace.execute(if name == "undo" {
                    Command::Undo
                } else {
                    Command::Redo
                })?;
                if workspace.file().session.revision() != before_revision {
                    self.discard();
                }
                Ok(
                    json!({"revision":workspace.file().session.revision(),"dirty":workspace.file().session.is_dirty()}),
                )
            }
            "save_chart" => {
                if !allow_save {
                    return invalid(
                        "Disk saving is disabled for this assistant connection; use the editor Save command",
                    );
                }
                workspace.execute(Command::Save)?;
                Ok(json!({"saved":!workspace.file().session.is_dirty()}))
            }
            _ => invalid("Unknown DeadSync assistant tool"),
        }
    }
    fn install(&mut self, change: Change, mut description: Value) -> Result<Value, Error> {
        let id = uuid::Uuid::new_v4().to_string();
        description["preview_id"] = json!(id);
        self.preview = Some(Preview {
            id,
            change,
            description: description.clone(),
        });
        Ok(description)
    }
}

fn unsigned(v: &Value, key: &str, default: u64) -> Result<u64, Error> {
    v.get(key).map_or(Ok(default), |v| {
        v.as_u64()
            .ok_or_else(|| Error::InvalidEdit(format!("Invalid {key}")))
    })
}
fn invalid<T>(s: &str) -> Result<T, Error> {
    Err(Error::InvalidEdit(s.into()))
}

fn validate_arguments(name: &str, args: &Value) -> Result<(), Error> {
    let tools = protocol::tools();
    let tool = tools
        .as_array()
        .and_then(|all| all.iter().find(|t| t["name"] == name))
        .ok_or_else(|| Error::InvalidEdit(format!("Unknown assistant tool: {name}")))?;
    validate_value(args, &tool["inputSchema"], "arguments")
}
fn validate_value(value: &Value, schema: &Value, path: &str) -> Result<(), Error> {
    if let Some(choices) = schema.get("enum").and_then(Value::as_array) {
        if !choices.contains(value) {
            return invalid(&format!("Invalid value at {path}"));
        }
    }
    let matches_type = |kind: &str| match kind {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "number" => value.is_number(),
        "null" => value.is_null(),
        "boolean" => value.is_boolean(),
        _ => false,
    };
    if let Some(kind) = schema.get("type") {
        let valid = kind
            .as_str()
            .map(matches_type)
            .or_else(|| {
                kind.as_array()
                    .map(|a| a.iter().filter_map(Value::as_str).any(matches_type))
            })
            .unwrap_or(false);
        if !valid {
            return invalid(&format!("Invalid type at {path}"));
        }
    }
    if let Some(number) = value.as_f64() {
        if !number.is_finite()
            || schema["minimum"].as_f64().is_some_and(|min| number < min)
            || schema["maximum"].as_f64().is_some_and(|max| number > max)
        {
            return invalid(&format!("Number outside allowed range at {path}"));
        }
    }
    if let Some(object) = value.as_object() {
        if let Some(required) = schema["required"].as_array() {
            for key in required.iter().filter_map(Value::as_str) {
                if !object.contains_key(key) {
                    return invalid(&format!("Missing {path}.{key}"));
                }
            }
        }
        for (key, child) in object {
            if let Some(property) = schema["properties"].get(key) {
                validate_value(child, property, &format!("{path}.{key}"))?;
            } else if schema["additionalProperties"] == false {
                return invalid(&format!("Unknown {path}.{key}"));
            }
        }
    }
    if let Some(items) = value.as_array() {
        if schema["minItems"]
            .as_u64()
            .is_some_and(|n| items.len() < (n as usize))
            || schema["maxItems"]
                .as_u64()
                .is_some_and(|n| items.len() > (n as usize))
        {
            return invalid(&format!("Array outside allowed size at {path}"));
        }
        if let Some(item_schema) = schema.get("items") {
            for (i, item) in items.iter().enumerate() {
                validate_value(item, item_schema, &format!("{path}[{i}]"))?;
            }
        }
    }
    Ok(())
}

fn generation_options(
    args: &Value,
    layout: &str,
) -> Result<crate::generation::job::ChartOptions, Error> {
    use crate::generation::config::{
        BracketDetermination, BracketMethod, PerformedConfig, PerformedPreset,
    };
    let preset = match args
        .get("profile")
        .and_then(Value::as_str)
        .unwrap_or("balanced")
    {
        "balanced" => PerformedPreset::Balanced,
        "stamina_single" => PerformedPreset::StaminaSingle,
        "stamina_double" => PerformedPreset::StaminaDouble,
        "no_candles" => PerformedPreset::PatternNoCandles,
        "no_inward" => PerformedPreset::PatternNoInward,
        "no_transition_limit" => PerformedPreset::PatternNoTransitionLimit,
        _ => return invalid("Unsupported generation profile"),
    };
    let mut options = crate::generation::job::ChartOptions::balanced(layout);
    options.performed = PerformedConfig::preset(preset);
    if args.get("brackets").and_then(Value::as_str) == Some("none") {
        options.expressed.default_bracket_parsing_method = BracketMethod::NoBrackets;
        options.expressed.bracket_parsing_determination = BracketDetermination::UseDefaultMethod;
    }
    Ok(options)
}

fn note_kind(kind: Kind) -> &'static str {
    match kind {
        Kind::Tap => "tap",
        Kind::Hold => "hold",
        Kind::Roll => "roll",
        Kind::Mine => "mine",
        Kind::Lift => "lift",
        Kind::Fake => "fake",
        Kind::Keysound => "keysound",
        Kind::Attack => "attack",
    }
}
