//! Keyboard draft for the source receptor effects. Save is one global undo action.
use super::*;
use deadsync_editor::receptor_preferences::Preferences;
const KEYS: [&str; 18] = [
    "ReceptorHideArrows",
    "ReceptorLightHolds",
    "ReceptorAutoRim",
    "ReceptorAutoGlow",
    "ReceptorAutoShrink",
    "ReceptorTapRim",
    "ReceptorTapShrink",
    "ReceptorPulse",
    "ScrollSettingsDefaults",
    "ScrollSettingsSave",
    "ScrollSettingsCancel",
    "ReceptorCenter",
    "ReceptorPositionX",
    "ReceptorPositionY",
    "ReceptorLockX",
    "ReceptorLockY",
    "ReceptorAllowHeader",
    "ReceptorHeaderMove",
];
fn flag(p: &mut Preferences, index: usize) -> Option<&mut bool> {
    match index {
        0 => Some(&mut p.hide_arrows),
        1 => Some(&mut p.light_holds),
        2 => Some(&mut p.animation.autoplay_rim),
        3 => Some(&mut p.animation.autoplay_glow),
        4 => Some(&mut p.animation.autoplay_shrink),
        5 => Some(&mut p.animation.tap_rim),
        6 => Some(&mut p.animation.tap_shrink),
        7 => Some(&mut p.animation.pulse),
        11 => Some(&mut p.placement.center),
        14 => Some(&mut p.placement.lock_x),
        15 => Some(&mut p.placement.lock_y),
        16 => Some(&mut p.placement.allow_header),
        _ => None,
    }
}
fn enabled(p:Preferences,index:usize)->bool {
    match index {11=>!p.placement.lock_x,12=>!p.placement.lock_x&&!p.placement.center,13=>!p.placement.lock_y,17=>!p.placement.center&&(!p.placement.lock_x||p.placement.allow_header),_=>true}
}
impl State {
    pub(super) fn receptor_control_enabled(&self,index:usize)->bool {enabled(self.receptor_draft.unwrap_or_else(||self.receptor_preferences()),index)}

