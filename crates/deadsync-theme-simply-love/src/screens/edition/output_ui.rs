//! Keyboard output policy and exact, per-issue publication acknowledgements.
//! Drafts never mutate the persisted policy; storage owns that publication edge.
use super::*;
use deadsync_editor::{
    advanced_export::{Issue, IssueKind, Options, Severity},
    document::Format,
    generation::metadata::Guid,
    history::file_intent::IntentGuard,
    output_policy::OutputPolicy,
    storage::{SaveAttempt, SaveReview},
};
use std::collections::BTreeSet;
#[derive(Default)]
pub(super) struct Ui {
    draft: Option<Draft>,
    pub(super) acknowledged: BTreeSet<usize>,
    save: Option<SaveReview>,
    save_as: bool,
    preferences: Option<super::file_menu::PublishedPreferences>,
}
struct Draft {
    guard: IntentGuard,
    policy_revision: u64,
    advanced: bool,
    format: Format,
    options: Options,
    chart: Option<Guid>,
}
impl State {
    pub(super) fn open_output_settings(&mut self) {
        if self.save_as_pending_guard() || !self.save_as_guard() {
            return;
        }
        self.stop_audio();
        self.minimap_release_input();
        self.lane_entry_reconcile();
        let file = self.workspace.file();
        let (advanced, format, options, chart) = match file.output_policy() {
            OutputPolicy::NativeLossless => (
                false,
                file.session.document().format,
                self.audio_preferences_file
                    .as_ref()
                    .map(|file| file.advanced_save())
                    .unwrap_or_default(),
                self.workspace
                    .active_chart()
                    .and_then(|i| file.session.chart_ids().get(i))
                    .cloned(),
            ),
            OutputPolicy::Advanced {
                format,
                options,
                timing_chart,
            } => (true, *format, *options, Some(timing_chart.clone())),
        };
        self.output_ui = Ui {
            draft: Some(Draft {
                guard: IntentGuard::capture(&file.session),
                policy_revision: file.output_policy_revision(),
                advanced,
                format,
                options,
                chart,
            }),
            ..Default::default()
        };
        self.panel = Some(Panel::OutputSettings(0));
        self.status = i18n::tr("Edition", "OutputSettingsHelp");
    }
    fn output_policy_draft(&self) -> Result<OutputPolicy, deadsync_editor::Error> {
        let draft =
            self.output_ui.draft.as_ref().ok_or_else(|| {
                deadsync_editor::Error::InvalidEdit("Missing output draft".into())
            })?;
        let file = self.workspace.file();
        if !draft.guard.is_current(&file.session)
            || draft.policy_revision != file.output_policy_revision()
        {
            return Err(deadsync_editor::Error::InvalidEdit(
                "Output settings became stale".into(),
            ));
        }
        if !draft.advanced {
            return Ok(OutputPolicy::NativeLossless);
        }
        let chart = draft
            .chart
            .as_ref()
            .filter(|chart| file.session.chart_ids().contains(chart))
            .ok_or_else(|| {
                deadsync_editor::Error::InvalidEdit("Select an existing timing chart".into())
            })?;
        Ok(OutputPolicy::Advanced {
            format: draft.format,
            options: draft.options,
            timing_chart: chart.clone(),
        })
    }
    pub(super) fn output_candidate_policy(&self) -> Result<OutputPolicy, deadsync_editor::Error> {
        if self.output_ui.draft.is_some() {
            self.output_policy_draft()
        } else {
            Ok(self.workspace.file().output_policy().clone())
        }
    }
    pub(super) fn output_global_options(&self) -> Option<Options> {
        self.output_ui.draft.as_ref().map(|draft| draft.options)
    }
    pub(super) fn output_clear(&mut self) {
        self.output_ui = Default::default();
    }
    pub(super) fn output_save_current(&mut self, use_draft: bool) {
        if !self.save_as_guard() {
            return;
        }
        if self.workspace.file().requires_save_as() {
            if use_draft {
                match self.output_policy_draft() {
                    Ok(_) => self.open_save_as_for_output(),
                    Err(error) => self.save_as_error(error),
                }
            } else {
                self.open_save_as();
            }
            return;
        }
        let path = self
            .workspace
            .file()
            .require_existing_path()
            .map(Path::to_owned);
        let prepared = path.and_then(|path| {
            self.prepare_published_preferences(
                &path,
                if use_draft {
                    self.output_global_options()
                } else {
                    None
                },
            )
        });
        match prepared {
            Ok(prepared) => self.output_ui.preferences = Some(prepared),
            Err(error) => {
                self.save_as_error(error);
                return;
            }
        }
        if !use_draft {
            match self.workspace.save_or_review() {
                Ok(attempt) => self.output_save_attempt(attempt),
                Err(error) => {
                    self.file_save_cancelled();
                    self.save_as_error(error);
                }
            }
            return;
        }
        match self
            .output_policy_draft()
            .and_then(|policy| self.workspace.prepare_save_with_policy(policy))
        {
            Ok(review) => {
                self.output_ui.save = Some(review);
                self.output_ui.save_as = false;
                self.output_ui.acknowledged.clear();
                let back = self.output_issues().len() + 1;
                self.panel = Some(Panel::OutputLoss(back));
                self.status = i18n::tr("Edition", "OutputReviewHelp");
            }
            Err(error) => self.save_as_error(error),
        }
    }
    pub(super) fn output_save_attempt(&mut self, attempt: SaveAttempt) {
        match attempt {
            SaveAttempt::NeedsReview(review) => {
                self.output_ui.save = Some(review);
                self.output_ui.save_as = false;
                self.output_ui.acknowledged.clear();
                let back = self.output_issues().len() + 1;
                self.panel = Some(Panel::OutputLoss(back));
                self.status = i18n::tr("Edition", "OutputReviewHelp");
            }
            other => {
                let preferences = self.output_ui.preferences.take();
                self.output_clear();
                if let Some(prepared) = preferences {
                    self.publish_output_preferences(prepared);
                }
                self.save_output_completed(other);
            }
        }
    }
    pub(super) fn output_review_save_as(&mut self) {
        self.output_ui.save = None;
        self.output_ui.save_as = true;
        self.output_ui.acknowledged.clear();
        let back = self.output_issues().len() + 1;
        self.panel = Some(Panel::OutputLoss(back));
        self.status = i18n::tr("Edition", "OutputReviewHelp");
    }
    fn output_issues(&self) -> &[Issue] {
        if self.output_ui.save_as {
            self.save_as_issues()
        } else {
            self.output_ui
                .save
                .as_ref()
                .map(SaveReview::issues)
                .unwrap_or(&[])
        }
    }
    fn output_review_authorized(&self) -> bool {
        self.output_issues().iter().enumerate().all(|(i, issue)| {
            issue.severity == Severity::Warning && self.output_ui.acknowledged.contains(&i)
        })
    }
    pub(super) fn output_key(&mut self, index: usize, code: KeyCode) {
        let enter = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter);
        if matches!(self.panel, Some(Panel::OutputSettings(_))) {
            if code == KeyCode::Escape || (enter && index == 11) {
                self.output_cancel();
                return;
            }
            if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
                let back = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
                self.panel = Some(Panel::OutputSettings(
                    (index + if back { 11 } else { 1 }) % 12,
                ));
                return;
            }
            if !(enter || matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight)) {
                return;
            }
            if index < 9 {
                let ids = self.workspace.file().session.chart_ids();
                let Some(draft) = self.output_ui.draft.as_mut() else {
                    return;
                };
                if index >= 3 {
                    draft.advanced = true;
                }
                match index {
                    0 => draft.advanced = !draft.advanced,
                    1 => {
                        draft.advanced = true;
                        draft.format = if draft.format == Format::Sm {
                            Format::Ssc
                        } else {
                            Format::Sm
                        };
                    }
                    2 => {
                        draft.advanced = true;
                        if !ids.is_empty() {
                            let old = draft
                                .chart
                                .as_ref()
                                .and_then(|id| ids.iter().position(|x| x == id));
                            let next = match (old, code == KeyCode::ArrowLeft) {
                                (Some(i), true) => (i + ids.len() - 1) % ids.len(),
                                (Some(i), false) => (i + 1) % ids.len(),
                                (None, _) => 0,
                            };
                            draft.chart = Some(ids[next].clone());
                        }
                    }
                    3 => {
                        draft.options.require_identical_timing_in_sm =
                            !draft.options.require_identical_timing_in_sm
                    }
                    4 => draft.options.omit_chart_timing = !draft.options.omit_chart_timing,
                    5 => draft.options.omit_custom_data = !draft.options.omit_custom_data,
                    6 => draft.options.anonymize = !draft.options.anonymize,
                    7 => {
                        draft.options.stepf2_pump_multiplayer =
                            !draft.options.stepf2_pump_multiplayer
                    }
                    8 => draft.options.outfox_smx = !draft.options.outfox_smx,
                    _ => {}
                }
            } else if enter && index == 9 {
                self.output_save_current(true);
            } else if enter && index == 10 {
                match self.output_policy_draft() {
                    Ok(_) => self.open_save_as_for_output(),
                    Err(error) => self.save_as_error(error),
                }
            }
            return;
        }
        let count = self.output_issues().len();
        let length = count + 3;
        if code == KeyCode::Escape || (enter && index == count + 2) {
            self.output_cancel();
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let back = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            self.panel = Some(Panel::OutputLoss(
                (index + if back { length - 1 } else { 1 }) % length,
            ));
            return;
        }
        if !enter {
            return;
        }
        if index < count {
            if self.output_issues()[index].severity == Severity::Warning
                && !self.output_ui.acknowledged.remove(&index)
            {
                self.output_ui.acknowledged.insert(index);
            }
        } else if index == count {
            if !self.output_review_authorized() {
                self.status = i18n::tr("Edition", "OutputUnacknowledged");
                return;
            }
            if self.output_ui.save_as {
                self.panel = Some(Panel::SaveAsReview(1));
                self.status = i18n::tr("Edition", "OutputSaveAsConfirm");
            } else if let Some(review) = self.output_ui.save.take() {
                match self
                    .workspace
                    .save_reviewed(review, &self.output_ui.acknowledged)
                {
                    Ok(attempt) => self.output_save_attempt(attempt),
                    Err(error) => {
                        self.panel = None;
                        self.file_save_cancelled();
                        self.output_clear();
                        self.save_as_error(error);
                    }
                }
            }
        } else if self.output_ui.save_as {
            self.save_as_back_to_destination();
        } else if self.output_ui.draft.is_some() {
            self.output_ui.save = None;
            self.output_ui.acknowledged.clear();
            self.panel = Some(Panel::OutputSettings(9));
        } else {
            self.output_cancel();
        }
    }
    fn output_cancel(&mut self) {
        self.output_clear();
        self.save_as_cancel_output_review();
        self.file_save_cancelled();
        self.panel = None;
        self.status = i18n::tr("Edition", "OutputCancelled");
    }
    pub(super) fn output_actors(&self, index: usize, actors: &mut Vec<Actor>) {
        let settings = matches!(self.panel, Some(Panel::OutputSettings(_)));
        let keys = [
            "OutputPolicy",
            "OutputFormat",
            "OutputChart",
            "OutputIdentical",
            "OutputOmitTiming",
            "OutputOmitCustom",
            "OutputAnonymize",
            "OutputStepF2",
            "OutputOutfox",
            "OutputSaveCurrent",
            "OutputSaveAs",
            "ScrollSettingsCancel",
        ];
        let mut rows = Vec::new();
        if settings {
            if let Some(d) = &self.output_ui.draft {
                for (i, key) in keys.iter().enumerate() {
                    let label = i18n::tr("Edition", key);
                    let value = match i {
                        0 => i18n::tr(
                            "Edition",
                            if d.advanced {
                                "OutputAdvanced"
                            } else {
                                "OutputNative"
                            },
                        )
                        .to_string(),
                        1 => d.format.extension().to_uppercase(),
                        2 => d
                            .chart
                            .as_ref()
                            .map(|id| {
                                format!(
                                    "{} / {}",
                                    self.workspace
                                        .file()
                                        .session
                                        .chart_ids()
                                        .iter()
                                        .position(|x| x == id)
                                        .map(|i| (i + 1).to_string())
                                        .unwrap_or_else(|| "?".into()),
                                    id
                                )
                            })
                            .unwrap_or_else(|| i18n::tr("Edition", "OutputNoChart").to_string()),
                        3..=8 => {
                            let enabled = [
                                d.options.require_identical_timing_in_sm,
                                d.options.omit_chart_timing,
                                d.options.omit_custom_data,
                                d.options.anonymize,
                                d.options.stepf2_pump_multiplayer,
                                d.options.outfox_smx,
                            ][i - 3];
                            i18n::tr("Edition", if enabled { "OutputYes" } else { "OutputNo" })
                                .to_string()
                        }
                        _ => String::new(),
                    };
                    rows.push(if value.is_empty() {
                        label.to_string()
                    } else {
                        format!("{label}: {value}")
                    });
                }
            }
        } else {
            for (i, issue) in self.output_issues().iter().enumerate() {
                let severity = i18n::tr(
                    "Edition",
                    if issue.severity == Severity::Error {
                        "OutputError"
                    } else if self.output_ui.acknowledged.contains(&i) {
                        "OutputAcknowledged"
                    } else {
                        "OutputWarning"
                    },
                );
                let kind = i18n::tr("Edition", issue_key(issue.kind));
                let chart = issue
                    .identity
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| i18n::tr("Edition", "OutputSongScope").to_string());
                let at = issue
                    .position
                    .map(|p| format!(" @ {} / P{} / L{}", p.row, p.player, p.lane))
                    .unwrap_or_default();
                rows.push(format!(
                    "{severity}: {kind} | {chart} | {}{at}",
                    issue.field
                ));
            }
            rows.push(
                i18n::tr(
                    "Edition",
                    if self.output_ui.save_as {
                        "OutputAcceptLosses"
                    } else {
                        "OutputPublish"
                    },
                )
                .to_string(),
            );
            rows.push(i18n::tr("Edition", "SaveAsBack").to_string());
            rows.push(i18n::tr("Edition", "ScrollSettingsCancel").to_string());
        }
        let x = screen_width() * 0.5;
        let y = (screen_height() * 0.5 - 135.0).max(90.0);
        actors.push(
            act!(quad: xy(x,y+130.0): setsize(760.0,330.0): diffuse(0.04,0.06,0.1,0.99): z(8050)),
        );
        actors.push(act!(text: font("miso"): settext(i18n::tr("Edition",if settings{"OutputTitle"}else{"OutputReviewTitle"})): xy(x,y-15.0): zoom(0.65): maxwidth(720.0): z(8051)));
        if !settings {
            let path = if self.output_ui.save_as {
                self.save_as_review_destination()
            } else {
                self.workspace.file().existing_path()
            };
            let format = if self.output_ui.save_as {
                self.save_as_review_policy()
            } else {
                self.output_ui.save.as_ref().map(SaveReview::policy)
            };
            let format = format
                .map(|policy| match policy {
                    OutputPolicy::NativeLossless => self.workspace.file().session.document().format,
                    OutputPolicy::Advanced { format, .. } => *format,
                })
                .map(|format| format.extension().to_uppercase())
                .unwrap_or_default();
            let path = path
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default();
            actors.push(act!(text: font("miso"): settext(format!("{format}: {path}")): xy(x,y+245.0): zoom(0.45): maxwidth(700.0): z(8051)));
        }
        let start = index.saturating_sub(3);
        for (i, label) in rows.iter().enumerate().skip(start).take(7) {
            actors.push(act!(text: font("miso"): settext(label.clone()): align(0.0,0.5): xy(x-350.0,y+20.0+(i-start)as f32*31.0): zoom(0.55): maxwidth(700.0): diffuse(1.0,if i==index{0.8}else{1.0},if i==index{0.3}else{1.0},1.0): z(8051)));
        }
        actors.push(act!(text: font("miso"): settext(format!("{} / {}",index+1,rows.len())): xy(x,y+270.0): zoom(0.5): z(8051)));
    }
}
fn issue_key(kind: IssueKind) -> &'static str {
    match kind {
        IssueKind::UnsupportedChart => "OutputIssueUnsupportedChart",
        IssueKind::InvalidChart => "OutputIssueInvalidChart",
        IssueKind::UnequalTiming => "OutputIssueUnequalTiming",
        IssueKind::UnequalSharedEvent => "OutputIssueUnequalSharedEvent",
        IssueKind::OmittedEvent => "OutputIssueOmittedEvent",
        IssueKind::OmittedChartMetadata => "OutputIssueOmittedChartMetadata",
        IssueKind::OmittedCustomData => "OutputIssueOmittedCustomData",
        IssueKind::AnonymizedField => "OutputIssueAnonymizedField",
        IssueKind::StepF2Players => "OutputIssueStepF2Players",
        IssueKind::StepF2NoteKind => "OutputIssueStepF2NoteKind",
        IssueKind::StepF2MineOwner => "OutputIssueStepF2MineOwner",
        IssueKind::StepF2Layout => "OutputIssueStepF2Layout",
        IssueKind::StepF2Collision => "OutputIssueStepF2Collision",
        IssueKind::StepF2Attributes => "OutputIssueStepF2Attributes",
        IssueKind::UnknownProperty => "OutputIssueUnknownProperty",
        IssueKind::ChartAttacksWithoutSplitTiming => "OutputIssueChartAttacksWithoutSplitTiming",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state(language: &str) -> (tempfile::TempDir, State) {
        i18n::init(deadsync_assets::language::load_for_tests(language));
        deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
            854.0, 480.0,
        ));
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.ssc");
        std::fs::write(&path,b"#VERSION:0.83;#TITLE:Policy;#CREDIT:Private author;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#CREDIT:Private chart;#NOTES:1000;").unwrap();
        let state = State::open(&path, 0).unwrap();
        (root, state)
    }
    fn anonymize(state: &mut State) {
        state.open_output_settings();
        state.output_key(0, KeyCode::Enter);
        state.output_key(6, KeyCode::Enter);
    }
    fn acknowledge(state: &mut State) {
        let count = state.output_issues().len();
        assert!(count > 0);
        assert!(
            state
                .output_issues()
                .iter()
                .all(|i| i.severity == Severity::Warning)
        );
        for index in 0..count {
            state.output_key(index, KeyCode::Enter);
        }
        assert!(state.output_review_authorized());
    }
    #[test]
    fn settings_cancel_and_loss_cancel_publish_nothing_en_fr() {
        for language in ["en", "fr"] {
            let (_root, mut state) = state(language);
            let path = state
                .workspace
                .file()
                .require_existing_path()
                .unwrap()
                .to_owned();
            let before = std::fs::read(&path).unwrap();
            let ids = state.workspace.file().session.chart_ids().to_vec();
            anonymize(&mut state);
            state.output_key(11, KeyCode::Enter);
            assert!(state.panel.is_none());
            assert!(matches!(
                state.workspace.file().output_policy(),
                OutputPolicy::NativeLossless
            ));
            anonymize(&mut state);
            state.output_key(9, KeyCode::Enter);
            assert!(matches!(state.panel, Some(Panel::OutputLoss(_))));
            let count = state.output_issues().len();
            state.output_key(count + 2, KeyCode::Enter);
            assert_eq!(std::fs::read(path).unwrap(), before);
            assert_eq!(state.workspace.file().session.chart_ids(), ids);
            assert!(matches!(
                state.workspace.file().output_policy(),
                OutputPolicy::NativeLossless
            ));
            assert!(state.output_ui.draft.is_none());
            assert!(!state.save_as_pending());
        }
    }
    #[test]
    fn current_save_requires_each_loss_ack_and_keeps_native_private_fields_en_fr() {
        for language in ["en", "fr"] {
            let (_root, mut state) = state(language);
            let path = state
                .workspace
                .file()
                .require_existing_path()
                .unwrap()
                .to_owned();
            let before = std::fs::read(&path).unwrap();
            anonymize(&mut state);
            state.output_key(9, KeyCode::Enter);
            let count = state.output_issues().len();
            assert!(count > 0);
            state.output_key(count, KeyCode::Enter);
            assert_eq!(std::fs::read(&path).unwrap(), before);
            assert_eq!(state.status, i18n::tr("Edition", "OutputUnacknowledged"));
            acknowledge(&mut state);
            state.output_key(count, KeyCode::Enter);
            assert!(state.panel.is_none());
            assert!(matches!(
                state.workspace.file().output_policy(),
                OutputPolicy::Advanced { .. }
            ));
            assert_eq!(
                state
                    .workspace
                    .file()
                    .session
                    .document()
                    .text(None, "CREDIT")
                    .as_deref(),
                Some("Private author")
            );
            assert!(
                !String::from_utf8(std::fs::read(&path).unwrap())
                    .unwrap()
                    .contains("Private author")
            );
            assert!(!state.workspace.file().session.is_dirty());
            state.workspace.execute(Command::DuplicateChart).unwrap();
            state.command(Command::Save);
            assert!(
                matches!(state.panel, Some(Panel::OutputLoss(_))),
                "persisted advanced policy must receive a fresh review"
            );
            assert!(state.output_ui.acknowledged.is_empty());
        }
    }
    #[test]
    fn save_as_accepts_losses_then_requires_explicit_destination_confirmation_en_fr() {
        for language in ["en", "fr"] {
            let (root, mut state) = state(language);
            let source = state
                .workspace
                .file()
                .require_existing_path()
                .unwrap()
                .to_owned();
            let original = std::fs::read(&source).unwrap();
            let ids = state.workspace.file().session.chart_ids().to_vec();
            let destination = root.path().join("export.ssc");
            anonymize(&mut state);
            state.output_key(10, KeyCode::Enter);
            assert!(matches!(state.panel, Some(Panel::SaveAsDestination(0))));
            state.save_as.destination = destination.to_string_lossy().into_owned();
            state.text_buffer = None;
            state.save_as_prepare_review();
            assert!(matches!(state.panel, Some(Panel::OutputLoss(_))));
            let count = state.output_issues().len();
            acknowledge(&mut state);
            state.output_key(count, KeyCode::Enter);
            assert!(matches!(state.panel, Some(Panel::SaveAsReview(1))));
            assert!(!destination.exists());
            state.save_as_key(1, KeyCode::ArrowUp);
            state.save_as_key(0, KeyCode::Enter);
            assert!(destination.exists());let destination=destination.canonicalize().unwrap();
            assert_eq!(
                state.workspace.file().existing_path(),
                Some(destination.as_path())
            );
            assert_eq!(state.workspace.file().session.chart_ids(), ids);
            assert_eq!(std::fs::read(source).unwrap(), original);
            assert!(state.panel.is_none());
        }
    }
    #[test]
    fn edit_then_undo_revokes_settings_and_current_review_en_fr() {
        for language in ["en", "fr"] {
            let (_root, mut state) = state(language);
            let path = state
                .workspace
                .file()
                .require_existing_path()
                .unwrap()
                .to_owned();
            let bytes = std::fs::read(&path).unwrap();
            anonymize(&mut state);
            state.workspace.execute(Command::DuplicateChart).unwrap();
            state.workspace.execute(Command::Undo).unwrap();
            state.output_key(9, KeyCode::Enter);
            assert!(matches!(state.panel, Some(Panel::OutputSettings(_))));
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
            anonymize(&mut state);
            state.output_key(9, KeyCode::Enter);
            let count = state.output_issues().len();
            acknowledge(&mut state);
            state.workspace.execute(Command::DuplicateChart).unwrap();
            state.workspace.execute(Command::Undo).unwrap();
            state.output_key(count, KeyCode::Enter);
            assert!(state.panel.is_none());
            assert_eq!(std::fs::read(path).unwrap(), bytes);
            assert!(matches!(
                state.workspace.file().output_policy(),
                OutputPolicy::NativeLossless
            ));
        }
    }
    #[test]
    fn issue_classes_are_localized_for_both_languages() {
        for language in ["en", "fr"] {
            i18n::init(deadsync_assets::language::load_for_tests(language));
            for kind in [
                IssueKind::UnsupportedChart,
                IssueKind::InvalidChart,
                IssueKind::UnequalTiming,
                IssueKind::UnequalSharedEvent,
                IssueKind::OmittedEvent,
                IssueKind::OmittedChartMetadata,
                IssueKind::OmittedCustomData,
                IssueKind::AnonymizedField,
                IssueKind::StepF2Players,
                IssueKind::StepF2NoteKind,
                IssueKind::StepF2MineOwner,
                IssueKind::StepF2Layout,
                IssueKind::StepF2Collision,
                IssueKind::StepF2Attributes,
                IssueKind::UnknownProperty,
                IssueKind::ChartAttacksWithoutSplitTiming,
            ] {
                let key = issue_key(kind);
                assert_ne!(i18n::tr("Edition", key).as_ref(), key);
            }
        }
    }
    #[test]
    fn imported_global_options_publish_to_preferences_without_persisting_document_guid_en_fr() {
        for language in ["en", "fr"] {
            let (root, mut state) = state(language);
            let path = root.path().join("preferences.json");
            std::fs::write(&path,b"{\"Version\":1,\"OmitCustomSaveData\":true,\"AnonymizeSaveData\":false,\"OtherOwner\":{\"keep\":true}}").unwrap();
            state.audio_preferences_file =
                Some(deadsync_editor::audio_preferences::File::open(&path).unwrap());
            let before = std::fs::read(&path).unwrap();
            state.open_output_settings();
            assert!(
                state
                    .output_ui
                    .draft
                    .as_ref()
                    .unwrap()
                    .options
                    .omit_custom_data
            );
            state.output_key(6, KeyCode::Enter);
            state.output_key(11, KeyCode::Enter);
            assert!(
                !state
                    .audio_preferences_file
                    .as_ref()
                    .unwrap()
                    .advanced_save()
                    .anonymize
            );
            assert_eq!(std::fs::read(&path).unwrap(), before);
            state.open_output_settings();
            state.output_key(6, KeyCode::Enter);
            state.output_key(9, KeyCode::Enter);
            let count = state.output_issues().len();
            acknowledge(&mut state);
            state.output_key(count, KeyCode::Enter);
            let expected = state
                .audio_preferences_file
                .as_ref()
                .unwrap()
                .advanced_save();
            assert!(expected.omit_custom_data && expected.anonymize);
            state
                .audio_preferences_file
                .as_mut()
                .unwrap()
                .save()
                .unwrap();
            let reopened = deadsync_editor::audio_preferences::File::open(&path).unwrap();
            assert_eq!(reopened.advanced_save(), expected);
            let raw: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            assert_eq!(raw["OtherOwner"]["keep"], true);
            assert!(raw.get("timing_chart").is_none());
            assert!(raw.get("output_format").is_none());
            let workspace = Workspace::new_song(root.path(), 0.009).unwrap();
            let mut next = State::from_workspace(workspace).unwrap();
            next.audio_preferences_file = Some(reopened);
            next.open_output_settings();
            assert_eq!(next.output_ui.draft.as_ref().unwrap().options, expected);
            assert!(matches!(
                next.workspace.file().output_policy(),
                OutputPolicy::NativeLossless
            ));
            assert!(next.output_ui.draft.as_ref().unwrap().chart.is_none());
        }
    }
}
