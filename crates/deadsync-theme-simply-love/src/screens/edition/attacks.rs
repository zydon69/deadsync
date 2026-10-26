//! Optional attack inspector; draft changes enter song history only on Apply.
use super::{Panel, State};
use crate::i18n;
use deadlib_platform::input::KeyCode;
use deadsync_editor::{
    Error,
    attacks::{Modifier, inspector::Draft},
    timing_edit::Scope,
};
pub(super) const FIELD_KEYS: [&str; 5] = [
    "AttackTime",
    "AttackName",
    "AttackLevel",
    "AttackSpeed",
    "AttackLength",
];
pub(super) struct Settings {
    draft: Draft,
    event: usize,
    modifier: usize,
    pub fields: [String; 5],
}
impl Settings {
    fn new(draft: Draft) -> Self {
        let mut s = Self {
            draft,
            event: 0,
            modifier: 0,
            fields: Default::default(),
        };
        s.load();
        s
    }
    fn load(&mut self) {
        let Some(event) = self.draft.attacks().events().get(self.event) else {
            self.fields = Default::default();
            return;
        };
        let Some(modifier) = event.modifiers.get(self.modifier) else {
            self.fields = Default::default();
            self.fields[0] = event.time.to_string();
            return;
        };
        self.fields = [
            event.time.to_string(),
            modifier.name.clone(),
            modifier.level.to_string(),
            modifier.speed.to_string(),
            modifier.length.to_string(),
        ];
    }
    pub fn editable(&self) -> bool {
        self.draft
            .attacks()
            .events()
            .get(self.event)
            .is_some_and(|e| self.modifier < e.modifiers.len())
    }
    pub fn has_event(&self) -> bool {
        self.event < self.draft.attacks().events().len()
    }
    pub fn can_edit_field(&self, index: usize) -> bool {
        if index >= self.fields.len() {
            return false;
        }
        if index == 0 {
            self.has_event()
        } else {
            self.editable()
        }
    }
    fn commit_time(&mut self) -> Result<(), Error> {
        if !self.has_event() {
            return Ok(());
        }
        let time = self.fields[0]
            .parse::<f64>()
            .map_err(|_| Error::InvalidEdit("Invalid attack number".into()))?;
        self.event = self.draft.set_time(self.event, time)?;
        self.fields[0] = self.draft.attacks().events()[self.event].time.to_string();
        Ok(())
    }
    fn modifier_value(&self) -> Result<Modifier, Error> {
        let number = |index: usize| {
            self.fields[index]
                .parse::<f64>()
                .map_err(|_| Error::InvalidEdit("Invalid attack number".into()))
        };
        Ok(Modifier {
            name: self.fields[1].clone(),
            level: number(2)?,
            speed: number(3)?,
            length: number(4)?,
        })
    }
    fn commit(&mut self) -> Result<(), Error> {
        if !self.has_event() {
            return Ok(());
        }
        if !self.editable() {
            return self.commit_time();
        }
        let time = self.fields[0]
            .parse::<f64>()
            .map_err(|_| Error::InvalidEdit("Invalid attack number".into()))?;
        self.event = self
            .draft
            .set(self.event, self.modifier, time, self.modifier_value()?)?;
        self.fields[0] = self.draft.attacks().events()[self.event].time.to_string();
        Ok(())
    }
    fn clamp(&mut self) {
        self.event = self
            .event
            .min(self.draft.attacks().events().len().saturating_sub(1));
        let count = self
            .draft
            .attacks()
            .events()
            .get(self.event)
            .map_or(0, |e| e.modifiers.len());
        self.modifier = self.modifier.min(count.saturating_sub(1));
        self.load();
    }
}
impl State {
    pub(super) fn select_attack_source(&mut self, index: usize) {
        if let Some(settings) = &mut self.attacks {
            if let Some(event) = settings.draft.event_for_source(index) {
                settings.event = event;
                settings.modifier = 0;
                settings.load();
                self.focus_text();
            }
        }
    }
    pub(super) fn open_attacks(&mut self, scope: Scope) {
        if self.is_generating() {
            return;
        }
        match self.workspace.attack_inspector(scope) {
            Ok(draft) => {
                self.timing = None;
                self.attacks = Some(Settings::new(draft));
                self.panel = Some(Panel::Attacks(0));
                self.focus_text();
                self.status = i18n::tr("Edition", "AttacksHelp");
            }
            Err(_) => self.status = i18n::tr("Edition", "AttacksUnavailable"),
        }
    }
    pub(super) fn attacks_key(&mut self, index: usize, code: KeyCode) {
        if matches!(code, KeyCode::F2 | KeyCode::F3 | KeyCode::F4 | KeyCode::F5) {
            if !self.sync_text() {
                return;
            }
            let Some(mut settings) = self.attacks.take() else {
                return;
            };
            let result = (|| -> Result<(), Error> {
                if !settings.has_event() {
                    return Err(Error::InvalidEdit("No attack selected".into()));
                }
                if matches!(code, KeyCode::F3 | KeyCode::F4) && !settings.editable() {
                    return Err(Error::InvalidEdit("No modifier selected".into()));
                }
                if code != KeyCode::F2 {
                    settings.commit()?;
                }
                if code == KeyCode::F2 {
                    settings.event = if settings.editable() {
                        settings.draft.set_at_row(
                            settings.event,
                            settings.modifier,
                            self.workspace.row(),
                            settings.modifier_value()?,
                        )?
                    } else {
                        settings
                            .draft
                            .move_to_row(settings.event, self.workspace.row())?
                    };
                    settings.load();
                } else if code == KeyCode::F3 {
                    settings.fields[4] = settings
                        .draft
                        .length_to_cursor(settings.event, self.workspace.row())?
                        .to_string();
                    settings.commit()?;
                } else {
                    let row = if code == KeyCode::F4 {
                        settings.draft.end_row(settings.event, settings.modifier)?
                    } else {
                        settings.draft.start_row(settings.event)?
                    };
                    self.stop_audio();
                    self.command(deadsync_editor::workspace::Command::Seek(row));
                }
                Ok(())
            })();
            self.attacks = Some(settings);
            self.focus_text();
            self.status = i18n::tr(
                "Edition",
                if result.is_ok() {
                    "AttacksTimeHelp"
                } else {
                    "AttacksInvalid"
                },
            );
            return;
        }
        if code == KeyCode::Escape || (index == 13 && code == KeyCode::Enter) {
            self.attacks = None;
            self.text_buffer = None;
            self.label_preedit.clear();
            self.text_edit_serial = self.text_edit_serial.wrapping_add(1);
            self.panel = None;
            self.status = i18n::tr("Edition", "AttacksCancelled");
            return;
        }
        if matches!(
            code,
            KeyCode::ArrowUp
                | KeyCode::ArrowDown
                | KeyCode::Tab
                | KeyCode::PageUp
                | KeyCode::PageDown
        ) {
            if !self.sync_text() {
                return;
            }
            let backward = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            let next = match code {
                KeyCode::PageUp => index.saturating_sub(8) / 8 * 8,
                KeyCode::PageDown => (index / 8 * 8 + 8).min(13),
                _ => (index + if backward { 13 } else { 1 }) % 14,
            };
            self.panel = Some(Panel::Attacks(next));
            self.focus_text();
            return;
        }
        if (3..8).contains(&index) {
            self.text_field_key(index, code);
            return;
        }
        if !matches!(
            code,
            KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Enter
        ) {
            return;
        }
        let Some(mut settings) = self.attacks.take() else {
            return;
        };
        let forward = code != KeyCode::ArrowLeft;
        let result = (|| -> Result<(), Error> {
            if index == 0 {
                settings.commit()?;
                if settings.draft.has_changes() {
                    return Err(Error::InvalidEdit(
                        "Apply or cancel pending attacks before switching scope".into(),
                    ));
                }
                let scope = if settings.draft.scope() == Scope::Song {
                    Scope::Chart
                } else {
                    Scope::Song
                };
                settings = Settings::new(self.workspace.attack_inspector(scope)?);
                return Ok(());
            }
            // Deleting a modifier discards its fields, but keeps pending event time.
            // Deleting the whole event discards all of that event's pending fields.
            if index == 9 {
                if !settings.editable() {
                    return Err(Error::InvalidEdit("No modifier selected".into()));
                }
                settings.commit_time()?;
            } else if index != 11 {
                settings.commit()?;
            }
            match index {
                1 => {
                    let count = settings.draft.attacks().events().len();
                    if count > 0 {
                        settings.event =
                            (settings.event + if forward { 1 } else { count - 1 }) % count;
                        settings.modifier = 0;
                        settings.load();
                    }
                }
                2 => {
                    let count = settings
                        .draft
                        .attacks()
                        .events()
                        .get(settings.event)
                        .map_or(0, |e| e.modifiers.len());
                    if count > 0 {
                        settings.modifier =
                            (settings.modifier + if forward { 1 } else { count - 1 }) % count;
                        settings.load();
                    }
                }
                8 if code == KeyCode::Enter => {
                    if let Some(event) = settings.draft.attacks().events().get(settings.event) {
                        let length = match event.modifiers.first() {
                            Some(modifier) => modifier.length,
                            None => settings.draft.default_length(event.row)?,
                        };
                        settings.modifier = settings.draft.add_modifier(
                            settings.event,
                            Modifier {
                                length,
                                ..Modifier::default()
                            },
                        )?;
                        settings.load();
                    }
                }
                9 if code == KeyCode::Enter => {
                    settings
                        .draft
                        .remove_modifier(settings.event, settings.modifier)?;
                    settings.clamp();
                }
                10 if code == KeyCode::Enter => {
                    let row = self.workspace.row();
                    let modifier = Modifier {
                        length: settings.draft.default_length(row)?,
                        ..Modifier::default()
                    };
                    settings.event = settings.draft.add(row, modifier)?;
                    settings.modifier = 0;
                    settings.load();
                }
                11 if code == KeyCode::Enter => {
                    settings.draft.remove(settings.event)?;
                    settings.modifier = 0;
                    settings.clamp();
                }
                12 if code == KeyCode::Enter => {
                    self.apply_dialog_operation(
                        |workspace| {
                            workspace
                                .apply_attack_inspector(&settings.draft)
                                .map(|_| ())
                        },
                        "AttacksApplied",
                        "AttacksNoChange",
                        "AttacksFailed",
                    );
                }
                _ => {}
            }
            Ok(())
        })();
        if result.is_err() {
            self.status = i18n::tr(
                "Edition",
                if index == 0 {
                    "AttacksScopeFailed"
                } else {
                    "AttacksInvalid"
                },
            );
        }
        if self.panel.is_some() {
            self.attacks = Some(settings);
        } else {
            self.text_buffer = None;
            self.label_preedit.clear();
        }
    }
    pub(super) fn attack_label(&self, index: usize, key: &str) -> std::sync::Arc<str> {
        let Some(settings) = &self.attacks else {
            return i18n::tr("Edition", key);
        };
        if (3..8).contains(&index) {
            return if settings.can_edit_field(index - 3) {
                self.text_field_label(index)
            } else {
                i18n::tr("Edition", "AttacksEmpty")
            };
        }
        let value = match index {
            0 => i18n::tr(
                "Edition",
                match (settings.draft.scope(), settings.draft.inherited()) {
                    (Scope::Song, _) => "AttackScopeSong",
                    (Scope::Chart, true) => "AttackScopeInherited",
                    _ => "AttackScopeChart",
                },
            )
            .to_string(),
            1 => format!(
                "{}/{}",
                if settings.draft.attacks().events().is_empty() {
                    0
                } else {
                    settings.event + 1
                },
                settings.draft.attacks().events().len()
            ),
            2 => {
                let count = settings
                    .draft
                    .attacks()
                    .events()
                    .get(settings.event)
                    .map_or(0, |e| e.modifiers.len());
                format!(
                    "{}/{}",
                    if count == 0 { 0 } else { settings.modifier + 1 },
                    count
                )
            }
            _ => return i18n::tr("Edition", key),
        };
        i18n::tr_fmt("Edition", key, &[("value", &value)])
    }
}
