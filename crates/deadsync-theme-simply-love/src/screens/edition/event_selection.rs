mod presets;
use super::{Panel, State};
use crate::{act, i18n};
use deadlib_platform::input::KeyCode;
use deadlib_present::{
    actors::Actor,
    space::{screen_height, screen_width},
};
use deadsync_editor::event_clipboard::{Event, Selection, TimingKind};
use std::collections::BTreeSet;
const MAX_PAGE: usize = 8;
const FIRST_EVENT_Y: f32 = 146.0;
const EVENT_ROW_HEIGHT: f32 = 25.0;
const HELP_BOTTOM_RESERVE: f32 = 215.0;

fn page_size(height: f32) -> usize {
    // Leave a full text row between the final event and the help block.
    (((height - HELP_BOTTOM_RESERVE - FIRST_EVENT_Y - 24.0) / EVENT_ROW_HEIGHT)
        .floor()
        .max(0.0) as usize
        + 1)
    .min(MAX_PAGE)
}
pub(super) struct Settings {
    pub(super) selection: Selection,
    index: usize,
    presets: Option<presets::Settings>,
}
impl State {
    pub(super) fn selected_note_positions_for_render(
        &self,
        chart: usize,
    ) -> Option<BTreeSet<deadsync_editor::notes::Position>> {
        if self.preview.is_some() || self.workspace.active_chart() != Some(chart) {
            return None;
        }
        let settings = self.event_selection.as_ref().filter(|settings| {
            settings.selection.chart() == chart
                && settings
                    .selection
                    .is_current(&self.workspace.file().session)
        })?;
        Some(
            settings
                .selection
                .selected_events()
                .filter_map(|event| match event {
                    Event::Note(position) => Some(position),
                    _ => None,
                })
                .collect(),
        )
    }

    /// Source OnDelete consumes only selected events. No selection is a no-op;
    /// the legacy native row-delete command deliberately keeps its own contract.
    pub(super) fn delete_source_event_selection(&mut self) {
        if self.event_selection.is_none() {
            if self.workspace.selection().is_none() {
                self.status = i18n::tr("Edition", "EventSelectionNoChoice");
                return;
            }
            match self.workspace.note_range_event_selection() {
                Ok(selection) => self.replace_region_event_selection(selection),
                Err(error) => {
                    self.status = i18n::editor_error(&error).into();
                    return;
                }
            }
        }
        let settings = self
            .event_selection
            .take()
            .expect("selection prepared above");
        let result = self.workspace.delete_event_selection(&settings.selection);
        if settings
            .selection
            .is_current(&self.workspace.file().session)
        {
            self.event_selection = Some(settings);
        }
        self.status = match result {
            Ok(true) => i18n::tr("Edition", "EventSelectionDeleted"),
            Ok(false) => i18n::tr("Edition", "EventSelectionNoChange"),
            Err(error) => i18n::editor_error(&error).into(),
        };
        self.refresh();
    }
    pub(super) fn replace_region_event_selection(&mut self, selection: Selection) {
        self.event_selection = Some(Settings {
            selection,
            index: 0,
            presets: None,
        });
    }

