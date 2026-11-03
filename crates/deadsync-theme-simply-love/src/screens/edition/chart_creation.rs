use super::{Panel, State};
use crate::i18n;
use deadlib_platform::input::KeyCode;
use deadsync_editor::chart_creation::{Draft, LAYOUTS};
pub(super) struct Settings {
    pub(super) draft: Draft,
    layout: usize,
}
impl State {
    pub(super) fn open_new_chart(&mut self) {
        if !self.has_song(){self.status=i18n::tr("Edition","FileNoSongHelp");return;}
        if !self.workspace.file().session.document().charts().is_empty()
            && deadsync_editor::timing_edit::migration::needed(
                self.workspace.file().session.document(),
            )
        {
            self.open_timing_migration();
            return;
        }
        match self.workspace.new_chart_draft() {
            Ok(draft) => {
                let kind = self
                    .workspace
                    .active_chart()
                    .and_then(|i| self.workspace.file().session.document().charts().get(i))
                    .map_or(LAYOUTS[0], |chart| chart.steps_type.as_str());
                self.new_chart = Some(Settings {
                    draft,
                    layout: LAYOUTS.iter().position(|name| *name == kind).unwrap_or(0),
                });
                self.panel = Some(Panel::NewChart(0));
                self.status = i18n::tr("Edition", "NewChartHelp");
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &crate::i18n::editor_error(&error))])
            }
        }
    }
    pub(super) fn new_chart_key(&mut self, index: usize, code: KeyCode) {
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
            self.panel = Some(Panel::NewChart(
                (index + if code == KeyCode::ArrowUp { 2 } else { 1 }) % 3,
            ));
            return;
        }
        if index == 0 && matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight) {
            if let Some(settings) = &mut self.new_chart {
                settings.layout = (settings.layout
                    + if code == KeyCode::ArrowLeft {
                        LAYOUTS.len() - 1
                    } else {
                        1
                    })
                    % LAYOUTS.len();
            }
        } else if matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) {
            if index == 2 {
                self.new_chart = None;
                self.panel = None;
                self.status = i18n::tr("Edition", "Ready");
                return;
            }
            if index != 1 {
                return;
            }
            let Some(settings) = &self.new_chart else {
                return;
            };
            let presentation=match self.capture_structural_chart_presentation(){Ok(value)=>value,Err(error)=>{self.status=i18n::editor_error(&error).into();return;}};
            match self
                .workspace
                .create_chart(&settings.draft, LAYOUTS[settings.layout])
            {
                Ok(_) => {
                    self.new_chart = None;
                    self.panel = None;
                    self.status = i18n::tr("Edition", "NewChartCreated");
                    if let Err(error)=self.finish_structural_chart_presentation(presentation){self.status=i18n::editor_error(&error).into();}
                    self.refresh();
                }
                Err(error) => {
                    self.status =
                        i18n::tr_fmt("Edition", "Error", &[("message", &crate::i18n::editor_error(&error))])
                }
            }
        }
    }
    pub(super) fn new_chart_label(&self) -> std::sync::Arc<str> {
        let layout = self
            .new_chart
            .as_ref()
            .map_or(LAYOUTS[0], |s| LAYOUTS[s.layout]);
        i18n::tr_fmt("Edition", "NewChartLayout", &[("layout", layout)])
    }
    pub(super) fn new_chart_summary(&self) -> std::sync::Arc<str> {
        let Some(settings) = &self.new_chart else {
            return i18n::tr("Edition", "NewChartHelp");
        };
        let defaults = settings.draft.defaults();
        let key = if self.workspace.file().session.document().format
            == deadsync_editor::document::Format::Sm
        {
            "NewChartSummarySm"
        } else {
            "NewChartSummary"
        };
        i18n::tr_fmt(
            "Edition",
            key,
            &[
                ("bpm", &defaults.tempo.to_string()),
                ("signature", &defaults.signature.to_string()),
            ],
        )
    }
}

#[cfg(test)]
mod sm_initial_timing_tests {
    use super::*;
    #[test]
    fn actual_sm_new_chart_dialog_describes_and_creates_independent_initial_timing() {
        for language in ["en","fr"] {
            i18n::init(deadsync_assets::language::load_for_tests(language));
            let root=tempfile::tempdir().unwrap();let path=root.path().join("new.sm");
            std::fs::write(&path,b"#BPMS:0=120,4=180;#STOPS:2=0.5;#ATTACKS:TIME=1:LEN=1:MODS=100% Drunk;#NOTES:dance-single::Hard:9::1000,0100;").unwrap();
            let mut state=State::open(&path,0).unwrap();let before=state.workspace.file().session.serialize().unwrap();state.open_new_chart();
            let summary=state.new_chart_summary();assert!(summary.contains("120"));assert!(summary.contains("4/4"));assert!(summary.contains(if language=="en"{"Independent initial timing"}else{"Timing initial indépendant"}));
            state.new_chart_key(1,KeyCode::Enter);assert_eq!(state.workspace.file().session.document().charts().len(),2);
            let session=&state.workspace.file().session;let chart=state.workspace.chart();assert_eq!(chart,1);
            assert_eq!(deadsync_editor::playback::Clock::capture(session,chart).unwrap().chart_time(240).unwrap(),2.5);
            assert!(deadsync_editor::attacks::placement::Timeline::capture(session,chart).unwrap().markers().is_empty());
            assert_eq!(deadsync_editor::attacks::placement::Timeline::capture(session,0).unwrap().markers().len(),1);
            state.command(deadsync_editor::workspace::Command::Undo);assert_eq!(state.workspace.file().session.serialize().unwrap(),before);
        }
    }
}
