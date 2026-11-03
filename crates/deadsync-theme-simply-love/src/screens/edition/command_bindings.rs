//! First executable configurable subset. Input stays semantic, never synthesized.
#[cfg(test)]
#[path = "command_bindings/source_navigation_tests.rs"]
mod source_navigation_tests;
use super::State;
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadsync_editor::{
    key_binding_preferences::{Action, Preferences},
    key_commands::{Binding, Effect, Key, Registry},
    workspace::Command,
};
use std::{collections::BTreeSet, sync::Arc, time::Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Context { Global, Selection, Region, Marker, Minimap, Blocked,
}
impl Context {
    fn allows(self, action: Action) -> bool {
        match self {
            Self::Global => true,
            Self::Region => action==Action::CancelGoBack,
            Self::Selection => {
                action.is_selected_review() || action.is_selection_preset()
                || action.is_source_view_setting() || action.is_source_event_navigation()
                || action.is_source_pattern_action() || matches!(
                        action,
                        Action::Save | Action::Undo | Action::Redo | Action::CancelGoBack
                    )
            }
            Self::Marker => {
                action.is_source_pattern_action() || matches!(action,
                Action::Save
                            | Action::Undo
                            | Action::Redo
                            | Action::NavigateNextPattern | Action::NavigatePreviousPattern
                | Action::MoveToNextPattern | Action::MoveToPreviousPattern)
            }
            Self::Minimap => matches!(action,Action::ToggleMinimapFocus | Action::OpenMinimapSettings),
            Self::Blocked => false,
        }
    }
}
pub(super) struct Runtime {
    pub(super) preferences: Arc<Preferences>,
    registry: Registry,
    context: Option<Context>,
    source_blockers: Vec<Binding>,
    navigation: Option<deadsync_editor::navigation::Index>,
    effects: deadsync_editor::key_binding_preferences::physical::Effects,
    down: BTreeSet<KeyCode>,
    physical_down: BTreeSet<KeyCode>,
    fresh_press: Option<KeyCode>,
    quarantine: BTreeSet<KeyCode>,
    held_query_quarantine: BTreeSet<KeyCode>,
    alt: u8,
    origin: Instant,
}
impl Default for Runtime {
    fn default() -> Self {
        let preferences = Arc::new(Preferences::default());
        Self {
            navigation: None,
            registry: native_registry(&preferences),
            context: Some(Context::Global),
            source_blockers: deadsync_editor::key_binding_preferences::source_profile::blockers(
                &preferences,
            ),
            effects: Default::default(),
            preferences,
            down: BTreeSet::new(),
            physical_down: BTreeSet::new(),
            fresh_press: None,
            quarantine: BTreeSet::new(),
            held_query_quarantine: BTreeSet::new(),
            alt: 0,
            origin: Instant::now(),
        }
    }
}
fn legacy_id(chord: &[String]) -> String {
    format!("__native_reserved_{}", chord.join("+"))
}
fn native_registry(preferences: &Preferences) -> Registry { Registry::new(native_bindings(preferences)) }
fn native_bindings(preferences: &Preferences) -> Vec<Binding> {
    let mut bindings = preferences.bindings();
    let mut legacy = deadsync_editor::key_binding_preferences::reservations::blocking_chords();
    // Preserve the existing ignored modifier variants of migrated default keys;
    // an explicit custom assignment can take one of these formerly unused chords.
    let mut ignored_chords = vec![
        vec!["Ctrl", "Home"],
        vec!["Ctrl", "End"],
        vec!["Ctrl", "Left"],
        vec!["Ctrl", "Right"],
        vec!["Ctrl", "Delete"],
        vec!["Ctrl", "Back"],
        vec!["Ctrl", "Space"],
        vec!["LeftShift", "Enter"],
        vec!["LeftShift", "NumPadEnter"],
    ];
    // The old native editor ignored Ctrl+vertical arrows. Source profiles use
    // those keys for real stream callbacks and retain original subset semantics.
    if !preferences.source_order {
        ignored_chords.extend([vec!["Ctrl", "Up"], vec!["Ctrl", "Down"]]);
    }
    for chord in ignored_chords {
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
    bindings
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
        KeyCode::ControlLeft | KeyCode::ControlRight => "LeftControl",
        KeyCode::SuperLeft | KeyCode::SuperRight => "LeftWindows",
        KeyCode::ShiftLeft | KeyCode::ShiftRight => "LeftShift",
        KeyCode::AltLeft | KeyCode::AltRight => "LeftAlt",
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
        self.fresh_press = (event.pressed && !event.repeat && !self.physical_down.contains(&event.code)).then_some(event.code);
        if event.pressed && (modifier(event.code) || held_name(event.code).is_some()) {
            if !self.quarantine.is_empty() { self.quarantine.insert(event.code); }
            if !self.held_query_quarantine.is_empty() { self.held_query_quarantine.insert(event.code); }
            self.physical_down.insert(event.code);
        }
        else { self.physical_down.remove(&event.code); self.quarantine.remove(&event.code); self.held_query_quarantine.remove(&event.code); }

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
    pub(super) fn alt_held(&self) -> bool { self.alt != 0 }
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
        self.quarantine.extend(self.physical_down.iter().copied());
    }
    pub(super) fn release_input(&mut self) {
        let raw = self.registry.cancel();
        self.effects.coalesce(&self.registry, raw);
        self.down.clear();
        self.quarantine = self.physical_down.clone();
        self.held_query_quarantine = self.physical_down.clone();
        self.alt = 0;
    }
}
impl State {
    pub(super) fn configure_key_binding_preferences(&mut self, preferences: Arc<Preferences>) {
        // Finish every old physical ownership before replacing the evaluator.
        let held = self.command_bindings.down.clone();
        let physical_held = self.command_bindings.physical_down.clone();
        self.release_input();
        self.command_bindings.down = held.clone();
        self.command_bindings.quarantine = physical_held;
        self.command_bindings.registry = native_registry(&preferences);
        self.command_bindings.context = None;
        self.command_bindings.source_blockers =
            deadsync_editor::key_binding_preferences::source_profile::blockers(&preferences);
        self.command_bindings.preferences = preferences;
        self.command_bindings_sync_context(Instant::now(),None);
        // Cached toolbar labels must follow the newly installed binding Arc.
        self.refresh();
    }
    pub(super) fn command_bindings_observe(&mut self, event: &RawKeyboardEvent) {
        self.command_bindings.observe(event);
    }
    pub(super) fn physical_clipboard_modifier_down(&self) -> bool {
        self.command_bindings.physical_down.iter().any(|code| {
            matches!(
                code,
                KeyCode::ControlLeft
                    | KeyCode::ControlRight
                    | KeyCode::SuperLeft
                    | KeyCode::SuperRight
            )
        })
    }
    pub(super) fn publish_note_entry_mode(&mut self) {
        let mode=if self.workspace.auto_advance(){deadsync_editor::lane_entry::Mode::AdvanceBySnap}else{deadsync_editor::lane_entry::Mode::Normal};
        if let Some(file)=self.application_preferences_owner_mut(){file.set_note_entry_mode(mode);}

    }
    fn command_binding_context(&self) -> Context {
        if !self.command_bindings_allowed() { return Context::Blocked; }
        if self.minimap_focused() { return Context::Minimap; }
        if self.region_selection.is_some(){return Context::Region;}
        match self.panel {
            None => Context::Global,
            Some(super::Panel::EventSelection) => Context::Selection,
            Some(super::Panel::Marker(_)) => Context::Marker,
            _ => Context::Blocked,
        }
    }
    fn command_binding_context_allows(&self, action: Action) -> bool {
        let context = self.command_binding_context();
        if context == Context::Marker
            && matches!(action, Action::Save | Action::Undo | Action::Redo)
            && self
                .marker
                .as_ref()
                .is_some_and(|draft| draft.has_changes())
        {
            return false;
        }
        context.allows(action) }
    /// Project the native evaluator before dispatch, so disabled global commands
    /// cannot block local picker/minimap controls through held arbitrary prefixes.
    fn command_bindings_sync_context(&mut self, now: Instant, fresh: Option<KeyCode>) {
        let context = self.command_binding_context();
        if self.command_bindings.context == Some(context) { return; }
        self.lane_entry_release_input();
        let raw = self.command_bindings.registry.cancel();
        self.command_bindings.effects.coalesce(&self.command_bindings.registry, raw);
        self.command_bindings.quarantine.extend(self.command_bindings.down.iter().copied().filter(|code|Some(*code)!=fresh),
        );
        let mut bindings = native_bindings(&self.command_bindings.preferences);
        if context != Context::Global {
            bindings.retain(|binding| {
                Action::from_id(&binding.id).is_some_and(|action|context.allows(action))
            });
        }
        self.command_bindings.registry = Registry::new(bindings);
        self.command_bindings.effects = Default::default();
        self.command_bindings.context = Some(context);
        let snapshot: Vec<_> = self.command_bindings.physical_down.iter().filter(|code|Some(**code)!=fresh)
            .filter_map(|code|name(*code).map(|key|Key::named(&key))).collect();
        let elapsed = now.checked_duration_since(self.command_bindings.origin).unwrap_or_default().as_secs_f64();
        // Prime held prefixes silently: entering a context is not another key
        // press. This permits Ctrl+A, release A, Ctrl+Alt+A in the picker.
        self.command_bindings.evaluate(elapsed,snapshot.clone());
        let modifier = |key:&str| {
            matches!(key,"Ctrl"|"LeftControl"|"RightControl"|"LeftWindows"|"RightWindows"|"LeftShift"|"RightShift"|"LeftAlt"|"RightAlt")
        };
        let held_modifier_finals: Vec<_> = Action::ALL.into_iter().filter(|action| {
                action.repeats()
            && self.command_bindings.registry.is_active(action.id())
            })
            .flat_map(|action| self.command_bindings.preferences.commands[action.id()].iter())
            .flat_map(|keys| deadsync_editor::key_binding_preferences::physical::expand(keys))
            .filter(|keys| keys.iter().all(|key|snapshot.contains(key)))
            .filter_map(|keys|keys.last().filter(|key|modifier(key.name())).cloned()).collect();
        for code in &self.command_bindings.physical_down {
            if name(*code).is_some_and(|key|held_modifier_finals.contains(&Key::named(&key))) {
                self.command_bindings.quarantine.insert(*code);
            }
        }
        // ArrowModification carries read-only gesture policy. Reconstruct it
        // from a still-held modifier only after cancellation/quarantine is over;
        // never replay any editing, saving, navigation or lane-press callback.
        if context == Context::Global && self.command_bindings.quarantine.is_empty()
            && self.command_bindings.registry.is_active(Action::ArrowModification.id()) {
            self.lane_entry_semantic_press(Action::ArrowModification);
        }
    }
    /// Source OnEscape: cancel a gesture, stop transport, then clear selection.
    /// Closing the native editor is the separate CloseEditor extension.
    fn source_navigation_position(&self)->Result<f64,deadsync_editor::Error>{
        Ok(self.capture_song_property_presentation()?.row())
    }
    fn command_source_measure_navigation(&mut self, previous:bool){
        let result=(||->Result<f64,deadsync_editor::Error>{
            let row=self.source_navigation_position()?;
            let signatures=deadsync_editor::timing_edit::signature::Signatures::from_document(self.workspace.file().session.document(),self.workspace.chart(),
            )?;
            let integral=row.max(0.0).floor().min(i32::MAX as f64)as u64;
            let delta=signatures.measure_destination(integral,false)?-integral as i64;
            Ok(row+if previous{-(delta as f64)}else{delta as f64})
        })();
        match result{Ok(row)=>self.navigate_exact_position(row,true),Err(error)=>self.status=crate::i18n::editor_error(&error).into(),
        }
    }
    fn command_source_label_navigation(&mut self, previous:bool){
        let result=(||->Result<Option<f64>,deadsync_editor::Error>{
            let row=self.source_navigation_position()?;
            let labels=deadsync_editor::timing_edit::label::Labels::from_document(self.workspace.file().session.document(),self.workspace.chart(),
            )?;
            let values=labels.values();
            Ok(if previous {values.keys().rev().find(|&&candidate|(candidate as f64)<row).or_else(||values.keys().next_back())}else{values.keys().find(|&&candidate|(candidate as f64)>row).or_else(||values.keys().next())}.map(|&row|row as f64))
        })();
        match result{Ok(Some(row))=>self.navigate_exact_position(row,true),Ok(None)=>{}
            Err(error)=>self.status=crate::i18n::editor_error(&error).into(),
        }
    }
    fn command_source_chart_edge(&mut self,end:bool){
        let result=if end{self.source_navigation_position().and_then(|position|self.workspace.source_chart_end_position_at(position))}else{Ok(0.0)};
        match result{Ok(row)=>self.navigate_exact_position(row,true),Err(error)=>self.status=crate::i18n::editor_error(&error).into(),
        }
    }
    fn command_source_event_navigation(&mut self, command: deadsync_editor::navigation::Command) {
        let playing_row = if self.chart_autoplay_transport().is_some() {
            self.playback_position().map(|(_, row)| row)
        } else {
            None
        };
        let cursor = playing_row
            .or_else(|| {
                self.focus_position
                    .as_ref()
                    .filter(|position| position.is_current(&self.workspace))
                    .map(|position| position.row())
            })
            .unwrap_or(self.workspace.row() as f64);
        let result = (|| -> Result<Option<f64>, deadsync_editor::Error> {
            let session = &self.workspace.file().session;
            let chart = self.workspace.chart();
            if !self
                .command_bindings
                .navigation
                .as_ref()
                .is_some_and(|index| index.is_current(session, chart))
            {
                self.command_bindings.navigation =
                    Some(deadsync_editor::navigation::Index::capture(session, chart)?);
            }
            self.command_bindings
                .navigation
                .as_ref()
                .expect("prepared navigation index")
                .target(
                    session,
                    chart,
                    command,
                    cursor,
                    self.workspace.source_row_selection_at(cursor)?.map(|range|f64::from(range.0)),
                )
        })();
        match result {
            Ok(Some(target)) => self.navigate_exact_position(target, true),
            Ok(None) => {}
            Err(error) => {
                self.status =
                    crate::i18n::tr_fmt("Edition", "Error", &[("message", &crate::i18n::editor_error(&error))],
                )
            }
        }
    }
    fn command_source_pattern_navigation(&mut self, forward: bool) {
        if self
            .marker
            .as_ref()
            .is_some_and(|draft| draft.has_changes())
        {
            self.status = crate::i18n::tr("Edition", "MarkerPending");
            return;
        }
        let result=(||->Result<Option<u64>,deadsync_editor::Error>{
            let position=self.source_navigation_position()?;
            Ok(deadsync_editor::generation::metadata::ChartData::read(self.workspace.file().session.document(),self.workspace.chart(),
            )?.and_then(|data|data.adjacent_marker_at(position,forward)))
        })();
        match result {
            Ok(Some(row))=>{
                let result=self.navigate_wheel_position(false,row as f64,true);
                match result {Ok(())=>{self.wheel.cancel();self.open_marker();self.refresh();}
                    Err(error)=>self.status=crate::i18n::editor_error(&error).into(),
                }
            }
            Ok(None)=>{}
            Err(error)=>self.status=crate::i18n::editor_error(&error).into(),
        }
    }
    fn command_cancel_go_back(&mut self) {
        if self.lane_entry_cancel() {
            return;
        }
        if self.is_playing() {
            self.stop_audio();
            return;
        }
        self.event_selection = None;
        self.region_selection = None;
        if matches!(self.panel, Some(super::Panel::EventSelection)) {
            self.panel = None;
        }
        if self.workspace.selection().is_some() {
            self.command(Command::ToggleSelection);
        } else {
            self.status = crate::i18n::tr("Edition", "Ready");
            self.refresh();
        }
    }
    pub(super) fn command_binding_close_final(&self, code: KeyCode) -> bool {
        let Some(key) = name(code).map(|key|Key::named(&key)) else {return false;};
        let down: BTreeSet<_> = self.command_bindings.physical_down.iter()
            .filter_map(|code|name(*code).map(|key|Key::named(&key))).collect();
        ["CloseEditor","CancelGoBack"].into_iter()
            .flat_map(|id| &self.command_bindings.preferences.commands[id])
            .flat_map(|chord|deadsync_editor::key_binding_preferences::physical::expand(chord))
            .any(|chord|chord.last()==Some(&key)&&chord.iter().all(|key|down.contains(key)))
    }
    pub(super) fn command_binding_entry_modifiers_allowed(&self, action: Action) -> bool {
        if self.command_bindings.preferences.source_order || !self.control && !self.command_bindings.alt_held() { return true; }
        let snapshot = self.command_bindings.snapshot(self.control_keys, self.shift_keys);
        self.command_bindings.preferences.commands[action.id()].iter().any(|keys| {
            let control = keys.iter().any(|key| {
                    matches!(key.as_str(),"Ctrl"|"LeftControl"|"RightControl"|"LeftWindows"|"RightWindows")
                });
            let alt = keys.iter().any(|key|matches!(key.as_str(),"LeftAlt"|"RightAlt"));
            (!self.control || control) && (!self.command_bindings.alt_held() || alt)
                && deadsync_editor::key_binding_preferences::physical::expand(keys).iter()
                    .any(|chord| {
                            !chord.is_empty() && chord.iter().all(|key| snapshot.contains(key))
                        })
        })
    }
    pub(super) fn command_binding_quarantine_held(&mut self) {
        self.command_bindings.quarantine.extend(self.command_bindings.physical_down.iter().copied());
    }
    pub(super) fn command_binding_quarantine(&mut self, code: KeyCode) {
        self.command_bindings.quarantine.insert(code);
    }
    fn command_bindings_allowed(&self) -> bool {
        (self.panel.is_none()
            || matches!(
                self.panel,
                Some(super::Panel::EventSelection | super::Panel::Marker(_))
            ))
            && (self.region_selection.is_none() || self.command_bindings.preferences.source_order)
            && !self.help
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
            if let Effect::Release(id) = &effect {
                if let Some(action) = Action::from_id(id).filter(|action| action.is_lane_entry()) {
                    self.lane_entry_semantic_release(action);
                    dispatched = true;
                }
                continue;
            }
            let (Effect::Press(id) | Effect::Repeat(id)) = effect else { continue; };
            if let Some(source) =
                deadsync_editor::key_binding_preferences::source_profile::blocker_source(&id)
            {
                if self.panel.is_some() || self.minimap_focused() {
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
            if !self.command_binding_context_allows(action) {
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
                    Action::AutoApplyAllSongAssets
                        | Action::AutoApplyUnsetSongAssets
                        | Action::Save
                        | Action::Undo
                        | Action::Redo
                        | Action::DecreaseRate
                        | Action::IncreaseRate
                        | Action::PreviousChart
                        | Action::NextChart
                        | Action::PreviousTab
                        | Action::NextTab
                        | Action::SaveAs
                        | Action::OpenMenu
                        | Action::OpenSongProperties
                        | Action::OpenGeneration
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
                        | Action::OpenZoomSettings
                        | Action::WaveformSettings
                        | Action::ToggleWaveform
                        | Action::FileNew
                        | Action::FileOpen
                        | Action::FileReload
                        | Action::FileRecent
                        | Action::ToggleAssistTick
                        | Action::ToggleBeatTick
                        | Action::PlayPause
                        | Action::TogglePreview
                        | Action::ToggleNoteEntryMode
                        | Action::ToggleSpacingMode
                        | Action::ToggleDark
                        | Action::OpenDarkSettings
                        | Action::OpenContainingFolder
                        | Action::CloseEditor
                        | Action::Close
                        | Action::SavePackFile
                        | Action::Exit
                        | Action::CancelGoBack
                        | Action::SnapToNone
                        | Action::SnapToQuarters
                        | Action::SnapToEighths
                        | Action::SnapToTwelfths
                        | Action::SnapToSixteenths
                        | Action::SnapToTwentyFourths
                        | Action::SnapToThirtySeconds
                        | Action::SnapToFortyEighths
                        | Action::SnapToSixtyFourths
                        | Action::SnapToOneHundredNinetySeconds
                )
            {
                self.status = crate::i18n::tr("Edition", "NoChartHelp");
                continue;
            }
            let application_history=match action{Action::Undo=>self.workspace.file().session.can_undo_application_change(),Action::Redo=>self.workspace.file().session.can_redo_application_change(),_=>false,
            };
            if !self.has_song()&&!application_history&&!action.is_source_view_setting()&&!matches!(action,
                Action::FileNew|Action::FileOpen|Action::FileReload|Action::FileRecent|Action::OpenMenu|Action::OpenKeyBindings|Action::CloseEditor|Action::Exit|Action::Close
                |Action::ToggleNoteEntryMode|Action::ToggleSpacingMode|Action::OpenReceptors|Action::OpenNoteColors|Action::OpenScrollSettings|Action::OpenZoomSettings|Action::WaveformSettings|Action::ToggleWaveform|Action::ToggleDark|Action::OpenDarkSettings){
                self.status=crate::i18n::tr("Edition","FileNoSongHelp");continue;
            }
            dispatched = true;
            if action.is_lane_entry() {
                self.lane_entry_semantic_press(action);
                continue;
            }
            let command = match action {
                Action::SelectAllNotes => {
                    self.command_selection_preset(
                        deadsync_editor::event_clipboard::presets::Preset::Notes,
                        false,
                    );
                    continue;
                }
                Action::SelectAllTaps => {
                    self.command_selection_preset(
                        deadsync_editor::event_clipboard::presets::Preset::Taps,
                        false,
                    );
                    continue;
                }
                Action::SelectAllMines => {
                    self.command_selection_preset(
                        deadsync_editor::event_clipboard::presets::Preset::Mines,
                        false,
                    );
                    continue;
                }
                Action::SelectAllFakes => {
                    self.command_selection_preset(
                        deadsync_editor::event_clipboard::presets::Preset::Fakes,
                        false,
                    );
                    continue;
                }
                Action::SelectAllLifts => {
                    self.command_selection_preset(
                        deadsync_editor::event_clipboard::presets::Preset::Lifts,
                        false,
                    );
                    continue;
                }
                Action::SelectAllHolds => {
                    self.command_selection_preset(
                        deadsync_editor::event_clipboard::presets::Preset::Holds,
                        false,
                    );
                    continue;
                }
                Action::SelectAllRolls => {
                    self.command_selection_preset(
                        deadsync_editor::event_clipboard::presets::Preset::Rolls,
                        false,
                    );
                    continue;
                }
                Action::SelectAllHoldsAndRolls => {
                    self.command_selection_preset(
                        deadsync_editor::event_clipboard::presets::Preset::HoldsAndRolls,
                        false,
                    );
                    continue;
                }
                Action::SelectAllMiscEvents => {
                    self.command_selection_preset(
                        deadsync_editor::event_clipboard::presets::Preset::Miscellaneous,
                        false,
                    );
                    continue;
                }
                Action::SelectAll => {
                    self.command_selection_preset(
                        deadsync_editor::event_clipboard::presets::Preset::NotesAndMiscellaneous,
                        false,
                    );
                    continue;
                }
                Action::SelectAllPatterns => {
                    self.command_selection_preset(
                        deadsync_editor::event_clipboard::presets::Preset::Patterns,
                        false,
                    );
                    continue;
                }
                Action::SelectAllCurrentPlayerNotes => {
                    self.command_selection_preset(
                        deadsync_editor::event_clipboard::presets::Preset::Notes,
                        true,
                    );
                    continue;
                }
                Action::SelectAllCurrentPlayerTaps => {
                    self.command_selection_preset(
                        deadsync_editor::event_clipboard::presets::Preset::Taps,
                        true,
                    );
                    continue;
                }
                Action::SelectAllCurrentPlayerMines => {
                    self.command_selection_preset(
                        deadsync_editor::event_clipboard::presets::Preset::Mines,
                        true,
                    );
                    continue;
                }
                Action::SelectAllCurrentPlayerFakes => {
                    self.command_selection_preset(
                        deadsync_editor::event_clipboard::presets::Preset::Fakes,
                        true,
                    );
                    continue;
                }
                Action::SelectAllCurrentPlayerLifts => {
                    self.command_selection_preset(
                        deadsync_editor::event_clipboard::presets::Preset::Lifts,
                        true,
                    );
                    continue;
                }
                Action::SelectAllCurrentPlayerHolds => {
                    self.command_selection_preset(
                        deadsync_editor::event_clipboard::presets::Preset::Holds,
                        true,
                    );
                    continue;
                }
                Action::SelectAllCurrentPlayerRolls => {
                    self.command_selection_preset(
                        deadsync_editor::event_clipboard::presets::Preset::Rolls,
                        true,
                    );
                    continue;
                }
                Action::SelectAllCurrentPlayerHoldsAndRolls => {
                    self.command_selection_preset(
                        deadsync_editor::event_clipboard::presets::Preset::HoldsAndRolls,
                        true,
                    );
                    continue;
                }
                Action::SnapToNone => {
                    self.command(Command::SetSnap(9));
                    continue;
                }
                Action::SnapToQuarters => {
                    self.command(Command::SetSnap(0));
                    continue;
                }
                Action::SnapToEighths => {
                    self.command(Command::SetSnap(1));
                    continue;
                }
                Action::SnapToTwelfths => {
                    self.command(Command::SetSnap(2));
                    continue;
                }
                Action::SnapToSixteenths => {
                    self.command(Command::SetSnap(3));
                    continue;
                }
                Action::SnapToTwentyFourths => {
                    self.command(Command::SetSnap(4));
                    continue;
                }
                Action::SnapToThirtySeconds => {
                    self.command(Command::SetSnap(5));
                    continue;
                }
                Action::SnapToFortyEighths => {
                    self.command(Command::SetSnap(6));
                    continue;
                }
                Action::SnapToSixtyFourths => {
                    self.command(Command::SetSnap(7));
                    continue;
                }
                Action::SnapToOneHundredNinetySeconds => {
                    self.command(Command::SetSnap(8));
                    continue;
                }
                Action::SetPlayer1 => {
                    self.command(Command::SelectPlayer(0u32.min(self.workspace.grid().players().saturating_sub(1)),
                    ));
                    continue;
                }
                Action::SetPlayer2 => {
                    self.command(Command::SelectPlayer(1u32.min(self.workspace.grid().players().saturating_sub(1)),
                    ));
                    continue;
                }
                Action::SetPlayer3 => {
                    self.command(Command::SelectPlayer(2u32.min(self.workspace.grid().players().saturating_sub(1)),
                    ));
                    continue;
                }
                Action::SetPlayer4 => {
                    self.command(Command::SelectPlayer(3u32.min(self.workspace.grid().players().saturating_sub(1)),
                    ));
                    continue;
                }
                Action::RegenerateAllPatternsFixedSeeds => {
                    self.command_pattern_action_review(
                        deadsync_editor::pattern_actions::Action::GenerateFixed,
                        false,
                    );
                    continue;
                }
                Action::RegenerateAllPatternsNewSeeds => {
                    self.command_pattern_action_review(
                        deadsync_editor::pattern_actions::Action::GenerateNewSeeds,
                        false,
                    );
                    continue;
                }
                Action::RegenerateSelectedPatternsFixedSeeds => {
                    self.command_pattern_action_review(
                        deadsync_editor::pattern_actions::Action::GenerateFixed,
                        true,
                    );
                    continue;
                }
                Action::RegenerateSelectedPatternsNewSeeds => {
                    self.command_pattern_action_review(
                        deadsync_editor::pattern_actions::Action::GenerateNewSeeds,
                        true,
                    );
                    continue;
                }
                Action::Arrow0
                | Action::Arrow1
                | Action::Arrow2
                | Action::Arrow3
                | Action::Arrow4
                | Action::Arrow5
                | Action::Arrow6
                | Action::Arrow7
                | Action::Arrow8
                | Action::Arrow9
                | Action::ArrowModification
                => unreachable!("lane entry dispatched above"),
                Action::MoveToPreviousRowWithSteps => {
                    self.command_source_event_navigation(
                        deadsync_editor::navigation::Command::PreviousStep,
                    );
                    continue;
                }
                Action::MoveToNextRowWithSteps => {
                    self.command_source_event_navigation(
                        deadsync_editor::navigation::Command::NextStep,
                    );
                    continue;
                }
                Action::MoveToPreviousRowWithEvent => {
                    self.command_source_event_navigation(
                        deadsync_editor::navigation::Command::PreviousEvent,
                    );
                    continue;
                }
                Action::MoveToNextRowWithEvent => {
                    self.command_source_event_navigation(
                        deadsync_editor::navigation::Command::NextEvent,
                    );
                    continue;
                }
                Action::MoveToStartOfStream => {
                    self.command_source_event_navigation(
                        deadsync_editor::navigation::Command::StreamStart,
                    );
                    continue;
                }
                Action::MoveToEndOfStream => {
                    self.command_source_event_navigation(
                        deadsync_editor::navigation::Command::StreamEnd,
                    );
                    continue;
                }
                Action::MoveToNextPattern => {
                    self.command_source_pattern_navigation(true);
                    continue;
                }
                Action::MoveToPreviousPattern => {
                    self.command_source_pattern_navigation(false);
                    continue;
                }

                Action::CancelGoBack => {
                    self.command_cancel_go_back();
                    continue;
                }
                Action::NavigateNextPattern | Action::NavigatePreviousPattern => {
                    self.stop_audio();
                    self.navigate_native_marker(action == Action::NavigateNextPattern);
                    continue;
                }
                Action::ToggleMinimapFocus => {
                    self.command_toggle_minimap_focus();
                    continue;
                }
                Action::OpenMinimapSettings => {
                    self.open_minimap_settings();
                    continue;
                }
                Action::SavePackFile => {self.command_save_pack_file();continue;}
                Action::Exit => {self.request_application_exit();continue;}
                Action::Close => {self.command_file_close();continue;}
                Action::CloseEditor => {
                    self.stop_audio();
                    self.command_request_close();
                    break;
                }
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
                Action::ConvertSelectedWarpsToNegativeStops => {self.stop_audio();self.command_selected_timing_conversion(deadsync_editor::timing_edit::conversion::Direction::WarpsToNegativeStops,
                    );continue;}
                Action::ConvertSelectedNegativeStopsToWarps => {self.stop_audio();self.command_selected_timing_conversion(deadsync_editor::timing_edit::conversion::Direction::NegativeStopsToWarps,
                    );continue;}
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
                Action::AutoApplyAllSongAssets | Action::AutoApplyUnsetSongAssets => {
                    self.command_auto_song_assets(action == Action::AutoApplyUnsetSongAssets);
                    continue;
                }
                Action::ShiftEarlier | Action::ShiftLater => {
                    self.stop_audio();
                    self.command_checked_shift_review(action == Action::ShiftLater);
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

                Action::Undo | Action::Redo if self.workspace.file().session.history_changes_chart_event_copy(action==Action::Redo) => {
                    self.apply_chart_event_copy_history(action==Action::Redo);
                    continue;
                }
                Action::Undo | Action::Redo if self.workspace.file().session.history_preserves_song_properties_metadata(action==Action::Redo) => {
                    self.apply_song_properties_metadata_history(action==Action::Redo);
                    continue;
                }
                Action::Undo | Action::Redo if self.workspace.file().session.history_preserves_marker_metadata(action==Action::Redo) => {
                    self.apply_marker_metadata_history(action==Action::Redo);
                    continue;
                }
                Action::Undo | Action::Redo if self.workspace.file().session.history_changes_song_properties_live(action==Action::Redo) => {
                    self.apply_live_song_properties_history(action==Action::Redo);
                    continue;
                }
                Action::Undo | Action::Redo if self.workspace.file().session.history_changes_timing_edit(action==Action::Redo) => {
                    self.apply_live_timing_history(action==Action::Redo);
                    continue;
                }
                Action::Undo | Action::Redo if self.workspace.file().session.history_preserves_timing_migration(action==Action::Redo) => {
                    self.apply_timing_migration_history(action==Action::Redo);
                    continue;
                }
                Action::Save => Command::Save,
                Action::Undo => Command::Undo,
                Action::Redo => Command::Redo,
                Action::Copy if self.command_bindings.preferences.source_order=>{self.copy_source_region_selection(false);continue;}
                Action::Copy => Command::Copy,
                Action::Cut if self.command_bindings.preferences.source_order=>{self.copy_source_region_selection(true);continue;}
                Action::Cut => Command::Cut,
                Action::Paste if self.command_bindings.preferences.source_order=>{self.paste_selected_events();continue;}
                Action::Paste => Command::Paste,
                Action::Delete if self.command_bindings.preferences.source_order=>{self.delete_source_event_selection();continue;}
                Action::Delete => Command::Delete,
                Action::PreviousMeasure => {self.command_source_measure_navigation(true);continue;}
                Action::NextMeasure => {self.command_source_measure_navigation(false);continue;}
                Action::SelectRowRange => {self.toggle_source_row_selection();continue;}
                Action::TogglePlayer => Command::SelectPlayer(
                    (self.workspace.player() + 1) % self.workspace.grid().players(),
                ),
                Action::ChartStart => {self.command_source_chart_edge(false);continue;}
                Action::ChartEnd => {self.command_source_chart_edge(true);continue;}
                Action::PreviousLabel => {self.command_source_label_navigation(true);continue;}
                Action::NextLabel => {self.command_source_label_navigation(false);continue;}
                Action::DecreaseSnap | Action::IncreaseSnap => {
                    self.cycle_snap(action == Action::IncreaseSnap);
                    continue;
                }
                Action::PreviousChart | Action::NextChart => {
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
                    self.lane_entry_release_input();
                    self.panel = Some(if !self.has_song(){super::Panel::FileMenu(0)}else if self.workspace.has_chart() {
                        super::Panel::Menu(0)
                    } else {
                        super::Panel::EmptyMenu(0)
                    });
                    continue;
                }
                Action::OpenAssistant => { self.open_assistant(); continue; }
                Action::OpenAnalysis => {
                    self.open_analysis();
                    continue;
                }
                Action::OpenSongProperties => {
                    self.open_song_properties();
                    continue;
                }
                Action::OpenTiming => {
                    self.open_timing();
                    continue;
                }
                Action::OpenPatternSet => {
                    self.open_pattern_set();
                    continue;
                }
                Action::OpenBatch => {
                    self.open_batch();
                    continue;
                }
                Action::OpenChartProperties => {
                    self.open_chart_properties();
                    continue;
                }
                Action::OpenMarker => {
                    self.open_marker();
                    continue;
                }
                Action::RegeneratePattern => {
                    self.stop_audio();
                    self.regenerate_saved_pattern();
                    continue;
                }
                Action::OpenPattern => {
                    self.open_pattern();
                    continue;
                }
                Action::OpenGeneration => {
                    self.open_generation();
                    continue;
                }
                Action::NewChart => {
                    self.open_new_chart();
                    continue;
                }
                Action::DeleteChart => {
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
                    self.open_event_selection();
                    continue;
                }
                Action::OpenChartEventCopy => {
                    self.open_chart_event_copy();
                    continue;
                }
                Action::InspectAttack => {
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
                Action::ToggleNoteEntryMode | Action::ToggleAutoAdvance => {
                    self.workspace.toggle_note_entry_mode();
                    self.publish_note_entry_mode();
                    self.status = crate::i18n::tr("Edition", if self.workspace.auto_advance() {"NoteEntryAdvanceBySnap"} else {"NoteEntryNormal"},
                    );
                    self.refresh();
                    continue;
                }
                Action::MoveEventPreview | Action::MoveEventEndHint => {
                    self.move_source_song_marker(action == Action::MoveEventEndHint);
                    continue;
                }
                Action::ToggleSpacingMode => {
                    self.cycle_source_spacing_mode();
                    self.refresh();
                    continue;
                }
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
                    self.open_note_attributes();
                    continue;
                }
                Action::OpenNoteConversion => {
                    self.command_open_note_conversion();
                    continue;
                }
                Action::OpenNoteOwners => {
                    self.command_open_note_owners();
                    continue;
                }
                Action::OpenTransforms => {
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
                Action::ToggleAssistTick=>{self.toggle_source_tick(false);break;}
                Action::ToggleBeatTick=>{self.toggle_source_tick(true);break;}
                Action::TogglePreview => { self.command_toggle_sample_audio(); break; }
                Action::ToggleDark => { self.toggle_dark(); break; }
                Action::OpenDarkSettings => { self.open_dark_settings(); break; }
                Action::OpenContainingFolder => {
                    self.command_open_containing_folder();
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
            let chart_history=matches!(command,Command::Undo|Command::Redo)
                && (self.workspace.file().session.history_changes_chart_set(matches!(command,Command::Redo))
                    || self.workspace.file().session.chart_properties_history_chart(matches!(command,Command::Redo)).is_some()
                    || self.workspace.file().session.history_changes_pack_properties(matches!(command,Command::Redo)));
            // Application preferences own no chart content. Validate their
            // transaction before publication without pausing an unrelated
            // transport, including when external preferences cause a conflict.
            if !chart_history && !self.is_application_history_command(command) { self.stop_audio(); }
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
        self.command_bindings_sync_context(event.timestamp,self.command_bindings.fresh_press);
        if !self.command_bindings_allowed() {
            self.system_alert_blocked_key(event);
            self.lane_entry_release_input();
            self.command_bindings.suspend();
            return false;
        }
        let keys = self
            .command_bindings
            .snapshot(self.control_keys, self.shift_keys);
        let limited_context = self.panel.is_some() || self.minimap_focused();
        let final_key = name(event.code);
        let consumes = final_key.as_ref().is_some_and(|key| {
            self.command_bindings
                .preferences
                .commands
                .iter()
                .filter(|(id, _)| {
                    !limited_context
                        || Action::from_id(id)
                            .is_some_and(|action| self.command_binding_context_allows(action))
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
        if limited_context {
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
        self.command_bindings_sync_context(now,None);
        if !self.command_bindings_allowed() {
            self.lane_entry_release_input();
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
    #[test]
    fn event_picker_preserves_global_file_history_commands_without_enabling_note_entry() {
        for action in [Action::Save, Action::Undo, Action::Redo] {
            assert!(Context::Selection.allows(action));
        }
        assert!(!Context::Selection.allows(Action::Arrow0));
        assert!(!Context::Selection.allows(Action::OpenMenu));
    }
    #[test]
    fn marker_history_commands_preserve_pending_draft_values() {
        use deadsync_editor::generation::{
            marker_edit::Draft,
            metadata::{ChartData, Definition},
        };
        let (directory, original) = lane11_fixture();
        let source = original.workspace.file().session.serialize().unwrap();
        let document = deadsync_editor::document::Document::parse(
            source,
            deadsync_editor::document::Format::Ssc,
        )
        .unwrap();
        let mut session = deadsync_editor::history::Session::new(document);
        let mut data = ChartData::existing_or_initial(session.document(), 0).unwrap();
        data.patterns = Some(std::collections::BTreeMap::from([(
            0,
            Definition::default(),
        )]));
        data.save(&mut session, 0).unwrap();
        let path = directory.path().join("marker-history.ssc");
        std::fs::write(&path, session.serialize().unwrap()).unwrap();
        let mut state = State::open(&path, 0).unwrap();
        state.marker = Some(Draft::at_cursor(&state.workspace.file().session, 0, 0).unwrap());
        state.panel = Some(super::super::Panel::Marker(0));
        for action in [Action::Save, Action::Undo, Action::Redo] {
            assert!(state.command_binding_context_allows(action));
        }
        state.marker.as_mut().unwrap().definition.random_seed += 1;
        for action in [Action::Save, Action::Undo, Action::Redo] {
            assert!(!state.command_binding_context_allows(action));
        }
        assert!(!Context::Marker.allows(Action::Arrow0));
    }
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
        let prefs = source_profile::import(&Preferences::default(), r#"{"New":[["LeftControl","F13"]]}"#, Platform::Other,
        ).unwrap().preferences;
        // Hypothetical future marker fixture: no existing source callback is
        // falsely classified as unavailable and unknown persisted IDs stay inert.
        let blockers:Vec<_>=["F13","F14"].into_iter().enumerate().map(|(index,key)|Binding{id:format!("{}FutureCallback/{index}",source_profile::BLOCKER_PREFIX),alternatives:vec![keys(&["LeftControl",key])],repeat:false,blocks:true,
            }).collect();
        let mut runtime = Runtime::default();
        runtime.registry = Registry::new(native_bindings(&prefs).into_iter().chain(blockers.clone()));
        runtime.source_blockers = blockers;
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
            r#"{"New":[["F1"]]}"#,
            Platform::Other,
        )
        .unwrap()
        .preferences;
        state.configure_key_binding_preferences(Arc::new(prefs));
        let blocker=Binding{id:format!("{}FutureCallback/0",source_profile::BLOCKER_PREFIX),alternatives:vec![keys(&["F13"])],repeat:false,blocks:true,
        };
        state.command_bindings.registry=Registry::new(native_bindings(&state.command_bindings.preferences).into_iter().chain([blocker.clone()]),
        );
        state.command_bindings.source_blockers=vec![blocker];
        let event = RawKeyboardEvent {
            code: KeyCode::F13,
            pressed: true,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        };
        state.command_bindings.observe(&event);
        assert!(state.command_bindings_key(&event));
        assert!(state.status.contains("FutureCallback"));
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
    fn preset_uses_semantic_lane_callbacks_and_preserves_menu_rescue() {
        use deadsync_editor::key_binding_preferences::source_profile::{self, Platform};
        let prefs = source_profile::preset(&Preferences::default(), Platform::Other)
            .unwrap()
            .preferences;
        let mut runtime = Runtime::default();
        runtime.registry = native_registry(&prefs);
        runtime.source_blockers = source_profile::blockers(&prefs);
        runtime.preferences = Arc::new(prefs);
        // Configured lanes are real callbacks; no digit reservation or unavailable
        // marker may steal their source default. F1 remains a rescue callback.
        for chord in [vec!["D1"], vec!["LeftShift", "D2"], vec!["F1"]] {
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
    fn physical_lane_keys_keep_left_and_right_in_their_chart_columns() {
        use deadsync_editor::notes::{Kind, Position};
        for (code, lane) in [(KeyCode::Digit1, 0), (KeyCode::Digit4, 3)] {
            let (_dir, mut state) = reviewed_test_state();
            state.command(Command::Seek(48));
            for pressed in [true, false] {
                state.raw_key(&RawKeyboardEvent {
                    code, pressed, repeat: false,
                    timestamp: Instant::now(), host_nanos: 0,
                });
            }
            for column in 0..4 {
                let note = state.workspace.grid().get(Position { row: 48, lane: column, player: 0,
                });
                if column == lane {
                    assert_eq!(note.unwrap().kind, Kind::Tap);
                } else {
                    assert!(note.is_none(), "key {code:?} unexpectedly wrote column {column}");
                }
            }
        }
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
            if action.id()=="AddEventPattern" {
                assert!(matches!(state.panel,Some(super::super::Panel::Marker(0))));
                assert_eq!(state.marker.as_ref().unwrap().start,48);
            } else {
                assert!(state.panel.is_none(), "{}: {}", action.id(), state.status);
            }
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
                assert_eq!(
                    state.event_selection.as_ref().map(|settings| settings.selection.selected_events().collect::<Vec<_>>()),
                    Some(vec![Event::Note(Position { lane: 3, ..target })]),
                    "source mirror selects its transformed notes"
                );
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
                assert_eq!(
                    state.event_selection.as_ref().map(|settings| settings.selection.selected_events().collect::<Vec<_>>()),
                    Some(vec![Event::Note(target)]),
                    "source conversion selects its transformed notes"
                );
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
            {
                assert_eq!(
                    state.event_selection.as_ref().map(|settings| settings.selection.selected_events().collect::<Vec<_>>()),
                    Some(vec![Event::Note(target)]),
                    "Undo restores the transformed originals as the selection"
                );
                state.command(Command::Redo);
                let transformed = if action == Action::Mirror { Position { lane: 3, ..target } } else { target };
                assert_eq!(state.workspace.grid().get(transformed).unwrap().kind,
                    if action == Action::Mirror { Kind::Tap } else { Kind::Mine });
                assert_eq!(
                    state.event_selection.as_ref().map(|settings| settings.selection.selected_events().collect::<Vec<_>>()),
                    Some(vec![Event::Note(transformed)])
                );
            }
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
    #[test]
    fn contextual_pattern_navigation_uses_configured_keys_and_source_repeat_without_global_leakage()
    {
        let (_dir, mut state) = reviewed_test_state();
        let mut prefs = Preferences::default();
        prefs
            .commands
            .insert("NavigateNextPattern".into(), vec![vec!["F13".into()]]);
        state.configure_key_binding_preferences(Arc::new(prefs));
        let marker = state
            .workspace
            .prepare_quick_add(deadsync_editor::quick_add::Kind::Pattern)
            .unwrap();
        state.workspace.apply_quick_add(&marker).unwrap();
        state.open_marker();
        assert!(matches!(state.panel, Some(super::super::Panel::Marker(_))));
        assert!(state.command_bindings_allowed());
        assert!(state.command_binding_context_allows(Action::NavigateNextPattern));
        assert!(!state.command_binding_context_allows(Action::Delete));
        assert_eq!(
            state.command_bindings.evaluate(0., keys(&["F13"])),
            vec![Effect::Press("NavigateNextPattern".into())]
        );
        assert!(
            state
                .command_bindings
                .evaluate(0.5, keys(&["F13"]))
                .is_empty()
        );
        assert_eq!(
            state.command_bindings.evaluate(0.501, keys(&["F13"])),
            vec![Effect::Repeat("NavigateNextPattern".into())]
        );
        state.command_bindings.release_input();
        assert!(
            !state
                .command_bindings
                .effects
                .is_active("NavigateNextPattern")
        );
    }
    #[test]
    fn minimap_focus_and_settings_accept_rebindings_while_focused_and_old_keys_do_not_reactivate() {
        let (_dir, mut state) = reviewed_test_state();
        let mut prefs = Preferences::default();
        prefs
            .commands
            .insert("ToggleMinimapFocus".into(), vec![vec!["F13".into()]]);
        prefs
            .commands
            .insert("OpenMinimapSettings".into(), vec![vec!["F14".into()]]);
        state.configure_key_binding_preferences(Arc::new(prefs));
        fn press(s: &mut State, code: KeyCode) {
            for pressed in [true, false] {
                s.raw_key(&RawKeyboardEvent {
                    code,
                    pressed,
                    repeat: false,
                    timestamp: Instant::now(),
                    host_nanos: 0,
                });
            }
        }
        press(&mut state, KeyCode::F13);
        assert!(state.minimap_focused());
        assert!(!state.command_binding_context_allows(Action::Delete));
        press(&mut state, KeyCode::F13);
        assert!(!state.minimap_focused());
        press(&mut state, KeyCode::F13);
        press(&mut state, KeyCode::F14);
        assert!(matches!(
            state.panel,
            Some(super::super::Panel::MinimapSettings(0))
        ));
        assert!(!state.minimap_focused());
        state.minimap_settings_key(0, KeyCode::Escape);
        state.raw_key(&RawKeyboardEvent {
            code: KeyCode::ControlLeft,
            pressed: true,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
        press(&mut state, KeyCode::KeyM);
        assert!(!state.minimap_focused());
    }
    #[test]
    fn configurable_close_propagates_safe_host_decision_and_old_keys_do_not_close() {
        let (_dir, mut state) = reviewed_test_state();
        let mut prefs = Preferences::default();
        prefs
            .commands
            .insert("CloseEditor".into(), vec![vec!["F13".into()]]);
        state.configure_key_binding_preferences(Arc::new(prefs));
        fn event(code: KeyCode, pressed: bool) -> RawKeyboardEvent {
            RawKeyboardEvent {
                code,
                pressed,
                repeat: false,
                timestamp: Instant::now(),
                host_nanos: 0,
            }
        }
        assert!(!state.raw_key(&event(KeyCode::Escape, true)));
        state.raw_key(&event(KeyCode::Escape, false));
        assert!(!state.raw_key(&event(KeyCode::F2, true)));
        state.raw_key(&event(KeyCode::F2, false));
        assert!(state.raw_key(&event(KeyCode::F13, true)));
        assert!(
            !state.take_file_close(),
            "Host close intent must be consumed exactly once"
        );
        let (_dir, mut dirty) = reviewed_test_state();
        dirty.configure_key_binding_preferences(state.command_bindings.preferences.clone());
        dirty.command(Command::Place(2));
        assert!(!dirty.raw_key(&event(KeyCode::F13, true)));
        assert!(matches!(
            dirty.panel,
            Some(super::super::Panel::FileUnsaved(_))
        ));
    }
    #[test]
    fn cancelling_a_lane_draft_with_the_configured_close_key_is_quarantined_before_timer_update() {
        let (_dir, mut state) = reviewed_test_state();
        let mut prefs = Preferences::default();
        prefs
            .commands
            .insert("CloseEditor".into(), vec![vec!["F13".into()]]);
        state.configure_key_binding_preferences(Arc::new(prefs));
        state.command(Command::Seek(48));
        fn event(code: KeyCode, pressed: bool) -> RawKeyboardEvent {
            RawKeyboardEvent {
                code,
                pressed,
                repeat: false,
                timestamp: Instant::now(),
                host_nanos: 0,
            }
        }
        state.raw_key(&event(KeyCode::Digit1, true));
        assert!(state.lane_input.model.is_some());
        assert!(!state.raw_key(&event(KeyCode::F13, true)));
        assert!(state.command_bindings.quarantine.contains(&KeyCode::F13));
        state.command_bindings_update(Instant::now());
        assert!(!state.take_file_close());
        assert!(state.panel.is_none());
        state.raw_key(&event(KeyCode::F13, false));
        state.raw_key(&event(KeyCode::Digit1, false));
        // Close is available again only after all physical ownership is released.
        assert!(!state.command_bindings.quarantine.contains(&KeyCode::F13));
    }
    #[test]
    fn source_cancel_go_back_clears_selection_without_closing_editor_or_editing_bytes() {
        use deadsync_editor::key_binding_preferences::source_profile::{self, Platform};
        let (_dir, mut state) = reviewed_test_state();
        let prefs = source_profile::preset(&Preferences::default(), Platform::Other)
            .unwrap()
            .preferences;
        state.configure_key_binding_preferences(Arc::new(prefs));
        state.command(Command::ToggleSelection);
        state.command(Command::Seek(48));
        assert!(state.workspace.selection().is_some());
        let before = reviewed_bytes(&state);
        assert!(!state.raw_key(&RawKeyboardEvent {
            code: KeyCode::Escape,
            pressed: true,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0
        }));
        assert!(state.workspace.selection().is_none());
        assert!(state.panel.is_none());
        assert!(!state.take_file_close());
        assert_eq!(reviewed_bytes(&state), before);
    }
    #[test]
    fn source_cancel_clears_pending_region_before_reopening_selection_picker() {
        use deadsync_editor::key_binding_preferences::source_profile::{self, Platform};
        for rebound in [false,true] {
            let (_dir,mut state)=reviewed_test_state();
            let mut prefs=source_profile::preset(&Preferences::default(),Platform::Other).unwrap().preferences;
            if rebound {prefs.commands.insert("CancelGoBack".into(),vec![vec!["F13".into()]]);}
            state.configure_key_binding_preferences(Arc::new(prefs));
            state.open_event_selection();state.open_region_selection();
            assert!(state.region_selection.is_some());assert!(state.event_selection.is_some());
            let before=reviewed_bytes(&state);let undo=state.workspace.file().session.undo_label().map(str::to_owned);
            for pressed in [true,false] {state.raw_key(&RawKeyboardEvent{code:if rebound{KeyCode::F13}else{KeyCode::Escape},pressed,repeat:false,timestamp:Instant::now(),host_nanos:0,
                });}
            assert!(state.region_selection.is_none(),"a configured cancel must discard the pending region");
            assert!(state.event_selection.is_none());assert!(state.panel.is_none());
            assert!(!state.take_file_close());assert_eq!(before,reviewed_bytes(&state));assert_eq!(undo,state.workspace.file().session.undo_label().map(str::to_owned));
            state.open_event_selection();assert!(state.region_selection.is_none(),"opening the picker must not resurrect the cancelled gesture");
        }
    }
    #[test]
    fn source_delete_requires_selection_and_uses_selected_events_not_cursor_row() {
        use deadsync_editor::{event_clipboard::Event,key_binding_preferences::source_profile::{self,Platform},
        };
        let (_dir,mut state)=reviewed_test_state();
        state.configure_key_binding_preferences(Arc::new(source_profile::preset(&Preferences::default(),Platform::Other).unwrap().preferences,
        ));
        let before=reviewed_bytes(&state);let token=deadsync_editor::history::file_intent::IntentGuard::capture(&state.workspace.file().session,
        );
        let press=|state:&mut State|{for pressed in [true,false]{state.raw_key(&RawKeyboardEvent{code:KeyCode::Delete,pressed,repeat:false,timestamp:Instant::now(),host_nanos:0,
                });}};
        press(&mut state);assert_eq!(before,reviewed_bytes(&state));assert!(token.is_current(&state.workspace.file().session));assert!(state.workspace.file().session.undo_label().is_none());
        let mut selection=state.workspace.event_selection_between(0,192).unwrap();selection.select_all(false);
        let selected=selection.events().iter().copied().find(|event|matches!(event,Event::Note(_))).expect("fixture note");
        let index=selection.events().iter().position(|event|*event==selected).unwrap();selection.set_selected(index,true).unwrap();
        state.replace_region_event_selection(selection);state.navigate_exact_position(384.,true);let cursor=state.capture_song_property_presentation().unwrap().row();
        press(&mut state);assert_ne!(before,reviewed_bytes(&state));assert_eq!(state.workspace.grid().iter().count(),1,"delete only the checked note, not its chord peer");assert_eq!(state.capture_song_property_presentation().unwrap().row(),cursor);
        state.command(Command::Undo);assert_eq!(before,reviewed_bytes(&state));
    }
    fn source39_fixture(routine: bool) -> (tempfile::TempDir, State) {
        use deadsync_editor::{
            document::{Document, Format},
            generation::metadata::{ChartData, Definition, PATTERN_BALANCED, SIXTEENTHS},
            history::Session,
        };
        crate::i18n::init_for_tests();
        let notes = if routine {
            "10000000\n0M000000\n00F00000\n000L0000\n&\n10000000\n0M000000\n00F00000\n000L0000"
        } else {
            "1000\n0M00\n00F0\n000L"
        };
        let text = format!(
            "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:{};#NOTES:{notes};",
            if routine {
                "dance-routine"
            } else {
                "dance-single"
            }
        );
        let mut session = Session::new(Document::parse(text.as_bytes(), Format::Ssc).unwrap());
        ChartData {
            patterns: Some(
                [0, 48, 96]
                    .into_iter()
                    .map(|row| {
                        (
                            row,
                            Definition {
                                length: 24,
                                pattern_config_guid: SIXTEENTHS.parse().unwrap(),
                                performed_chart_config_guid: PATTERN_BALANCED.parse().unwrap(),
                                ..Default::default()
                            },
                        )
                    })
                    .collect(),
            ),
            ..Default::default()
        }
        .save(&mut session, 0)
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("source39.ssc");
        std::fs::write(&path, session.serialize().unwrap()).unwrap();
        let state = State::open(&path, 0).unwrap();
        (dir, state)
    }
    fn source39_chosen(
        state: &State,
    ) -> std::collections::BTreeSet<deadsync_editor::event_clipboard::Event> {
        let selection = &state.event_selection.as_ref().unwrap().selection;
        selection
            .events()
            .iter()
            .enumerate()
            .filter(|(index, _)| selection.is_selected(*index))
            .map(|(_, event)| *event)
            .collect()
    }
    #[test]
    fn all_nineteen_source_presets_target_the_exact_kind_and_current_owner_across_the_chart() {
        use deadsync_editor::{event_clipboard::Event, notes::Kind};
        for action in Action::ALL
            .into_iter()
            .filter(|action| action.is_selection_preset())
        {
            let (dir, _) = source39_fixture(true);
            let path = dir.path().join("source39.ssc");
            let text = std::fs::read_to_string(&path).unwrap();
            assert_eq!(text.matches("000L0000").count(), 2);
            // Both owners have a hold and a roll in a later measure. Empty
            // sustain selections cannot detect swapped Hold/Roll callbacks.
            std::fs::write(&path, text.replace(
                "000L0000",
                "000L0000,\n20004000\n00000000\n00000000\n30003000",
            ),
            ).unwrap();
            let mut state = State::open(&path, 0).unwrap();
            state.command(Command::SelectPlayer(1));
            let before = reviewed_bytes(&state);
            state.command_binding_dispatch(vec![Effect::Press(action.id().into())], Instant::now());
            assert!(
                matches!(state.panel, Some(super::super::Panel::EventSelection)),
                "{}: {}",
                action.id(),
                state.status
            );
            let selection = &state.event_selection.as_ref().unwrap().selection;
            let current = action.id().starts_with("SelectAllCurrentPlayer");
            let expected:std::collections::BTreeSet<_>=selection.events().iter().copied().filter(|event| {
                if current && !matches!(event,Event::Note(position) if position.player==1) {return false;}
                match action {
                    Action::SelectAll=>true,
                    Action::SelectAllMiscEvents=>!matches!(event,Event::Note(_)),
                    Action::SelectAllPatterns=>matches!(event,Event::Pattern{..}),
                    Action::SelectAllNotes|Action::SelectAllCurrentPlayerNotes=>matches!(event,Event::Note(_)),
                    Action::SelectAllTaps|Action::SelectAllCurrentPlayerTaps=>matches!(event,Event::Note(position) if state.workspace.grid().get(*position).unwrap().kind==Kind::Tap),
                    Action::SelectAllMines|Action::SelectAllCurrentPlayerMines=>matches!(event,Event::Note(position) if state.workspace.grid().get(*position).unwrap().kind==Kind::Mine),
                    Action::SelectAllFakes|Action::SelectAllCurrentPlayerFakes=>matches!(event,Event::Note(position) if state.workspace.grid().get(*position).unwrap().kind==Kind::Fake),
                    Action::SelectAllLifts|Action::SelectAllCurrentPlayerLifts=>matches!(event,Event::Note(position) if state.workspace.grid().get(*position).unwrap().kind==Kind::Lift),
                    Action::SelectAllHolds|Action::SelectAllCurrentPlayerHolds=>matches!(event,Event::Note(position) if state.workspace.grid().get(*position).unwrap().kind==Kind::Hold),
                    Action::SelectAllRolls|Action::SelectAllCurrentPlayerRolls=>matches!(event,Event::Note(position) if state.workspace.grid().get(*position).unwrap().kind==Kind::Roll),
                    Action::SelectAllHoldsAndRolls|Action::SelectAllCurrentPlayerHoldsAndRolls=>matches!(event,Event::Note(position) if matches!(state.workspace.grid().get(*position).unwrap().kind,Kind::Hold|Kind::Roll)),
                    _=>panic!("unexpected preset"),
                }
            }).collect();
            if action.id().contains("Holds") || action.id().contains("Rolls") {
                let kinds = if action.id().contains("HoldsAndRolls") { 2 } else { 1 };
                assert_eq!(expected.len(), kinds * if current { 1 } else { 2 }, "{}", action.id());
            }
            assert_eq!(source39_chosen(&state), expected, "{}", action.id());
            assert_eq!(reviewed_bytes(&state), before);
        }
    }
    #[test]
    fn source_presets_add_to_checked_choices_and_never_implicitly_select_the_cursor_row() {
        use deadsync_editor::{event_clipboard::Event, notes::Kind};
        let (_dir, mut state) = source39_fixture(true);
        let before = reviewed_bytes(&state);
        state.command_selection_preset(
            deadsync_editor::event_clipboard::presets::Preset::Mines,
            false,
        );
        let mines = source39_chosen(&state);
        assert_eq!(mines.len(), 2);
        assert!(mines.iter().all(|event|matches!(event,Event::Note(position) if state.workspace.grid().get(*position).unwrap().kind==Kind::Mine)));
        state.command(Command::SelectPlayer(1));
        state.command_selection_preset(
            deadsync_editor::event_clipboard::presets::Preset::Taps,
            true,
        );
        let expanded = source39_chosen(&state);
        assert!(mines.is_subset(&expanded));
        assert_eq!(expanded.len(), 3);
        assert!(
            expanded.contains(&Event::Note(deadsync_editor::notes::Position {
                row: 0,
                lane: 0,
                player: 1
            }))
        );
        assert!(
            !expanded.contains(&Event::Note(deadsync_editor::notes::Position {
                row: 0,
                lane: 0,
                player: 0
            }))
        );
        assert_eq!(reviewed_bytes(&state), before);
    }
    #[test]
    fn source_player3_and4_clamp_and_clear_peer_cache_even_when_solo_is_already_player1() {
        for action in [Action::SetPlayer3, Action::SetPlayer4] {
            crate::i18n::init_for_tests();
            let root=tempfile::tempdir().unwrap();let path=root.path().join("owners.ssc");
            std::fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:00000000\n&\n00000000;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;").unwrap();
            let mut state=State::open(&path,0).unwrap();
            state.command_binding_dispatch(vec![Effect::Press(Action::SetPlayer2.id().into())],Instant::now(),
            );
            assert_eq!(state.workspace.player(),1);
            let music=state.chart_focus_time().unwrap();assert!(state.focus_chart_at_time(1,music));assert_eq!(state.workspace.player(),0);
            let before=reviewed_bytes(&state);let revision=state.workspace.file().session.revision();
            state.command_binding_dispatch(vec![Effect::Press(action.id().into())],Instant::now());
            assert_eq!(state.workspace.player(),0);assert_eq!(state.status,crate::i18n::tr("Edition","Ready"));
            assert_eq!(reviewed_bytes(&state),before);assert_eq!(state.workspace.file().session.revision(),revision);
            assert!(state.lane_entry_model().is_none_or(|model|model.drafts().next().is_none()));
            assert!(state.focus_chart_at_time(0,music));assert_eq!(state.workspace.player(),0,"{} must clear prior routine player2",action.id());
            assert_eq!(reviewed_bytes(&state),before);
        }
    }
    #[test]
    fn source_snap_setters_do_not_seek_and_explicit_players_never_wrap_invalid_owners() {
        let (_dir, mut state) = source39_fixture(true);
        state.command(Command::Seek(48));
        state.focus_position = deadsync_editor::chart_focus::Position::at_fractional_row(
            &state.workspace.file().session,
            state.workspace.chart(),
            48.5,
        )
        .ok();
        let before = reviewed_bytes(&state);
        for (action, index) in [
            (Action::SnapToNone, 9),
            (Action::SnapToQuarters, 0),
            (Action::SnapToEighths, 1),
            (Action::SnapToTwelfths, 2),
            (Action::SnapToSixteenths, 3),
            (Action::SnapToTwentyFourths, 4),
            (Action::SnapToThirtySeconds, 5),
            (Action::SnapToFortyEighths, 6),
            (Action::SnapToSixtyFourths, 7),
            (Action::SnapToOneHundredNinetySeconds, 8),
        ] {
            state.command_binding_dispatch(vec![Effect::Press(action.id().into())], Instant::now());
            assert_eq!(state.workspace.snap(), index, "{}", action.id());
            assert_eq!(state.workspace.row(), 48);
            assert_eq!(state.focus_position.as_ref().unwrap().row(), 48.5);
        }
        for (action, expected) in [
            (Action::SetPlayer1, 0),
            (Action::SetPlayer2, 1),
            (Action::SetPlayer3, 1),
            (Action::SetPlayer4, 1),
        ] {
            state.command_binding_dispatch(vec![Effect::Press(action.id().into())], Instant::now());
            assert_eq!(state.workspace.player(), expected, "{}", action.id());
        }
        assert_eq!(reviewed_bytes(&state), before);
    }
    #[test]
    fn source_pattern_navigation_uses_fractional_position_and_opens_the_exact_marker_inspector() {
        let (_dir, mut state) = source39_fixture(false);
        state.command(Command::Seek(48));
        state.focus_position = deadsync_editor::chart_focus::Position::at_fractional_row(
            &state.workspace.file().session,
            state.workspace.chart(),
            48.5,
        )
        .ok();
        let before = reviewed_bytes(&state);
        state.command_binding_dispatch(
            vec![Effect::Press("MoveToPreviousPattern".into())],
            Instant::now(),
        );
        assert_eq!(state.workspace.row(), 48);
        assert!(matches!(state.panel, Some(super::super::Panel::Marker(_))));
        state.command_binding_dispatch(
            vec![Effect::Press("MoveToNextPattern".into())],
            Instant::now(),
        );
        assert_eq!(state.workspace.row(), 96);
        state.command_binding_dispatch(
            vec![Effect::Press("MoveToNextPattern".into())],
            Instant::now(),
        );
        assert_eq!(state.workspace.row(), 0);
        assert_eq!(reviewed_bytes(&state), before);
    }
    #[test]
    fn four_source_regenerate_callbacks_prepare_exact_scope_and_seed_mode_without_committing_seeds()
    {
        use deadsync_editor::event_clipboard::Event;
        for (action, checked, new_seeds) in [
            (Action::RegenerateAllPatternsFixedSeeds, false, false),
            (Action::RegenerateAllPatternsNewSeeds, false, true),
            (Action::RegenerateSelectedPatternsFixedSeeds, true, false),
            (Action::RegenerateSelectedPatternsNewSeeds, true, true),
        ] {
            let (_dir, mut state) = source39_fixture(false);
            if checked {
                state.open_event_selection();
                let selection = &mut state.event_selection.as_mut().unwrap().selection;
                selection.select_all(false);
                let index = selection
                    .events()
                    .iter()
                    .position(|event| matches!(event, Event::Pattern { row: 0 }))
                    .unwrap();
                selection.set_selected(index, true).unwrap();
            }
            let before = reviewed_bytes(&state);
            state.command_binding_dispatch(vec![Effect::Press(action.id().into())], Instant::now());
            assert!(
                matches!(state.panel, Some(super::super::Panel::PatternActions(2))),
                "{}: {}",
                action.id(),
                state.status
            );
            assert_eq!(
                state.status,
                crate::i18n::tr("Edition", "PatternActionsPrepared"),
                "{}",
                action.id()
            );
            assert!(
                state
                    .pattern_actions_label(0, "PatternActionsKind")
                    .contains(
                        crate::i18n::tr(
                            "Edition",
                            if new_seeds {
                                "PatternActionsNew"
                            } else {
                                "PatternActionsFixed"
                            }
                        )
                        .as_ref()
                    )
            );
            assert!(
                state
                    .pattern_actions_label(1, "PatternActionsScope")
                    .contains(
                        crate::i18n::tr(
                            "Edition",
                            if checked {
                                "PatternActionsChecked"
                            } else {
                                "PatternActionsAll"
                            }
                        )
                        .as_ref()
                    )
            );
            assert_eq!(
                state.pattern_actions_label(2, "PatternActionsPrepare"),
                crate::i18n::tr_fmt(
                    "Edition",
                    "PatternActionsPrepare",
                    &[("count", if checked { "1" } else { "3" })]
                )
            );
            assert_eq!(
                reviewed_bytes(&state),
                before,
                "{} committed before Apply",
                action.id()
            );
            assert!(state.job.is_none());
        }
    }
    #[test]
    fn missing_pattern_selection_and_stale_preset_expansion_refuse_without_replacing_choices() {
        let (_dir, mut state) = source39_fixture(false);
        let before = reviewed_bytes(&state);
        state.command_binding_dispatch(
            vec![Effect::Press("RegenerateSelectedPatternsFixedSeeds".into())],
            Instant::now(),
        );
        assert!(state.panel.is_none());
        assert_eq!(
            state.status,
            crate::i18n::tr("Edition", "PatternActionsNoSelection")
        );
        assert_eq!(reviewed_bytes(&state), before);
        state.open_event_selection();
        let chosen = source39_chosen(&state);
        state.workspace.execute(Command::Place(3)).unwrap();
        let edited = reviewed_bytes(&state);
        state.command_selection_preset(
            deadsync_editor::event_clipboard::presets::Preset::Mines,
            false,
        );
        assert_eq!(
            state.status,
            crate::i18n::tr("Edition", "SelectionPresetsFailed")
        );
        assert_eq!(source39_chosen(&state), chosen);
        assert!(matches!(
            state.panel,
            Some(super::super::Panel::EventSelection)
        ));
        assert_eq!(reviewed_bytes(&state), edited);
    }
    #[test]
    fn source_selection_shortcuts_dispatch_once_in_both_languages_from_global_and_checked_contexts()
    {
        use deadsync_editor::{
            event_clipboard::Event,
            key_binding_preferences::source_profile::{self, Platform},
        };
        for locale in ["en", "fr"] {
            let (_dir, mut state) = source39_fixture(true);
            crate::i18n::init(deadsync_assets::language::load_for_tests(locale));
            state.configure_key_binding_preferences(Arc::new(
                source_profile::preset(&Preferences::default(), Platform::current())
                    .unwrap()
                    .preferences,
            ));
            let control = if Platform::current() == Platform::Mac {
                KeyCode::SuperLeft
            } else {
                KeyCode::ControlLeft
            };
            fn raw(state: &mut State, code: KeyCode, pressed: bool) {
                state.raw_key(&RawKeyboardEvent {
                    code,
                    pressed,
                    repeat: false,
                    timestamp: Instant::now(),
                    host_nanos: 0,
                });
            }
            for code in [control, KeyCode::KeyA] {
                raw(&mut state, code, true);
            }
            for code in [KeyCode::KeyA, control] {
                raw(&mut state, code, false);
            }
            let notes = source39_chosen(&state);
            assert_eq!(notes.len(), 8);
            assert!(notes.iter().all(|event| matches!(event, Event::Note(_))));
            let before = reviewed_bytes(&state);
            for code in [control, KeyCode::AltLeft, KeyCode::KeyA] {
                raw(&mut state, code, true);
            }
            for code in [KeyCode::KeyA, KeyCode::AltLeft, control] {
                raw(&mut state, code, false);
            }
            let combined = source39_chosen(&state);
            assert!(notes.is_subset(&combined));
            assert_eq!(
                combined.len(),
                state
                    .event_selection
                    .as_ref()
                    .unwrap()
                    .selection
                    .events()
                    .len()
            );
            assert!(matches!(
                state.panel,
                Some(super::super::Panel::EventSelection)
            ));
            assert_ne!(state.status.as_ref(), "SelectionPresetsApplied");
            assert_eq!(reviewed_bytes(&state), before);
        }
    }
    #[test]
    fn six_source_navigation_callbacks_use_complete_fractional_index_without_changing_checked_selection()
     {
        use deadsync_editor::navigation::{Command as Navigation, Index};
        for locale in ["en", "fr"] {
            crate::i18n::init(deadsync_assets::language::load_for_tests(locale));
            for (action, navigation) in [
                (Action::MoveToPreviousRowWithSteps, Navigation::PreviousStep),
                (Action::MoveToNextRowWithSteps, Navigation::NextStep),
                (
                    Action::MoveToPreviousRowWithEvent,
                    Navigation::PreviousEvent,
                ),
                (Action::MoveToNextRowWithEvent, Navigation::NextEvent),
                (Action::MoveToStartOfStream, Navigation::StreamStart),
                (Action::MoveToEndOfStream, Navigation::StreamEnd),
            ] {
                let (_dir, mut state) = source39_fixture(true);
                state.command_selection_preset(
                    deadsync_editor::event_clipboard::presets::Preset::Mines,
                    false,
                );
                let chosen = source39_chosen(&state);
                state.workspace.execute(Command::Seek(48)).unwrap();
                state.focus_position = Some(
                    deadsync_editor::chart_focus::Position::at_fractional_row(
                        &state.workspace.file().session,
                        state.workspace.chart(),
                        48.5,
                    )
                    .unwrap(),
                );
                let before = reviewed_bytes(&state);
                let index =
                    Index::capture(&state.workspace.file().session, state.workspace.chart())
                        .unwrap();
                let expected = index
                    .target(
                        &state.workspace.file().session,
                        state.workspace.chart(),
                        navigation,
                        48.5,
                        None,
                    )
                    .unwrap();
                state.command_binding_dispatch(
                    vec![Effect::Press(action.id().into())],
                    Instant::now(),
                );
                assert!(action.repeats());
                assert_eq!(
                    state.focus_position.as_ref().map(|position| position.row()),
                    expected.or(Some(48.5)),
                    "{} {locale}",
                    action.id()
                );
                assert_eq!(source39_chosen(&state), chosen);
                assert_eq!(reviewed_bytes(&state), before);
                assert!(matches!(
                    state.panel,
                    Some(super::super::Panel::EventSelection)
                ));
                assert!(state.command_bindings.navigation.is_some());
                assert_ne!(
                    crate::i18n::tr("Edition", &format!("KeyBindingAction{}", action.id()))
                        .as_ref(),
                    format!("KeyBindingAction{}", action.id())
                );
            }
        }
    }
    #[test]
    fn source_event_navigation_preserves_fractional_widgets_range_and_no_wrap() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("navigation.ssc");
        std::fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#SAMPLESTART:0.13;#LASTSECONDHINT:0.33;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0000\n0100\n0000;").unwrap();
        let mut state = State::open(&path, 0).unwrap();
        state.workspace.execute(Command::Seek(48)).unwrap();
        state.workspace.execute(Command::ToggleSelection).unwrap();
        state.workspace.execute(Command::Seek(96)).unwrap();
        let range = state.workspace.selection();
        let before = reviewed_bytes(&state);
        state.workspace.seek_preserving_selection(1).unwrap();
        state.focus_position = Some(
            deadsync_editor::chart_focus::Position::at_fractional_row(
                &state.workspace.file().session,
                state.workspace.chart(),
                1.0,
            )
            .unwrap(),
        );
        assert!(state.focus_position.as_ref().unwrap().is_current(&state.workspace));
        let index = deadsync_editor::navigation::Index::capture(&state.workspace.file().session, state.workspace.chart(),
        ).expect("complete initialized event index");
        let selected_rows = state.workspace.selection().map(|range| range.0 as f64);
        let target = index.target(&state.workspace.file().session, state.workspace.chart(), deadsync_editor::navigation::Command::NextEvent, 1.0, selected_rows,
            ).unwrap().unwrap();
        assert!((target - 12.48).abs() < 1e-10, "indexed target {target}");
        assert!(state.command_bindings_allowed(), "panel open: {}; status: {}", state.panel.is_some(), state.status);
        assert!(!state.minimap_focused(), "unexpected minimap ownership");
        assert!(state.command_binding_dispatch(
            vec![Effect::Press("MoveToNextRowWithEvent".into())],
            Instant::now(),
        ), "dispatch refused: {}", state.status);
        let exact = state.focus_position.as_ref().unwrap().row();
        assert!((exact - 12.48).abs() < 1e-10, "focus {exact}; target {target}; status {}", state.status);
        assert_eq!(state.workspace.selection(), range);
        state.command_binding_dispatch(
            vec![Effect::Press("MoveToNextRowWithEvent".into())],
            Instant::now(),
        );
        let exact = state.focus_position.as_ref().unwrap().row();
        assert!((exact - 31.68).abs() < 1e-10, "{exact}");
        state.command_binding_dispatch(
            vec![Effect::Press("MoveToNextRowWithEvent".into())],
            Instant::now(),
        );
        assert_eq!(state.focus_position.as_ref().unwrap().row(), 48.0); // SelectedRows widget
        assert_eq!(state.workspace.selection(), range);
        state.command_binding_dispatch(
            vec![Effect::Press("MoveToPreviousRowWithSteps".into())],
            Instant::now(),
        );
        assert_eq!(state.workspace.row(), 0);
        state.command_binding_dispatch(
            vec![Effect::Press("MoveToPreviousRowWithSteps".into())],
            Instant::now(),
        );
        assert_eq!(state.workspace.row(), 0); // does not wrap to96
        assert_eq!(state.workspace.selection(), range);
        assert_eq!(reviewed_bytes(&state), before);
    }
    #[test]
    fn source_stream_shortcuts_route_physical_control_in_both_languages() {
        use deadsync_editor::key_binding_preferences::source_profile::{self, Platform};
        for locale in ["en", "fr"] {
            crate::i18n::init(deadsync_assets::language::load_for_tests(locale));
            for (code, expected) in [(KeyCode::ArrowUp, 0.0), (KeyCode::ArrowDown, 96.0)] {
                let (_dir, mut state) = source39_fixture(true);
                state.configure_key_binding_preferences(Arc::new(
                    source_profile::preset(&Preferences::default(), Platform::current())
                        .unwrap()
                        .preferences,
                ));
                state.workspace.execute(Command::Seek(48)).unwrap();
                state.focus_position = Some(
                    deadsync_editor::chart_focus::Position::at_fractional_row(
                        &state.workspace.file().session,
                        state.workspace.chart(),
                        48.5,
                    )
                    .unwrap(),
                );
                let before = reviewed_bytes(&state);
                let control = if Platform::current() == Platform::Mac {
                    KeyCode::SuperLeft
                } else {
                    KeyCode::ControlLeft
                };
                for (code, pressed) in [
                    (control, true),
                    (code, true),
                    (code, false),
                    (control, false),
                ] {
                    state.raw_key(&RawKeyboardEvent {
                        code,
                        pressed,
                        repeat: false,
                        timestamp: Instant::now(),
                        host_nanos: 0,
                    });
                }
                assert_eq!(
                    state.focus_position.as_ref().unwrap().row(),
                    expected,
                    "{locale} {code:?}"
                );
                assert_eq!(reviewed_bytes(&state), before);
            }
        }
    }
    fn lane11_fixture() -> (tempfile::TempDir,State) {
        crate::i18n::init_for_tests();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("lane.ssc");
        std::fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000\n0000\n0000\n0000;").unwrap();
        (dir,State::open(&path,0).unwrap())
    }
    #[test]
    fn snap_none_raw_keys_move_the_real_chart_projection_with_signed_fractional_phase() {
        use deadsync_editor::zoom::SpacingMode;
        for mode in [SpacingMode::ConstantTime,SpacingMode::ConstantRow] {
            for reverse in [false,true] {for start in [-12.75,400.5] {
                crate::i18n::init_for_tests();
                deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(854.,480.),
                    );
                let root=tempfile::tempdir().unwrap();let path=root.path().join("asymmetric.ssc");
                let notes=(0..12).map(|measure|["1000","0100","0010","0001"][measure%4]).collect::<Vec<_>>().join(",");
                std::fs::write(&path,format!("#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:{notes};")).unwrap();
                let mut state=State::open(&path,0).unwrap();
                multi_configure(&mut state,&[(Action::DecreaseSnap,&["Left"]),(Action::IncreaseSnap,&["Right"]),(Action::MoveUp,&["Up"]),(Action::MoveDown,&["Down"]),
                        ],
                    );
                state.spacing_mode=mode;state.reverse_display=reverse;state.navigate_exact_position(start,true);
                state.command(Command::SetSnap(0));
                let press=|s:&mut State,key|{lane11_raw(s,key,true);lane11_raw(s,key,false);};
                press(&mut state,KeyCode::ArrowLeft);assert_eq!(state.workspace.snap(),9);
                press(&mut state,KeyCode::ArrowRight);assert_eq!(state.workspace.snap(),0);
                press(&mut state,KeyCode::ArrowLeft);assert_eq!(state.workspace.snap(),9);
                let before=reviewed_bytes(&state);let viewport=state.chart_viewport();
                let y=state.display_projection(viewport.top,viewport.bottom,viewport.cursor).unwrap().note_y(432).unwrap();
                let time=state.capture_song_property_presentation().unwrap().music_time();
                let mini=state.minimap_viewport_geometry().unwrap().projection.editor_start;
                let toolbar=state.toolbar.clone();
                let sprite_positions=|s:&State|{let mut actors=vec![];s.actors(&mut actors);actors.into_iter().filter_map(|a|match a {deadlib_present::actors::Actor::Sprite{offset,z:8004,..}=>Some(offset),_=>None,
                            }).collect::<Vec<_>>()};
                let notes_before=sprite_positions(&state);
                press(&mut state,KeyCode::ArrowUp);
                let expected=start+if reverse{192.}else{-192.};
                assert_eq!(state.focus_position.as_ref().unwrap().row(),expected);
                let moved=state.display_projection(viewport.top,viewport.bottom,viewport.cursor).unwrap().note_y(432).unwrap();
                assert_ne!(moved,y);assert!(moved>y,"Up moves a fixed chart note down the screen");
                assert_ne!(state.capture_song_property_presentation().unwrap().music_time(),time);
                assert_ne!(state.minimap_viewport_geometry().unwrap().projection.editor_start,mini);
                assert_ne!(state.toolbar,toolbar);
                assert_ne!(sprite_positions(&state),notes_before,"asymmetric actual notes change on screen");
                press(&mut state,KeyCode::ArrowDown);assert_eq!(state.focus_position.as_ref().unwrap().row(),start);
                assert_eq!(state.display_projection(viewport.top,viewport.bottom,viewport.cursor).unwrap().note_y(432).unwrap(),y);
                assert_eq!(reviewed_bytes(&state),before);assert!(state.workspace.file().session.undo_label().is_none());
            }}
        }
    }
    fn lane11_raw(state:&mut State,code:KeyCode,pressed:bool){state.raw_key(&RawKeyboardEvent{code,pressed,repeat:false,timestamp:Instant::now(),host_nanos:0,
        });}
    fn lane11_configure(state:&mut State, lane_keys:Vec<Vec<&str>>, modifier:Vec<Vec<&str>>) {
        let mut preferences=Preferences::default();
        preferences.commands.insert("Arrow0".into(),lane_keys.into_iter().map(|keys|keys.into_iter().map(str::to_owned).collect()).collect(),
        );
        preferences.commands.insert("ArrowModification".into(),modifier.into_iter().map(|keys|keys.into_iter().map(str::to_owned).collect()).collect(),
        );
        preferences.validate().unwrap();state.configure_key_binding_preferences(Arc::new(preferences));
    }
    fn lane11_note(state:&State,lane:u16)->Option<deadsync_editor::notes::Note>{state.workspace.grid().get(deadsync_editor::notes::Position{player:0,lane,row:0,
            }).cloned()}
    #[test]
    fn configured_lane_and_modifier_callbacks_extend_hold_change_roll_and_commit_only_on_release() {
        use deadsync_editor::notes::Kind;
        for locale in ["en","fr"] {
            let (_dir,mut state)=lane11_fixture();crate::i18n::init(deadsync_assets::language::load_for_tests(locale));
            lane11_configure(&mut state,vec![vec!["F13"]],vec![vec!["F14"]]);
            for action in [Action::Arrow0,Action::ArrowModification] {
                let key=format!("KeyBindingAction{}",action.id());
                assert_ne!(crate::i18n::tr("Edition",&key).as_ref(),key);
                assert!(state.command_binding_label(action).contains(if action==Action::Arrow0{"F13"}else{"F14"}));
            }
            let before=reviewed_bytes(&state);lane11_raw(&mut state,KeyCode::F13,true);
            assert_eq!(state.lane_entry_model().unwrap().drafts().next().unwrap().kind,Kind::Tap);
            assert_eq!(reviewed_bytes(&state),before);
            state.command(Command::Seek(96));state.lane_entry_reconcile();
            assert_eq!(state.lane_entry_model().unwrap().drafts().next().unwrap().kind,Kind::Hold);
            lane11_raw(&mut state,KeyCode::F14,true);
            assert_eq!(state.lane_entry_model().unwrap().drafts().next().unwrap().kind,Kind::Roll);
            lane11_raw(&mut state,KeyCode::F14,false);
            assert_eq!(state.lane_entry_model().unwrap().drafts().next().unwrap().kind,Kind::Hold);
            assert_eq!(reviewed_bytes(&state),before);
            lane11_raw(&mut state,KeyCode::F13,false);
            let note=lane11_note(&state,0).unwrap();assert_eq!(note.kind,Kind::Hold);assert_eq!(note.end,Some(96));
            assert_ne!(reviewed_bytes(&state),before);
        }
    }
    #[test]
    fn modifier_only_shift_ownership_survives_individual_side_release() {
        use deadsync_editor::notes::Kind;
        let (_dir,mut state)=lane11_fixture();lane11_configure(&mut state,vec![vec!["F13"]],vec![vec!["LeftShift"]]);
        lane11_raw(&mut state,KeyCode::ShiftLeft,true);lane11_raw(&mut state,KeyCode::ShiftRight,true);lane11_raw(&mut state,KeyCode::F13,true);
        assert_eq!(state.lane_entry_model().unwrap().drafts().next().unwrap().kind,Kind::Mine);
        lane11_raw(&mut state,KeyCode::ShiftLeft,false);
        assert_eq!(state.lane_entry_model().unwrap().drafts().next().unwrap().kind,Kind::Mine);
        lane11_raw(&mut state,KeyCode::ShiftRight,false);
        assert_eq!(state.lane_entry_model().unwrap().drafts().next().unwrap().kind,Kind::Tap);
        lane11_raw(&mut state,KeyCode::F13,false);assert_eq!(lane11_note(&state,0).unwrap().kind,Kind::Tap);
    }
    #[test]
    fn physical_control_and_super_coalesce_lane_release_without_early_commit() {
        let (_dir,mut state)=lane11_fixture();lane11_configure(&mut state,vec![vec!["Ctrl","F13"]],vec![vec!["F14"]]);
        let before=reviewed_bytes(&state);
        for code in [KeyCode::ControlLeft,KeyCode::ControlRight,KeyCode::SuperLeft,KeyCode::SuperRight,KeyCode::F13,
        ]{lane11_raw(&mut state,code,true);}
        for code in [KeyCode::ControlLeft,KeyCode::ControlRight,KeyCode::SuperLeft,
        ]{lane11_raw(&mut state,code,false);assert_eq!(reviewed_bytes(&state),before);assert!(state.lane_entry_model().unwrap().is_held(0));}
        lane11_raw(&mut state,KeyCode::SuperRight,false);assert!(lane11_note(&state,0).is_some());
        let committed=reviewed_bytes(&state);lane11_raw(&mut state,KeyCode::F13,false);assert_eq!(reviewed_bytes(&state),committed);
    }
    #[test]
    fn modal_and_focus_cancellation_never_turn_a_late_release_into_a_commit() {
        for focus in [false,true] {
            let (_dir,mut state)=lane11_fixture();lane11_configure(&mut state,vec![vec!["F13"]],vec![vec!["F14"]]);let before=reviewed_bytes(&state);
            lane11_raw(&mut state,KeyCode::F13,true);state.command(Command::Seek(96));state.lane_entry_reconcile();
            if focus {state.release_input();}else{state.panel=Some(super::super::Panel::Analysis);state.command_bindings_update(Instant::now());state.panel=None;}
            assert!(state.lane_entry_model().is_none());assert_eq!(reviewed_bytes(&state),before);
            state.raw_key(&RawKeyboardEvent{code:KeyCode::F13,pressed:true,repeat:true,timestamp:Instant::now(),host_nanos:0,
            });
            lane11_raw(&mut state,KeyCode::F13,false);assert_eq!(reviewed_bytes(&state),before);
            lane11_raw(&mut state,KeyCode::F13,true);lane11_raw(&mut state,KeyCode::F13,false);assert_ne!(reviewed_bytes(&state),before);
        }
    }
    #[test]
    fn rebinding_a_held_lane_cancels_draft_and_unassigned_digits_have_no_legacy_route() {
        let (_dir,mut state)=lane11_fixture();lane11_configure(&mut state,vec![vec!["F13"]],vec![vec!["F15"]]);let before=reviewed_bytes(&state);
        lane11_raw(&mut state,KeyCode::F13,true);
        lane11_configure(&mut state,vec![vec!["F14"]],vec![vec!["F15"]]);
        lane11_raw(&mut state,KeyCode::F13,false);lane11_raw(&mut state,KeyCode::Digit1,true);lane11_raw(&mut state,KeyCode::Digit1,false);
        assert_eq!(reviewed_bytes(&state),before);assert!(state.lane_entry_model().is_none());
        lane11_raw(&mut state,KeyCode::F14,true);lane11_raw(&mut state,KeyCode::F14,false);assert!(lane11_note(&state,0).is_some());
    }
    #[test]
    fn configured_lane_preserves_explicit_tools_and_os_repeat_never_toggles_twice() {
        use deadsync_editor::notes::Kind;
        let (_dir,mut state)=lane11_fixture();lane11_configure(&mut state,vec![vec!["F13"]],vec![vec!["F14"]]);state.command(Command::SelectTool(Kind::Fake));
        lane11_raw(&mut state,KeyCode::F13,true);assert_eq!(lane11_note(&state,0).unwrap().kind,Kind::Fake);let committed=reviewed_bytes(&state);
        state.raw_key(&RawKeyboardEvent{code:KeyCode::F13,pressed:true,repeat:true,timestamp:Instant::now(),host_nanos:0,
        });lane11_raw(&mut state,KeyCode::F13,false);
        assert_eq!(reviewed_bytes(&state),committed);
    }
    #[test]
    fn two_physical_key_alternatives_own_one_lane_until_the_last_release() {
        let (_dir,mut state)=lane11_fixture();lane11_configure(&mut state,vec![vec!["F13"],vec!["F14"]],vec![vec!["F15"]],
        );let before=reviewed_bytes(&state);
        lane11_raw(&mut state,KeyCode::F13,true);lane11_raw(&mut state,KeyCode::F14,true);lane11_raw(&mut state,KeyCode::F13,false);
        assert_eq!(reviewed_bytes(&state),before);assert!(state.lane_entry_model().unwrap().is_held(0));
        lane11_raw(&mut state,KeyCode::F14,false);assert!(lane11_note(&state,0).is_some());
    }

    #[test]
    fn native_default_digits_keep_tap_modifier_and_control_noop_policies() {
        use deadsync_editor::notes::Kind;
        let (_dir,mut state)=lane11_fixture();let before=reviewed_bytes(&state);
        lane11_raw(&mut state,KeyCode::ControlLeft,true);lane11_raw(&mut state,KeyCode::Digit1,true);lane11_raw(&mut state,KeyCode::Digit1,false);lane11_raw(&mut state,KeyCode::ControlLeft,false);
        assert_eq!(reviewed_bytes(&state),before);assert!(state.lane_entry_model().is_none());
        lane11_raw(&mut state,KeyCode::Digit1,true);assert_eq!(state.lane_entry_model().unwrap().drafts().next().unwrap().kind,Kind::Tap);
        lane11_raw(&mut state,KeyCode::Digit1,false);assert_eq!(lane11_note(&state,0).unwrap().kind,Kind::Tap);
        lane11_raw(&mut state,KeyCode::ShiftRight,true);lane11_raw(&mut state,KeyCode::Digit2,true);lane11_raw(&mut state,KeyCode::Digit2,false);lane11_raw(&mut state,KeyCode::ShiftRight,false);
        assert_eq!(lane11_note(&state,1).unwrap().kind,Kind::Mine);
    }

    #[test]
    fn quarantined_ownership_cannot_start_another_held_lane_when_the_old_key_releases() {
        let (_dir,mut state)=lane11_fixture();lane11_configure(&mut state,vec![vec!["F13"],vec!["F14"]],vec![vec!["F15"]],
        );let before=reviewed_bytes(&state);
        lane11_raw(&mut state,KeyCode::F13,true);state.release_input();
        lane11_raw(&mut state,KeyCode::F14,true);lane11_raw(&mut state,KeyCode::F13,false);
        state.command_bindings_update(Instant::now());assert!(state.lane_entry_model().is_none());
        lane11_raw(&mut state,KeyCode::F14,false);assert_eq!(reviewed_bytes(&state),before);
        lane11_raw(&mut state,KeyCode::F14,true);lane11_raw(&mut state,KeyCode::F14,false);assert!(lane11_note(&state,0).is_some());
    }
    #[test]
    fn immediate_auto_advance_entry_does_not_become_a_hold_on_owned_release() {
        use deadsync_editor::notes::Kind;
        let (_dir,mut state)=lane11_fixture();lane11_configure(&mut state,vec![vec!["F13"]],vec![vec!["F14"]]);state.command(Command::ToggleAutoAdvance);
        lane11_raw(&mut state,KeyCode::F13,true);let before=reviewed_bytes(&state);
        assert!(state.workspace.row()>0);assert_eq!(lane11_note(&state,0).unwrap().kind,Kind::Tap);
        lane11_raw(&mut state,KeyCode::F13,false);assert_eq!(reviewed_bytes(&state),before);assert_eq!(lane11_note(&state,0).unwrap().end,None);
    }

    #[test]
    fn limited_selection_panel_cancels_lane_ownership_even_while_other_shortcuts_remain_available() {
        let (_dir,mut state)=lane11_fixture();lane11_configure(&mut state,vec![vec!["F13"],vec!["F14"]],vec![vec!["F15"]],
        );let before=reviewed_bytes(&state);
        lane11_raw(&mut state,KeyCode::F13,true);state.open_event_selection();
        assert!(state.lane_entry_model().is_none());assert!(matches!(state.panel,Some(super::super::Panel::EventSelection)));
        state.panel=None;lane11_raw(&mut state,KeyCode::F14,true);lane11_raw(&mut state,KeyCode::F13,false);
        assert!(state.lane_entry_model().is_none());lane11_raw(&mut state,KeyCode::F14,false);assert_eq!(reviewed_bytes(&state),before);
        lane11_raw(&mut state,KeyCode::F14,true);lane11_raw(&mut state,KeyCode::F14,false);assert!(lane11_note(&state,0).is_some());
    }

    #[test]
    fn f1_rescue_opens_on_first_press_while_cancelling_a_held_lane_draft() {
        let (_dir,mut state)=lane11_fixture();lane11_configure(&mut state,vec![vec!["F13"]],vec![vec!["F14"]]);let before=reviewed_bytes(&state);
        lane11_raw(&mut state,KeyCode::F13,true);lane11_raw(&mut state,KeyCode::F1,true);
        assert!(state.panel.is_some());assert!(state.lane_entry_model().is_none());assert_eq!(reviewed_bytes(&state),before);
        lane11_raw(&mut state,KeyCode::F1,false);lane11_raw(&mut state,KeyCode::F13,false);assert_eq!(reviewed_bytes(&state),before);
    }

    #[test]
    fn native_vertical_control_guards_preserve_remapped_navigation_and_source_stream_callbacks() {
        for terminal in ["Up","Down"] {
            let mut remapped=Preferences::default();remapped.commands.insert(Action::ZoomIn.id().into(),vec![vec!["F13".into()]]);remapped.commands.insert(Action::ZoomOut.id().into(),vec![vec!["F14".into()]]);
            let mut registry=native_registry(&remapped);registry.update(0.0,keys(&["Ctrl"]));registry.update(0.1,keys(&["Ctrl",terminal]));
            assert!(!registry.is_active(if terminal=="Up"{"MoveUp"}else{"MoveDown"}));assert!(registry.is_active_prefix("__native_reserved_"));
        }
        use deadsync_editor::key_binding_preferences::source_profile::{self,Platform};
        let mut source=source_profile::preset(&Preferences::default(),Platform::Other).unwrap().preferences;
        let mut registry=native_registry(&source);registry.update(0.0,keys(&["Ctrl"]));assert_eq!(registry.update(0.1,keys(&["Ctrl","Up"])),vec![Effect::Press("MoveToStartOfStream".into())]);
        source.commands.insert("MoveToStartOfStream".into(),vec![vec![]]);let mut registry=native_registry(&source);registry.update(0.0,keys(&["Ctrl"]));assert_eq!(registry.update(0.1,keys(&["Ctrl","Up"])),vec![Effect::Press("MoveUp".into())]);
        let mut custom=Preferences::default();custom.commands.insert(Action::ZoomIn.id().into(),vec![vec![]]);custom.commands.insert("MoveUp".into(),vec![vec!["Ctrl".into(),"Up".into()]]);let mut registry=native_registry(&custom);registry.update(0.0,keys(&["Ctrl"]));assert_eq!(registry.update(0.1,keys(&["Ctrl","Up"])),vec![Effect::Press("MoveUp".into())]);
    }
    fn multi_configure(state:&mut State, assignments:&[(Action,&[&str])]) {
        let mut preferences=Preferences::default();for (action,keys) in assignments{preferences.commands.insert(action.id().into(),vec![keys.iter().map(|key|(*key).to_owned()).collect()],
            );}
        preferences.validate().unwrap();state.configure_key_binding_preferences(Arc::new(preferences));
    }
    #[test]
    fn ordinary_prefixes_and_modifier_finals_dispatch_in_en_fr_without_dropping_keys() {
        for locale in ["en","fr"] {
            let (_dir,mut state)=lane11_fixture();crate::i18n::init(deadsync_assets::language::load_for_tests(locale));
            multi_configure(&mut state,&[(Action::SnapToEighths,&["F13","F14"]),(Action::SnapToQuarters,&["F15","RightShift"]),
                ],
            );
            lane11_raw(&mut state,KeyCode::F13,true);assert_eq!(state.workspace.snap(),0);lane11_raw(&mut state,KeyCode::F14,true);assert_eq!(state.workspace.snap(),1);
            lane11_raw(&mut state,KeyCode::F14,false);lane11_raw(&mut state,KeyCode::F13,false);
            lane11_raw(&mut state,KeyCode::F15,true);assert_eq!(state.workspace.snap(),1);lane11_raw(&mut state,KeyCode::ShiftRight,true);assert_eq!(state.workspace.snap(),0);
            lane11_raw(&mut state,KeyCode::ShiftRight,false);lane11_raw(&mut state,KeyCode::F15,false);
        }
    }
    #[test]
    fn configured_navigation_ordinary_prefix_does_not_cancel_a_held_lane() {
        use deadsync_editor::notes::Kind;
        let (_dir,mut state)=lane11_fixture();multi_configure(&mut state,&[(Action::Arrow0,&["F15"]),(Action::ArrowModification,&["F16"]),(Action::MoveDown,&["F13","F14"]),
            ],
        );
        lane11_raw(&mut state,KeyCode::F15,true);lane11_raw(&mut state,KeyCode::F13,true);assert!(state.lane_entry_model().unwrap().is_held(0));
        lane11_raw(&mut state,KeyCode::F14,true);let destination=state.workspace.row();assert!(destination>0);lane11_raw(&mut state,KeyCode::F15,false);
        let note=lane11_note(&state,0).unwrap();assert_eq!(note.kind,Kind::Hold);assert_eq!(note.end,Some(destination));
        lane11_raw(&mut state,KeyCode::F14,false);lane11_raw(&mut state,KeyCode::F13,false);
    }
    #[test]
    fn ordinary_prefix_lane_and_modifier_sets_have_real_release_ownership() {
        use deadsync_editor::notes::Kind;
        let (_dir,mut state)=lane11_fixture();multi_configure(&mut state,&[(Action::Arrow0,&["F13","F14"]),(Action::ArrowModification,&["F15","F16"]),
            ],
        );let before=reviewed_bytes(&state);
        lane11_raw(&mut state,KeyCode::F13,true);assert!(state.lane_entry_model().is_none());lane11_raw(&mut state,KeyCode::F14,true);
        lane11_raw(&mut state,KeyCode::F15,true);assert_eq!(state.lane_entry_model().unwrap().drafts().next().unwrap().kind,Kind::Tap);lane11_raw(&mut state,KeyCode::F16,true);assert_eq!(state.lane_entry_model().unwrap().drafts().next().unwrap().kind,Kind::Mine);
        lane11_raw(&mut state,KeyCode::F15,false);assert_eq!(state.lane_entry_model().unwrap().drafts().next().unwrap().kind,Kind::Tap);assert_eq!(reviewed_bytes(&state),before);
        lane11_raw(&mut state,KeyCode::F13,false);assert_eq!(lane11_note(&state,0).unwrap().kind,Kind::Tap);
        let committed=reviewed_bytes(&state);lane11_raw(&mut state,KeyCode::F14,false);lane11_raw(&mut state,KeyCode::F16,false);assert_eq!(reviewed_bytes(&state),committed);
    }
    #[test]
    fn global_arbitrary_key_sets_cannot_block_allowed_selection_shortcuts() {
        let (_dir,mut state)=lane11_fixture();multi_configure(&mut state,&[(Action::ToggleAutoAdvance,&["F13","F14"]),(Action::SnapToEighths,&["F13"]),
            ],
        );
        state.open_event_selection();lane11_raw(&mut state,KeyCode::F14,true);lane11_raw(&mut state,KeyCode::F13,true);
        assert_eq!(state.workspace.snap(),1);assert!(!state.workspace.auto_advance());assert!(matches!(state.panel,Some(super::super::Panel::EventSelection)));
        assert!(!state.command_bindings.registry.is_active(Action::ToggleAutoAdvance.id()));
    }
    #[test]
    fn context_priming_never_replays_modifier_final_actions_or_their_repeat_timer() {
        let (_dir,mut state)=lane11_fixture();multi_configure(&mut state,&[(Action::MoveDown,&["RightShift"]),(Action::ArrowModification,&["F14"]),
            ],
        );
        lane11_raw(&mut state,KeyCode::ShiftRight,true);let row=state.workspace.row();assert!(row>0);
        state.open_event_selection();state.command_bindings_update(Instant::now());state.panel=None;
        let now=Instant::now();state.command_bindings_update(now);state.command_bindings_update(now+std::time::Duration::from_secs(2));assert_eq!(state.workspace.row(),row);
        lane11_raw(&mut state,KeyCode::ShiftRight,false);lane11_raw(&mut state,KeyCode::ShiftRight,true);assert!(state.workspace.row()>row);
    }
    #[test]
    fn local_selection_controls_and_f1_rescue_survive_global_multi_key_configuration() {
        let (_dir,mut state)=source39_fixture(false);multi_configure(&mut state,&[(Action::ToggleAutoAdvance,&["F13","Down"])]);state.open_event_selection();
        lane11_raw(&mut state,KeyCode::F13,true);lane11_raw(&mut state,KeyCode::ArrowDown,true);lane11_raw(&mut state,KeyCode::ArrowDown,false);lane11_raw(&mut state,KeyCode::F13,false);
        assert!(!state.workspace.auto_advance());assert!(matches!(state.panel,Some(super::super::Panel::EventSelection)));
        state.panel=None;state.command_bindings_update(Instant::now());lane11_raw(&mut state,KeyCode::F1,true);assert!(state.panel.is_some());
    }
    #[test]
    fn cancel_checked_timing_review_keeps_exact_picker_identities_and_delete_removes_only_the_checked_warp() {
        use deadsync_editor::event_clipboard::{Event,TimingKind};
        for locale in ["en","fr"] {
            crate::i18n::init(deadsync_assets::language::load_for_tests(locale));
            let dir=tempfile::tempdir().unwrap();let path=dir.path().join("checked.ssc");
            std::fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#WARPS:1=1,3=1;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
            let mut state=State::open(&path,0).unwrap();
            fn press(state:&mut State,code:KeyCode){lane11_raw(state,code,true);lane11_raw(state,code,false);}
            lane11_raw(&mut state,KeyCode::SuperLeft,true);lane11_raw(&mut state,KeyCode::ShiftLeft,true);press(&mut state,KeyCode::KeyE);lane11_raw(&mut state,KeyCode::ShiftLeft,false);lane11_raw(&mut state,KeyCode::SuperLeft,false);
            press(&mut state,KeyCode::F3);for _ in 0..8{press(&mut state,KeyCode::ArrowRight);}
            press(&mut state,KeyCode::ArrowDown);press(&mut state,KeyCode::ArrowDown);press(&mut state,KeyCode::ArrowRight);press(&mut state,KeyCode::ArrowDown);press(&mut state,KeyCode::Enter);press(&mut state,KeyCode::Space);
            let chosen=source39_chosen(&state);assert!(chosen.contains(&Event::Timing{row:48,kind:TimingKind::Warp}),"{locale}");assert!(!chosen.contains(&Event::Timing{row:144,kind:TimingKind::Warp}),"{locale}");
            let before=reviewed_bytes(&state);press(&mut state,KeyCode::F7);press(&mut state,KeyCode::Escape);
            assert!(matches!(state.panel,Some(super::super::Panel::EventSelection)));assert_eq!(source39_chosen(&state),chosen);assert_eq!(reviewed_bytes(&state),before);
            assert_eq!(state.command_bindings.context,Some(Context::Selection));assert!(state.command_bindings.quarantine.is_empty(),"{locale}");
            press(&mut state,KeyCode::Delete);
            let document=state.workspace.file().session.document();let origin=deadsync_editor::timing_edit::uses_chart_timing(document,0).then_some(0);
            assert_eq!(document.text(origin,"WARPS").as_deref(),Some("3=1"),"{locale}: {}",state.status);
            assert_eq!(document.text(origin,"BPMS").as_deref(),Some("0=120"));
            assert_eq!(document.text(None,"WARPS").as_deref(),Some("1=1,3=1"),"inherited song timing stays available to other charts");
            state.command(Command::Undo);assert_eq!(reviewed_bytes(&state),before);
        }
    }

    #[test]
    fn close_final_without_its_ordinary_prefix_does_not_cancel_a_lane() {
        let (_dir,mut state)=lane11_fixture();multi_configure(&mut state,&[(Action::Arrow0,&["F15"]),(Action::ArrowModification,&["F16"]),(Action::CloseEditor,&["F13","F14"]),
            ],
        );let before=reviewed_bytes(&state);
        lane11_raw(&mut state,KeyCode::F15,true);lane11_raw(&mut state,KeyCode::F14,true);assert!(state.lane_entry_model().unwrap().is_held(0));lane11_raw(&mut state,KeyCode::F14,false);
        lane11_raw(&mut state,KeyCode::F13,true);assert!(state.lane_entry_model().unwrap().is_held(0));lane11_raw(&mut state,KeyCode::F14,true);assert!(state.lane_entry_model().is_none());assert!(!state.take_file_close());assert_eq!(reviewed_bytes(&state),before);
        lane11_raw(&mut state,KeyCode::F14,false);lane11_raw(&mut state,KeyCode::F13,false);lane11_raw(&mut state,KeyCode::F15,false);assert_eq!(reviewed_bytes(&state),before);
    }

    #[test]
    fn source_mode_callbacks_cycle_real_state_without_chart_edits_in_both_languages() {
        use deadsync_editor::{key_binding_preferences::source_profile::{self,Platform},zoom::SpacingMode,
        };
        for locale in ["en","fr"] {
            let (_dir,mut state)=lane11_fixture();crate::i18n::init(deadsync_assets::language::load_for_tests(locale));
            let preferences=source_profile::preset(&Preferences::default(),Platform::current()).unwrap().preferences;state.configure_key_binding_preferences(Arc::new(preferences));
            let before=reviewed_bytes(&state);let row=state.workspace.row();let snap=state.workspace.snap();let revision=state.workspace.file().session.revision();
            for expected in [true,false,true,false] {lane11_raw(&mut state,KeyCode::KeyM,true);assert_eq!(state.workspace.auto_advance(),expected);lane11_raw(&mut state,KeyCode::KeyM,false);}
            state.configure_spacing_preference(deadsync_editor::scroll_preferences::spacing::Preference::from_effective(SpacingMode::ConstantTime,
                ),
            );
            for expected in [SpacingMode::ConstantRow,SpacingMode::Variable,SpacingMode::ConstantTime,
            ] {lane11_raw(&mut state,KeyCode::KeyS,true);assert_eq!(state.spacing_mode(),expected);lane11_raw(&mut state,KeyCode::KeyS,false);assert!(state.display_projection(0.,600.,300.).is_ok());}
            assert_eq!(reviewed_bytes(&state),before);assert_eq!(state.workspace.row(),row);assert_eq!(state.workspace.snap(),snap);assert_eq!(state.workspace.file().session.revision(),revision);
            for action in [Action::ToggleNoteEntryMode,Action::ToggleSpacingMode] {let label=format!("KeyBindingAction{}",action.id());assert_ne!(crate::i18n::tr("Edition",&label).to_string(),label);}
        }
    }
    #[test]
    fn source_mode_changes_preserve_owned_lane_and_next_press_uses_real_advance_policy() {
        use deadsync_editor::notes::Kind;
        let (_dir,mut state)=lane11_fixture();multi_configure(&mut state,&[(Action::Arrow0,&["F13"]),(Action::ArrowModification,&["F14"]),(Action::ToggleNoteEntryMode,&["F15"]),(Action::ToggleSpacingMode,&["F16"]),
            ],
        );
        lane11_raw(&mut state,KeyCode::F13,true);lane11_raw(&mut state,KeyCode::F15,true);lane11_raw(&mut state,KeyCode::F15,false);assert!(state.workspace.auto_advance());assert!(state.lane_entry_model().unwrap().is_held(0));
        lane11_raw(&mut state,KeyCode::F16,true);lane11_raw(&mut state,KeyCode::F16,false);assert!(state.lane_entry_model().unwrap().is_held(0));assert_eq!(state.workspace.row(),0);
        lane11_raw(&mut state,KeyCode::F13,false);assert_eq!(lane11_note(&state,0).unwrap().kind,Kind::Tap);
        // Choose an empty row: editing the existing tap would delete it without advancing.
        state.workspace.execute(Command::Seek(48)).unwrap();lane11_raw(&mut state,KeyCode::F13,true);assert_eq!(state.workspace.row(),96);lane11_raw(&mut state,KeyCode::F13,false);
        assert_eq!(state.workspace.grid().get(deadsync_editor::notes::Position{player:0,lane:0,row:48}).unwrap().kind,Kind::Tap);
    }
    #[test]
    fn note_entry_source_and_native_control_share_one_persisted_preference() {
        use deadsync_editor::{audio_preferences,lane_entry::Mode,zoom::SpacingMode};
        let (dir,mut state)=lane11_fixture();let path=dir.path().join("prefs.json");state.configure_audio_preferences(&path).unwrap();
        multi_configure(&mut state,&[(Action::ToggleNoteEntryMode,&["F13"]),(Action::ToggleSpacingMode,&["F14"]),
            ],
        );
        lane11_raw(&mut state,KeyCode::F13,true);lane11_raw(&mut state,KeyCode::F13,false);assert!(state.workspace.auto_advance());
        lane11_raw(&mut state,KeyCode::KeyA,true);lane11_raw(&mut state,KeyCode::KeyA,false);assert!(!state.workspace.auto_advance());
        lane11_raw(&mut state,KeyCode::F13,true);lane11_raw(&mut state,KeyCode::F13,false);state.configure_spacing_preference(deadsync_editor::scroll_preferences::spacing::Preference::from_effective(SpacingMode::ConstantRow,
            ),
        );lane11_raw(&mut state,KeyCode::F14,true);lane11_raw(&mut state,KeyCode::F14,false);
        state.audio_preferences_file.as_mut().unwrap().save().unwrap();let file=audio_preferences::File::open(&path).unwrap();assert_eq!(file.note_entry_mode(),Mode::AdvanceBySnap);assert_eq!(file.spacing_mode(),SpacingMode::Variable);
        let song=dir.path().join("lane.ssc");let mut fresh=State::open(&song,0).unwrap();fresh.configure_audio_preferences(&path).unwrap();assert!(fresh.workspace.auto_advance());assert_eq!(fresh.spacing_mode(),SpacingMode::Variable);
    }

    #[test]
    fn source_modes_remain_effective_without_a_song_in_en_fr() {
        for locale in ["en","fr"] {
            let (_dir,mut state)=lane11_fixture();crate::i18n::init(deadsync_assets::language::load_for_tests(locale));state.command_file_close();assert!(!state.has_song());
            multi_configure(&mut state,&[(Action::ToggleNoteEntryMode,&["F13"]),(Action::ToggleSpacingMode,&["F14"]),
                ],
            );
            let before=state.spacing_mode();lane11_raw(&mut state,KeyCode::F13,true);lane11_raw(&mut state,KeyCode::F13,false);assert!(state.workspace.auto_advance());
            lane11_raw(&mut state,KeyCode::F14,true);lane11_raw(&mut state,KeyCode::F14,false);assert_eq!(state.spacing_mode(),before.next());assert!(!state.has_song());
        }
    }
    #[test]
    fn source_modes_do_not_stop_or_seek_active_chart_transport() {
        let dir=tempfile::tempdir().unwrap();std::fs::write(dir.path().join("music.wav"),b"command-only fixture").unwrap();let song=dir.path().join("song.ssc");
        std::fs::write(&song,b"#VERSION:0.83;#MUSIC:music.wav;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;").unwrap();let mut state=State::open(&song,0).unwrap();
        multi_configure(&mut state,&[(Action::ToggleNoteEntryMode,&["F13"]),(Action::ToggleSpacingMode,&["F14"]),
            ],
        );state.toggle_audio();assert!(state.is_playing());state.take_audio_commands();let row=state.workspace.row();
        for code in [KeyCode::F13,KeyCode::F14] {lane11_raw(&mut state,code,true);lane11_raw(&mut state,code,false);assert!(state.is_playing());assert_eq!(state.workspace.row(),row);}
        assert!(state.take_audio_commands().is_empty());
    }

    fn marker2_fixture(
        format: deadsync_editor::document::Format,
        music: bool,
    ) -> (tempfile::TempDir, State) {
        use deadsync_editor::document::Format;
        let dir = tempfile::tempdir().unwrap();
        if music {
            std::fs::write(dir.path().join("music.wav"), b"command-only fixture").unwrap();
        }
        let path = dir.path().join(if format == Format::Sm {
            "song.sm"
        } else {
            "song.ssc"
        });
        let text = format!(
            "#TITLE:Keep;#VENDOR:opaque;#BPMS:0=120;#OFFSET:0;#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0}};#SAMPLESTART:1.25;#SAMPLELENGTH:5;#LASTSECONDHINT:2;{}{}",
            if music { "#MUSIC:music.wav;" } else { "" },
            if format == Format::Sm {
                "#NOTES:dance-single::Easy:1::0000;"
            } else {
                "#VERSION:0.83;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Easy;#METER:1;#NOTES:0000;"
            }
        );
        std::fs::write(&path, text).unwrap();
        crate::i18n::init_for_tests();
        let state = State::open(&path, 0).unwrap();
        (dir, state)
    }
    #[test]
    fn source_song_markers_use_fresh_drafts_and_exact_fractional_focus_in_sm_ssc_en_fr() {
        use deadsync_editor::{
            document::Format,
            key_binding_preferences::source_profile::{self, Platform},
            song_properties::Draft,
            song_text::Field,
        };
        for locale in ["en", "fr"] {
            for format in [Format::Sm, Format::Ssc] {
                for (action, code, end) in [
                    (Action::MoveEventPreview, KeyCode::F13, false),
                    (Action::MoveEventEndHint, KeyCode::F14, true),
                ] {
                    let (_dir, mut state) = marker2_fixture(format, false);
                    crate::i18n::init(deadsync_assets::language::load_for_tests(locale));
                    let raw = r#"{"MoveEventPreview":[["F13"]],"MoveEventEndHint":[["F14"]]}"#;
                    let prepared =
                        source_profile::import(&Preferences::default(), raw, Platform::current())
                            .unwrap();
                    assert!(prepared.report.applied.iter().any(|id| id == action.id()));
                    state.configure_key_binding_preferences(Arc::new(prepared.preferences));
                    state.workspace.execute(Command::ToggleSelection).unwrap();
                    state.workspace.execute(Command::Seek(96)).unwrap();
                    state.navigate_exact_position(12.48, true);
                    let range = state.workspace.selection();
                    let row = state.workspace.row();
                    let expected = state.chart_focus_music_time().unwrap();
                    assert!((expected-0.13).abs()<1e-12);
                    let before = reviewed_bytes(&state);
                    let mut unrelated = state.workspace.song_properties().unwrap();
                    unrelated
                        .set_text(Field::Title, "Unapplied inspector edit")
                        .unwrap();
                    state.song_properties = Some(unrelated);
                    assert!(state.panel.is_none());
                    lane11_raw(&mut state, code, true);
                    lane11_raw(&mut state, code, false);
                    let draft = Draft::capture(&state.workspace.file().session).unwrap();
                    assert_eq!(
                        if end {
                            draft.last_second_hint()
                        } else {
                            draft.sample_start()
                        },
                        expected
                    );
                    assert_eq!(draft.sample_length(), 5.0);
                    assert_eq!(draft.text(Field::Title), "Keep");
                    assert_eq!(state.workspace.row(), row);
                    assert_eq!(state.workspace.selection(), range);
                    assert!(state.song_properties.is_none());
                    assert!(
                        state
                            .focus_position
                            .as_ref()
                            .unwrap()
                            .is_current(&state.workspace)
                    );
                    assert!((state.focus_position.as_ref().unwrap().row() - 12.48).abs() < 1e-10);
                    let changed = reviewed_bytes(&state);
                    let revision = state.workspace.file().session.revision();
                    lane11_raw(&mut state, code, true);
                    lane11_raw(&mut state, code, false);
                    assert_eq!(state.workspace.file().session.revision(), revision);
                    assert_eq!(reviewed_bytes(&state), changed);
                    state.command(Command::Undo);
                    assert_eq!(reviewed_bytes(&state), before);
                    state.command(Command::Redo);
                    assert_eq!(reviewed_bytes(&state), changed);
                    let label = format!("KeyBindingAction{}", action.id());
                    assert_ne!(crate::i18n::tr("Edition", &label).to_string(), label);
                }
            }
        }
    }
    #[test]
    fn source_song_markers_capture_playback_time_without_stopping_or_quantizing_it() {
        use deadsync_editor::{document::Format, song_properties::Draft};
        use deadsync_gameplay::GameplayAudioSnapshot;
        for action in [Action::MoveEventPreview, Action::MoveEventEndHint] {
            let (_dir, mut state) = marker2_fixture(Format::Sm, true);
            multi_configure(&mut state, &[(action, &["F13"])]);
            state.toggle_audio();
            assert!(state.is_playing());
            let mut snapshot = GameplayAudioSnapshot::default();
            snapshot.stream_clock.valid_at = Instant::now();
            snapshot.stream_clock.stream_seconds = 1.;
            snapshot.stream_clock.has_music_mapping = true;
            snapshot.stream_clock.music_nanos = 125_000_000;
            snapshot.stream_clock.music_seconds_per_second = 1.;
            state.update_audio(snapshot);
            assert_eq!(state.chart_autoplay_transport(),Some((None,0.125)));
            assert_eq!(state.workspace.row(), 12);
            state.take_audio_commands();
            lane11_raw(&mut state, KeyCode::F13, true);
            lane11_raw(&mut state, KeyCode::F13, false);
            assert!(state.is_playing());
            assert_eq!(state.chart_autoplay_transport(),Some((None,0.125)));
            assert!(state.take_audio_commands().is_empty());
            let draft = Draft::capture(&state.workspace.file().session).unwrap();
            assert_eq!(if action==Action::MoveEventEndHint{draft.last_second_hint()}else{draft.sample_start()},0.125);
            let revision = state.workspace.file().session.revision();
            lane11_raw(&mut state, KeyCode::F13, true);
            lane11_raw(&mut state, KeyCode::F13, false);
            assert_eq!(state.workspace.file().session.revision(), revision);
            assert!(state.is_playing());
            assert!(state.take_audio_commands().is_empty());
        }
    }
    #[test]
    fn source_song_markers_clamp_negative_time_and_refuse_no_song_without_mutation() {
        use deadsync_editor::{document::Format, song_properties::Draft};
        let (_dir, mut state) = marker2_fixture(Format::Sm, false);
        multi_configure(
            &mut state,
            &[
                (Action::MoveEventPreview, &["F13"]),
                (Action::MoveEventEndHint, &["F14"]),
            ],
        );
        state.navigate_exact_position(-12.48, true);
        for code in [KeyCode::F13, KeyCode::F14] {
            lane11_raw(&mut state, code, true);
            lane11_raw(&mut state, code, false);
        }
        let draft = Draft::capture(&state.workspace.file().session).unwrap();
        assert_eq!(draft.sample_start(), 0.);
        assert_eq!(draft.last_second_hint(), 0.);
        assert_eq!(draft.sample_length(), 5.);
        state.command(Command::Undo);
        state.command(Command::Undo);
        state.command_file_close();
        assert!(!state.has_song());
        let before = reviewed_bytes(&state);
        lane11_raw(&mut state, KeyCode::F13, true);
        lane11_raw(&mut state, KeyCode::F13, false);
        assert_eq!(reviewed_bytes(&state), before);
        assert!(!state.has_song());
    }
    #[test]
    fn marker_metadata_revision_cancels_pending_lane_without_committing_on_late_release() {
        use deadsync_editor::document::Format;
        let (_dir, mut state) = marker2_fixture(Format::Sm, false);
        multi_configure(
            &mut state,
            &[
                (Action::Arrow0, &["F13"]),
                (Action::ArrowModification, &["F14"]),
                (Action::MoveEventPreview, &["F15"]),
            ],
        );
        let before = reviewed_bytes(&state);
        lane11_raw(&mut state, KeyCode::F13, true);
        assert!(state.lane_entry_model().unwrap().is_held(0));
        lane11_raw(&mut state, KeyCode::F15, true);
        assert!(state.lane_entry_model().is_none());
        assert!(lane11_note(&state, 0).is_none());
        let changed = reviewed_bytes(&state);
        lane11_raw(&mut state, KeyCode::F15, false);
        lane11_raw(&mut state, KeyCode::F13, false);
        assert_eq!(reviewed_bytes(&state), changed);
        state.command(Command::Undo);
        assert_eq!(reviewed_bytes(&state), before);
    }

    fn publish_marker_audio_fixture(state:&mut State)->u64 {
        let commands=state.take_audio_commands();
        let owners:Vec<_>=commands.iter().filter_map(|c|match c{deadsync_gameplay::GameplayAudioCommand::EditorBeginAudio{owner,..}=> {
                    Some(*owner)
                }
                _=>None,
            }).collect();
        assert_eq!(owners.len(),1);assert_ne!(owners[0],0);
        state.update_audio(deadsync_gameplay::GameplayAudioSnapshot::default());
        assert!(state.take_audio_commands().is_empty());owners[0]
    }
    #[test]
    fn marker_receipt_keeps_the_chosen_fractional_warp_row_at_equal_time() {
        use deadsync_editor::document::Format;
        let (dir,_)=marker2_fixture(Format::Ssc,false);let path=dir.path().join("song.ssc");let text=std::fs::read_to_string(&path).unwrap().replace("#BPMS:0=120;","#BPMS:0=120;#WARPS:1=2;");std::fs::write(&path,text).unwrap();
        for row in [60.5,90.75] {let mut state=State::open(&path,0).unwrap();publish_marker_audio_fixture(&mut state);multi_configure(&mut state,&[(Action::MoveEventPreview,&["F13"])]);state.navigate_exact_position(row,true);let before=state.focus_position.as_ref().unwrap().clone();let edit=state.workspace.row();
            lane11_raw(&mut state,KeyCode::F13,true);lane11_raw(&mut state,KeyCode::F13,false);let after=state.focus_position.as_ref().unwrap();assert!(after.is_current(&state.workspace));assert_eq!(after.row().to_bits(),before.row().to_bits());assert_eq!(after.music_time().to_bits(),before.music_time().to_bits());assert_eq!(after.chart_time().to_bits(),before.chart_time().to_bits());assert_eq!(state.workspace.row(),edit);assert!(state.take_audio_commands().is_empty());}
    }
    #[test]
    fn pause_marker_and_resume_preserve_stop_interior_time_without_seek_or_restart_during_edit() {
        use deadsync_editor::document::Format;use deadsync_gameplay::{GameplayAudioCommand, GameplayAudioSnapshot};
        let (dir,_)=marker2_fixture(Format::Ssc,true);let path=dir.path().join("song.ssc");let text=std::fs::read_to_string(&path).unwrap().replace("#BPMS:0=120;","#BPMS:0=120;#STOPS:1=1;");std::fs::write(&path,text).unwrap();let mut state=State::open(&path,0).unwrap();let owner=publish_marker_audio_fixture(&mut state);multi_configure(&mut state,&[(Action::MoveEventEndHint,&["F13"])]);state.toggle_audio();
        let mut snapshot=GameplayAudioSnapshot::default();snapshot.stream_clock.valid_at=Instant::now();snapshot.stream_clock.stream_seconds=1.0;snapshot.stream_clock.has_music_mapping=true;snapshot.stream_clock.music_nanos=750_000_000;snapshot.stream_clock.music_seconds_per_second=1.0;state.update_audio(snapshot);assert_eq!(state.workspace.row(),48);state.toggle_audio();assert!(!state.is_playing());assert_eq!(state.playback_position(),Some((0.75,48.0)));state.take_audio_commands();
        lane11_raw(&mut state,KeyCode::F13,true);lane11_raw(&mut state,KeyCode::F13,false);assert_eq!(state.playback_position(),Some((0.75,48.0)));assert!(!state.is_playing());assert!(state.take_audio_commands().is_empty());let focus=state.focus_position.as_ref().unwrap();assert!(focus.is_current(&state.workspace));assert_eq!(focus.music_time(),0.75);assert_eq!(focus.row(),48.0);
        state.toggle_audio();assert!(state.is_playing());let commands=state.take_audio_commands();assert!(commands.iter().any(|command|matches!(command,GameplayAudioCommand::EditorResumeMusic{owner:actual,seconds,..}if *actual==owner && *seconds==0.75)));assert!(!commands.iter().any(|command|matches!(command,GameplayAudioCommand::EditorSeekMusic{owner:actual,..} if *actual==owner)));assert_eq!(state.workspace.row(),48);
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

// Insert inside command_bindings.rs. Keep callback recognition `name` unchanged.
fn held_name(code: KeyCode) -> Option<String> {
    let extended = match code {
        KeyCode::Numpad1 => "NumPad1",
        KeyCode::Numpad2 => "NumPad2",
        KeyCode::Numpad3 => "NumPad3",
        KeyCode::Numpad4 => "NumPad4",
        KeyCode::Numpad5 => "NumPad5",
        KeyCode::Numpad6 => "NumPad6",
        KeyCode::Numpad7 => "NumPad7",
        KeyCode::Numpad8 => "NumPad8",
        KeyCode::Numpad9 => "NumPad9",
        KeyCode::NumpadMultiply => "Multiply",
        KeyCode::NumpadAdd => "Add",
        KeyCode::NumpadComma => "Separator",
        KeyCode::NumpadSubtract => "Subtract",
        KeyCode::NumpadDecimal => "Decimal",
        KeyCode::NumpadDivide => "Divide",
        KeyCode::Pause => "Pause",
        KeyCode::PrintScreen => "PrintScreen",
        KeyCode::Help => "Help",
        KeyCode::NumLock => "NumLock",
        KeyCode::ScrollLock => "Scroll",
        KeyCode::AudioVolumeMute => "VolumeMute",
        KeyCode::AudioVolumeDown => "VolumeDown",
        KeyCode::AudioVolumeUp => "VolumeUp",
        KeyCode::MediaTrackNext => "MediaNextTrack",
        KeyCode::MediaTrackPrevious => "MediaPreviousTrack",
        KeyCode::MediaStop => "MediaStop",
        KeyCode::MediaPlayPause => "MediaPlayPause",
        KeyCode::MediaSelect => "SelectMedia",
        KeyCode::Semicolon => "OemSemicolon",
        KeyCode::Equal => "OemPlus",
        KeyCode::Comma => "OemComma",
        KeyCode::Minus => "OemMinus",
        KeyCode::Period => "OemPeriod",
        KeyCode::Backquote => "OemTilde",
        KeyCode::BracketLeft => "OemOpenBrackets",
        KeyCode::Backslash => "OemPipe",
        KeyCode::BracketRight => "OemCloseBrackets",
        KeyCode::Quote => "OemQuotes",
        KeyCode::IntlBackslash => "OemBackslash",
        _ => return name(code),
    };
    Some(extended.into())
}
impl State {
    pub(super) fn scroll_modifier_down(
        &self,
        kind: deadsync_editor::key_binding_preferences::scroll_modifiers::Kind,
    ) -> bool {
        let runtime = &self.command_bindings;
        runtime
            .preferences
            .scroll_modifiers
            .get(kind)
            .is_down(|key| {
                runtime
                    .physical_down
                    .iter()
                    .filter(|code| !runtime.quarantine.contains(code))
                    .any(|code| held_name(*code).as_deref() == Some(key))
            })
    }
}

impl State {
 pub(super) fn wheel_allowed(&self)->bool {self.panel.is_none()&&self.command_bindings_allowed()&&!self.audio_output_busy()&&!self.minimap_focused()}
}

impl State {// Capture owns a separate wait-for-release barrier. Modal suspension
 // quarantines every pressed key for executable callbacks; filtering that
 // quarantine here would suppress every key the user tries to record.
 pub(super) fn scroll_modifier_capture_keys(&self)->Vec<String>{self.command_bindings.physical_down.iter().filter_map(|code|held_name(*code)).collect()}}

#[cfg(test)]mod end_selected_rows_tests{
 use super::*;
 fn press(state:&mut State,code:KeyCode){for pressed in [true,false]{state.raw_key(&deadlib_platform::input::RawKeyboardEvent{code,pressed,repeat:false,timestamp:std::time::Instant::now(),host_nanos:0,
            });}}
 #[test]fn physical_end_reads_live_signed_selected_rows_nearest_end_without_editing_document_en_fr(){
  use deadsync_editor::notes::{Grid, Kind, Note, Position};
  for language in ["en","fr"]{crate::i18n::init_for_tests();crate::i18n::set_locale(deadsync_assets::language::load_for_tests(language));let root=tempfile::tempdir().unwrap();let path=root.path().join("selected-end.ssc");let mut grid=Grid::new(4,1).unwrap();grid.set(Position{row:0,lane:0,player:0,
                },Some(Note{kind:Kind::Hold,end:Some(960),..Note::tap()}),
            ).unwrap();let mut text=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:".to_vec();text.extend(grid.serialize().unwrap());text.push(b';');std::fs::write(&path,text).unwrap();let mut state=State::open(&path,0).unwrap();state.panel=None;let before=state.workspace.file().session.serialize().unwrap();assert_eq!(state.workspace.source_chart_end_position().unwrap(),960.);
  state.navigate_exact_position(400.5,true);press(&mut state,KeyCode::Space);state.navigate_exact_position(451.75,true);assert_eq!(state.workspace.source_row_selection_at(451.75).unwrap(),Some((400,452)));press(&mut state,KeyCode::End);assert_eq!(state.focus_position.as_ref().unwrap().row(),452.);press(&mut state,KeyCode::Space);press(&mut state,KeyCode::End);assert_eq!(state.focus_position.as_ref().unwrap().row(),960.);
  state.navigate_exact_position(-96.75,true);press(&mut state,KeyCode::Space);state.navigate_exact_position(-50.75,true);assert_eq!(state.workspace.source_row_selection_at(-50.75).unwrap(),Some((-96,-50)));state.command_source_event_navigation(deadsync_editor::navigation::Command::PreviousEvent,
            );assert_eq!(state.focus_position.as_ref().unwrap().row(),-96.);press(&mut state,KeyCode::End);assert_eq!(state.focus_position.as_ref().unwrap().row(),960.);assert_eq!(state.workspace.file().session.serialize().unwrap(),before);assert!(state.workspace.file().session.undo_label().is_none());
  }
 }
}

impl State {
    fn system_alert_blocked_key(&mut self, event: &RawKeyboardEvent) {
        if !event.pressed || event.repeat || self.command_bindings.fresh_press != Some(event.code)
            || self.panel.is_some() || self.help || self.wants_text_input() || !self.system_alert_edit_busy() { return; }
        let keys = self.command_bindings.capture_snapshot(self.control_keys,self.shift_keys);
        let final_key = name(event.code);
        let reason = self.command_bindings.preferences.commands.iter().find_map(|(id,alternatives)| {
            let action = Action::from_id(id)?;
            let reason = super::system_alert::blocked_action(action)?;
            alternatives.iter().any(|chord| {
                            !chord.is_empty() && chord.last()==final_key.as_ref()
                && chord.iter().all(|key|keys.contains(key))
                        }).then_some(reason)
        });
        if let Some(reason)=reason { self.queue_system_alert(reason); }
    }
}

impl State {
    /// Read a held query only inside its owning keyboard-adapted gesture. The
    /// executable registry remains suspended/quarantined in a panel; this read
    /// does not activate a callback or reuse callback key reservations.
    pub(super) fn owned_gesture_query(
        &self,
        kind: deadsync_editor::key_binding_preferences::scroll_modifiers::Kind,
    ) -> bool {
        use deadsync_editor::key_binding_preferences::scroll_modifiers::Kind;
        let owned = match kind {
            Kind::SelectionMisc | Kind::SelectionToggle | Kind::SelectionExtend => {
                matches!(self.panel, Some(super::Panel::EventSelection))
            }
            Kind::ReceptorAxis => matches!(self.panel, Some(super::Panel::ReceptorSettings(_))),
            _ => false,
        };
        if !owned
            || self.help
            || self.audio_output_busy()
            || self.file_io_active()
            || self.preference_numeric.is_some()
        {
            return false;
        }
        self.command_bindings
            .preferences
            .scroll_modifiers
            .get(kind)
            .is_down(|key| {
                self.command_bindings
                    .physical_down
                    .iter()
                    .filter(|code| !self.command_bindings.held_query_quarantine.contains(code))
                    .any(|code| held_name(*code).as_deref() == Some(key))
            })
    }
}