    pub(super) fn sync_event_selection_history(&mut self) {
        let Some(effect) = self.workspace.take_event_selection_update() else {
            return;
        };
        let session = &self.workspace.file().session;
        let focused = self
            .workspace
            .active_chart()
            .is_some_and(|chart| session.chart_ids().get(chart) == Some(effect.chart()));
        if !effect.is_current(session) || !focused || effect.events().is_empty() {
            self.event_selection = None;
            if matches!(self.panel, Some(Panel::EventSelection)) {
                self.panel = None;
            }
            return;
        }
        let chart = self.workspace.chart();
        let result = (|| -> Result<Selection, deadsync_editor::Error> {
            let mut selected = self.workspace.event_selection_between(0, i32::MAX as u64)?;
            selected.select_all(false);
            let indices: std::collections::BTreeMap<_, _> = selected
                .events()
                .iter()
                .copied()
                .enumerate()
                .map(|(i, e)| (e, i))
                .collect();
            for event in effect.events() {
                if let Some(&i) = indices.get(event) {
                    selected.set_selected(i, true)?;
                }
            }
            Ok(selected)
        })();
        match result {
            Ok(selection) => {
                debug_assert_eq!(selection.chart(), chart);
                self.event_selection = Some(Settings {
                    selection,
                    index: 0,
                    presets: None,
                });
            }
            Err(_) => {
                self.event_selection = None;
                self.status = i18n::tr("Edition", "TimingConversionFailed");
            }
        }
    }
    pub(super) fn command_selection_preset(
        &mut self,
        preset: deadsync_editor::event_clipboard::presets::Preset,
        current_player: bool,
    ) {
        use deadsync_editor::event_clipboard::presets::Mode;
        let player = current_player.then_some(self.workspace.player());
        let result = if let Some(existing) = &self.event_selection {
            self.workspace.expand_event_selection(&existing.selection)
        } else {
            self.workspace.event_selection().and_then(|mut captured| {
                // A capture checks its cursor row by default. That is not an
                // existing user selection unless a row-range anchor exists.
                if self.workspace.selection().is_none() {
                    captured.select_all(false);
                }
                self.workspace.expand_event_selection(&captured)
            })
        }
        .and_then(|mut selection| {
            let report = selection.apply_preset(preset, player, Mode::Add)?;
            Ok((selection, report))
        });
        match result {
            Ok((selection, report)) => {
                self.lane_entry_release_input();
                let index = report
                    .last_match
                    .and_then(|event| {
                        selection
                            .events()
                            .iter()
                            .position(|candidate| *candidate == event)
                    })
                    .unwrap_or(0);
                self.event_selection = Some(Settings {
                    selection,
                    index,
                    presets: None,
                });
                self.panel = Some(Panel::EventSelection);
                self.status = i18n::tr_fmt(
                    "Edition",
                    "SelectionPresetsApplied",
                    &[
                        ("matched", &report.matched.to_string()),
                        ("selected", &report.selected.to_string()),
                    ],
                );
            }
            Err(_) => self.status = i18n::tr("Edition", "SelectionPresetsFailed"),
        }
    }

