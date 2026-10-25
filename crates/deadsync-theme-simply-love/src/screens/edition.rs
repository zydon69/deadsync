//! Compact native editor screen. The document and every edit command live in
//! deadsync-editor; this module owns presentation, shortcuts, and exit prompts.
use crate::{act, i18n};
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::{
    actors::Actor,
    space::{screen_center_x, screen_height, screen_width},
};
use deadsync_editor::{
    generation::{
        config::{PerformedConfig, PerformedPreset},
        job::{ChartOptions, ChartPreview, Job, Phase, Report, TARGET_LAYOUTS},
    },
    notes::{Kind, ROWS_PER_BEAT},
    workspace::{Command, SNAP_NAMES, SNAP_ROWS, TOOLS, Workspace},
};
use std::{path::Path, sync::Arc};

#[derive(Clone, Copy)]
enum Panel {
    Menu(usize),
    Transforms(usize),
    Unsaved(usize),
    ConfirmDiscard,
    Generation(usize),
    Generating,
    GenerationPreview(usize),
}

struct GenerationSettings {
    target: usize,
    preset: usize,
    seed: i32,
}

pub struct State {
    workspace: Workspace,
    generation: GenerationSettings,
    job: Option<Job>,
    preview: Option<ChartPreview>,
    control: bool,
    shift: bool,
    help: bool,
    panel: Option<Panel>,
    status: Arc<str>,
    toolbar: Arc<str>,
    chart_label: Arc<str>,
    last_recovery_revision: u64,
}

impl State {
    pub fn open(path: &Path, chart: usize) -> Result<Self, deadsync_editor::Error> {
        Self::from_workspace(Workspace::open(path, chart)?)
    }

    pub fn open_selected(
        path: &Path,
        chart_type: &str,
        difficulty: &str,
        description: &str,
        meter: u32,
    ) -> Result<Self, deadsync_editor::Error> {
        let file = deadsync_editor::storage::FileSession::open(path)?;
        let matches: Vec<_> = file
            .session
            .document()
            .charts()
            .iter()
            .enumerate()
            .filter(|(_, chart)| {
                chart.steps_type == chart_type
                    && chart.difficulty.eq_ignore_ascii_case(difficulty)
                    && chart.description == description
                    && chart.meter.parse::<u32>().ok() == Some(meter)
            })
            .map(|(index, _)| index)
            .collect();
        if matches.len() != 1 {
            return Err(deadsync_editor::Error::InvalidEdit(
                "Cannot identify one source chart unambiguously; reload the song selection".into(),
            ));
        }
        Self::from_workspace(Workspace::from_file(file, matches[0])?)
    }

    fn from_workspace(workspace: Workspace) -> Result<Self, deadsync_editor::Error> {
        let layout = &workspace.file().session.document().charts()[workspace.chart()].steps_type;
        let target = TARGET_LAYOUTS
            .iter()
            .position(|candidate| *candidate == layout)
            .unwrap_or(0);
        let mut state = Self {
            generation: GenerationSettings {
                target,
                preset: 0,
                seed: 0,
            },
            job: None,
            preview: None,
            workspace,
            control: false,
            shift: false,
            help: false,
            panel: None,
            status: i18n::tr("Edition", "Ready"),
            toolbar: Arc::from(""),
            chart_label: Arc::from(""),
            last_recovery_revision: 0,
        };
        state.refresh();
        Ok(state)
    }

    pub fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    pub fn is_generating(&self) -> bool {
        self.job.is_some()
    }
    pub fn generation_report(&self) -> Option<Report> {
        self.preview.as_ref().map(ChartPreview::report)
    }

