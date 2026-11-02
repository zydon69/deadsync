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
    selected: std::collections::BTreeSet<u64>,
    anchor: Option<u64>,
    pub fields: [String; 5],
}
impl Settings {
    fn new(draft: Draft) -> Self {
        let mut s = Self {
            draft,
            event: 0,
            modifier: 0,
            selected: Default::default(),
            anchor: None,
            fields: Default::default(),
        };
        s.load();
        s
    }
    fn from_memory(draft: Draft, row: Option<u64>) -> Self {
        let mut settings = Self::new(draft);
        settings.event = row.and_then(|row|settings.draft.attacks().events().iter().position(|event|event.row==row))
            .unwrap_or(settings.draft.attacks().events().len());
        settings.load();
        settings
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
        let old = self.draft.start_row(self.event)?;
        self.event = self.draft.set_time(self.event, time)?;
        self.moved_selection(old);
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
        let old = self.draft.start_row(self.event)?;
        self.event = self
            .draft
            .set(self.event, self.modifier, time, self.modifier_value()?)?;
        self.moved_selection(old);
        self.fields[0] = self.draft.attacks().events()[self.event].time.to_string();
        Ok(())
    }
    fn selected_indices(&self) -> Vec<usize> {
        if self.selected.is_empty() {
            return if self.has_event() {
                vec![self.event]
            } else {
                Vec::new()
            };
        }
        let selected: Vec<_> = self
            .draft
            .attacks()
            .events()
            .iter()
            .enumerate()
            .filter_map(|(index, event)| self.selected.contains(&event.row).then_some(index))
            .collect();
        if selected.is_empty() && self.has_event() {
            vec![self.event]
        } else {
            selected
        }
    }
    fn clear_selection(&mut self) {
        self.selected.clear();
        self.anchor = None;
    }
    fn selected_count(&self) -> usize {
        if self.selected.is_empty() {
            usize::from(self.has_event())
        } else {
            self.selected.len()
        }
    }
    fn moved_selection(&mut self, old: u64) {
        let row = self.draft.attacks().events()[self.event].row;
        if self.selected.remove(&old) {
            self.selected.insert(row);
        }
        if self.anchor == Some(old) {
            self.anchor = Some(row);
        }
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
                settings.clear_selection();
                settings.event = event;
                settings.modifier = 0;
                settings.load();
                if let Some(row) = settings.draft.original_row(event) {
                    let _=self.workspace.select_attack(settings.draft.scope(),row);
                }
                self.focus_text();
            }
        }
    }
    pub(super) fn sync_attack_presentation(&mut self) {
        if !matches!(self.panel,Some(Panel::Attacks(_))) {return;}
        if !self.workspace.has_chart() { self.attacks=None; self.focus_text(); return; }
        let session=&self.workspace.file().session;
        let chart=self.workspace.chart();
        if self.attacks.as_ref().is_some_and(|settings|settings.draft.is_current_for(session,chart)) {return;}
        let scope=self.attacks.as_ref().map(|settings|settings.draft.scope())
            .unwrap_or(Scope::Chart);
        self.attacks=self.workspace.attack_inspector(scope).ok().map(|draft| {
            Settings::from_memory(draft,session.selected_attack_row(chart,scope))
        });
        self.focus_text();
    }
    pub(super) fn open_attacks(&mut self, scope: Scope) {
        if self.is_generating() {
            return;
        }
        match self.workspace.attack_inspector(scope) {
            Ok(draft) => {
                self.timing = None;
                let row=self.workspace.file().session.selected_attack_row(self.workspace.chart(),scope);
                self.attacks = Some(Settings::from_memory(draft,row));
                self.panel = Some(Panel::Attacks(0));
                self.focus_text();
                self.status = i18n::tr("Edition", "AttacksHelp");
            }
            Err(_) => self.status = i18n::tr("Edition", "AttacksUnavailable"),
        }
    }
    pub(super) fn attacks_key(&mut self, index: usize, code: KeyCode) {
        if index == 1 && self.control && code == KeyCode::KeyA {
            if let Some(settings) = &mut self.attacks {
                if settings.commit().is_err() {
                    self.status = i18n::tr("Edition", "AttacksInvalid");
                    return;
                }
                settings.selected = settings
                    .draft
                    .attacks()
                    .events()
                    .iter()
                    .map(|e| e.row)
                    .collect();
                settings.anchor = settings.draft.attacks().events().first().map(|e| e.row);
            }
            self.status = i18n::tr("Edition", "AttackSelectionHelp");
            return;
        }
        // Event clipboard is internal to this editor, like the lane-note clipboard.
        // Text fields retain their existing OS clipboard behavior.
        if index == 1
            && self.control
            && matches!(code, KeyCode::KeyC | KeyCode::KeyX | KeyCode::KeyV)
        {
            let Some(mut settings) = self.attacks.take() else {
                return;
            };
            let result = (|| -> Result<&str, Error> {
                if code == KeyCode::KeyV && self.attack_clipboard.is_none() {
                    return Ok("AttackClipboardEmpty");
                }
                settings.commit()?;
                if code == KeyCode::KeyV {
                    let events = settings.draft.paste_events(
                        self.attack_clipboard
                            .as_ref()
                            .expect("Clipboard checked above"),
                        self.workspace.row(),
                    )?;
                    settings.selected = events
                        .iter()
                        .map(|&index| settings.draft.attacks().events()[index].row)
                        .collect();
                    settings.anchor = settings.selected.first().copied();
                    settings.event = events[0];
                    settings.modifier = 0;
                    settings.load();
                    Ok("AttackEventsPasted")
                } else {
                    let indices = settings.selected_indices();
                    let copied = settings.draft.copy_events(&indices)?;
                    if code == KeyCode::KeyX {
                        settings.draft.remove_events(&indices)?;
                        settings.clear_selection();
                        settings.clamp();
                    }
                    self.attack_clipboard = Some(copied);
                    Ok(if code == KeyCode::KeyX {
                        "AttackEventsCut"
                    } else {
                        "AttackEventsCopied"
                    })
                }
            })();
            self.attacks = Some(settings);
            self.focus_text();
            self.status = i18n::tr("Edition", result.unwrap_or("AttacksInvalid"));
            return;
        }
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
                    let old = settings.draft.start_row(settings.event)?;
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
                    settings.moved_selection(old);
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
                let row=self.workspace.file().session.selected_attack_row(self.workspace.chart(),scope);
                settings = Settings::from_memory(self.workspace.attack_inspector(scope)?,row);
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
                        let previous=settings.event;
                        let old = settings.draft.attacks().events().get(previous).map(|event|event.row);
                        settings.event = if previous>=count {
                            if forward {0} else {count-1}
                        } else if self.shift {
                            if forward {
                                (settings.event + 1).min(count - 1)
                            } else {
                                settings.event.saturating_sub(1)
                            }
                        } else {
                            (settings.event + if forward { 1 } else { count - 1 }) % count
                        };
                        if self.shift && old.is_some() {
                            let anchor = *settings.anchor.get_or_insert(old.expect("Existing event"));
                            let end = settings.draft.attacks().events()[settings.event].row;
                            settings.selected = settings
                                .draft
                                .attacks()
                                .events()
                                .iter()
                                .filter(|e| (anchor.min(end)..=anchor.max(end)).contains(&e.row))
                                .map(|e| e.row)
                                .collect();
                        } else {
                            settings.clear_selection();
                        }
                        settings.modifier = 0;
                        settings.load();
                        if let Some(row)=settings.draft.original_row(settings.event) {
                            self.workspace.select_attack(settings.draft.scope(),row)?;
                        }
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
                    settings.clear_selection();
                    settings.modifier = 0;
                    settings.load();
                }
                11 if code == KeyCode::Enter => {
                    settings.draft.remove_events(&settings.selected_indices())?;
                    settings.clear_selection();
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
                if !settings.has_event() {
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
        let value = if index == 1 && settings.selected_count() > 1 {
            i18n::tr_fmt(
                "Edition",
                "AttackSelectedCount",
                &[
                    ("value", &value),
                    ("count", &settings.selected_count().to_string()),
                ],
            )
            .to_string()
        } else {
            value
        };
        i18n::tr_fmt("Edition", key, &[("value", &value)])
    }
}