    pub(super) fn open_event_selection(&mut self) {
        if self.is_generating() {
            return;
        }
        match self.workspace.event_selection() {
            Ok(selection) => {
                // This limited modal keeps other shortcuts enabled, so cancel
                // lane ownership at publication rather than waiting for suspend.
                self.lane_entry_release_input();
                self.event_selection = Some(Settings {
                    selection,
                    index: 0,
                    presets: None,
                });
                self.panel = Some(Panel::EventSelection);
                self.status = i18n::tr("Edition", "EventSelectionTitle");
            }
            Err(error) => {
                self.status = i18n::tr_fmt(
                    "Edition",
                    "Error",
                    &[("message", &crate::i18n::editor_error(&error))],
                )
            }
        }
    }
    pub(super) fn prepare_checked_timing_conversion(
        &self,
        direction: deadsync_editor::timing_edit::conversion::Direction,
    ) -> Result<
        deadsync_editor::timing_edit::conversion::Plan,
        deadsync_editor::timing_edit::conversion::Failure,
    > {
        let selection = &self
            .event_selection
            .as_ref()
            .ok_or_else(|| {
                deadsync_editor::Error::InvalidEdit("No checked event selection".into())
            })?
            .selection;
        self.workspace
            .prepare_selected_timing_conversion(selection, direction)
    }
    pub(super) fn event_selection_key(&mut self, code: KeyCode) {
        if self.region_selection.is_some() {
            self.region_selection_key(code);
            return;
        }
        if code == KeyCode::F4 {
            self.open_region_selection();
            return;
        }
        if code == KeyCode::F6 {
            self.open_pattern_actions(true);
            return;
        }
        if code == KeyCode::F9 {
            self.open_selected_transform_review(true);
            return;
        }
        if code == KeyCode::F7 {
            self.open_checked_timing_conversion();
            return;
        }
        if code == KeyCode::F8 {
            self.open_note_operation_review(true);
            return;
        }
        if code == KeyCode::F3 {
            self.open_selection_presets();
            return;
        }
        if code == KeyCode::Escape {
            self.event_selection = None;
            self.panel = None;
            self.status = i18n::tr("Edition", "Ready");
            return;
        }
        let Some(mut settings) = self.event_selection.take() else {
            return;
        };
        if self.control && matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight) {
            self.shift_event_choices(&mut settings, code == KeyCode::ArrowRight);
            self.event_selection = Some(settings);
            return;
        }
        let length = settings.selection.events().len();
        let mut close = false;
        let result = (|| -> Result<&str, deadsync_editor::Error> {
            if settings.selection.selected_count() == 0
                && (matches!(code, KeyCode::Delete | KeyCode::Backspace)
                    || (self.control && matches!(code, KeyCode::KeyC | KeyCode::KeyX)))
            {
                return Ok("EventSelectionNoChoice");
            }
            match code {
                KeyCode::ArrowUp => settings.index = settings.index.saturating_sub(1),
                KeyCode::ArrowDown => {
                    settings.index = (settings.index + 1).min(length.saturating_sub(1))
                }
                KeyCode::PageUp => {
                    settings.index = settings.index.saturating_sub(page_size(screen_height()))
                }
                KeyCode::PageDown => {
                    settings.index =
                        (settings.index + page_size(screen_height())).min(length.saturating_sub(1))
                }
                KeyCode::Home => settings.index = 0,
                KeyCode::End => settings.index = length.saturating_sub(1),
                KeyCode::Space | KeyCode::Enter if length > 0 => {
                    let checked = !settings.selection.is_selected(settings.index);
                    use deadsync_editor::key_binding_preferences::scroll_modifiers::Kind as Held;
                    if self.owned_gesture_query(Held::SelectionExtend) {
                        let event = settings.selection.events()[settings.index];
                        let mut expanded =
                            self.workspace.expand_event_selection(&settings.selection)?;
                        expanded.finish_region(
                            &self.workspace.file().session,
                            &[event],
                            deadsync_editor::event_clipboard::region::Modifiers {
                                alt: self.owned_gesture_query(Held::SelectionMisc),
                                control: self.owned_gesture_query(Held::SelectionToggle),
                                shift: true,
                            },
                            self.selection_preferences.region,
                        )?;
                        settings.index = expanded
                            .events()
                            .iter()
                            .position(|e| *e == event)
                            .unwrap_or(0);
                        settings.selection = expanded;
                    } else {
                        settings.selection.set_selected(settings.index, checked)?;
                    }
                }
                KeyCode::KeyA if self.control => settings.selection.select_all(!self.shift),
                KeyCode::KeyC if self.control => {
                    self.workspace.copy_event_selection(&settings.selection)?;
                    close = true;
                    return Ok("EventSelectionCopied");
                }
                KeyCode::KeyX if self.control => {
                    self.workspace.cut_event_selection(&settings.selection)?;
                    close = true;
                    return Ok("EventSelectionCut");
                }
                KeyCode::Delete | KeyCode::Backspace => {
                    let changed = self.workspace.delete_event_selection(&settings.selection)?;
                    close = true;
                    return Ok(if changed {
                        "EventSelectionDeleted"
                    } else {
                        "EventSelectionNoChange"
                    });
                }
                _ => {}
            }
            Ok("EventSelectionTitle")
        })();
        match result {
            Ok(key) => self.status = i18n::tr("Edition", key),
            Err(error) => {
                self.status = i18n::tr_fmt(
                    "Edition",
                    "Error",
                    &[("message", &crate::i18n::editor_error(&error))],
                )
            }
        }
        if close {
            if settings
                .selection
                .is_current(&self.workspace.file().session)
            {
                self.event_selection = Some(settings);
            }
            self.panel = None;
            self.refresh();
        } else {
            self.event_selection = Some(settings);
        }
    }
    fn shift_event_choices(&mut self, settings: &mut Settings, later: bool) {
        if settings.selection.selected_count() == 0 {
            self.status = i18n::tr("Edition", "EventSelectionNoChoice");
            return;
        }
        let mut start = settings
            .selection
            .events()
            .iter()
            .map(|event| event.row())
            .min()
            .unwrap_or(self.workspace.row());
        let mut end = settings
            .selection
            .events()
            .iter()
            .map(|event| event.row())
            .max()
            .unwrap_or(start);
        let rows = deadsync_editor::workspace::snap_shift_rows(self.workspace.snap()) as i64
            * if later { 1 } else { -1 };
        match self
            .workspace
            .shift_event_selection(&settings.selection, rows)
        {
            Ok(report) => {
                self.refresh();
                for event in &report.selection_after {
                    start = start.min(event.row());
                    end = end.max(event.row());
                }
                match self.workspace.event_selection_between(start, end) {
                    Ok(mut refreshed) => {
                        refreshed.select_all(false);
                        let selected: BTreeSet<_> =
                            report.selection_after.iter().copied().collect();
                        for index in 0..refreshed.events().len() {
                            if selected.contains(&refreshed.events()[index]) {
                                refreshed
                                    .set_selected(index, true)
                                    .expect("index belongs to captured events");
                            }
                        }
                        settings.index = (0..refreshed.events().len())
                            .find(|index| refreshed.is_selected(*index))
                            .unwrap_or(0);
                        settings.selection = refreshed;
                        self.status = i18n::tr_fmt(
                            "Edition",
                            "EventShiftReport",
                            &[
                                ("moved", &report.moved.to_string()),
                                ("deleted", &report.deleted.to_string()),
                                ("immutable", &report.immutable.to_string()),
                            ],
                        );
                    }
                    Err(_) => {
                        // The atomic shift succeeded; never imply a rollback
                        // merely because rebuilding its presentation failed.
                        settings.selection.select_all(false);
                        self.status = i18n::tr("Edition", "EventShiftRefreshFailed");
                    }
                }
            }
            Err(_) => self.status = i18n::tr("Edition", "EventShiftFailed"),
        }
    }
    pub(super) fn event_selection_actors(&self, actors: &mut Vec<Actor>) {
        if self.region_selection.is_some() {
            self.region_selection_actors(actors);
            return;
        }
        let Some(settings) = &self.event_selection else {
            return;
        };
        let width = screen_width();
        let height = screen_height();
        let page = page_size(height);
        actors.push(act!(quad: xy(width * 0.5, height * 0.5): zoomto(width - 36.0, height - 150.0): diffuse(0.045, 0.065, 0.095, 0.98): z(8010)));
        label(
            actors,
            &i18n::tr("Edition", "EventSelectionTitle"),
            88.0,
            [0.5, 0.85, 1.0, 1.0],
        );
        let total = settings.selection.events().len();
        let summary = i18n::tr_fmt(
            "Edition",
            "EventSelectionCount",
            &[
                ("selected", &settings.selection.selected_count().to_string()),
                ("total", &total.to_string()),
                ("page", &(settings.index / page + 1).to_string()),
                ("pages", &total.div_ceil(page).max(1).to_string()),
            ],
        );
        label(actors, &summary, 114.0, [0.75, 0.8, 0.85, 1.0]);
        let start = settings.index / page * page;
        if total == 0 {
            label(
                actors,
                &i18n::tr("Edition", "EventSelectionEmpty"),
                150.0,
                [1.0; 4],
            );
        }
        for (offset, &event) in settings
            .selection
            .events()
            .iter()
            .skip(start)
            .take(page)
            .enumerate()
        {
            let ui_row_start = actors.len();
            let index = start + offset;
            let y = FIRST_EVENT_Y + offset as f32 * EVENT_ROW_HEIGHT;
            let color = if index == settings.index {
                [1.0, 0.85, 0.4, 1.0]
            } else {
                [0.85, 0.9, 0.95, 1.0]
            };
            let name = match event {
                Event::Note(position) => i18n::tr_fmt(
                    "Edition",
                    "EventSelectionNote",
                    &[
                        (
                            "kind",
                            &i18n::tr(
                                "Edition",
                                super::tool_key(
                                    settings
                                        .selection
                                        .note_kind(position)
                                        .expect("Captured note kind"),
                                ),
                            ),
                        ),
                        ("player", &(position.player + 1).to_string()),
                        ("lane", &(position.lane + 1).to_string()),
                    ],
                ),
                Event::SongWidget(widget) => i18n::tr(
                    "Edition",
                    match widget.kind() {
                        deadsync_editor::event_clipboard::WidgetKind::Preview => {
                            "EventSelectionPreview"
                        }
                        deadsync_editor::event_clipboard::WidgetKind::EndHint => {
                            "EventSelectionEndHint"
                        }
                    },
                ),
                Event::Attack { .. } => i18n::tr("Edition", "EventSelectionAttack"),
                Event::Pattern { .. } => i18n::tr("Edition", "EventSelectionPattern"),
                Event::Timing { kind, .. } => i18n::tr("Edition", timing_key(kind)),
            };
            let text = i18n::tr_fmt(
                "Edition",
                "EventSelectionRow",
                &[
                    (
                        "check",
                        if settings.selection.is_selected(index) {
                            "[x]"
                        } else {
                            "[ ]"
                        },
                    ),
                    ("beat", &format!("{:.3}", event.chart_position() / 48.0)),
                    ("name", &name),
                ],
            );
            label(actors, &text, y, color);

            self.ui_row(actors, ui_row_start, start + offset == settings.index, 25.);
        }
        // Reserve separate wrapped footer blocks inside the modal. The full
        // keyboard help is several lines in both locales and must not extend
        // through the shift help or the editor's status below the card.
        actors.push(act!(text: font("miso"): settext(i18n::tr("Edition", "EventSelectionHelp")): align(0.0,0.0): xy(42.0,height-HELP_BOTTOM_RESERVE): zoom(0.5): wrapwidthpixels((width-84.0)/0.5): maxwidth(width-84.0): maxheight(100.0): diffuse(0.7,0.8,0.9,1.0): z(8012)));
        actors.push(act!(text: font("miso"): settext(i18n::tr("Edition", "EventShiftHelp")): align(0.0,0.0): xy(42.0,height-110.0): zoom(0.5): wrapwidthpixels((width-84.0)/0.5): maxwidth(width-84.0): maxheight(30.0): diffuse(0.7,0.8,0.9,1.0): z(8012)));
    }
}
fn label(actors: &mut Vec<Actor>, value: &str, y: f32, color: [f32; 4]) {
    actors.push(act!(text: font("miso"): settext(value.to_owned()): align(0.0, 0.0): xy(42.0, y): zoom(0.60): diffuse(color[0], color[1], color[2], color[3]): z(8012)));
}
pub(super) fn timing_key(kind: TimingKind) -> &'static str {
    match kind {
        TimingKind::Tempo => "EventSelectionTempo",
        TimingKind::Stop => "EventSelectionStop",
        TimingKind::Delay => "EventSelectionDelay",
        TimingKind::Signature => "EventSelectionSignature",
        TimingKind::Tick => "EventSelectionTick",
        TimingKind::Combo => "EventSelectionCombo",
        TimingKind::Warp => "EventSelectionWarp",
        TimingKind::Fake => "EventSelectionFake",
        TimingKind::Scroll => "EventSelectionScroll",
        TimingKind::Speed => "EventSelectionSpeed",
        TimingKind::Label => "EventSelectionLabel",
    }
}

