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
        alternatives: vec![chord.iter().map(|key| Key::named(key)).collect()],
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
        if control != 0 {
            keys.push(Key::named("Ctrl"));
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
            .filter(|key| !matches!(key.name(), "LeftControl" | "LeftWindows"))
            .map(|key| key.name().to_owned())
            .collect()
    }
    fn suspend(&mut self) {
        self.registry.cancel();
        self.quarantine.extend(self.down.iter().copied());
    }
    pub(super) fn release_input(&mut self) {
        self.registry.cancel();
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
        self.command_bindings.preferences = preferences;
    }
    pub(super) fn command_bindings_observe(&mut self, event: &RawKeyboardEvent) {
        self.command_bindings.observe(event);
    }
    fn command_bindings_allowed(&self) -> bool {
        self.panel.is_none()
            && !self.help
            && !self.minimap_focused()
            && self.preview.is_none()
            && self.job.is_none()
            && self.catalog_job.is_none()
            && self.analysis_job.is_none()
            && !self.wants_text_input()
            && self.label_preedit.is_empty()
    }
    fn command_binding_dispatch(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            let (Effect::Press(id) | Effect::Repeat(id)) = effect else {
                continue;
            };
            let Some(action) = Action::from_id(&id) else {
                continue;
            };
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
                )
            {
                self.status = crate::i18n::tr("Edition", "NoChartHelp");
                continue;
            }
            let command = match action {
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
        let final_key = name(event.code);
        let consumes = final_key.is_some_and(|key| {
            self.command_bindings
                .preferences
                .commands
                .values()
                .flatten()
                .any(|chord| chord.last() == Some(&key))
        });
        if !self.command_bindings.quarantine.is_empty() {
            return consumes;
        }
        // OS repeat cannot feed the source timer a second callback path.
        if !event.repeat {
            let elapsed = event
                .timestamp
                .checked_duration_since(self.command_bindings.origin)
                .unwrap_or_default()
                .as_secs_f64();
            let effects = self.command_bindings.registry.update(elapsed, keys);
            self.command_binding_dispatch(effects);
        }
        consumes
            && !self
                .command_bindings
                .registry
                .is_active_prefix("__native_reserved_")
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
        let effects = self.command_bindings.registry.update(elapsed, keys);
        self.command_binding_dispatch(effects);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn keys(names: &[&str]) -> Vec<Key> {
        names.iter().map(|name| Key::named(name)).collect()
    }
    #[test]
    fn legacy_siblings_preempt_subset_actions_without_dispatching_save_or_copy() {
        for (key, action, _index) in [("C", "Copy", 1), ("X", "Cut", 2), ("V", "Paste", 3)] {
            let mut registry = native_registry(&Preferences::default());
            registry.update(0., keys(&["Ctrl", "LeftShift"]));
            let effects = registry.update(0.1, keys(&["Ctrl", "LeftShift", key]));
            assert_eq!(
                effects,
                vec![Effect::Press(legacy_id(&[
                    "Ctrl".into(),
                    "LeftShift".into(),
                    key.into()
                ]))]
            );
            assert!(!registry.is_active(action));
            registry.update(0.2, keys(&[key]));
            assert!(!registry.is_active(action));
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
            assert!(registry.is_active_prefix("__native_reserved_"));
            assert!(!registry.is_active("ToggleWaveForm"));
            assert!(!registry.is_active("OpenWaveformSettings"));
        }
        for held in [vec!["Ctrl"], vec!["Ctrl", "LeftShift"]] {
            let mut registry = native_registry(&Preferences::default());
            registry.update(0., keys(&held));
            let mut pressed = held;
            pressed.push("Right");
            registry.update(0.1, keys(&pressed));
            assert!(registry.is_active_prefix("__native_reserved_"));
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
        assert!(registry.is_active_prefix("__native_reserved_"));
        assert!(!registry.is_active("OpenChartList"));
    }
    #[test]
    fn physical_names_preserve_key_identity_without_mapping_text_characters() {
        assert_eq!(name(KeyCode::KeyA).as_deref(), Some("A"));
        assert_eq!(name(KeyCode::Digit1).as_deref(), Some("D1"));
        assert_eq!(name(KeyCode::NumpadEnter).as_deref(), Some("NumPadEnter"));
        assert_eq!(name(KeyCode::F24).as_deref(), Some("F24"));
        assert_eq!(name(KeyCode::AudioVolumeUp), None);
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
                "LeftShift" | "RightShift" => Some("Shift"),
                "LeftAlt" | "RightAlt" => Some("Alt"),
                "Enter" => Some("Enter"),
                "NumPadEnter" => Some("NumPadEnter"),
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
                .map(|suffix| crate::i18n::tr("Edition", &format!("KeyBindingKey{suffix}")).to_string())
                .unwrap_or_else(|| key.clone())
        })
        .collect::<Vec<_>>()
        .join(" + ")
}
impl State {
    pub(super) fn key_binding_help_lines(&self) -> Vec<String> {
        Action::ALL
            .into_iter()
            .map(|action| {
                let alternatives = self.command_bindings.preferences.commands[action.id()]
                    .iter()
                    .map(|chord| chord_label(chord))
                    .collect::<Vec<_>>()
                    .join(" / ");
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
                ).to_string()
            })
            .collect()
    }
}