    pub(super) fn open_receptor_settings(&mut self) {
        self.minimap_release_input();
        self.receptor_views.release_input();
        self.receptor_move=None;
        self.receptor_draft = Some(self.receptor_preferences());
        self.panel = Some(Panel::ReceptorSettings(0));
        self.status = i18n::tr("Edition", "ReceptorSettingsHelp");
    }
    pub(super) fn sync_receptor_preferences_history(&mut self) {
        if let Some(update) = self.workspace.take_receptor_preferences_update() {
            let mut preferences=update.preferences;
            if update.file.is_none(){let p=&mut preferences.placement;let current=self.receptor_preferences().placement;for(i,(target,value))in [(&mut p.x,current.x),(&mut p.y,current.y)].into_iter().enumerate(){if !update.placement_changed[i]{*target=value;}}for(i,(target,value))in [(&mut p.center,current.center),(&mut p.lock_x,current.lock_x),(&mut p.lock_y,current.lock_y),(&mut p.allow_header,current.allow_header)].into_iter().enumerate(){if !update.placement_changed[i+2]{*target=value;}}
                if let Some(file)=self.source_preferences_import.memory.as_mut(){file.set_receptors(preferences);}
            }
            self.configure_receptor_preferences(preferences);
            if let Some(file) = update.file {self.publish_application_preferences_owner(file);}
        }
    }
    pub(super) fn receptor_settings_key(&mut self, index: usize, code: KeyCode) {
        if self.receptor_movement_key(index,code){return;}
        let enter = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter);
        if code == KeyCode::Escape || (index == 10 && enter) {
            self.receptor_draft = None;
            self.panel = None;
            self.status = i18n::tr("Edition", "ScrollSettingsDiscarded");
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let back = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            self.panel = Some(Panel::ReceptorSettings(
                (index + if back { KEYS.len() - 1 } else { 1 }) % KEYS.len(),
            ));
            return;
        }
        if !self.receptor_control_enabled(index) && (enter||matches!(code,KeyCode::ArrowLeft|KeyCode::ArrowRight|KeyCode::Home)) {
            self.status=i18n::tr("Edition","ReceptorControlUnavailable");return;
        }
        if matches!(index, 12 | 13 | 17)
            && matches!(
                code,
                KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Home
            )
        {
            if let Some(draft) = self.receptor_draft.as_mut() {
                let p = &mut draft.placement;
                let step = if self.shift { 10 } else { 1 };
                let delta = if code == KeyCode::ArrowLeft {
                    -step
                } else {
                    step
                };
                let reset = code == KeyCode::Home;
                if index == 13 {
                    if !p.lock_y {
                        if reset {
                            p.y = 110
                        } else {
                            p.move_y(delta);
                        }
                    }
                } else if !p.center && (!p.lock_x || index == 17 && p.allow_header) {
                    if reset {
                        p.x = 842
                    } else {
                        p.move_x(delta, index == 17);
                    }
                }
            }
            return;
        }
        if (index < 8 || matches!(index, 11 | 14 | 15 | 16))
            && (enter
                || matches!(
                    code,
                    KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Home
                ))
        {
            if let Some(draft) = self.receptor_draft.as_mut() {
                if index == 11 && draft.placement.lock_x {
                    return;
                }
                let default = *flag(&mut Preferences::default(), index).unwrap();
                let value = flag(draft, index).unwrap();
                *value = if code == KeyCode::Home {
                    default
                } else {
                    !*value
                };
            }
            return;
        }
        if !enter {
            return;
        }
        if index == 8 {
            self.receptor_draft = Some(Preferences::default());
            return;
        }
        if index != 9 {
            return;
        }
        let Some(after) = self.receptor_draft else {
            return;
        };
        let before = self.receptor_preferences();
        let result = (|| -> Result<bool, deadsync_editor::Error> {
            // Programmatic chart focus may have staged a non-undoable X move.
            // Persist that live owner before the changed-field CAS action reads it.
            self.flush_wheel_settings()?;
            let file = self.capture_application_preferences_file()?;
            let changed = self
                .workspace
                .set_receptor_preferences(before, after, Some(&file))?;
            self.sync_receptor_preferences_history();
            Ok(changed)
        })();
        match result {
            Ok(_) => {
                self.receptor_draft = None;
                self.panel = None;
                self.status = i18n::tr("Edition", "ScrollSettingsSaved");
            }
            Err(error) => {
                self.status = i18n::tr_fmt(
                    "Edition",
                    "Error",
                    &[("message", &crate::i18n::editor_error(&error))],
                );
            }
        }
    }
    pub(super) fn receptor_settings_actors(&self, index: usize, actors: &mut Vec<Actor>) {
        let mut settings = self.receptor_draft.unwrap_or(self.receptor_preferences());
        let x = screen_width() * 0.5;
        let height = (screen_height() - 30.).min(620.).max(160.);
        let top = (screen_height() - height) * 0.5 + 22.;
        let visible = ((height - 44.) / 30.).max(1.) as usize;
        let first = index
            .saturating_sub(visible / 2)
            .min(KEYS.len().saturating_sub(visible));
        actors.push(
            act!(quad:xy(x,top+height*0.5-22.):setsize((screen_width()-30.).min(700.),height):diffuse(0.04,0.06,0.1,0.98):z(8050)),
        );
        for (row, key) in KEYS.iter().enumerate().skip(first).take(visible) {
            let ui_row_start = actors.len();
            let label = if let Some(label)=self.preference_numeric_label(row){label}else if row < 8 || matches!(row, 11 | 14 | 15 | 16) {
                let enabled = *flag(&mut settings, row).unwrap();
                i18n::tr_fmt(
                    "Edition",
                    key,
                    &[(
                        "value",
                        &i18n::tr(
                            "Edition",
                            if enabled { "WaveformYes" } else { "WaveformNo" },
                        ),
                    )],
                )
            } else if matches!(row, 12 | 13) {
                i18n::tr_fmt(
                    "Edition",
                    key,
                    &[(
                        "value",
                        &if row == 12 {
                            settings.placement.x
                        } else {
                            settings.placement.y
                        }
                        .to_string(),
                    )],
                )
            } else {
                i18n::tr("Edition", key)
            };
            let active=enabled(settings,row);
            let label=if active{label}else{i18n::tr_fmt("Edition","ReceptorControlInactive",&[("value",&label)])};
            let color=if !active{[0.5,0.5,0.5,1.]}else if row==index{[1.,0.8,0.3,1.]}else{[1.;4]};
            actors.push(act!(text:font("miso"):settext(label):xy(x,top+(row-first) as f32*30.0):zoom(0.65):maxwidth(520.0):diffuse(color[0],color[1],color[2],color[3]):z(8051)));

            self.ui_row(actors, ui_row_start, row == index, 31.);
        }
    }
}