#[cfg(test)]
mod selected_attack_tests {
    use super::*;
    fn fixture()->(tempfile::TempDir,State) {
        crate::i18n::init_for_tests();
        let directory=tempfile::tempdir().unwrap();let path=directory.path().join("attacks.ssc");
        std::fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#ATTACKS:TIME=0:LEN=1:MODS=100% Dizzy:TIME=1:LEN=1:MODS=100% Reverse;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000,0100;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0010,0001;").unwrap();
        (directory,State::open(&path,0).unwrap())
    }
    fn selected_row(state:&State)->Option<u64> {
        let settings=state.attacks.as_ref()?;
        settings.draft.attacks().events().get(settings.event).map(|event|event.row)
    }
    #[test]
    fn actual_attack_panel_follows_open_chart_memory_and_close_discards_it() {
        let (_directory,mut state)=fixture();let scope=Scope::Chart;
        let before=state.workspace.file().session.serialize().unwrap();
        let revision=state.workspace.file().session.revision();
        state.open_attacks(scope);assert_eq!(selected_row(&state),None);
        state.select_attack_source(1);let a=selected_row(&state).unwrap();
        state.chart_views_apply(super::super::chart_views::ViewAction::Pin);
        let time=state.chart_focus_time().unwrap();
        assert!(!state.focus_chart_at_time(999,time));assert_eq!(selected_row(&state),Some(a));
        assert!(state.focus_chart_at_time(0,time));assert_eq!(selected_row(&state),Some(a));
        assert!(state.focus_chart_at_time(1,time));assert_eq!(selected_row(&state),None);
        state.select_attack_source(0);let b=selected_row(&state).unwrap();assert_ne!(a,b);
        state.chart_views_apply(super::super::chart_views::ViewAction::Pin);
        assert!(state.focus_chart_at_time(0,time));assert_eq!(selected_row(&state),Some(a));
        state.chart_views_apply(super::super::chart_views::ViewAction::Move(true));
        assert_eq!(selected_row(&state),Some(a));
        state.open_attacks(scope);assert_eq!(selected_row(&state),Some(a));
        assert_eq!(state.workspace.file().session.serialize().unwrap(),before);
        assert_eq!(state.workspace.file().session.revision(),revision);
        state.chart_views_apply(super::super::chart_views::ViewAction::Close);
        assert_eq!(selected_row(&state),Some(b));
        assert!(state.focus_chart_at_time(0,time));assert_eq!(selected_row(&state),None);
    }
    #[test]
    fn actual_panel_apply_tracks_moves_and_delete_undo_never_selects_a_readded_attack() {
        let (_directory,mut state)=fixture();let scope=Scope::Chart;
        state.open_attacks(scope);state.select_attack_source(1);
        let original=selected_row(&state).unwrap();
        state.workspace.execute(deadsync_editor::workspace::Command::Seek(240)).unwrap();
        state.attacks_key(1,KeyCode::F2);
        assert_eq!(selected_row(&state),Some(240));
        state.attacks_key(12,KeyCode::Enter);
        state.open_attacks(scope);assert_eq!(selected_row(&state),Some(240));
        state.command(deadsync_editor::workspace::Command::Undo);
        state.open_attacks(scope);assert_eq!(selected_row(&state),Some(original));
        state.command(deadsync_editor::workspace::Command::Redo);
        state.open_attacks(scope);assert_eq!(selected_row(&state),Some(240));
        state.attacks_key(11,KeyCode::Enter);state.attacks_key(12,KeyCode::Enter);
        state.open_attacks(scope);assert_eq!(selected_row(&state),None);
        state.command(deadsync_editor::workspace::Command::Undo);
        state.open_attacks(scope);assert_eq!(selected_row(&state),None);
    }
}

