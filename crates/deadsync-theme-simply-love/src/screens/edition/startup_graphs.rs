//! Application-lifetime preload is lazy until a real editor host frame.
//! Detached candidates cannot start workers or replace the first captured set.
use super::*;
use deadsync_editor::{
    generation::job::Job,
    options_preferences::{
        General,
        startup::{Report, Task},
    },
};
use std::sync::{Mutex, OnceLock};
struct Startup {
    requested: Vec<&'static str>,
    job: Option<Job<Report>>,
}
static STARTUP: OnceLock<Mutex<Startup>> = OnceLock::new();
impl State {
    pub fn startup_graph_preload_started(&self) -> bool {
        STARTUP.get().is_some()
    }
    pub fn startup_graphs_pending_restart(&self) -> bool {
        STARTUP.get().is_some_and(|runtime| {
            runtime.lock().unwrap_or_else(|p| p.into_inner()).requested
                != Task::capture(&self.general_options).layouts()
        })
    }
    pub(super) fn update_general_startup_graphs(&mut self) {
        let runtime = STARTUP.get_or_init(|| {
            let requested = Task::capture(&self.general_options).layouts().to_vec();
            let job = match Task::capture(&self.general_options).spawn() {
                Ok(job) => Some(job),
                Err(error) => {
                    log::warn!("Editor startup graph worker could not start: {error}");
                    None
                }
            };
            Mutex::new(Startup { requested, job })
        });
        let mut runtime = runtime.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(result) = runtime.job.as_mut().and_then(Job::poll) {
            runtime.job = None;
            match result {
                Ok(report) => {
                    for issue in report.issues {
                        log::warn!("Editor startup graph {}: {}", issue.layout, issue.detail);
                    }
                    log::debug!("Editor startup graphs ready: {:?}", report.loaded);
                }
                Err(error) => log::warn!("Editor startup graph worker failed: {error}"),
            }
        }
    }
    pub(super) fn startup_graph_label(&self) -> String {
        let layout = deadsync_editor::generation::job::TARGET_LAYOUTS[self.startup_graph_index];
        let enabled = i18n::tr(
            "Edition",
            if self
                .general_options
                .startup_graph_enabled(self.startup_graph_index)
            {
                "WaveformYes"
            } else {
                "WaveformNo"
            },
        );
        i18n::tr_fmt(
            "Edition",
            "GeneralStartupGraphValue",
            &[
                ("layout", layout),
                ("value", &enabled),
                (
                    "restart",
                    &if self.startup_graphs_pending_restart() {
                        i18n::tr("Edition", "GeneralStartupRestart")
                    } else {
                        Arc::from("")
                    },
                ),
            ],
        )
        .to_string()
    }
    pub(super) fn startup_graph_key(&mut self, index: usize, code: KeyCode) -> bool {
        if self.general_field != 6 || index != 1 {
            return false;
        }
        if matches!(code, KeyCode::PageUp | KeyCode::PageDown) {
            let len = deadsync_editor::generation::job::TARGET_LAYOUTS.len();
            self.startup_graph_index = (self.startup_graph_index
                + if code == KeyCode::PageUp { len - 1 } else { 1 })
                % len;
            return true;
        }
        if !matches!(
            code,
            KeyCode::ArrowLeft
                | KeyCode::ArrowRight
                | KeyCode::Enter
                | KeyCode::NumpadEnter
                | KeyCode::Home
                | KeyCode::KeyA
        ) {
            return false;
        }
        let mut value = self.general_options.clone();
        match code {
            KeyCode::Home => value.startup_graphs = General::default().startup_graphs,
            KeyCode::KeyA => value.select_all_startup_graphs(),
            _ => {
                if value
                    .toggle_startup_graph(self.startup_graph_index)
                    .is_err()
                {
                    return true;
                }
            }
        }
        match self.commit_general_options(value) {
            Ok(_) => self.status = i18n::tr("Edition", "GeneralOptionsSaved"),
            Err(error) => {
                self.status = i18n::tr_fmt(
                    "Edition",
                    "GeneralOptionsSaveFailedReason",
                    &[("error", &crate::i18n::editor_error(&error))],
                )
            }
        };
        true
    }
}
