//! First executable configurable subset. Input stays semantic, never synthesized.
use super::State;
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadsync_editor::{
    key_binding_preferences::{Action, Preferences},
    key_commands::{Binding, Effect, Key, Registry},
    workspace::Command,
};
use std::{collections::BTreeSet, sync::Arc, time::Instant};

pub(super) struct Runtime {
    pub(super) preferences: Arc<Preferences>,
    registry: Registry,
    source_blockers: Vec<Binding>,
    effects: deadsync_editor::key_binding_preferences::physical::Effects,
    down: BTreeSet<KeyCode>,
    quarantine: BTreeSet<KeyCode>,
    alt: u8,
    origin: Instant,
}
impl Default for Runtime {
    fn default() -> Self {
        let preferences = Arc::new(Preferences::default());
        Self {
            registry: native_registry(&preferences),
            source_blockers: deadsync_editor::key_binding_preferences::source_profile::blockers(
                &preferences,
            ),
            effects: Default::default(),
            preferences,
            down: BTreeSet::new(),
            quarantine: BTreeSet::new(),
            alt: 0,
            origin: Instant::now(),
        }
    }
}
fn legacy_id(chord: &[String]) -> String {
    format!("__native_reserved_{}", chord.join("+"))
}
fn native_registry(preferences: &Preferences) -> Registry {
    let mut bindings = preferences.bindings();
    let mut legacy = deadsync_editor::key_binding_preferences::reservations::blocking_chords();
    // Preserve the existing ignored modifier variants of migrated default keys;
    // an explicit custom assignment can take one of these formerly unused chords.
    for chord in [
        vec!["Ctrl", "Home"],
        vec!["Ctrl", "End"],
        vec!["Ctrl", "Left"],
        vec!["Ctrl", "Right"],
        vec!["Ctrl", "Delete"],
        vec!["Ctrl", "Back"],
        vec!["Ctrl", "Space"],
        vec!["LeftShift", "Enter"],
        vec!["LeftShift", "NumPadEnter"],
    ] {
        let chord: Vec<String> = chord.into_iter().map(str::to_owned).collect();
        if !preferences
            .commands
            .values()
            .flatten()
            .any(|configured| *configured == chord)
        {
            legacy.push(chord);
        }
    }
    bindings.extend(legacy.into_iter().map(|chord| Binding {
        id: legacy_id(&chord),
        alternatives: deadsync_editor::key_binding_preferences::physical::expand(&chord),
        repeat: false,
        blocks: true,
    }));
    Registry::new(bindings)
}
fn modifier(code: KeyCode) -> bool {
    matches!(
        code,
        KeyCode::ControlLeft
            | KeyCode::ControlRight
            | KeyCode::SuperLeft
            | KeyCode::SuperRight
            | KeyCode::ShiftLeft
            | KeyCode::ShiftRight
            | KeyCode::AltLeft
            | KeyCode::AltRight
    )
}
fn name(code: KeyCode) -> Option<String> {
    let fixed = match code {
        KeyCode::Enter => "Enter",
        KeyCode::NumpadEnter => "NumPadEnter",
        KeyCode::Numpad0 => "NumPad0",
        KeyCode::Slash => "OemQuestion",
        KeyCode::Space => "Space",
        KeyCode::Escape => "Escape",
        KeyCode::Tab => "Tab",
        KeyCode::Backspace => "Back",
        KeyCode::Delete => "Delete",
        KeyCode::Insert => "Insert",
        KeyCode::Home => "Home",
        KeyCode::End => "End",
        KeyCode::PageUp => "PageUp",
        KeyCode::PageDown => "PageDown",
        KeyCode::ArrowUp => "Up",
        KeyCode::ArrowDown => "Down",
        KeyCode::ArrowLeft => "Left",
        KeyCode::ArrowRight => "Right",
        _ => "",
    };
    if !fixed.is_empty() {
        return Some(fixed.into());
    }
    // KeyCode's named letter/digit/function variants have an explicit prefix;
    // other physical codes are rejected, never used as opaque serialized keys.
    let debug = format!("{code:?}");
    if let Some(letter) = debug
        .strip_prefix("Key")
        .filter(|s| s.len() == 1 && s.as_bytes()[0].is_ascii_uppercase())
    {
        return Some(letter.into());
    }
    if let Some(digit) = debug
        .strip_prefix("Digit")
        .filter(|s| s.len() == 1 && s.as_bytes()[0].is_ascii_digit())
    {
        return Some(format!("D{digit}"));
    }
    if deadsync_editor::key_binding_preferences::supported_key(&debug) && debug.starts_with('F') {
        return Some(debug);
    }
    None
}
impl Runtime {
    fn source_blocks_final(&self, key: &str) -> bool {
        self.preferences.source_order
            && self.source_blockers.iter().any(|binding| {
                self.registry.is_active(&binding.id)
                    && binding
                        .alternatives
                        .iter()
                        .any(|keys| keys.last().is_some_and(|last| last.name() == key))
            })
    }
    fn source_blocks_action(&self, action: Action) -> bool {
        self.preferences.commands[action.id()]
            .iter()
            .any(|keys| keys.last().is_some_and(|key| self.source_blocks_final(key)))
    }