#[cfg(test)]
mod placement_keyboard_tests {
    use super::*;
    fn press(state: &mut State, index: usize, code: KeyCode) {
        state.panel = Some(Panel::ReceptorSettings(index));
        for pressed in [true, false] {
            state.raw_key(&RawKeyboardEvent {
                code,
                pressed,
                repeat: false,
                timestamp: std::time::Instant::now(),
                host_nanos: 0,
            });
        }
    }
    #[test]
    fn keyboard_placement_locks_header_projection_persistence_and_undo_en_fr() {
        for language in ["en", "fr"] {
            i18n::init_for_tests();
            i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
                854., 480.,
            ));
            let root = tempfile::tempdir().unwrap();
            let song = root.path().join("position.ssc");
            std::fs::write(&song,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Edit;#NOTES:1000\n0100\n0010\n0001;").unwrap();
            let mut state = State::open(&song, 0).unwrap();
            let path = root.path().join("prefs.json");
            std::fs::write(&path, b"{\"Version\":1}").unwrap();
            state.configure_audio_preferences(&path).unwrap();
            let before = state.workspace.file().session.serialize().unwrap();
            let row = state.workspace.row();
            let baseline = state.receptor_canvas_position();
            assert_eq!(baseline, (427., 160.));
            state.open_receptor_settings();
            press(&mut state, 12, KeyCode::ArrowRight);
            press(&mut state, 13, KeyCode::ArrowRight);
            press(&mut state, 14, KeyCode::Enter);
            press(&mut state, 15, KeyCode::Enter);
            let locked = state.receptor_draft.unwrap().placement;
            press(&mut state, 12, KeyCode::ArrowRight);
            press(&mut state, 13, KeyCode::ArrowRight);
            assert_eq!(state.receptor_draft.unwrap().placement, locked);
            press(&mut state, 17, KeyCode::ArrowRight);
            assert_eq!(state.receptor_draft.unwrap().placement.x, locked.x + 1);
            press(&mut state, 9, KeyCode::Enter);
            assert_eq!(state.receptor_canvas_position(), (429., 161.));
            let geometry = state
                .visible_view_geometry()
                .into_iter()
                .find(|g| g.focused)
                .unwrap();
            assert_eq!(geometry.left + geometry.field_width * 0.5, 429.);
            let viewport = state.chart_viewport();
            assert_eq!(viewport.cursor, 161.);
            assert_eq!(
                state
                    .row_projection(viewport.top, viewport.bottom, viewport.cursor)
                    .y(row),
                161.
            );
            let mut actors = Vec::new();
            state.actors(&mut actors);
            assert!(!actors.is_empty());
            assert_eq!(state.workspace.row(), row);
            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
            assert_eq!(
                deadsync_editor::audio_preferences::File::open(&path)
                    .unwrap()
                    .receptors(),
                state.receptor_preferences()
            );
            state.command(Command::Undo);
            assert_eq!(state.receptor_canvas_position(), baseline);
            state.command(Command::Redo);
            assert_eq!(state.receptor_canvas_position(), (429., 161.));
            let mut scroll = state.scroll_preferences;
            scroll.set_reverse(true);
            state.commit_scroll_preferences(scroll).unwrap();
            assert_eq!(state.receptor_canvas_position(), (429., 339.));
            assert_eq!(state.chart_viewport().cursor, 339.);
            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
            state.open_receptor_settings();
            press(&mut state, 14, KeyCode::Enter);
            press(&mut state, 11, KeyCode::Enter);
            press(&mut state, 12, KeyCode::ArrowRight);
            press(&mut state, 17, KeyCode::ArrowRight);
            assert_eq!(state.receptor_draft.unwrap().placement.x, 844);
            press(&mut state, 9, KeyCode::Enter);
            assert_eq!(state.receptor_canvas_position().0, 427.);
        }
    }
}