    /// Polling is nonblocking and is called by Practice's normal update loop.
    pub fn update(&mut self) {
        let Some(job) = self.job.as_mut() else {
            return;
        };
        let cancelled = job.is_cancelled();
        if let Some(result) = job.poll() {
            self.job = None;
            match result {
                Ok(preview) => {
                    self.preview = Some(preview);
                    self.panel = Some(Panel::GenerationPreview(0));
                    self.status = i18n::tr("Edition", "GenerationPreviewHelp");
                }
                Err(error) => {
                    self.panel = None;
                    self.status = if cancelled {
                        i18n::tr("Edition", "GenerationCancelled")
                    } else {
                        i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
                    };
                }
            }
            self.refresh();
        } else if !cancelled {
            let progress = job.progress();
            let phase = match progress.phase {
                Phase::Preparing => "GenerationPreparing",
                Phase::Interpreting => "GenerationInterpreting",
                Phase::Converting => "GenerationConverting",
                Phase::Assembling => "GenerationAssembling",
                Phase::Ready => "GenerationReady",
            };
            self.status = i18n::tr_fmt(
                "Edition",
                "GenerationProgress",
                &[
                    ("phase", &i18n::tr("Edition", phase)),
                    ("done", &progress.completed.to_string()),
                    ("total", &progress.total.to_string()),
                ],
            );
        }
    }