impl State {
    pub(super) fn toggle_source_row_selection(&mut self) {
        let result = self
            .capture_song_property_presentation()
            .and_then(|position| {
                self.workspace
                    .toggle_source_row_selection_at(position.row())
            });
        match result {
            Ok(selection) => {
                self.lane_entry_release_input();
                self.region_selection = None;
                self.event_selection = None;
                if let Some(selection) = selection {
                    self.replace_region_event_selection(selection);
                }
                self.refresh();
            }
            Err(error) => self.status = i18n::editor_error(&error).into(),
        }
    }
}

#[cfg(test)]
mod source_row_range_tests {
    use super::*;
    #[test]
    fn event_pages_leave_room_for_help_at_supported_small_heights() {
        for height in [480.0, 540.0, 600.0, 720.0, 1080.0] {
            let page = page_size(height);
            assert!((1..=MAX_PAGE).contains(&page));
            let final_row_bottom = FIRST_EVENT_Y + (page - 1) as f32 * EVENT_ROW_HEIGHT + 24.0;
            assert!(final_row_bottom <= height - HELP_BOTTOM_RESERVE);
            // Page navigation and rendering use this same capacity.
            for index in 0..32 {
                let start = index / page * page;
                assert!((start..start + page).contains(&index));
            }
        }
        assert_eq!(page_size(480.0), 4);
        assert_eq!(page_size(720.0), MAX_PAGE);
    }

