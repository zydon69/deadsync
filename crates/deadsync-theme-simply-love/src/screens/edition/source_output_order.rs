use super::{Panel, State};
use crate::i18n;
use deadsync_editor::{
    Error, chart_order::Orderer, history::file_intent::IntentGuard, output_policy::OutputPolicy,
    source_output_order::CultureRevision,
};
use std::{
    mem::{Discriminant, discriminant},
    sync::{Arc, atomic::AtomicBool},
};
struct NativeCulture;
impl CultureRevision for NativeCulture {
    fn revision(&self) -> u64 {
        i18n::revision()
    }
}
#[derive(Clone, Copy)]
pub(super) enum Purpose {
    Save(bool),
    SaveAs,
}
pub(super) struct Request {
    epoch: Arc<()>,
    pack: Option<super::pack_properties::CouplingGuard>,
    guard: IntentGuard,
    locale: u64,
    panel: Option<Discriminant<Panel>>,
    destination: String,
    policy: OutputPolicy,
    purpose: Purpose,
}
impl Request {
    fn is_current(&self, state: &State) -> bool {
        Arc::ptr_eq(&self.epoch, &state.source_output_request_epoch)
            && self.guard.is_current(&state.workspace.file().session)
            && state.pack_coupling_current(&self.pack)
            && self.locale == i18n::revision()
            && self.panel == state.panel.as_ref().map(discriminant)
            && self.destination == state.save_as.destination
            && state.label_preedit.is_empty()
            && state
                .source_output_policy(self.purpose)
                .is_ok_and(|policy| policy == self.policy)
    }
}
impl State {
    pub(super) fn invalidate_source_output_request(&mut self) {
        self.source_output_request = None;
        self.source_output_request_epoch = Arc::new(());
    }
    fn source_output_policy(&self, purpose: Purpose) -> Result<OutputPolicy, Error> {
        match purpose {
            Purpose::Save(true) => self.output_policy_draft(),
            Purpose::Save(false) => Ok(self.workspace.file().output_policy().clone()),
            Purpose::SaveAs => self.output_candidate_policy(),
        }
    }
    pub(super) fn require_source_output_order(&mut self, purpose: Purpose) -> bool {
        let policy = match self.source_output_policy(purpose) {
            Ok(policy) => policy,
            Err(error) => {
                self.file_save_cancelled();
                self.save_as_error(error);
                return false;
            }
        };
        if matches!(policy, OutputPolicy::NativeLossless) {
            return true;
        }
        if self
            .source_output_order
            .as_ref()
            .is_some_and(|order| order.is_current(&self.workspace.file().session))
        {
            return true;
        }
        match self.workspace.capture_unambiguous_source_output_order(
            Arc::new(NativeCulture),
            &AtomicBool::new(false),
        ) {
            Ok(Some(order)) => {
                self.source_output_order = Some(order);
                return true;
            }
            Ok(None) => {}
            Err(error) => {
                self.file_save_cancelled();
                self.save_as_error(error);
                return false;
            }
        }
        self.invalidate_source_output_request();
        self.source_output_request = Some(Request {
            epoch: Arc::clone(&self.source_output_request_epoch),
            pack: self.capture_pack_coupling(),
            guard: IntentGuard::capture(&self.workspace.file().session),
            locale: i18n::revision(),
            panel: self.panel.as_ref().map(discriminant),
            destination: self.save_as.destination.clone(),
            policy,
            purpose,
        });
        self.status = i18n::tr("Edition", "OutputOrderLoading");
        false
    }
    pub(super) fn process_source_output_order(&mut self, orderer: &mut impl Orderer) -> bool {
        let Some(request) = self.source_output_request.take() else {
            return false;
        };
        if !request.is_current(self) {
            self.file_save_cancelled();
            self.status = i18n::tr("Edition", "OutputOrderStale");
            return false;
        }
        let order = self.workspace.capture_source_output_order(
            orderer,
            Arc::new(NativeCulture),
            &AtomicBool::new(false),
        );
        if !request.is_current(self) {
            self.file_save_cancelled();
            self.status = i18n::tr("Edition", "OutputOrderStale");
            return false;
        }
        match order {
            Ok(order) => {
                self.source_output_order = Some(order);
                match request.purpose {
                    Purpose::Save(use_draft) => self.output_save_current(use_draft),
                    Purpose::SaveAs => self.save_as_prepare_review(),
                }
            }
            Err(error) => {
                self.file_save_cancelled();
                log::warn!("Source-compatible chart ordering: {error}");
                self.status = format!(
                    "{}: {}",
                    i18n::tr("Edition", "OutputOrderFailed"),
                    i18n::editor_error(&error)
                )
                .into();
            }
        }
        true
    }
    pub(super) fn prepare_ordered_current_output(
        &mut self,
        policy: OutputPolicy,
    ) -> Result<deadsync_editor::storage::SaveReview, Error> {
        if matches!(policy, OutputPolicy::NativeLossless) {
            self.workspace.prepare_save_with_policy(policy)
        } else {
            let order = self.source_output_order.as_ref().ok_or_else(|| {
                Error::InvalidEdit(
                    "Source-compatible output requires a fresh cultural order".into(),
                )
            })?;
            self.workspace
                .prepare_save_with_policy_ordered(policy, order)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use deadlib_platform::input::KeyCode;
    use deadsync_editor::{
        document::{Document, Format},
        workspace::{Command, Workspace},
    };
    struct ReferenceOrder(&'static str);
    impl Orderer for ReferenceOrder {
        fn order(
            &mut self,
            snapshot: &deadsync_editor::chart_order::Snapshot,
        ) -> Result<Vec<usize>, Error> {
            let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../deadsync-editor/tests/fixtures/grooveauthor/chart-order.reference.json"
            )))
            .unwrap();
            let strings = fixture["strings"].as_array().unwrap();
            let culture = fixture["cultures"]
                .as_array()
                .unwrap()
                .iter()
                .find(|entry| entry["culture"] == self.0)
                .unwrap();
            snapshot.indices_with(|a, b| {
                let a = strings.iter().position(|s| s == a).unwrap();
                let b = strings.iter().position(|s| s == b).unwrap();
                culture["text_comparisons"][a][b].as_i64().unwrap().cmp(&0)
            })
        }
    }
    fn setup(language: &str) -> (tempfile::TempDir, State) {
        i18n::init(deadsync_assets::language::load_for_tests(language));
        deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
            854.0, 480.0,
        ));
        let root = tempfile::tempdir().unwrap();
        let mut source = "#VERSION:0.83;#TITLE:Sort;#BPMS:0=120;".to_owned();
        for name in ["zebra", "Ångström", "Éclair"] {
            source += &format!(
                "#NOTEDATA:;#STEPSTYPE:dance-single;#CHARTNAME:{name};#DESCRIPTION:{name};#DIFFICULTY:Hard;#METER:9;#NOTES:1000;"
            );
        }
        let path = root.path().join("song.ssc");
        std::fs::write(&path, source).unwrap();
        let state = State::from_workspace(Workspace::open(&path, 0).unwrap()).unwrap();
        (root, state)
    }
    fn advanced(state: &mut State) {
        state.open_output_settings();
        state.output_key(0, KeyCode::Enter);
    }
    fn acknowledge(state: &mut State) -> usize {
        let count = match state.panel {
            Some(Panel::OutputLoss(back)) => back - 1,
            _ => panic!("expected loss review"),
        };
        for row in 0..count {
            state.output_key(row, KeyCode::Enter);
        }
        count
    }
    #[test]
    fn source_output_keyboard_waits_for_cultural_host_then_writes_sorted_without_reordering_live_charts_en_fr()
     {
        for (language, locale) in [("en", "en-US"), ("fr", "fr-FR")] {
            let (_root, mut state) = setup(language);
            let path = state
                .workspace
                .file()
                .require_existing_path()
                .unwrap()
                .to_owned();
            let before = std::fs::read(&path).unwrap();
            let ids = state.workspace.file().session.chart_ids().to_vec();
            let focus = state.focus_position.as_ref().map(|p| p.row().to_bits());
            advanced(&mut state);
            state.output_key(9, KeyCode::Enter);
            assert!(state.source_output_request.is_some());
            assert_eq!(std::fs::read(&path).unwrap(), before);
            assert!(state.process_chart_order(&mut ReferenceOrder(locale)));
            let count = acknowledge(&mut state);
            state.output_key(count, KeyCode::Enter);
            let output = Document::parse(std::fs::read(path).unwrap(), Format::Ssc).unwrap();
            assert_eq!(
                output
                    .charts()
                    .iter()
                    .map(|c| c.description.as_str())
                    .collect::<Vec<_>>(),
                ["Ångström", "Éclair", "zebra"]
            );
            assert_eq!(state.workspace.file().session.chart_ids(), ids);
            assert_eq!(state.workspace.chart(), 0);
            assert_eq!(
                state.focus_position.as_ref().map(|p| p.row().to_bits()),
                focus
            );
            assert_eq!(
                state.workspace.file().session.document().charts()[0].description,
                "zebra"
            );
        }
    }
    #[test]
    fn cancel_and_stale_edit_undo_or_language_never_resume_a_queued_save() {
        for language in ["en", "fr"] {
            for cause in [0, 1, 2] {
                let (_root, mut state) = setup(language);
                let path = state
                    .workspace
                    .file()
                    .require_existing_path()
                    .unwrap()
                    .to_owned();
                let before = std::fs::read(&path).unwrap();
                advanced(&mut state);
                state.output_key(9, KeyCode::Enter);
                match cause {
                    0 => state.output_key(0, KeyCode::Escape),
                    1 => {
                        state.workspace.execute(Command::DuplicateChart).unwrap();
                        state.workspace.execute(Command::Undo).unwrap();
                    }
                    _ => i18n::init(deadsync_assets::language::load_for_tests(
                        if language == "en" { "fr" } else { "en" },
                    )),
                };
                assert!(!state.process_chart_order(&mut ReferenceOrder("en-US")));
                assert_eq!(std::fs::read(&path).unwrap(), before);
                assert!(state.source_output_request.is_none());
            }
        }
    }
    #[test]
    fn save_as_locale_change_without_frame_revokes_confirmation_before_publication_en_fr() {
        for (language, locale) in [("en", "en-US"), ("fr", "fr-FR")] {
            let (root, mut state) = setup(language);
            let target = root.path().join("sorted.ssc");
            advanced(&mut state);
            state.output_key(10, KeyCode::Enter);
            state.save_as.destination = target.to_string_lossy().into_owned();
            state.text_buffer = None;
            state.save_as_prepare_review();
            assert!(!target.exists());
            assert!(state.source_output_request.is_some());
            state.process_chart_order(&mut ReferenceOrder(locale));
            // This lossless SSC fixture may go directly to the destination
            // review. If the policy reports losses, acknowledge them explicitly.
            if matches!(state.panel, Some(Panel::OutputLoss(_))) {
                let count = acknowledge(&mut state);
                state.output_key(count, KeyCode::Enter);
            }
            assert!(
                matches!(state.panel, Some(Panel::SaveAsReview(1))),
                "expected destination review: {}",
                state.status
            );
            i18n::init(deadsync_assets::language::load_for_tests(
                if language == "en" { "fr" } else { "en" },
            ));
            state.save_as_key(1, KeyCode::ArrowUp);
            state.save_as_key(0, KeyCode::Enter);
            assert!(!target.exists());
            assert_eq!(state.status, i18n::tr("Edition", "OutputOrderStale"));
        }
    }
    #[test]
    fn cancelling_and_reopening_the_same_panel_never_revives_the_old_save_intent() {
        let (_root, mut state) = setup("en");
        advanced(&mut state);
        state.output_key(9, KeyCode::Enter);
        let old = state.source_output_request.take().unwrap();
        state.output_key(0, KeyCode::Escape);
        advanced(&mut state);
        state.output_key(9, KeyCode::Enter);
        assert!(!old.is_current(&state));
        assert!(
            state
                .source_output_request
                .as_ref()
                .unwrap()
                .is_current(&state)
        );
    }
}