    fn start_generation(&mut self) {
        if self.job.is_some() {
            return;
        }
        let mut options = ChartOptions::balanced(TARGET_LAYOUTS[self.generation.target]);
        options.seed = self.generation.seed;
        options.performed = PerformedConfig::preset(match self.generation.preset {
            1 => PerformedPreset::StaminaSingle,
            2 => PerformedPreset::StaminaDouble,
            _ => PerformedPreset::Balanced,
        });
        match self
            .workspace
            .generation_task(options)
            .and_then(|task| task.spawn())
        {
            Ok(job) => {
                self.preview = None;
                self.job = Some(job);
                self.panel = Some(Panel::Generating);
                self.status = i18n::tr("Edition", "GenerationPreparing");
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
    }

    fn cancel_generation(&mut self) {
        if let Some(job) = &self.job {
            job.cancel();
            self.status = i18n::tr("Edition", "GenerationCancelling");
        }
    }

    fn refresh(&mut self) {
        let dirty = if self.workspace.file().session.is_dirty() {
            " *"
        } else {
            ""
        };
        self.toolbar = i18n::tr_fmt(
            "Edition",
            "Toolbar",
            &[
                ("dirty", dirty),
                (
                    "beat",
                    &format!("{:.3}", self.workspace.row() as f64 / ROWS_PER_BEAT as f64),
                ),
                ("snap", SNAP_NAMES[self.workspace.snap()]),
                ("player", &(self.workspace.player() + 1).to_string()),
                (
                    "tool",
                    &i18n::tr("Edition", tool_key(self.workspace.tool())),
                ),
            ],
        );
        let chart = &self.workspace.file().session.document().charts()[self.workspace.chart()];
        self.chart_label = Arc::from(format!(
            "{} / {}   {}   {}   [{}]",
            self.workspace.chart() + 1,
            self.workspace.file().session.document().charts().len(),
            chart.steps_type,
            chart.difficulty,
            chart.meter
        ));
    }

    fn command(&mut self, command: Command) {
        match self.workspace.execute(command) {
            Ok(()) => {
                self.status = match command {
                    Command::Save => i18n::tr("Edition", "Saved"),
                    Command::SaveRecovery => i18n::tr("Edition", "DraftSaved"),
                    Command::Recover => i18n::tr("Edition", "DraftRecovered"),
                    _ => i18n::tr("Edition", "Ready"),
                }
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
        self.refresh();
    }

    /// The enclosing Practice screen ignores its logical pad navigation while
    /// this screen is open, so an Escape cannot silently abandon edited data.
    /// Returns true only after an explicit, safe close decision.
    pub fn raw_key(&mut self, event: &RawKeyboardEvent) -> bool {
        match event.code {
            KeyCode::ControlLeft
            | KeyCode::ControlRight
            | KeyCode::SuperLeft
            | KeyCode::SuperRight => {
                self.control = event.pressed;
                return false;
            }
            KeyCode::ShiftLeft | KeyCode::ShiftRight => {
                self.shift = event.pressed;
                return false;
            }
            _ => {}
        }
        if !event.pressed {
            return false;
        }
        if event.repeat
            && !matches!(
                event.code,
                KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::PageUp | KeyCode::PageDown
            )
        {
            return false;
        }
        if self.panel.is_some() {
            return self.panel_key(event.code);
        }
        if event.code == KeyCode::Escape || event.code == KeyCode::F2 {
            return self.request_close();
        }
        if event.code == KeyCode::F3 {
            self.panel = Some(Panel::Generation(0));
            self.status = i18n::tr("Edition", "GenerationOptionsHelp");
            return false;
        }
        if event.code == KeyCode::F1 {
            self.panel = Some(Panel::Menu(0));
            return false;
        }
        let command = if self.control {
            match event.code {
                KeyCode::KeyS if self.shift => Some(Command::SaveRecovery),
                KeyCode::KeyS => Some(Command::Save),
                KeyCode::KeyZ if self.shift => Some(Command::Redo),
                KeyCode::KeyZ => Some(Command::Undo),
                KeyCode::KeyY => Some(Command::Redo),
                KeyCode::KeyC => Some(Command::Copy),
                KeyCode::KeyX => Some(Command::Cut),
                KeyCode::KeyV => Some(Command::Paste),
                KeyCode::KeyR => Some(Command::Recover),
                KeyCode::Tab => {
                    let count = self.workspace.file().session.document().charts().len();
                    let delta = if self.shift { count - 1 } else { 1 };
                    Some(Command::SelectChart(
                        (self.workspace.chart() + delta) % count,
                    ))
                }
                _ => None,
            }
        } else {
            match event.code {
                KeyCode::ArrowUp => Some(Command::Move(-1)),
                KeyCode::ArrowDown => Some(Command::Move(1)),
                KeyCode::PageUp => Some(Command::Move(-16)),
                KeyCode::PageDown => Some(Command::Move(16)),
                KeyCode::Home => Some(Command::Seek(0)),
                KeyCode::End => Some(Command::Seek(
                    self.workspace
                        .grid()
                        .iter()
                        .map(|(pos, note)| note.end.unwrap_or(pos.row))
                        .max()
                        .unwrap_or(0),
                )),
                KeyCode::ArrowLeft => {
                    Some(Command::SetSnap(self.workspace.snap().saturating_sub(1)))
                }
                KeyCode::ArrowRight => Some(Command::SetSnap(
                    (self.workspace.snap() + 1).min(SNAP_ROWS.len() - 1),
                )),
                KeyCode::Space => Some(Command::ToggleSelection),
                KeyCode::Delete | KeyCode::Backspace => Some(Command::Delete),
                KeyCode::KeyP => Some(Command::SelectPlayer(
                    (self.workspace.player() + 1) % self.workspace.grid().players(),
                )),
                KeyCode::KeyA => Some(Command::ToggleAutoAdvance),
                KeyCode::Tab => {
                    let index = TOOLS
                        .iter()
                        .position(|&tool| tool == self.workspace.tool())
                        .unwrap_or(0);
                    Some(Command::SelectTool(
                        TOOLS[(index + if self.shift { TOOLS.len() - 1 } else { 1 }) % TOOLS.len()],
                    ))
                }
                code => lane_for_key(code).map(Command::Place),
            }
        };
        if let Some(command) = command {
            if !event.repeat || matches!(command, Command::Move(_) | Command::Seek(_)) {
                self.command(command);
            }
        }
        false
    }

    fn request_close(&mut self) -> bool {
        if self.workspace.file().session.is_dirty() {
            self.panel = Some(Panel::Unsaved(0));
            false
        } else {
            true
        }
    }

    fn panel_key(&mut self, code: KeyCode) -> bool {
        let panel = self.panel.unwrap();
        if matches!(panel, Panel::Generating) {
            if matches!(code, KeyCode::Escape | KeyCode::Enter) {
                self.cancel_generation();
            }
            return false;
        }
        if matches!(panel, Panel::GenerationPreview(_))
            && matches!(
                code,
                KeyCode::PageUp | KeyCode::PageDown | KeyCode::Home | KeyCode::End
            )
        {
            let command = match code {
                KeyCode::Home => Command::Seek(0),
                KeyCode::End => Command::Seek(
                    self.preview
                        .as_ref()
                        .map(|preview| {
                            preview
                                .grid()
                                .iter()
                                .map(|(pos, note)| note.end.unwrap_or(pos.row))
                                .max()
                                .unwrap_or(0)
                        })
                        .unwrap_or(0),
                ),
                KeyCode::PageUp => Command::Move(-16),
                _ => Command::Move(16),
            };
            self.command(command);
            return false;
        }
        if code == KeyCode::Escape {
            self.preview = None;
            self.panel = None;
            return false;
        }
        match panel {
            Panel::Generation(index) => {
                if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
                    self.panel = Some(Panel::Generation(
                        (index + if code == KeyCode::ArrowUp { 4 } else { 1 }) % 5,
                    ));
                } else if matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight) {
                    let forward = code == KeyCode::ArrowRight;
                    match index {
                        0 => {
                            self.generation.target = (self.generation.target
                                + if forward { 1 } else { TARGET_LAYOUTS.len() - 1 })
                                % TARGET_LAYOUTS.len()
                        }
                        1 => {
                            self.generation.preset =
                                (self.generation.preset + if forward { 1 } else { 2 }) % 3
                        }
                        2 => {
                            self.generation.seed =
                                self.generation
                                    .seed
                                    .wrapping_add(if forward { 1 } else { -1 })
                        }
                        _ => {}
                    }
                } else if code == KeyCode::Enter {
                    match index {
                        3 => self.start_generation(),
                        4 => self.panel = None,
                        _ => {}
                    }
                }
            }
            Panel::GenerationPreview(index) => {
                if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
                    self.panel = Some(Panel::GenerationPreview(1 - index));
                } else if code == KeyCode::Enter {
                    if index == 1 {
                        self.preview = None;
                        self.panel = None;
                        self.status = i18n::tr("Edition", "GenerationDiscarded");
                    } else if let Some(preview) = &self.preview {
                        match self.workspace.apply_generated(preview) {
                            Ok(_) => {
                                self.preview = None;
                                self.panel = None;
                                self.status = i18n::tr("Edition", "GenerationApplied");
                                self.refresh();
                            }
                            Err(error) => {
                                self.status = i18n::tr_fmt(
                                    "Edition",
                                    "Error",
                                    &[("message", &error.to_string())],
                                )
                            }
                        }
                    }
                }
            }
            Panel::Menu(index) | Panel::Transforms(index) | Panel::Unsaved(index) => {
                let count = match panel {
                    Panel::Menu(_) => 9,
                    Panel::Transforms(_) => 4,
                    _ => 3,
                };
                if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
                    let next = (index
                        + if code == KeyCode::ArrowUp {
                            count - 1
                        } else {
                            1
                        })
                        % count;
                    self.panel = Some(match panel {
                        Panel::Menu(_) => Panel::Menu(next),
                        Panel::Transforms(_) => Panel::Transforms(next),
                        _ => Panel::Unsaved(next),
                    });
                } else if code == KeyCode::Enter {
                    self.panel = None;
                    if matches!(panel, Panel::Transforms(_)) {
                        use deadsync_editor::transform::Transform;
                        let transform = match index {
                            0 => Transform::Mirror,
                            1 => Transform::Flip,
                            2 => Transform::ShiftRows(-(SNAP_ROWS[self.workspace.snap()] as i64)),
                            _ => Transform::ShiftRows(SNAP_ROWS[self.workspace.snap()] as i64),
                        };
                        self.command(Command::Transform(transform));
                    } else if matches!(panel, Panel::Unsaved(_)) {
                        match index {
                            1 => {
                                self.command(Command::Save);
                                return !self.workspace.file().session.is_dirty();
                            }
                            2 => self.panel = Some(Panel::ConfirmDiscard),
                            _ => {}
                        }
                    } else {
                        match index {
                            1 => self.command(Command::Save),
                            2 => self.command(Command::Undo),
                            3 => self.command(Command::Redo),
                            4 => self.panel = Some(Panel::Transforms(0)),
                            5 => {
                                self.panel = Some(Panel::Generation(0));
                                self.status = i18n::tr("Edition", "GenerationOptionsHelp");
                            }
                            6 => self.command(Command::SaveRecovery),
                            7 => self.help = !self.help,
                            8 => return self.request_close(),
                            _ => {}
                        }
                    }
                }
            }
            Panel::ConfirmDiscard if code == KeyCode::Enter => return true,
            _ => {}
        }
        false
    }

