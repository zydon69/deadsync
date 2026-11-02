//! Source InitStepGraphDataAsync selection with native immutable graph caches.
//! Capture is cheap and detached. Only a real host starts the worker.
use crate::{
    Error,
    generation::{
        graph::StepGraph,
        job::{Job, Phase, Progress, TARGET_LAYOUTS},
        performed,
    },
    options_preferences::General,
    pad::Pad,
};
use std::sync::atomic::{AtomicBool, Ordering};
#[derive(Clone, Debug)]
pub struct Task {
    layouts: Vec<&'static str>,
}
#[derive(Debug)]
pub struct Issue {
    pub layout: &'static str,
    pub detail: String,
}
#[derive(Debug)]
pub struct Report {
    pub loaded: Vec<&'static str>,
    pub issues: Vec<Issue>,
}
impl Task {
    pub fn capture(preferences: &General) -> Self {
        Self {
            layouts: TARGET_LAYOUTS
                .into_iter()
                .filter(|layout| {
                    crate::document::chart_type::ALL
                        .iter()
                        .position(|kind| kind == layout)
                        .is_some_and(|i| preferences.startup_graphs.contains(&(i as i32)))
                })
                .collect(),
        }
    }
    pub fn layouts(&self) -> &[&'static str] {
        &self.layouts
    }
    pub fn available_layouts(
        &self,
        available: &std::collections::BTreeSet<&str>,
    ) -> Vec<&'static str> {
        self.layouts
            .iter()
            .copied()
            .filter(|layout| available.contains(layout))
            .collect()
    }
    pub fn spawn(self) -> Result<Job<Report>, Error> {
        Job::spawn_with("editor-startup-graphs", move |cancelled, progress| {
            self.run(cancelled, progress)
        })
    }
    pub fn run(
        self,
        cancelled: &AtomicBool,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<Report, Error> {
        let mut report = Report {
            loaded: Vec::new(),
            issues: Vec::new(),
        };
        // Native Pad::builtin reads and caches the complete embedded pad catalogue;
        // preserve source validation of every supported pad before selected graphs.
        let mut available = std::collections::BTreeSet::new();
        for layout in crate::chart_order::TYPE_ORDER {
            check(cancelled)?;
            if let Err(error) = Pad::builtin(layout) {
                report.issues.push(Issue {
                    layout,
                    detail: error.to_string(),
                });
            } else {
                available.insert(layout);
            }
        }
        let layouts = self.available_layouts(&available);
        let total = layouts.len();
        for (index, layout) in layouts.into_iter().enumerate() {
            check(cancelled)?;
            progress(Progress {
                phase: Phase::Preparing,
                completed: index,
                total,
            });
            let result = Pad::builtin(layout).and_then(|pad| {
                StepGraph::builtin(layout)
                    .and_then(|graph| performed::root_tiers(&graph, pad).map(|_| ()))
            });
            match result {
                Ok(()) => report.loaded.push(layout),
                Err(error) => report.issues.push(Issue {
                    layout,
                    detail: error.to_string(),
                }),
            }
        }
        check(cancelled)?;
        progress(Progress {
            phase: Phase::Ready,
            completed: total,
            total,
        });
        Ok(report)
    }
}
fn check(cancelled: &AtomicBool) -> Result<(), Error> {
    if cancelled.load(Ordering::Relaxed) {
        Err(Error::InvalidEdit("Startup graph loading cancelled".into()))
    } else {
        Ok(())
    }
}
impl General {
    pub fn startup_graph_enabled(&self, index: usize) -> bool {
        TARGET_LAYOUTS
            .get(index)
            .and_then(|layout| {
                crate::document::chart_type::ALL
                    .iter()
                    .position(|kind| kind == layout)
            })
            .is_some_and(|i| self.startup_graphs.contains(&(i as i32)))
    }
    pub fn toggle_startup_graph(&mut self, index: usize) -> Result<(), Error> {
        let layout = TARGET_LAYOUTS
            .get(index)
            .ok_or_else(|| Error::InvalidEdit("Invalid startup graph choice".into()))?;
        let value = crate::document::chart_type::ALL
            .iter()
            .position(|kind| kind == layout)
            .expect("supported source chart type") as i32;
        if !self.startup_graphs.remove(&value) {
            self.startup_graphs.insert(value);
        }
        Ok(())
    }
    pub fn select_all_startup_graphs(&mut self) {
        self.startup_graphs = TARGET_LAYOUTS
            .iter()
            .map(|layout| {
                crate::document::chart_type::ALL
                    .iter()
                    .position(|kind| kind == layout)
                    .expect("supported source chart type") as i32
            })
            .collect();
    }
}