    #[test]
    fn event_page_keys_and_rendered_rows_share_small_screen_capacity_en_fr() {
        use deadlib_present::space::{Metrics, with_metrics};
        for language in ["en", "fr"] {
            i18n::init(deadsync_assets::language::load_for_tests(language));
            i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("pages.ssc");
            std::fs::write(
                &path,
                format!(
                    "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:{};",
                    ["1000"; 16].join("\n")
                ),
            )
            .unwrap();
            let mut state = State::open(&path, 0).unwrap();
            for height in [480.0, 720.0] {
                with_metrics(Metrics::centered(1280.0, height), || {
                    let selection = state.workspace.event_selection_between(0, 192).unwrap();
                    state.replace_region_event_selection(selection);
                    state.panel = Some(Panel::EventSelection);
                    let page = page_size(height);
                    state.event_selection_key(KeyCode::PageDown);
                    assert_eq!(state.event_selection.as_ref().unwrap().index, page);
                    let mut actors = Vec::new();
                    state.event_selection_actors(&mut actors);
                    assert_eq!(
                        actors
                            .iter()
                            .filter(|a| matches!(a, Actor::Text { .. }))
                            .count(),
                        page + 4
                    );
                    state.event_selection_key(KeyCode::PageUp);
                    assert_eq!(state.event_selection.as_ref().unwrap().index, 0);
                });
            }
        }
    }