    /// Called at a low frequency by the screen. Durable drafts never write the
    /// original chart. No I/O is performed from actor construction.
    pub fn autosave(&mut self) {
        let session = &self.workspace.file().session;
        if session.is_dirty() && session.revision() != self.last_recovery_revision {
            match self.workspace.file().save_recovery() {
                Ok(()) => self.last_recovery_revision = session.revision(),
                Err(error) => {
                    self.status =
                        i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
                }
            }
        }
    }

    pub fn actors(&self, actors: &mut Vec<Actor>) {
        let width = screen_width();
        let height = screen_height();
        actors.push(act!(quad: xy(width * 0.5, height * 0.5): zoomto(width, height): diffuse(0.035, 0.045, 0.065, 1.0): z(8000)));
        text(
            actors,
            &i18n::tr("Edition", "Title"),
            18.0,
            15.0,
            0.85,
            [0.45, 0.85, 1.0, 1.0],
        );
        text(actors, &self.toolbar, 18.0, 45.0, 0.62, [1.0; 4]);
        text(
            actors,
            &self.chart_label,
            18.0,
            70.0,
            0.55,
            [0.65, 0.7, 0.8, 1.0],
        );
        text(
            actors,
            &i18n::tr("Edition", "Shortcuts"),
            18.0,
            height - 43.0,
            0.5,
            [0.65, 0.7, 0.8, 1.0],
        );
        text(
            actors,
            &self.status,
            18.0,
            height - 20.0,
            0.5,
            [0.9, 0.9, 0.9, 1.0],
        );
        let grid = self
            .preview
            .as_ref()
            .map(ChartPreview::grid)
            .unwrap_or_else(|| self.workspace.grid());
        let lane_width = ((width - 210.0).max(80.0) / f32::from(grid.columns())).min(48.0);
        let field_width = lane_width * f32::from(grid.columns());
        let left = screen_center_x() - field_width * 0.5;
        let top = 105.0;
        let bottom = height - 65.0;
        let cursor_y = top + 55.0;
        let pixels_per_row = 1.0;
        let row = self.workspace.row();
        let start = row.saturating_sub(55);
        let end = row.saturating_add((bottom - cursor_y).max(0.0) as u64);
        for lane in 0..=grid.columns() {
            actors.push(act!(quad: xy(left + f32::from(lane) * lane_width, (top + bottom) * 0.5): zoomto(1.0, bottom - top): diffuse(0.12, 0.16, 0.22, 1.0): z(8001)));
        }
        for beat in start / 48..=end / 48 {
            let y = cursor_y + (beat as f64 * 48.0 - row as f64) as f32 * pixels_per_row;
            let alpha = if beat % 4 == 0 { 0.4 } else { 0.15 };
            actors.push(act!(quad: xy(screen_center_x(), y): zoomto(field_width, 1.0): diffuse(0.6, 0.7, 0.8, alpha): z(8001)));
            if beat % 4 == 0 {
                text(
                    actors,
                    &(beat / 4 + 1).to_string(),
                    left - 30.0,
                    y,
                    0.45,
                    [0.6, 0.7, 0.8, 1.0],
                );
            }
        }
        if let Some((a, b)) = self.workspace.selection() {
            let y1 = (cursor_y + (a as f64 - row as f64) as f32).clamp(top, bottom);
            let y2 = (cursor_y + (b as f64 - row as f64) as f32).clamp(top, bottom);
            actors.push(act!(quad: xy(screen_center_x(), (y1 + y2) * 0.5): zoomto(field_width, (y2 - y1).abs().max(2.0)): diffuse(0.2, 0.6, 0.9, 0.2): z(8002)));
        }
        for (position, note) in grid.visible(start, end) {
            let x = left + (f32::from(position.lane) + 0.5) * lane_width;
            let y = cursor_y + (position.row as f64 - row as f64) as f32 * pixels_per_row;
            let color = player_color(position.player);
            if let Some(tail) = note.end {
                let a = y.max(top);
                let b = (cursor_y + (tail as f64 - row as f64) as f32).min(bottom);
                actors.push(act!(quad: xy(x, (a + b) * 0.5): zoomto(lane_width * 0.3, (b - a).max(0.0)): diffuse(color[0], color[1], color[2], 0.65): z(8003)));
            }
            if y < top || y > bottom {
                continue;
            }
            let symbol = match note.kind {
                Kind::Mine => "M",
                Kind::Lift => "L",
                Kind::Fake => "F",
                Kind::Keysound => "K",
                Kind::Attack => "A",
                _ => "",
            };
            if symbol.is_empty() {
                let rotation = lane_rotation(
                    self.preview
                        .as_ref()
                        .map(ChartPreview::target_layout)
                        .unwrap_or_else(|| {
                            &self.workspace.file().session.document().charts()
                                [self.workspace.chart()]
                            .steps_type
                        }),
                    position.lane,
                );
                actors.push(act!(sprite("test_input/highlightarrow.png"): xy(x, y): zoomto(lane_width * 0.7, lane_width * 0.7): rotationz(rotation): diffuse(color[0], color[1], color[2], 1.0): z(8004)));
            } else {
                text(actors, symbol, x - 6.0, y - 9.0, 0.65, color);
            }
        }
        actors.push(act!(quad: xy(screen_center_x(), cursor_y): zoomto(field_width, 2.0): diffuse(1.0, 0.85, 0.25, 1.0): z(8005)));
        if self.help {
            self.help_actors(actors);
        }
        if let Some(panel) = self.panel {
            self.panel_actors(panel, actors);
        }
    }