#[cfg(test)]
mod sm_local_attack_tests {
 use super::*;
 fn selected_row(state:&State)->Option<u64>{let s=state.attacks.as_ref()?;s.draft.attacks().events().get(s.event).map(|e|e.row)}
 #[test]
 fn actual_sm_widget_inspector_apply_delete_and_refocus_preserve_peer(){
  crate::i18n::init_for_tests();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("local.sm");
  std::fs::write(&path,b"#BPMS:0=120;#ATTACKS:TIME=1:LEN=1:MODS=100% Drunk;#NOTES:dance-single::Hard:9::1000,0100,0010;#NOTES:dance-single::Easy:2::0001,0010,0100;").unwrap();
  let mut state=State::open(&path,0).unwrap();let original=state.workspace.file().session.serialize().unwrap();
  state.workspace.execute(deadsync_editor::workspace::Command::Seek(96)).unwrap();state.open_attack_at_cursor();assert_eq!(selected_row(&state),Some(96));assert_eq!(state.attacks.as_ref().unwrap().draft.scope(),Scope::Chart);
  state.workspace.execute(deadsync_editor::workspace::Command::Seek(240)).unwrap();state.attacks_key(1,KeyCode::F2);state.attacks_key(12,KeyCode::Enter);state.open_attacks(Scope::Chart);assert_eq!(selected_row(&state),Some(240));
  let time=state.chart_focus_time().unwrap();assert!(state.focus_chart_at_time(1,time));state.open_attacks(Scope::Chart);state.select_attack_source(0);assert_eq!(selected_row(&state),Some(96));assert_eq!(state.attacks.as_ref().unwrap().draft.attacks().events().len(),1);
  assert!(state.focus_chart_at_time(0,time));state.open_attacks(Scope::Chart);state.select_attack_source(0);assert_eq!(selected_row(&state),Some(240));state.attacks_key(11,KeyCode::Enter);state.attacks_key(12,KeyCode::Enter);state.open_attacks(Scope::Chart);assert!(state.attacks.as_ref().unwrap().draft.attacks().events().is_empty());
  state.command(deadsync_editor::workspace::Command::Undo);state.command(deadsync_editor::workspace::Command::Undo);assert_eq!(state.workspace.file().session.serialize().unwrap(),original);
 }
}