    fn evaluate(&mut self, time: f64, keys: Vec<Key>) -> Vec<Effect> {
        let raw = self.registry.update(time, keys);
        self.effects.coalesce(&self.registry, raw)
    }
    fn observe(&mut self, event: &RawKeyboardEvent) {
        if matches!(event.code, KeyCode::AltLeft | KeyCode::AltRight) {
            let bit = if event.code == KeyCode::AltLeft { 1 } else { 2 };
            if event.pressed {
                self.alt |= bit;
            } else {
                self.alt &= !bit;
            }
        }
        if !modifier(event.code) {
            if event.pressed {
                if event.repeat && !self.down.contains(&event.code) {
                    self.quarantine.insert(event.code);
                }
                self.down.insert(event.code);
            } else {
                self.down.remove(&event.code);
                self.quarantine.remove(&event.code);
            }
        }
    }
    fn snapshot(&self, control: u8, shift: u8) -> Vec<Key> {
        let mut keys: Vec<_> = self
            .down
            .iter()
            .filter_map(|code| name(*code).map(|n| Key::named(&n)))
            .collect();
        if control & 3 != 0 {
            keys.push(Key::named("LeftControl"));
        }
        if control & 12 != 0 {
            keys.push(Key::named("LeftWindows"));
        }
        if shift != 0 {
            keys.push(Key::named("LeftShift"));
        }
        if self.alt != 0 {
            keys.push(Key::named("LeftAlt"));
        }
        keys
    }
    pub(super) fn capture_snapshot(&self, control: u8, shift: u8) -> Vec<String> {
        self.snapshot(control, shift)
            .into_iter()
            .map(|key| key.name().to_owned())
            .collect()
    }
    fn suspend(&mut self) {
        let raw = self.registry.cancel();
        self.effects.coalesce(&self.registry, raw);
        self.quarantine.extend(self.down.iter().copied());
    }
    pub(super) fn release_input(&mut self) {
        let raw = self.registry.cancel();
        self.effects.coalesce(&self.registry, raw);
        self.down.clear();
        self.quarantine.clear();
        self.alt = 0;
    }
}
impl State {
    pub(super) fn configure_key_binding_preferences(&mut self, preferences: Arc<Preferences>) {
        // Finish every old physical ownership before replacing the evaluator.
        let held = self.command_bindings.down.clone();
        self.release_input();
        self.command_bindings.down = held.clone();
        self.command_bindings.quarantine = held;
        self.command_bindings.registry = native_registry(&preferences);
        self.command_bindings.source_blockers =
            deadsync_editor::key_binding_preferences::source_profile::blockers(&preferences);
        self.command_bindings.preferences = preferences;
        // Cached toolbar labels must follow the newly installed binding Arc.
        self.refresh();
    }
    pub(super) fn command_bindings_observe(&mut self, event: &RawKeyboardEvent) {
        self.command_bindings.observe(event);
    }
    fn command_bindings_allowed(&self) -> bool {
        (self.panel.is_none() || matches!(self.panel, Some(super::Panel::EventSelection)))
            && !self.help
            && !self.minimap_focused()
            && self.preview.is_none()
            && self.job.is_none()
            && self.catalog_job.is_none()
            && self.analysis_job.is_none()
            && !self.wants_text_input()
            && self.label_preedit.is_empty()
    }
    fn command_binding_dispatch(&mut self, effects: Vec<Effect>, now: Instant) -> bool {
        let mut dispatched = false;
        for effect in effects {
            let (Effect::Press(id) | Effect::Repeat(id)) = effect else {
                continue;
            };
            if let Some(source) =
                deadsync_editor::key_binding_preferences::source_profile::blocker_source(&id)
            {
                if self.panel.is_some() {
                    continue;
                }
                self.status = crate::i18n::tr_fmt(
                    "Edition",
                    "KeyProfileUnavailableActivation",
                    &[("command", source)],
                );
                dispatched = true;
                continue;
            }
            let Some(action) = Action::from_id(&id) else {
                continue;
            };
            if self.panel.is_some() && !action.is_selected_review() {
                continue;
            }
            if self.command_bindings.source_blocks_action(action) {
                continue;
            }
            if !self.command_bindings_allowed() {
                break;
            }
            if !self.workspace.has_chart()
                && !matches!(
                    action,
                    Action::Save
                        | Action::Undo
                        | Action::Redo
                        | Action::DecreaseRate
                        | Action::IncreaseRate
                        | Action::PreviousTab
                        | Action::NextTab
                        | Action::SaveAs
                        | Action::OpenMenu
                        | Action::OpenSongProperties
                        | Action::NewChart
                        | Action::SaveRecovery
                        | Action::RecoverDraft
                        | Action::OpenChartList
                        | Action::OpenKeyBindings
                        | Action::OpenReceptors
                        | Action::OpenNoteColors
                        | Action::CloseTab
                        | Action::KeepTab
                        | Action::MoveTabLeft
                        | Action::MoveTabRight
                        | Action::OpenScrollSettings
                        | Action::FileNew
                        | Action::FileOpen
                        | Action::FileReload
                        | Action::FileRecent
                )
            {
                self.status = crate::i18n::tr("Edition", "NoChartHelp");
                continue;
            }
            dispatched = true;
            let command = match action {
                Action::AddEventTempo => {
                    self.stop_audio();
                    self.command_quick_add_review(deadsync_editor::quick_add::Kind::Tempo);
                    continue;
                }
                Action::AddEventInterpolatedScrollRate => {
                    self.stop_audio();
                    self.command_quick_add_review(deadsync_editor::quick_add::Kind::Speed);
                    continue;
                }
                Action::AddEventScrollRate => {
                    self.stop_audio();
                    self.command_quick_add_review(deadsync_editor::quick_add::Kind::Scroll);
                    continue;
                }
                Action::AddEventStop => {
                    self.stop_audio();
                    self.command_quick_add_review(deadsync_editor::quick_add::Kind::Stop);
                    continue;
                }
                Action::AddEventDelay => {
                    self.stop_audio();
                    self.command_quick_add_review(deadsync_editor::quick_add::Kind::Delay);
                    continue;
                }
                Action::AddEventWarp => {
                    self.stop_audio();
                    self.command_quick_add_review(deadsync_editor::quick_add::Kind::Warp);
                    continue;
                }
                Action::AddEventFakeRegion => {
                    self.stop_audio();
                    self.command_quick_add_review(deadsync_editor::quick_add::Kind::Fake);
                    continue;
                }
                Action::AddEventTicks => {
                    self.stop_audio();
                    self.command_quick_add_review(deadsync_editor::quick_add::Kind::Ticks);
                    continue;
                }
                Action::AddEventComboMultipliers => {
                    self.stop_audio();
                    self.command_quick_add_review(deadsync_editor::quick_add::Kind::Combo);
                    continue;
                }
                Action::AddEventTimeSignature => {
                    self.stop_audio();
                    self.command_quick_add_review(deadsync_editor::quick_add::Kind::Signature);
                    continue;
                }
                Action::AddEventLabel => {
                    self.stop_audio();
                    self.command_quick_add_review(deadsync_editor::quick_add::Kind::Label);
                    continue;
                }
                Action::AddEventAttack => {
                    self.stop_audio();
                    self.command_quick_add_review(deadsync_editor::quick_add::Kind::Attack);
                    continue;
                }
                Action::AddEventPattern => {
                    self.stop_audio();
                    self.command_quick_add_review(deadsync_editor::quick_add::Kind::Pattern);
                    continue;
                }
                Action::Mirror => {
                    self.stop_audio();
                    self.command_selected_transform_review(
                        deadsync_editor::transform::Transform::Mirror,
                    );
                    continue;
                }
                Action::Flip => {
                    self.stop_audio();
                    self.command_selected_transform_review(
                        deadsync_editor::transform::Transform::Flip,
                    );
                    continue;
                }
                Action::MirrorAndFlip => {
                    self.stop_audio();
                    self.command_selected_transform_review(
                        deadsync_editor::transform::Transform::MirrorAndFlip,
                    );
                    continue;
                }
                Action::ShiftLeft => {
                    self.stop_audio();
                    self.command_selected_transform_review(
                        deadsync_editor::transform::Transform::ShiftLanes {
                            right: false,
                            wrap: false,
                        },
                    );
                    continue;
                }
                Action::ShiftRight => {
                    self.stop_audio();
                    self.command_selected_transform_review(
                        deadsync_editor::transform::Transform::ShiftLanes {
                            right: true,
                            wrap: false,
                        },
                    );
                    continue;
                }
                Action::ShiftLeftAndWrap => {
                    self.stop_audio();
                    self.command_selected_transform_review(
                        deadsync_editor::transform::Transform::ShiftLanes {
                            right: false,
                            wrap: true,
                        },
                    );
                    continue;
                }
                Action::ShiftRightAndWrap => {
                    self.stop_audio();
                    self.command_selected_transform_review(
                        deadsync_editor::transform::Transform::ShiftLanes {
                            right: true,
                            wrap: true,
                        },
                    );
                    continue;
                }
                Action::ConvertSelectedNotesToPlayer1 => {
                    self.stop_audio();
                    self.command_selected_transform_review(
                        deadsync_editor::transform::Transform::AssignPlayer(0),
                    );
                    continue;
                }
                Action::ConvertSelectedNotesToPlayer2 => {
                    self.stop_audio();
                    self.command_selected_transform_review(
                        deadsync_editor::transform::Transform::AssignPlayer(1),
                    );
                    continue;
                }
                Action::ConvertSelectedNotesToPlayer3 => {
                    self.stop_audio();
                    self.command_selected_transform_review(
                        deadsync_editor::transform::Transform::AssignPlayer(2),
                    );
                    continue;
                }
                Action::ConvertSelectedNotesToPlayer4 => {
                    self.stop_audio();
                    self.command_selected_transform_review(
                        deadsync_editor::transform::Transform::AssignPlayer(3),
                    );
                    continue;
                }
                Action::SwapSelectedPlayer1And2Notes => {
                    self.stop_audio();
                    self.command_selected_transform_review(
                        deadsync_editor::transform::Transform::SwapPlayers(0, 1),
                    );
                    continue;
                }
                Action::SwapSelectedPlayer1And3Notes => {
                    self.stop_audio();
                    self.command_selected_transform_review(
                        deadsync_editor::transform::Transform::SwapPlayers(0, 2),
                    );
                    continue;
                }
                Action::SwapSelectedPlayer1And4Notes => {
                    self.stop_audio();
                    self.command_selected_transform_review(
                        deadsync_editor::transform::Transform::SwapPlayers(0, 3),
                    );
                    continue;
                }
                Action::SwapSelectedPlayer2And3Notes => {
                    self.stop_audio();
                    self.command_selected_transform_review(
                        deadsync_editor::transform::Transform::SwapPlayers(1, 2),
                    );
                    continue;
                }
                Action::SwapSelectedPlayer2And4Notes => {
                    self.stop_audio();
                    self.command_selected_transform_review(
                        deadsync_editor::transform::Transform::SwapPlayers(1, 3),
                    );
                    continue;
                }
                Action::SwapSelectedPlayer3And4Notes => {
                    self.stop_audio();
                    self.command_selected_transform_review(
                        deadsync_editor::transform::Transform::SwapPlayers(2, 3),
                    );
                    continue;
                }
                Action::ConvertSelectedTapsToMines => {
                    self.stop_audio();
                    self.command_note_operation_review(
                        deadsync_editor::note_operation::Operation::Convert {
                            from: deadsync_editor::notes::Kind::Tap,
                            to: deadsync_editor::notes::Kind::Mine,
                            sustain_rows: 0,
                        },
                    );
                    continue;
                }
                Action::ConvertSelectedTapsToFakes => {
                    self.stop_audio();
                    self.command_note_operation_review(
                        deadsync_editor::note_operation::Operation::Convert {
                            from: deadsync_editor::notes::Kind::Tap,
                            to: deadsync_editor::notes::Kind::Fake,
                            sustain_rows: 0,
                        },
                    );
                    continue;
                }
                Action::ConvertSelectedTapsToLifts => {
                    self.stop_audio();
                    self.command_note_operation_review(
                        deadsync_editor::note_operation::Operation::Convert {
                            from: deadsync_editor::notes::Kind::Tap,
                            to: deadsync_editor::notes::Kind::Lift,
                            sustain_rows: 0,
                        },
                    );
                    continue;
                }
                Action::ConvertSelectedMinesToTaps => {
                    self.stop_audio();
                    self.command_note_operation_review(
                        deadsync_editor::note_operation::Operation::Convert {
                            from: deadsync_editor::notes::Kind::Mine,
                            to: deadsync_editor::notes::Kind::Tap,
                            sustain_rows: 0,
                        },
                    );
                    continue;
                }
                Action::ConvertSelectedMinesToFakes => {
                    self.stop_audio();
                    self.command_note_operation_review(
                        deadsync_editor::note_operation::Operation::Convert {
                            from: deadsync_editor::notes::Kind::Mine,
                            to: deadsync_editor::notes::Kind::Fake,
                            sustain_rows: 0,
                        },
                    );
                    continue;
                }
                Action::ConvertSelectedMinesToLifts => {
                    self.stop_audio();
                    self.command_note_operation_review(
                        deadsync_editor::note_operation::Operation::Convert {
                            from: deadsync_editor::notes::Kind::Mine,
                            to: deadsync_editor::notes::Kind::Lift,
                            sustain_rows: 0,
                        },
                    );
                    continue;
                }
                Action::ConvertSelectedFakesToTaps => {
                    self.stop_audio();
                    self.command_note_operation_review(
                        deadsync_editor::note_operation::Operation::Convert {
                            from: deadsync_editor::notes::Kind::Fake,
                            to: deadsync_editor::notes::Kind::Tap,
                            sustain_rows: 0,
                        },
                    );
                    continue;
                }
                Action::ConvertSelectedLiftsToTaps => {
                    self.stop_audio();
                    self.command_note_operation_review(
                        deadsync_editor::note_operation::Operation::Convert {
                            from: deadsync_editor::notes::Kind::Lift,
                            to: deadsync_editor::notes::Kind::Tap,
                            sustain_rows: 0,
                        },
                    );
                    continue;
                }
                Action::ConvertSelectedHoldsToRolls => {
                    self.stop_audio();
                    self.command_note_operation_review(
                        deadsync_editor::note_operation::Operation::Convert {
                            from: deadsync_editor::notes::Kind::Hold,
                            to: deadsync_editor::notes::Kind::Roll,
                            sustain_rows: 0,
                        },
                    );
                    continue;
                }
                Action::ConvertSelectedHoldsToTaps => {
                    self.stop_audio();
                    self.command_note_operation_review(
                        deadsync_editor::note_operation::Operation::Convert {
                            from: deadsync_editor::notes::Kind::Hold,
                            to: deadsync_editor::notes::Kind::Tap,
                            sustain_rows: 0,
                        },
                    );
                    continue;
                }
                Action::ConvertSelectedHoldsToMines => {
                    self.stop_audio();
                    self.command_note_operation_review(
                        deadsync_editor::note_operation::Operation::Convert {
                            from: deadsync_editor::notes::Kind::Hold,
                            to: deadsync_editor::notes::Kind::Mine,
                            sustain_rows: 0,
                        },
                    );
                    continue;
                }
                Action::ConvertSelectedRollsToHolds => {
                    self.stop_audio();
                    self.command_note_operation_review(
                        deadsync_editor::note_operation::Operation::Convert {
                            from: deadsync_editor::notes::Kind::Roll,
                            to: deadsync_editor::notes::Kind::Hold,
                            sustain_rows: 0,
                        },
                    );
                    continue;
                }
                Action::ConvertSelectedRollsToTaps => {
                    self.stop_audio();
                    self.command_note_operation_review(
                        deadsync_editor::note_operation::Operation::Convert {
                            from: deadsync_editor::notes::Kind::Roll,
                            to: deadsync_editor::notes::Kind::Tap,
                            sustain_rows: 0,
                        },
                    );
                    continue;
                }
                Action::ConvertSelectedRollsToMines => {
                    self.stop_audio();
                    self.command_note_operation_review(
                        deadsync_editor::note_operation::Operation::Convert {
                            from: deadsync_editor::notes::Kind::Roll,
                            to: deadsync_editor::notes::Kind::Mine,
                            sustain_rows: 0,
                        },
                    );
                    continue;
                }
                Action::SwapSelectedTapsAndMines => {
                    self.stop_audio();
                    self.command_note_operation_review(
                        deadsync_editor::note_operation::Operation::Swap(
                            deadsync_editor::note_swap::Pair::TapsMines,
                        ),
                    );
                    continue;
                }
                Action::SwapSelectedTapsAndFakes => {
                    self.stop_audio();
                    self.command_note_operation_review(
                        deadsync_editor::note_operation::Operation::Swap(
                            deadsync_editor::note_swap::Pair::TapsFakes,
                        ),
                    );
                    continue;
                }
                Action::SwapSelectedTapsAndLifts => {
                    self.stop_audio();
                    self.command_note_operation_review(
                        deadsync_editor::note_operation::Operation::Swap(
                            deadsync_editor::note_swap::Pair::TapsLifts,
                        ),
                    );
                    continue;
                }
                Action::SwapSelectedMinesAndFakes => {
                    self.stop_audio();
                    self.command_note_operation_review(
                        deadsync_editor::note_operation::Operation::Swap(
                            deadsync_editor::note_swap::Pair::MinesFakes,
                        ),
                    );
                    continue;
                }
                Action::SwapSelectedMinesAndLifts => {
                    self.stop_audio();
                    self.command_note_operation_review(
                        deadsync_editor::note_operation::Operation::Swap(
                            deadsync_editor::note_swap::Pair::MinesLifts,
                        ),
                    );
                    continue;
                }
                Action::SwapSelectedHoldsAndRolls => {
                    self.stop_audio();
                    self.command_note_operation_review(
                        deadsync_editor::note_operation::Operation::Swap(
                            deadsync_editor::note_swap::Pair::HoldsRolls,
                        ),
                    );
                    continue;
                }
                Action::SwapSelectedHoldsAndTaps => {
                    self.stop_audio();
                    self.command_note_operation_review(
                        deadsync_editor::note_operation::Operation::Swap(
                            deadsync_editor::note_swap::Pair::HoldsTaps,
                        ),
                    );
                    continue;
                }
                Action::SwapSelectedHoldsAndMines => {
                    self.stop_audio();
                    self.command_note_operation_review(
                        deadsync_editor::note_operation::Operation::Swap(
                            deadsync_editor::note_swap::Pair::HoldsMines,
                        ),
                    );
                    continue;
                }
                Action::SwapSelectedRollsAndTaps => {
                    self.stop_audio();
                    self.command_note_operation_review(
                        deadsync_editor::note_operation::Operation::Swap(
                            deadsync_editor::note_swap::Pair::RollsTaps,
                        ),
                    );
                    continue;
                }
                Action::SwapSelectedRollsAndMines => {
                    self.stop_audio();
                    self.command_note_operation_review(
                        deadsync_editor::note_operation::Operation::Swap(
                            deadsync_editor::note_swap::Pair::RollsMines,
                        ),
                    );
                    continue;
                }
                Action::ShiftAllEarlier => {
                    self.stop_audio();
                    self.command_shift_all_review(false);
                    continue;
                }
                Action::ShiftAllLater => {
                    self.stop_audio();
                    self.command_shift_all_review(true);
                    continue;
                }

                Action::Save => Command::Save,
                Action::Undo => Command::Undo,
                Action::Redo => Command::Redo,
                Action::Copy => Command::Copy,
                Action::Cut => Command::Cut,
                Action::Paste => Command::Paste,
                Action::Delete => Command::Delete,
                Action::PreviousMeasure => Command::PreviousMeasure,
                Action::NextMeasure => Command::NextMeasure,
                Action::SelectRowRange => Command::ToggleSelection,
                Action::TogglePlayer => Command::SelectPlayer(
                    (self.workspace.player() + 1) % self.workspace.grid().players(),
                ),
                Action::ChartStart => Command::Seek(0),
                Action::ChartEnd => Command::Seek(
                    self.workspace
                        .grid()
                        .iter()
                        .map(|(pos, note)| note.end.unwrap_or(pos.row))
                        .max()
                        .unwrap_or(0),
                ),
                Action::PreviousLabel => Command::PreviousLabel,
                Action::NextLabel => Command::NextLabel,
                Action::DecreaseSnap | Action::IncreaseSnap => {
                    self.stop_audio();
                    self.cycle_snap(action == Action::IncreaseSnap);
                    continue;
                }
                Action::PreviousChart | Action::NextChart => {
                    self.stop_audio();
                    self.queue_chart_order(action == Action::PreviousChart);
                    continue;
                }
                Action::PreviousTab | Action::NextTab => {
                    self.chart_views_focus(action == Action::PreviousTab);
                    continue;
                }
                Action::ToggleWaveform => {
                    self.toggle_waveform();
                    continue;
                }
                Action::WaveformSettings => {
                    self.open_waveform_settings();
                    continue;
                }
                Action::DecreaseRate | Action::IncreaseRate => {
                    self.step_music_rate(action == Action::IncreaseRate);
                    continue;
                }
                Action::SaveAs => {
                    self.open_save_as();
                    continue;
                }
                Action::OpenMenu => {
                    self.stop_audio();
                    self.panel = Some(if self.workspace.has_chart() {
                        super::Panel::Menu(0)
                    } else {
                        super::Panel::EmptyMenu(0)
                    });
                    continue;
                }
                Action::OpenAnalysis => {
                    self.stop_audio();
                    self.open_analysis();
                    continue;
                }
                Action::OpenSongProperties => {
                    self.stop_audio();
                    self.open_song_properties();
                    continue;
                }
                Action::OpenTiming => {
                    self.stop_audio();
                    self.open_timing();
                    continue;
                }
                Action::OpenPatternSet => {
                    self.stop_audio();
                    self.open_pattern_set();
                    continue;
                }
                Action::OpenBatch => {
                    self.stop_audio();
                    self.open_batch();
                    continue;
                }
                Action::OpenChartProperties => {
                    self.stop_audio();
                    self.open_chart_properties();
                    continue;
                }
                Action::OpenMarker => {
                    self.stop_audio();
                    self.open_marker();
                    continue;
                }
                Action::RegeneratePattern => {
                    self.stop_audio();
                    self.regenerate_saved_pattern();
                    continue;
                }
                Action::OpenPattern => {
                    self.stop_audio();
                    self.open_pattern();
                    continue;
                }
                Action::OpenGeneration => {
                    self.stop_audio();
                    self.panel = Some(super::Panel::Generation(0));
                    self.status = crate::i18n::tr("Edition", "GenerationOptionsHelp");
                    continue;
                }
                Action::NewChart => {
                    self.stop_audio();
                    self.open_new_chart();
                    continue;
                }
                Action::DeleteChart => {
                    self.stop_audio();
                    self.open_chart_deletion();
                    continue;
                }
                Action::OpenChartList => {
                    self.open_chart_list();
                    continue;
                }
                Action::OpenKeyBindings => {
                    self.open_key_binding_settings();
                    continue;
                }
                Action::OpenReceptors => {
                    self.open_receptor_settings();
                    continue;
                }
                Action::OpenNoteColors => {
                    self.open_note_color_settings();
                    continue;
                }
                Action::SaveRecovery => Command::SaveRecovery,
                Action::RecoverDraft => Command::Recover,
                Action::DuplicateChart => Command::DuplicateChart,
                Action::CopyEvents => Command::CopyEvents,
                Action::CutEvents => Command::CutEvents,
                Action::PasteEvents => Command::PasteEvents,
                Action::OpenEventSelection => {
                    self.stop_audio();
                    self.open_event_selection();
                    continue;
                }
                Action::OpenChartEventCopy => {
                    self.open_chart_event_copy();
                    continue;
                }
                Action::InspectAttack => {
                    self.stop_audio();
                    self.open_attack_at_cursor();
                    continue;
                }
                Action::PreviousAttack => {
                    self.stop_audio();
                    self.navigate_attack(false);
                    continue;
                }
                Action::NextAttack => {
                    self.stop_audio();
                    self.navigate_attack(true);
                    continue;
                }
                Action::OpenScrollSettings => {
                    self.open_scroll_settings();
                    continue;
                }
                Action::OpenZoomSettings => {
                    self.open_zoom_settings();
                    continue;
                }
                Action::ToggleReverse => {
                    self.toggle_reverse_display();
                    continue;
                }
                Action::ToggleSpacing => {
                    self.toggle_spacing_mode(false);
                    continue;
                }
                Action::ToggleVariableSpacing => {
                    self.toggle_spacing_mode(true);
                    continue;
                }
                Action::ZoomIn => {
                    self.command_zoom(Some(1.0), now);
                    continue;
                }
                Action::ZoomOut => {
                    self.command_zoom(Some(-1.0), now);
                    continue;
                }
                Action::ResetZoom => {
                    self.command_zoom(None, now);
                    continue;
                }
                Action::ToggleAutoAdvance => Command::ToggleAutoAdvance,
                Action::NextTool => {
                    self.command_cycle_tool(false);
                    continue;
                }
                Action::PreviousTool => {
                    self.command_cycle_tool(true);
                    continue;
                }
                Action::MoveUp => {
                    self.navigate_snap(self.reverse_display);
                    continue;
                }
                Action::MoveDown => {
                    self.navigate_snap(!self.reverse_display);
                    continue;
                }
                Action::CloseTab => {
                    self.chart_views_apply(super::chart_views::ViewAction::Close);
                    continue;
                }
                Action::KeepTab => {
                    self.chart_views_apply(super::chart_views::ViewAction::Pin);
                    continue;
                }
                Action::MoveTabLeft => {
                    self.chart_views_apply(super::chart_views::ViewAction::Move(false));
                    continue;
                }
                Action::MoveTabRight => {
                    self.chart_views_apply(super::chart_views::ViewAction::Move(true));
                    continue;
                }
                Action::OpenNoteAttributes => {
                    self.stop_audio();
                    self.open_note_attributes();
                    continue;
                }
                Action::OpenNoteConversion => {
                    self.stop_audio();
                    self.command_open_note_conversion();
                    continue;
                }
                Action::OpenNoteOwners => {
                    self.stop_audio();
                    self.command_open_note_owners();
                    continue;
                }
                Action::OpenTransforms => {
                    self.stop_audio();
                    self.panel = Some(super::Panel::Transforms(0));
                    continue;
                }
                Action::FileNew => {
                    self.command_file_new();
                    break;
                }
                Action::FileOpen => {
                    self.command_file_open();
                    break;
                }
                Action::FileReload => {
                    self.command_file_reload();
                    break;
                }
                Action::FileRecent => {
                    self.command_file_recent();
                    break;
                }
                Action::PlayPause => {
                    self.receptor_views.reset();
                    self.toggle_audio();
                    continue;
                }
            };
            let bindings_before = Arc::clone(&self.command_bindings.preferences);
            self.stop_audio();
            self.command(command);
            // Source manager stops its iteration if Undo/Redo rebuilds bindings.
            if !Arc::ptr_eq(&bindings_before, &self.command_bindings.preferences) {
                break;
            }
        }
        dispatched
    }
    /// Called after global modifier bits are updated, before modal early returns.
    /// Releases are observed even when a panel owns the keyboard.
    pub(super) fn command_bindings_key(&mut self, event: &RawKeyboardEvent) -> bool {
        if !self.command_bindings_allowed() {
            self.command_bindings.suspend();
            return false;
        }
        let keys = self
            .command_bindings
            .snapshot(self.control_keys, self.shift_keys);
        let selection_context = matches!(self.panel, Some(super::Panel::EventSelection));
        let final_key = name(event.code);
        let consumes = final_key.as_ref().is_some_and(|key| {
            self.command_bindings
                .preferences
                .commands
                .iter()
                .filter(|(id, _)| {
                    !selection_context
                        || Action::from_id(id).is_some_and(Action::is_selected_review)
                })
                .flat_map(|(_, alternatives)| alternatives)
                .any(|chord| chord.last() == Some(key))
        });
        if !self.command_bindings.quarantine.is_empty() {
            return consumes;
        }
        let mut dispatched = false;
        // OS repeat cannot feed the source timer a second callback path.
        if !event.repeat {
            let elapsed = event
                .timestamp
                .checked_duration_since(self.command_bindings.origin)
                .unwrap_or_default()
                .as_secs_f64();
            let effects = self.command_bindings.evaluate(elapsed, keys);
            dispatched = self.command_binding_dispatch(effects, event.timestamp);
        }
        if selection_context {
            return dispatched && consumes;
        }
        if final_key
            .as_deref()
            .is_some_and(|key| self.command_bindings.source_blocks_final(key))
        {
            return true;
        }
        consumes
            && (dispatched
                || !self
                    .command_bindings
                    .registry
                    .is_active_prefix("__native_reserved_"))
    }
    pub(super) fn command_bindings_update(&mut self, now: Instant) {
        if !self.command_bindings_allowed() {
            self.command_bindings.suspend();
            return;
        }
        if !self.command_bindings.quarantine.is_empty() {
            return;
        }
        let elapsed = now
            .checked_duration_since(self.command_bindings.origin)
            .unwrap_or_default()
            .as_secs_f64();
        let keys = self
            .command_bindings
            .snapshot(self.control_keys, self.shift_keys);
        let effects = self.command_bindings.evaluate(elapsed, keys);
        self.command_binding_dispatch(effects, now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn keys(names: &[&str]) -> Vec<Key> {
        names
            .iter()
            .map(|name| Key::named(if *name == "Ctrl" { "LeftControl" } else { name }))
            .collect()
    }
    #[test]
    fn migrated_mixed_clipboard_commands_preempt_plain_clipboard_actions() {
        for (key, action, plain) in [
            ("C", "CopySelectedEvents", "Copy"),
            ("X", "CutSelectedEvents", "Cut"),
            ("V", "PasteSelectedEvents", "Paste"),
        ] {
            let mut registry = native_registry(&Preferences::default());
            registry.update(0., keys(&["Ctrl", "LeftShift"]));
            assert_eq!(
                registry.update(0.1, keys(&["Ctrl", "LeftShift", key])),
                vec![Effect::Press(action.into())]
            );
            assert!(!registry.is_active(plain));
        }
    }
    #[test]
    fn second_increment_longer_native_siblings_suppress_only_migrated_subsets() {
        for (held, final_key, active) in [
            (vec!["LeftShift"], "W", "OpenWaveformSettings"),
            (vec!["LeftShift"], "Right", "IncreaseMusicRate"),
            (vec!["Ctrl", "LeftShift"], "L", "MoveToPreviousLabel"),
            (vec!["Ctrl", "LeftShift"], "Tab", "OpenPreviousChart"),
            (vec!["Ctrl"], "PageDown", "FocusNextChart"),
        ] {
            let mut registry = native_registry(&Preferences::default());
            registry.update(0., keys(&held));
            let mut pressed = held;
            pressed.push(final_key);
            assert_eq!(
                registry.update(0.1, keys(&pressed)),
                vec![Effect::Press(active.into())]
            );
        }
        for held in [vec!["Ctrl"], vec!["Ctrl", "LeftShift"]] {
            let mut registry = native_registry(&Preferences::default());
            registry.update(0., keys(&held));
            let mut pressed = held;
            pressed.push("W");
            let effects = registry.update(0.1, keys(&pressed));
            assert_eq!(effects.len(), 1, "{effects:?}");
            assert!(registry.is_active("CloseFocusedChart"));
            assert!(!registry.is_active("ToggleWaveForm"));
            assert!(!registry.is_active("OpenWaveformSettings"));
        }
        for held in [vec!["Ctrl"], vec!["Ctrl", "LeftShift"]] {
            let mut registry = native_registry(&Preferences::default());
            registry.update(0., keys(&held));
            let mut pressed = held;
            pressed.push("Right");
            registry.update(0.1, keys(&pressed));
            assert!(
                registry.is_active_prefix("__native_reserved_")
                    || registry.is_active("MoveFocusedChartRight")
            );
            assert!(!registry.is_active("IncreaseSnap"));
            assert!(!registry.is_active("IncreaseMusicRate"));
        }
    }
    #[test]
    fn second_increment_rate_repeats_from_source_timer_not_os_repeat() {
        let mut registry = native_registry(&Preferences::default());
        registry.update(0., keys(&["LeftShift"]));
        let pressed = keys(&["LeftShift", "Right"]);
        assert_eq!(
            registry.update(0.1, pressed.clone()),
            vec![Effect::Press("IncreaseMusicRate".into())]
        );
        assert!(registry.update(0.6, pressed.clone()).is_empty());
        assert_eq!(
            registry.update(0.601, pressed),
            vec![Effect::Repeat("IncreaseMusicRate".into())]
        );
    }
    #[test]
    fn third_increment_save_as_recovery_and_chart_list_preserve_longer_siblings() {
        for (held, key, expected) in [
            (vec!["Ctrl"], "F12", "SaveAs"),
            (vec![], "F12", "OpenChartAnalysis"),
            (vec!["Ctrl", "LeftShift"], "S", "SaveRecoveryDraft"),
            (vec!["Ctrl"], "N", "CreateEmptyChart"),
            (vec!["Ctrl"], "B", "OpenChartList"),
            (vec!["Ctrl", "LeftShift"], "Back", "DeleteCurrentChart"),
        ] {
            let mut registry = native_registry(&Preferences::default());
            registry.update(0., keys(&held));
            let mut pressed = held;
            pressed.push(key);
            assert_eq!(
                registry.update(0.1, keys(&pressed)),
                vec![Effect::Press(expected.into())]
            );
        }
        let mut registry = native_registry(&Preferences::default());
        registry.update(0., keys(&["Ctrl", "LeftShift"]));
        let effects = registry.update(0.1, keys(&["Ctrl", "LeftShift", "B"]));
        assert_eq!(effects.len(), 1);
        assert!(registry.is_active("KeepChartOpen"));
        assert!(!registry.is_active("OpenChartList"));
    }
    #[test]
    fn fourth_increment_modifiers_preempt_navigation_without_doubling_enter_or_waveform() {
        for (held, key, expected) in [
            (vec!["Ctrl"], "Up", "IncreaseKeyboardZoom"),
            (vec!["Ctrl"], "Down", "DecreaseKeyboardZoom"),
            (vec!["Ctrl"], "NumPad0", "ResetKeyboardZoom"),
            (vec!["LeftShift"], "Tab", "PreviousNoteTool"),
            (vec!["Ctrl", "LeftShift"], "T", "ToggleVariableDisplay"),
            (vec!["Ctrl", "LeftShift"], "Enter", "InspectAttackAtCursor"),
            (
                vec!["Ctrl", "LeftShift"],
                "NumPadEnter",
                "InspectAttackAtCursor",
            ),
            (vec!["Ctrl", "LeftShift"], "W", "CloseFocusedChart"),
        ] {
            let mut registry = native_registry(&Preferences::default());
            registry.update(0., keys(&held));
            let mut pressed = held;
            pressed.push(key);
            assert_eq!(
                registry.update(0.1, keys(&pressed)),
                vec![Effect::Press(expected.into())]
            );
        }
    }
    #[test]
    fn an_executed_panel_callback_consumes_enter_even_with_an_equal_ignored_modifier_blocker() {
        crate::i18n::init_for_tests();
        let dir = tempfile::tempdir().unwrap();
        let song = dir.path().join("dispatch.ssc");
        std::fs::write(
            &song,
            b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;",
        )
        .unwrap();
        let mut state = State::open(&song, 0).unwrap();
        let mut prefs = Preferences::default();
        prefs.commands.insert(
            "OpenEditorMenu".into(),
            vec![vec!["LeftAlt".into(), "Enter".into()]],
        );
        prefs.validate().unwrap();
        state.configure_key_binding_preferences(Arc::new(prefs));
        for code in [KeyCode::ShiftRight, KeyCode::AltLeft, KeyCode::Enter] {
            state.raw_key(&RawKeyboardEvent {
                code,
                pressed: true,
                repeat: false,
                timestamp: Instant::now(),
                host_nanos: 0,
            });
        }
        // The opening Enter must not be reused to activate the menu's first row.
        assert!(matches!(state.panel, Some(super::super::Panel::Menu(0))));
    }
    #[test]
    fn physical_control_and_super_sides_coalesce_and_release_individually() {
        crate::i18n::init_for_tests();
        let dir = tempfile::tempdir().unwrap();
        let song = dir.path().join("sides.ssc");
        std::fs::write(
            &song,
            b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;",
        )
        .unwrap();
        let mut state = State::open(&song, 0).unwrap();
        let mut prefs = Preferences::default();
        prefs.commands.insert(
            "SelectRowRange".into(),
            vec![vec!["Ctrl".into(), "F13".into()]],
        );
        state.configure_key_binding_preferences(Arc::new(prefs));
        fn raw(s: &mut State, code: KeyCode, pressed: bool) {
            s.raw_key(&RawKeyboardEvent {
                code,
                pressed,
                repeat: false,
                timestamp: Instant::now(),
                host_nanos: 0,
            });
        }
        for code in [
            KeyCode::ControlLeft,
            KeyCode::ControlRight,
            KeyCode::SuperLeft,
            KeyCode::SuperRight,
            KeyCode::F13,
        ] {
            raw(&mut state, code, true);
        }
        assert_eq!(state.workspace.selection(), Some((0, 0)));
        assert!(state.command_bindings.effects.is_active("SelectRowRange"));
        for code in [
            KeyCode::ControlLeft,
            KeyCode::ControlRight,
            KeyCode::SuperLeft,
        ] {
            raw(&mut state, code, false);
            assert!(state.command_bindings.effects.is_active("SelectRowRange"));
        }
        raw(&mut state, KeyCode::SuperRight, false);
        assert!(!state.command_bindings.effects.is_active("SelectRowRange"));
        assert_eq!(state.workspace.selection(), Some((0, 0)));
        raw(&mut state, KeyCode::F13, false);
        raw(&mut state, KeyCode::ControlRight, true);
        raw(&mut state, KeyCode::SuperRight, true);
        raw(&mut state, KeyCode::F13, true);
        assert!(state.command_bindings.effects.is_active("SelectRowRange"));
        raw(&mut state, KeyCode::F1, true);
        state.update_with_time(Instant::now());
        assert!(!state.command_bindings.effects.is_active("SelectRowRange"));
        assert!(state.command_bindings.quarantine.contains(&KeyCode::F13));
        state.release_input();
        assert!(!state.command_bindings.effects.is_active("SelectRowRange"));
    }
    #[test]
    fn physical_names_preserve_key_identity_without_mapping_text_characters() {
        assert_eq!(name(KeyCode::KeyA).as_deref(), Some("A"));
        assert_eq!(name(KeyCode::Digit1).as_deref(), Some("D1"));
        assert_eq!(name(KeyCode::NumpadEnter).as_deref(), Some("NumPadEnter"));
        assert_eq!(name(KeyCode::F24).as_deref(), Some("F24"));
        assert_eq!(name(KeyCode::Numpad0).as_deref(), Some("NumPad0"));
        assert_eq!(name(KeyCode::AudioVolumeUp), None);
    }
    #[test]
    fn unavailable_markers_filter_equal_callbacks_and_only_their_active_final() {
        use deadsync_editor::key_binding_preferences::source_profile::{self, Platform};
        let prefs = source_profile::import(&Preferences::default(), r#"{"New":[["LeftControl","F13"]],"Close":[["LeftControl","F13"],["LeftControl","F14"]]}"#, Platform::Other).unwrap().preferences;
        let mut runtime = Runtime::default();
        runtime.registry = native_registry(&prefs);
        runtime.source_blockers = source_profile::blockers(&prefs);
        runtime.preferences = Arc::new(prefs);
        // Source accepts terminal first, then modifiers. Filtering observes the
        // complete registry state before dispatch, including equal chords.
        runtime.evaluate(0., keys(&["F13"]));
        runtime.evaluate(0.1, keys(&["LeftControl", "F13"]));
        assert!(runtime.source_blocks_action(Action::FileNew));
        assert!(runtime.source_blocks_final("F13"));
        assert!(!runtime.source_blocks_final("F14"));
        assert!(!runtime.source_blocks_final("D1"));
        runtime.evaluate(1., keys(&["LeftControl", "F13"]));
        assert!(runtime.source_blocks_action(Action::FileNew));
        runtime.evaluate(1.1, vec![]);
        assert!(!runtime.source_blocks_final("F13"));
    }
    #[test]
    fn unavailable_chord_is_consumed_without_callback_and_f1_rescue_remains_usable() {
        use deadsync_editor::key_binding_preferences::source_profile::{self, Platform};
        crate::i18n::init_for_tests();
        let dir = tempfile::tempdir().unwrap();
        let song = dir.path().join("blocked.ssc");
        std::fs::write(
            &song,
            b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;",
        )
        .unwrap();
        let mut state = State::open(&song, 0).unwrap();
        let prefs = source_profile::import(
            &Preferences::default(),
            r#"{"Close":[["F13"]],"New":[["F1"]]}"#,
            Platform::Other,
        )
        .unwrap()
        .preferences;
        state.configure_key_binding_preferences(Arc::new(prefs));
        let event = RawKeyboardEvent {
            code: KeyCode::F13,
            pressed: true,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        };
        state.command_bindings.observe(&event);
        assert!(state.command_bindings_key(&event));
        assert!(state.status.contains("Close"));
        assert!(state.panel.is_none());
        let release = RawKeyboardEvent {
            pressed: false,
            ..event
        };
        state.command_bindings.observe(&release);
        state.command_bindings_key(&release);
        state.raw_key(&RawKeyboardEvent {
            code: KeyCode::F1,
            pressed: true,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
        assert!(matches!(state.panel, Some(super::super::Panel::Menu(0))));
    }
    #[test]
    fn preset_blockers_do_not_disable_reserved_native_routes() {
        use deadsync_editor::key_binding_preferences::source_profile::{self, Platform};
        let prefs = source_profile::preset(&Preferences::default(), Platform::Other)
            .unwrap()
            .preferences;
        let mut runtime = Runtime::default();
        runtime.registry = native_registry(&prefs);
        runtime.source_blockers = source_profile::blockers(&prefs);
        runtime.preferences = Arc::new(prefs);
        for chord in [
            vec!["D1"],
            vec!["LeftShift", "D2"],
            vec!["Ctrl", "P"],
            vec!["Ctrl", "M"],
            vec!["Escape"],
            vec!["F2"],
        ] {
            runtime.evaluate(0., vec![]);
            runtime.evaluate(0.1, keys(&chord));
            assert!(!runtime.source_blocks_final(chord.last().unwrap()));
        }
    }
    fn reviewed_test_state() -> (tempfile::TempDir, State) {
        crate::i18n::init_for_tests();
        let dir = tempfile::tempdir().unwrap();
        let song = dir.path().join("review.ssc");
        std::fs::write(&song, b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1100\n0000\n0000\n0000;").unwrap();
        let state = State::open(&song, 0).unwrap();
        (dir, state)
    }
    fn reviewed_bytes(state: &State) -> Vec<u8> {
        state.workspace.file().session.serialize().unwrap()
    }
    #[test]
    fn every_quick_add_source_callback_prepares_without_mutation_then_explicit_apply_is_undoable() {
        for action in Action::ALL
            .into_iter()
            .filter(|action| action.id().starts_with("AddEvent"))
        {
            let (_dir, mut state) = reviewed_test_state();
            state.command(Command::Seek(48));
            let before = reviewed_bytes(&state);
            assert!(
                state.command_binding_dispatch(
                    vec![Effect::Press(action.id().into())],
                    Instant::now()
                )
            );
            assert!(
                matches!(state.panel, Some(super::super::Panel::QuickAdd(2))),
                "{}",
                action.id()
            );
            assert_eq!(
                reviewed_bytes(&state),
                before,
                "{} changed before review",
                action.id()
            );
            state.quick_add_key(3, KeyCode::Enter);
            assert!(state.panel.is_none(), "{}: {}", action.id(), state.status);
            assert_ne!(reviewed_bytes(&state), before, "{}", action.id());
            state.command(Command::Undo);
            assert_eq!(reviewed_bytes(&state), before, "{}", action.id());
        }
    }
    #[test]
    fn selected_source_callbacks_require_checked_selection_and_never_default_to_all_or_row_range() {
        for action in Action::ALL
            .into_iter()
            .filter(|action| action.is_selected_review())
        {
            let (_dir, mut state) = reviewed_test_state();
            let before = reviewed_bytes(&state);
            state.command_binding_dispatch(vec![Effect::Press(action.id().into())], Instant::now());
            assert!(state.panel.is_none(), "{}", action.id());
            assert_eq!(
                state.status,
                crate::i18n::tr("Edition", "NoteOperationMissingSelection")
            );
            assert_eq!(reviewed_bytes(&state), before);
        }
    }
    #[test]
    fn selected_mirror_and_convert_use_the_checked_heads_and_defer_changes_until_apply() {
        use deadsync_editor::{
            event_clipboard::Event,
            notes::{Kind, Position},
        };
        for action in [Action::Mirror, Action::ConvertSelectedTapsToMines] {
            let (_dir, mut state) = reviewed_test_state();
            let target = Position {
                row: 0,
                lane: 0,
                player: 0,
            };
            state.open_event_selection();
            let selection = &mut state.event_selection.as_mut().unwrap().selection;
            selection.select_all(false);
            let index = selection
                .events()
                .iter()
                .position(|event| matches!(event,Event::Note(position) if *position == target))
                .unwrap();
            selection.set_selected(index, true).unwrap();
            let before = reviewed_bytes(&state);
            state.command_binding_dispatch(vec![Effect::Press(action.id().into())], Instant::now());
            assert_eq!(reviewed_bytes(&state), before);
            if action == Action::Mirror {
                assert!(matches!(
                    state.panel,
                    Some(super::super::Panel::SelectedTransformReview(4))
                ));
                state.selected_transform_review_key(6, KeyCode::Enter);
                assert!(state.workspace.grid().get(target).is_none());
                assert!(
                    state
                        .workspace
                        .grid()
                        .get(Position { lane: 3, ..target })
                        .is_some()
                );
            } else {
                assert!(matches!(
                    state.panel,
                    Some(super::super::Panel::NoteOperationReview(2))
                ));
                state.note_operation_review_key(3, KeyCode::Enter);
                assert_eq!(state.workspace.grid().get(target).unwrap().kind, Kind::Mine);
            }
            assert_eq!(
                state
                    .workspace
                    .grid()
                    .get(Position { lane: 1, ..target })
                    .unwrap()
                    .kind,
                Kind::Tap
            );
            state.command(Command::Undo);
            assert_eq!(reviewed_bytes(&state), before);
        }
    }
    #[test]
    fn native_shift_all_callbacks_keep_their_direction_and_do_not_mutate_before_review() {
        for (action, later) in [
            (Action::ShiftAllEarlier, false),
            (Action::ShiftAllLater, true),
        ] {
            let (_dir, mut state) = reviewed_test_state();
            let before = reviewed_bytes(&state);
            state.command_binding_dispatch(vec![Effect::Press(action.id().into())], Instant::now());
            assert!(matches!(
                state.panel,
                Some(super::super::Panel::ShiftAll(1))
            ));
            assert_eq!(reviewed_bytes(&state), before);
            let text = state.shift_all_label(0, "ShiftAllDirection");
            assert!(
                text.contains(
                    crate::i18n::tr(
                        "Edition",
                        if later {
                            "ShiftAllLater"
                        } else {
                            "ShiftAllEarlier"
                        }
                    )
                    .as_ref()
                )
            );
        }
    }
    #[test]
    fn checked_selection_accepts_configured_source_review_keys_but_keeps_local_space_navigation() {
        let (_dir, mut state) = reviewed_test_state();
        let mut prefs = Preferences::default();
        prefs
            .commands
            .insert("Flip".into(), vec![vec!["F13".into()]]);
        prefs
            .commands
            .insert("PlayPause".into(), vec![vec!["Space".into()]]);
        state.configure_key_binding_preferences(Arc::new(prefs));
        state.open_event_selection();
        state
            .event_selection
            .as_mut()
            .unwrap()
            .selection
            .select_all(false);
        let before = reviewed_bytes(&state);
        for (code, pressed) in [(KeyCode::Space, true), (KeyCode::Space, false)] {
            state.raw_key(&RawKeyboardEvent {
                code,
                pressed,
                repeat: false,
                timestamp: Instant::now(),
                host_nanos: 0,
            });
        }
        assert!(matches!(
            state.panel,
            Some(super::super::Panel::EventSelection)
        ));
        assert_eq!(
            state
                .event_selection
                .as_ref()
                .unwrap()
                .selection
                .selected_count(),
            1
        );
        assert_eq!(reviewed_bytes(&state), before);
        state.raw_key(&RawKeyboardEvent {
            code: KeyCode::F13,
            pressed: true,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
        assert!(matches!(
            state.panel,
            Some(super::super::Panel::SelectedTransformReview(4))
        ));
        assert_eq!(reviewed_bytes(&state), before);
    }
}

pub(super) fn chord_label(keys: &[String]) -> String {
    if keys.is_empty() {
        return crate::i18n::tr("Edition", "KeyBindingUnbound").to_string();
    }
    keys.iter()
        .map(|key| {
            let localized = match key.as_str() {
                "Ctrl" => Some("Ctrl"),
                "LeftControl" | "RightControl" => Some("Control"),
                "LeftWindows" | "RightWindows" => Some("Super"),
                "OemQuestion" => Some("OemQuestion"),
                "LeftShift" | "RightShift" => Some("Shift"),
                "LeftAlt" | "RightAlt" => Some("Alt"),
                "Enter" => Some("Enter"),
                "NumPadEnter" => Some("NumPadEnter"),
                "NumPad0" => Some("NumPad0"),
                "Space" => Some("Space"),
                "Back" => Some("Back"),
                "Delete" => Some("Delete"),
                "Home" => Some("Home"),
                "End" => Some("End"),
                "PageUp" => Some("PageUp"),
                "PageDown" => Some("PageDown"),
                _ => None,
            };
            localized
                .map(|suffix| {
                    crate::i18n::tr("Edition", &format!("KeyBindingKey{suffix}")).to_string()
                })
                .unwrap_or_else(|| key.clone())
        })
        .collect::<Vec<_>>()
        .join(" + ")
}
impl State {
    pub(super) fn command_binding_label(&self, action: Action) -> String {
        self.command_bindings.preferences.commands[action.id()]
            .iter()
            .map(|chord| chord_label(chord))
            .collect::<Vec<_>>()
            .join(" / ")
    }
    pub(super) fn key_binding_help_lines(&self) -> Vec<String> {
        Action::ALL
            .into_iter()
            .map(|action| {
                let alternatives = self.command_binding_label(action);
                crate::i18n::tr_fmt(
                    "Edition",
                    "KeyBindingHelpBinding",
                    &[
                        ("keys", &alternatives),
                        (
                            "action",
                            &crate::i18n::tr(
                                "Edition",
                                &format!("KeyBindingAction{}", action.id()),
                            ),
                        ),
                    ],
                )
                .to_string()
            })
            .collect()
    }
}

impl State {
    fn command_cycle_tool(&mut self, previous: bool) {
        let index = super::TOOLS
            .iter()
            .position(|&tool| tool == self.workspace.tool())
            .unwrap_or(0);
        let next = (index + if previous { super::TOOLS.len() - 1 } else { 1 }) % super::TOOLS.len();
        self.stop_audio();
        self.command(Command::SelectTool(super::TOOLS[next]));
    }
    pub(super) fn command_open_note_conversion(&mut self) {
        self.note_conversion.duration = self.workspace.snap();
        self.panel = Some(super::Panel::NoteConversion(0));
        self.status = crate::i18n::tr("Edition", "NoteConversionHelp");
    }
    pub(super) fn command_open_note_owners(&mut self) {
        self.note_owners.count = self.workspace.grid().players();
        self.note_owners.a = self.workspace.player();
        self.note_owners.b = (self.workspace.player() + 1) % self.workspace.grid().players();
        self.panel = Some(super::Panel::NoteOwners(0));
        self.status = crate::i18n::tr("Edition", "NoteOwnersHelp");
    }
}