    fn help_actors(&self, actors: &mut Vec<Actor>) {
        actors.push(act!(quad: xy(screen_width() * 0.5, screen_height() * 0.5): zoomto(screen_width() - 30.0, 280.0): diffuse(0.05, 0.08, 0.12, 0.97): z(8010)));
        let label = i18n::tr("Edition", "Help");
        actors.push(act!(text: font("miso"): settext(label): align(0.0, 0.0): xy(35.0, screen_height() * 0.5 - 120.0): zoom(0.65): z(8011)));
    }

    fn panel_actors(&self, panel: Panel, actors: &mut Vec<Actor>) {
        let keys: &[&str] = match panel {
            Panel::Menu(_) => &[
                "Resume",
                "Save",
                "Undo",
                "Redo",
                "Transforms",
                "GenerateChart",
                "SaveDraft",
                "ToggleHelp",
                "Close",
            ],
            Panel::Transforms(_) => &["Mirror", "Flip", "ShiftEarlier", "ShiftLater"],
            Panel::Unsaved(_) => &["KeepEditing", "SaveAndClose", "Discard"],
            Panel::ConfirmDiscard => &["ConfirmDiscard"],
            Panel::Generation(_) => &[
                "GenerationTarget",
                "GenerationPreset",
                "GenerationSeed",
                "GenerationStart",
                "KeepEditing",
            ],
            Panel::Generating => &["GenerationCancel"],
            Panel::GenerationPreview(_) => &["GenerationApply", "GenerationDiscard"],
        };
        let selected = match panel {
            Panel::Menu(i)
            | Panel::Transforms(i)
            | Panel::Unsaved(i)
            | Panel::Generation(i)
            | Panel::GenerationPreview(i) => i,
            _ => 0,
        };
        let previewing = matches!(panel, Panel::GenerationPreview(_));
        let extra_height = if previewing { 128.0 } else { 0.0 };
        let panel_height = keys.len() as f32 * 32.0 + 30.0 + extra_height;
        let panel_y = if previewing {
            screen_height() - panel_height * 0.5 - 8.0
        } else {
            screen_height() * 0.5
        };
        actors.push(act!(quad: xy(screen_width() * 0.5, panel_y): zoomto(if previewing { screen_width() - 30.0 } else { 520.0 }, panel_height): diffuse(0.08, 0.1, 0.15, 1.0): z(8020)));
        if let Some(preview) = self.preview.as_ref().filter(|_| previewing) {
            let report = preview.report();
            let summary = i18n::tr_fmt(
                "Edition",
                "GenerationSummary",
                &[
                    ("layout", preview.target_layout()),
                    ("seed", &preview.seed().to_string()),
                    ("notes", &report.notes.to_string()),
                    ("source_notes", &report.source_notes.to_string()),
                    ("source_mines", &report.source_mines.to_string()),
                    ("mines", &report.mines.to_string()),
                    ("skipped", &report.skipped_events.to_string()),
                    ("dropped", &report.dropped_mines.to_string()),
                    ("merged", &report.merged_mines.to_string()),
                    ("keysounds", &report.ignored_keysounds.to_string()),
                ],
            );
            actors.push(act!(text: font("miso"): settext(summary): align(0.5, 0.0): xy(screen_width() * 0.5, panel_y - panel_height * 0.5 + 10.0): zoom(0.6): z(8021)));
            if report.empty {
                actors.push(act!(text: font("miso"): settext(i18n::tr("Edition", "GenerationEmpty")): align(0.5, 0.5): xy(screen_width() * 0.5, panel_y - panel_height * 0.5 + 108.0): zoom(0.6): diffuse(1.0, 0.65, 0.3, 1.0): z(8021)));
            }
        }
        for (index, key) in keys.iter().enumerate() {
            let color = if index == selected {
                [1.0, 0.85, 0.25, 1.0]
            } else {
                [0.8, 0.8, 0.85, 1.0]
            };
            let label = if matches!(panel, Panel::Generation(_)) {
                let value = match index {
                    0 => TARGET_LAYOUTS[self.generation.target].to_owned(),
                    1 => i18n::tr(
                        "Edition",
                        [
                            "GenerationBalanced",
                            "GenerationStaminaSingle",
                            "GenerationStaminaDouble",
                        ][self.generation.preset],
                    )
                    .to_string(),
                    2 => self.generation.seed.to_string(),
                    _ => String::new(),
                };
                if index < 3 {
                    i18n::tr_fmt("Edition", key, &[("value", &value)])
                } else {
                    i18n::tr("Edition", key)
                }
            } else {
                i18n::tr("Edition", key)
            };
            actors.push(act!(text: font("miso"): settext(label): align(0.5, 0.5): xy(screen_width() * 0.5, panel_y - panel_height * 0.5 + extra_height + 30.0 + index as f32 * 32.0): zoom(0.8): diffuse(color[0], color[1], color[2], color[3]): z(8021)));
        }
    }
}