#[cfg(test)]
mod control_gates_tests;

/// An explicit two-dimensional keyboard gesture. Numeric X/Y edits remain
/// independent of the held axis lock, just as source numeric controls are.
pub(super) struct Movement {
    binding: deadsync_editor::chart_order::SessionBinding,
    before: Preferences,
    last: Preferences,
    requested: (i32, i32),
    header: bool,
}
impl State {
    pub(super) fn cancel_receptor_move(&mut self) {
        if let Some(movement) = self.receptor_move.take() {
            if movement.binding.is_current(&self.workspace.file().session)
                && self.receptor_draft == Some(movement.last)
            {
                self.receptor_draft = Some(movement.before);
            }
        }
    }
    pub(super) fn update_receptor_move(&mut self) {
        let Some(mut movement) = self.receptor_move.take() else {
            return;
        };
        if !matches!(self.panel, Some(Panel::ReceptorSettings(_))) {
            return;
        }
        if !movement.binding.is_current(&self.workspace.file().session) {
            if self.receptor_draft == Some(movement.last) {
                self.receptor_draft = None;
                self.panel = None;
            }
            self.status = i18n::tr("Edition", "ReceptorMoveStale");
            return;
        }
        if self.receptor_draft != Some(movement.last) {
            // Do not overwrite a competing draft.
            self.status = i18n::tr("Edition", "ReceptorMoveStale");
            return;
        }
        let locked = self.owned_gesture_query(
            deadsync_editor::key_binding_preferences::scroll_modifiers::Kind::ReceptorAxis,
        );
        let mut next = movement.before;
        next.placement =
            movement
                .before
                .placement
                .moved_to(movement.requested, locked, movement.header);
        movement.last = next;
        self.receptor_draft = Some(next);
        self.receptor_move = Some(movement);
    }
    fn receptor_movement_key(&mut self, index: usize, code: KeyCode) -> bool {
        if self.receptor_move.is_none() {
            if code != KeyCode::F2 {
                return false;
            }
            let Some(before) = self.receptor_draft else {
                return true;
            };
            let header = index == 17;
            let can_x = !before.placement.center
                && (!before.placement.lock_x || header && before.placement.allow_header);
            let can_y = !before.placement.lock_y && !header;
            if !can_x && !can_y {
                self.status = i18n::tr("Edition", "ReceptorControlUnavailable");
                return true;
            }
            self.receptor_move = Some(Movement {
                binding: deadsync_editor::chart_order::SessionBinding::capture(
                    &self.workspace.file().session,
                ),
                before,
                last: before,
                requested: (before.placement.x, before.placement.y),
                header,
            });
            self.status = i18n::tr("Edition", "ReceptorMoveHelp");
            return true;
        }
        if code == KeyCode::Escape
            && !self.control
            && self.owned_gesture_query(
                deadsync_editor::key_binding_preferences::scroll_modifiers::Kind::ReceptorAxis,
            )
        {
            return true;
        }
        if code == KeyCode::Escape {
            let movement = self.receptor_move.take().expect("active receptor move");
            if movement.binding.is_current(&self.workspace.file().session)
                && self.receptor_draft == Some(movement.last)
            {
                self.receptor_draft = Some(movement.before);
            }
            self.status = i18n::tr("Edition", "ReceptorSettingsHelp");
            return true;
        }
        if matches!(code, KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::F2) {
            self.receptor_move = None;
            self.status = i18n::tr("Edition", "ReceptorSettingsHelp");
            return true;
        }
        if let Some(movement) = self.receptor_move.as_mut() {
            match code {
                KeyCode::ArrowLeft => {
                    movement.requested.0 = movement.requested.0.saturating_sub(1).max(0)
                }
                KeyCode::ArrowRight => {
                    movement.requested.0 = movement.requested.0.saturating_add(1).max(0)
                }
                KeyCode::ArrowUp => {
                    movement.requested.1 = movement.requested.1.saturating_sub(1).max(0)
                }
                KeyCode::ArrowDown => {
                    movement.requested.1 = movement.requested.1.saturating_add(1).max(0)
                }
                _ => return true,
            }
        }
        self.update_receptor_move();
        true
    }
}
