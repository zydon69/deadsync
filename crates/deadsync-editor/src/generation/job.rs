//! Snapshot-bound generation and one bounded background worker per job.
//! Computation never owns the editor session or writes files.
use super::{
    config::{ExpressedConfig, PerformedConfig},
    expressed,
    graph::StepGraph,
    output, performed, source,
};
use crate::{
    Error,
    document::Format,
    history::{RevisionToken, Session},
    notes::Grid,
    pad::Pad,
};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
};

pub const TARGET_LAYOUTS: [&str; 11] = [
    "dance-single",
    "dance-double",
    "dance-solo",
    "dance-threepanel",
    "pump-single",
    "pump-halfdouble",
    "pump-double",
    "smx-beginner",
    "smx-single",
    "smx-dual",
    "smx-full",
];

#[derive(Clone, Debug)]
pub struct ChartOptions {
    pub target_layout: String,
    pub expressed: ExpressedConfig,
    pub performed: PerformedConfig,
    pub seed: i32,
}
impl ChartOptions {
    pub fn balanced(target_layout: impl Into<String>) -> Self {
        Self {
            target_layout: target_layout.into(),
            seed: 0,
            // GrooveAuthor EditorExpressedChartConfig defaults, 48e18f71.
            expressed: ExpressedConfig {
                min_level_for_brackets: 7,
                aggressive_when_uncoverable: true,
                balanced_bpm_for_aggressive: 3.0,
                balanced_bpm_for_no_brackets: 1.0,
                ..ExpressedConfig::default()
            },
            performed: PerformedConfig::balanced(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Preparing,
    Interpreting,
    Converting,
    Assembling,
    Ready,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub phase: Phase,
    pub completed: usize,
    pub total: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Report {
    pub source_notes: usize,
    pub source_mines: usize,
    pub notes: usize,
    pub mines: usize,
    pub skipped_events: usize,
    pub dropped_mines: usize,
    pub merged_mines: usize,
    pub ignored_keysounds: usize,
    pub empty: bool,
}

pub struct ChartTask {
    bytes: Arc<[u8]>,
    format: Format,
    token: RevisionToken,
    source_chart: usize,
    options: ChartOptions,
}
impl ChartTask {
    pub fn capture(
        session: &Session,
        source_chart: usize,
        options: ChartOptions,
    ) -> Result<Self, Error> {
        if source_chart >= session.document().charts().len() {
            return Err(Error::InvalidEdit(
                "Generation source chart does not exist".into(),
            ));
        }
        if !TARGET_LAYOUTS.contains(&options.target_layout.as_str()) {
            return Err(Error::Unsupported(
                "No reference generation graph for this target layout".into(),
            ));
        }
        options.expressed.validate()?;
        options.performed.validate()?;
        options
            .performed
            .normalized_weights(&options.target_layout)?;
        Ok(Self {
            bytes: session.serialize()?.into(),
            format: session.document().format,
            token: session.revision_token(),
            source_chart,
            options,
        })
    }

    pub fn run(
        self,
        cancelled: &AtomicBool,
        mut progress: impl FnMut(Progress),
    ) -> Result<ChartPreview, Error> {
        check(cancelled)?;
        progress(Progress::default());
        let prepared =
            source::prepare_editor(self.bytes, self.format, self.source_chart, cancelled)?;
        check(cancelled)?;
        let source_graph = StepGraph::builtin(&prepared.layout)?;
        let interpreted = expressed::build_chart(
            &source_graph,
            Pad::builtin(&prepared.layout)?,
            &prepared.request(),
            &self.options.expressed,
            &prepared.mines,
            cancelled,
            |_, completed, total| {
                progress(Progress {
                    phase: Phase::Interpreting,
                    completed,
                    total,
                })
            },
        )?;
        check(cancelled)?;
        progress(Progress {
            phase: Phase::Converting,
            completed: 0,
            total: interpreted.events.len(),
        });
        let converted = performed::convert(
            &performed::Request {
                layout: &self.options.target_layout,
                events: &interpreted.events,
                config: &self.options.performed,
                fallbacks: performed::automatic_fallbacks(
                    &prepared.layout,
                    &self.options.target_layout,
                )?,
                seed: self.options.seed,
                roots: None,
            },
            &interpreted.mines,
            cancelled,
            |_, completed, total| {
                progress(Progress {
                    phase: Phase::Converting,
                    completed,
                    total,
                })
            },
        )?;
        progress(Progress {
            phase: Phase::Assembling,
            completed: 0,
            total: 0,
        });
        let assembled = output::assemble(&converted, cancelled)?;
        let mines = assembled
            .grid
            .iter()
            .filter(|(_, note)| note.kind == crate::notes::Kind::Mine)
            .count();
        let count = assembled.grid.iter().count();
        let report = Report {
            source_notes: prepared.source_notes,
            source_mines: prepared.mines.len(),
            notes: count - mines,
            mines,
            skipped_events: converted.steps.skipped_events,
            dropped_mines: converted.dropped_mines,
            merged_mines: assembled.merged_mines,
            ignored_keysounds: prepared.ignored_keysounds,
            empty: count == 0,
        };
        check(cancelled)?;
        progress(Progress {
            phase: Phase::Ready,
            completed: count,
            total: count,
        });
        check(cancelled)?;
        Ok(ChartPreview {
            token: self.token,
            source_chart: self.source_chart,
            target_layout: self.options.target_layout,
            seed: self.options.seed,
            grid: assembled.grid,
            report,
        })
    }

    pub fn spawn(self) -> Result<Job, Error> {
        let cancelled = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(Mutex::new(Progress::default()));
        let (sender, receiver) = mpsc::sync_channel(1);
        let worker_cancelled = cancelled.clone();
        let worker_progress = progress.clone();
        std::thread::Builder::new()
            .name("editor-chart-generation".into())
            .spawn(move || {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    self.run(&worker_cancelled, |value| {
                        *worker_progress
                            .lock()
                            .unwrap_or_else(|error| error.into_inner()) = value;
                    })
                }))
                .unwrap_or_else(|_| Err(Error::InvalidEdit("Generation worker failed".into())));
                let _ = sender.send(result);
            })?;
        Ok(Job {
            cancelled,
            progress,
            receiver: Some(receiver),
        })
    }
}

pub struct ChartPreview {
    token: RevisionToken,
    source_chart: usize,
    target_layout: String,
    seed: i32,
    grid: Grid,
    report: Report,
}
impl ChartPreview {
    pub fn grid(&self) -> &Grid {
        &self.grid
    }
    pub fn report(&self) -> Report {
        self.report
    }
    pub fn source_chart(&self) -> usize {
        self.source_chart
    }
    pub fn target_layout(&self) -> &str {
        &self.target_layout
    }
    pub fn seed(&self) -> i32 {
        self.seed
    }
    pub fn is_current(&self, session: &Session) -> bool {
        session.accepts_token(&self.token)
    }
    pub fn apply(&self, session: &mut Session) -> Result<usize, Error> {
        if !self.is_current(session) {
            return Err(Error::InvalidEdit(
                "Generation preview is stale; generate it again".into(),
            ));
        }
        session.append_chart(
            self.source_chart,
            &self.target_layout,
            &self.grid,
            session.revision(),
            "Generate chart",
        )
    }
}

pub struct Job {
    cancelled: Arc<AtomicBool>,
    progress: Arc<Mutex<Progress>>,
    receiver: Option<Receiver<Result<ChartPreview, Error>>>,
}
impl Job {
    pub fn progress(&self) -> Progress {
        *self
            .progress
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
    pub fn poll(&mut self) -> Option<Result<ChartPreview, Error>> {
        let receiver = self.receiver.as_ref()?;
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => {
                Err(Error::InvalidEdit("Generation worker disconnected".into()))
            }
        };
        self.receiver = None;
        Some(if self.is_cancelled() {
            Err(Error::InvalidEdit("Generation cancelled".into()))
        } else {
            result
        })
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel();
    }
}
fn check(cancelled: &AtomicBool) -> Result<(), Error> {
    if cancelled.load(Ordering::Relaxed) {
        Err(Error::InvalidEdit("Generation cancelled".into()))
    } else {
        Ok(())
    }
}