fn text(actors: &mut Vec<Actor>, label: &str, x: f32, y: f32, zoom: f32, color: [f32; 4]) {
    actors.push(act!(text: font("miso"): settext(label.to_owned()): align(0.0, 0.0): xy(x, y): zoom(zoom): diffuse(color[0], color[1], color[2], color[3]): z(8006)));
}

fn player_color(player: u32) -> [f32; 4] {
    const COLORS: [[f32; 4]; 6] = [
        [0.2, 0.65, 1.0, 1.0],
        [1.0, 0.35, 0.65, 1.0],
        [0.35, 1.0, 0.55, 1.0],
        [1.0, 0.75, 0.25, 1.0],
        [0.7, 0.4, 1.0, 1.0],
        [0.3, 1.0, 1.0, 1.0],
    ];
    COLORS[player as usize % COLORS.len()]
}

fn lane_rotation(layout: &str, lane: u16) -> f32 {
    if layout.starts_with("pump-") {
        [135.0, 225.0, 0.0, 45.0, 315.0][usize::from(lane % 5)]
    } else {
        [180.0, 90.0, 270.0, 0.0][usize::from(lane % 4)]
    }
}

fn lane_for_key(code: KeyCode) -> Option<u16> {
    Some(match code {
        KeyCode::Digit1 => 0,
        KeyCode::Digit2 => 1,
        KeyCode::Digit3 => 2,
        KeyCode::Digit4 => 3,
        KeyCode::Digit5 => 4,
        KeyCode::Digit6 => 5,
        KeyCode::Digit7 => 6,
        KeyCode::Digit8 => 7,
        KeyCode::Digit9 => 8,
        KeyCode::Digit0 => 9,
        _ => return None,
    })
}

fn tool_key(kind: Kind) -> &'static str {
    match kind {
        Kind::Tap => "Tap",
        Kind::Hold => "Hold",
        Kind::Roll => "Roll",
        Kind::Mine => "Mine",
        Kind::Lift => "Lift",
        Kind::Fake => "Fake",
        Kind::Keysound => "Keysound",
        Kind::Attack => "Attack",
    }
}