    #[test]
    fn selection_status_does_not_repeat_multiline_modal_help_en_fr() {
        for language in ["en", "fr"] {
            i18n::init(deadsync_assets::language::load_for_tests(language));
            i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("song.ssc");
            std::fs::write(
                &path,
                "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
            )
            .unwrap();
            let mut state = State::open(&path, 0).unwrap();
            state.open_event_selection();
            assert_eq!(state.status, i18n::tr("Edition", "EventSelectionTitle"));
            state.event_selection_key(KeyCode::ArrowDown);
            assert_eq!(state.status, i18n::tr("Edition", "EventSelectionTitle"));
            let mut actors = Vec::new();
            state.event_selection_actors(&mut actors);
            assert!(
                actors
                    .iter()
                    .any(|actor| matches!(actor, Actor::Text { content, .. }
                if content.as_str() == i18n::tr("Edition", "EventSelectionHelp").as_ref()))
            );
        }
    }

    fn press(state: &mut State, code: KeyCode) {
        for pressed in [true, false] {
            state.raw_key(&deadlib_platform::input::RawKeyboardEvent {
                code,
                pressed,
                repeat: false,
                timestamp: std::time::Instant::now(),
                host_nanos: 0,
            });
        }
    }
    #[test]
    fn two_source_range_toggles_materialize_notes_for_transform_without_changing_focus_en_fr() {
        for language in ["en", "fr"] {
            i18n::init(deadsync_assets::language::load_for_tests(language));
            i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("song.ssc");
            std::fs::write(&path, "#VERSION:0.83;#BPMS:0=120;#LABELS:1=not selected;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001;").unwrap();
            let mut state = State::open(&path, 0).unwrap();
            let before = state.workspace.file().session.serialize().unwrap();
            state.navigate_exact_position(0.5, true);
            press(&mut state, KeyCode::Space);
            assert!(state.workspace.selection().is_some());
            state.navigate_exact_position(47.6, true);
            let focus = state.capture_song_property_presentation().unwrap().row();
            press(&mut state, KeyCode::Space);
            assert!(state.workspace.selection().is_none());
            let selection = &state.event_selection.as_ref().unwrap().selection;
            assert_eq!(selection.selected_count(), 2);
            assert!(
                selection
                    .selected_events()
                    .all(|event| matches!(event, Event::Note(_)))
            );
            assert_eq!(
                state
                    .capture_song_property_presentation()
                    .unwrap()
                    .row()
                    .to_bits(),
                focus.to_bits()
            );
            state.command_selected_transform_review(deadsync_editor::transform::Transform::Mirror);
            assert!(matches!(
                state.panel,
                Some(Panel::SelectedTransformReview(_))
            ));
            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
            assert!(state.workspace.file().session.undo_label().is_none());
        }
    }
}

#[cfg(test)]
mod note_render_selection_tests {
    use super::*;

    #[test]
    fn explicit_note_render_selection_is_bound_to_the_current_chart() {
        i18n::init_for_tests();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("song.ssc");
        std::fs::write(&path,"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        let mut state = State::open(&path, 0).unwrap();
        state.open_event_selection();
        let selection = &mut state.event_selection.as_mut().unwrap().selection;
        for index in 0..selection.events().len() {
            selection.set_selected(index, true).unwrap();
        }
        let actual = state.selected_note_positions_for_render(0).unwrap();
        assert_eq!(actual.len(), 1);
        assert_eq!(actual.iter().next().unwrap().lane, 0);
        assert!(state.selected_note_positions_for_render(1).is_none());
    }
}
