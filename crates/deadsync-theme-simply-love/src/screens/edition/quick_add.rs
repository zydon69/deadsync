//! Optional compact source quick-add panel; all edits use the shared typed drafts.
use super::{Panel, State};
use crate::i18n;
use deadlib_platform::input::KeyCode;
use deadsync_editor::quick_add::{Kind, Plan, Refusal};
const KINDS: [Kind; 13] = [
    Kind::Tempo,
    Kind::Speed,
    Kind::Scroll,
    Kind::Stop,
    Kind::Delay,
    Kind::Warp,
    Kind::Fake,
    Kind::Ticks,
    Kind::Combo,
    Kind::Signature,
    Kind::Label,
    Kind::Attack,
    Kind::Pattern,
];
const KEYS: [&str; 13] = [
    "QuickAddTempo",
    "QuickAddSpeed",
    "QuickAddScroll",
    "QuickAddStop",
    "QuickAddDelay",
    "QuickAddWarp",
    "QuickAddFake",
    "QuickAddTicks",
    "QuickAddCombo",
    "QuickAddSignature",
    "QuickAddLabel",
    "QuickAddAttack",
    "QuickAddPattern",
];
#[derive(Default)]
pub(super) struct Settings {
    kind: usize,
    plan: Option<Plan>,
}
impl State {
    pub(super) fn command_quick_add_review(&mut self, kind: Kind) {
        self.open_quick_add();
        if !matches!(self.panel, Some(Panel::QuickAdd(_))) {
            return;
        }
        self.quick_add.kind = KINDS
            .iter()
            .position(|candidate| *candidate == kind)
            .expect("all quick-add kinds listed");
        self.prepare_quick_add_review();
        self.panel = Some(Panel::QuickAdd(2));
    }

    pub(super) fn open_quick_add(&mut self) {
        if self
            .timing
            .as_ref()
            .is_some_and(|settings| settings.has_changes())
        {
            self.status = i18n::tr("Edition", "TimingConversionPending");
            return;
        }
        self.quick_add = Default::default();
        self.panel = Some(Panel::QuickAdd(0));
        self.status = i18n::tr("Edition", "QuickAddHelp");
    }
    pub(super) fn quick_add_key(&mut self, index: usize, code: KeyCode) {
        match code {
            KeyCode::Escape => self.close_quick_add(),
            KeyCode::ArrowUp => self.panel = Some(Panel::QuickAdd((index + 4) % 5)),
            KeyCode::ArrowDown | KeyCode::Tab => {
                self.panel = Some(Panel::QuickAdd((index + 1) % 5))
            }
            KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Enter if index == 0 => {
                self.quick_add.kind =
                    (self.quick_add.kind + if code == KeyCode::ArrowLeft { 12 } else { 1 }) % 13;
                self.quick_add.plan = None;
            }
            KeyCode::Enter if index == 2 => {
                self.prepare_quick_add_review();
            }
            KeyCode::Enter if index == 3 => {
                let Some(plan) = &self.quick_add.plan else {
                    self.status = i18n::tr("Edition", "NoteOperationPrepareFirst");
                    return;
                };
                match self.workspace.apply_quick_add(plan) {
                    Ok(_) => {
                        self.quick_add = Default::default();
                        self.timing = None;
                        self.panel = None;
                        self.status = i18n::tr("Edition", "QuickAddApplied");
                        self.refresh();
                    }
                    Err(error) => self.status = i18n::tr("Edition", refusal(error.reason)),
                }
            }
            KeyCode::Enter if index == 4 => self.close_quick_add(),
            _ => {}
        }
    }
    fn prepare_quick_add_review(&mut self) {
        match self.workspace.prepare_quick_add(KINDS[self.quick_add.kind]) {
            Ok(plan) => {
                self.quick_add.plan = Some(plan);
                self.status = i18n::tr("Edition", "QuickAddPrepared");
            }
            Err(error) => {
                self.quick_add.plan = None;
                self.status = i18n::tr("Edition", refusal(error.reason));
            }
        }
    }
    fn close_quick_add(&mut self) {
        self.quick_add = Default::default();
        self.panel = Some(Panel::Timing(0));
    }
    pub(super) fn quick_add_label(&self, index: usize, key: &str) -> std::sync::Arc<str> {
        let text = match index {
            0 => i18n::tr("Edition", key)
                .replace("{value}", &i18n::tr("Edition", KEYS[self.quick_add.kind])),
            1 => i18n::tr("Edition", key).replace("{row}", &self.workspace.row().to_string()),
            _ => i18n::tr("Edition", key).to_string(),
        };
        text.into()
    }
}
fn refusal(reason: Refusal) -> &'static str {
    match reason {
        Refusal::Occupied => "QuickAddOccupied",
        Refusal::InvalidPosition => "QuickAddInvalidPosition",
        Refusal::UnsupportedLayout => "QuickAddUnsupportedLayout",
        Refusal::Stale => "QuickAddStale",
        Refusal::Invalid => "QuickAddRefused",
    }
}
