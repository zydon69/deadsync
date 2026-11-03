use super::{Panel, State};
use crate::{act, i18n};
use deadlib_platform::input::KeyCode;
use deadlib_present::{
    actors::Actor,
    space::{screen_height, screen_width},
};
use deadsync_editor::{chart_event_copy::EventType, chart_order::Binding};
use std::collections::BTreeSet;
const PAGE: usize = 8;
#[derive(Clone, Copy)]
enum TypePreset {
    Stepmania,
    Timing,
    TimingAndScroll,
    All,
}
impl TypePreset {
    fn types(self) -> BTreeSet<EventType> {
        EventType::ALL
            .into_iter()
            .filter(|kind| match self {
                Self::Stepmania => *kind != EventType::Pattern,
                Self::Timing => matches!(
                    kind,
                    EventType::Signature
                        | EventType::Tempo
                        | EventType::Stop
                        | EventType::Delay
                        | EventType::Warp
                ),
                Self::TimingAndScroll => matches!(
                    kind,
                    EventType::Signature
                        | EventType::Tempo
                        | EventType::Stop
                        | EventType::Delay
                        | EventType::Warp
                        | EventType::Scroll
                        | EventType::Speed
                ),
                Self::All => true,
            })
            .collect()
    }
}
#[derive(Clone, Copy)]
enum View {
    Edit,
    Quick,
    Source,
}
pub(super) struct Request {
    guard: deadsync_editor::history::file_intent::IntentGuard,
    focus: Option<Binding>,
    culture: u64,
    opening: bool,
    reset_source: bool,
}
pub(super) struct Settings {
    guard: deadsync_editor::history::file_intent::IntentGuard,
    focus: Option<Binding>,
    source: usize,
    index: usize,
    types: BTreeSet<EventType>,
    destinations: BTreeSet<usize>,
    choices: Vec<(usize, String)>,
    sources: Vec<(usize, String)>,
    view: View,
    return_song: bool,
    culture: u64,
}
impl Settings {
    fn current(&self, state: &State) -> bool {
        self.culture == i18n::revision()
            && self.guard.is_current(&state.workspace.file().session)
            && focus_current(&self.focus, state)
    }
    fn choose_source(&mut self, source: usize) {
        self.source = source;
        self.choices = self
            .sources
            .iter()
            .filter(|(chart, _)| *chart != source)
            .cloned()
            .collect();
        self.destinations = self.choices.iter().map(|(chart, _)| *chart).collect();
    }
}
fn focus_current(focus: &Option<Binding>, state: &State) -> bool {
    focus
        .as_ref()
        .map_or(!state.workspace.has_chart(), |binding| {
            state.workspace.has_chart()
                && binding.is_current(&state.workspace.file().session, state.workspace.chart())
        })
}
impl State {
    #[cfg(test)]
    pub(super) fn chart_event_copy_source(&self) -> Option<usize> {
        self.chart_event_copy
            .as_ref()
            .map(|settings| settings.source)
    }
    fn chart_copy_settings(
        &self,
        source: usize,
        view: View,
        return_song: bool,
    ) -> Result<Settings, deadsync_editor::Error> {
        let session = &self.workspace.file().session;
        if source >= session.chart_ids().len() {
            return Err(deadsync_editor::Error::ExternalChange);
        }
        let sources = session
            .document()
            .charts()
            .iter()
            .enumerate()
            .map(|(index, chart)| {
                let description =
                    deadsync_editor::chart_text::Text::read(session.document(), index)
                        .map(|text| {
                            text.get(deadsync_editor::chart_text::Field::Description)
                                .to_owned()
                        })
                        .unwrap_or_default();
                (
                    index,
                    format!(
                        "#{} {} | {} [{}] {}",
                        index + 1,
                        chart.steps_type,
                        chart.difficulty,
                        chart.meter,
                        description
                    ),
                )
            })
            .collect::<Vec<_>>();
        let focus = self
            .workspace
            .active_chart()
            .map(|chart| Binding::capture(session, chart))
            .transpose()?;
        let mut settings = Settings {
            guard: deadsync_editor::history::file_intent::IntentGuard::capture(session),
            focus,
            source,
            index: 0,
            types: TypePreset::Stepmania.types(),
            destinations: BTreeSet::new(),
            choices: Vec::new(),
            sources,
            view,
            return_song,
            culture: i18n::revision(),
        };
        settings.choose_source(source);
        Ok(settings)
    }
    pub(super) fn open_chart_event_copy(&mut self) {
        if self.is_generating() || !self.has_song() {
            return;
        }
        if self.workspace.file().session.chart_ids().is_empty() {
            self.status = i18n::tr("Edition", "ChartEventCopyNoDestination");
            return;
        }
        if let Some(source) = self.workspace.active_chart() {
            match self.chart_copy_settings(source, View::Edit, false) {
                Ok(settings) => {
                    self.chart_event_copy = Some(settings);
                    self.panel = Some(Panel::ChartEventCopy);
                    self.status = i18n::tr("Edition", "ChartEventCopyHelp");
                }
                Err(error) => self.status = i18n::editor_error(&error).into(),
            }
        } else {
            self.queue_chart_copy_sources(true, false);
        }
    }
    pub(super) fn open_song_timing_event_apply(&mut self) {
        if self.song_timing_request.is_some() {
            self.status = i18n::tr("Edition", "SongTimingLoading");
            return;
        }
        let result = (|| {
            let draft = self
                .song_properties
                .as_ref()
                .ok_or(deadsync_editor::Error::ExternalChange)?;
            if !draft.is_current(&self.workspace.file().session) {
                return Err(deadsync_editor::Error::ExternalChange);
            }
            let Some(source) = draft.timing_chart(&self.workspace.file().session)? else {
                self.status = i18n::tr("Edition", "SongTimingApplyNoSource");
                return Ok(());
            };
            let settings = self.chart_copy_settings(source, View::Quick, true)?;
            self.chart_event_copy = Some(settings);
            self.panel = Some(Panel::ChartEventCopy);
            self.status = i18n::tr("Edition", "SongTimingApplyHelp");
            Ok(())
        })();
        if let Err(error) = result {
            self.status = i18n::editor_error(&error).into();
        }
    }
    fn queue_chart_copy_sources(&mut self, opening: bool, reset_source: bool) {
        self.chart_event_copy_request = Some(Request {
            guard: deadsync_editor::history::file_intent::IntentGuard::capture(
                &self.workspace.file().session,
            ),
            focus: self
                .workspace
                .active_chart()
                .and_then(|chart| Binding::capture(&self.workspace.file().session, chart).ok()),
            culture: i18n::revision(),
            opening,
            reset_source,
        });
        self.panel = Some(Panel::ChartEventCopy);
        self.status = i18n::tr("Edition", "SongTimingLoading");
    }
    pub(super) fn process_chart_event_copy_order(
        &mut self,
        orderer: &mut impl deadsync_editor::chart_order::Orderer,
    ) -> bool {
        let Some(request) = self.chart_event_copy_request.take() else {
            return false;
        };
        let current = |state: &State| {
            matches!(state.panel, Some(Panel::ChartEventCopy))
                && request.culture == i18n::revision()
                && request.guard.is_current(&state.workspace.file().session)
                && focus_current(&request.focus, state)
        };
        if !current(self) {
            self.status = i18n::tr("Edition", "ChartEventCopySourceChanged");
            return true;
        }
        let order = self
            .chart_list_cache
            .refresh(&mut self.workspace, request.culture, orderer)
            .map(|rows| rows.iter().map(|row| row.source_index).collect::<Vec<_>>());
        if !current(self) {
            self.status = i18n::tr("Edition", "ChartEventCopySourceChanged");
            return true;
        }
        let result = order.and_then(|order| {
            let mut settings = if request.opening {
                let source = self
                    .workspace
                    .active_chart()
                    .or_else(|| order.first().copied())
                    .ok_or(deadsync_editor::Error::ExternalChange)?;
                self.chart_copy_settings(source, View::Edit, false)?
            } else {
                self.chart_event_copy
                    .take()
                    .ok_or(deadsync_editor::Error::ExternalChange)?
            };
            settings.sources = order
                .iter()
                .map(|chart| {
                    settings
                        .sources
                        .iter()
                        .find(|(index, _)| index == chart)
                        .cloned()
                        .ok_or(deadsync_editor::Error::ExternalChange)
                })
                .collect::<Result<_, _>>()?;
            if request.reset_source {
                if let Some((source, _)) = settings.sources.first() {
                    let source = *source;
                    settings.choose_source(source);
                }
            }
            if !request.opening {
                settings.view = View::Source;
                settings.index = settings
                    .sources
                    .iter()
                    .position(|(index, _)| *index == settings.source)
                    .unwrap_or(0);
            }
            self.chart_event_copy = Some(settings);
            Ok(())
        });
        self.status = match result {
            Ok(()) => i18n::tr("Edition", "ChartEventCopyHelp"),
            Err(error) => i18n::editor_error(&error).into(),
        };
        true
    }
    fn close_chart_event_copy(&mut self, return_song: bool) {
        self.chart_event_copy = None;
        self.chart_event_copy_request = None;
        self.panel = if return_song && self.song_properties.is_some() {
            Some(Panel::SongProperties(6))
        } else {
            None
        };
    }
    pub(super) fn chart_event_copy_key(&mut self, code: KeyCode) {
        if code == KeyCode::Escape {
            if let Some(settings) = self.chart_event_copy.as_mut() {
                if matches!(settings.view, View::Source) {
                    settings.view = View::Edit;
                    settings.index = 0;
                    self.chart_event_copy_request = None;
                    return;
                }
            }
            let return_song = self
                .chart_event_copy
                .as_ref()
                .is_some_and(|settings| settings.return_song);
            self.close_chart_event_copy(return_song);
            self.status = i18n::tr("Edition", "Ready");
            return;
        }
        if self.chart_event_copy_request.is_some() {
            return;
        }
        let Some(mut settings) = self.chart_event_copy.take() else {
            return;
        };
        if !settings.current(self) {
            self.close_chart_event_copy(settings.return_song);
            self.status = i18n::tr("Edition", "ChartEventCopySourceChanged");
            return;
        }
        let length = match settings.view {
            View::Quick => 5,
            View::Source => settings.sources.len(),
            View::Edit => EventType::ALL.len() + settings.choices.len() + 3,
        };
        let apply = EventType::ALL.len() + settings.choices.len() + 1;
        let mut close = false;
        let mut execute = false;
        let mut picker = false;
        let mut reset_source = false;
        match code {
            KeyCode::ArrowUp => settings.index = settings.index.saturating_sub(1),
            KeyCode::ArrowDown => settings.index = (settings.index + 1).min(length - 1),
            KeyCode::PageUp => settings.index = settings.index.saturating_sub(PAGE),
            KeyCode::PageDown => settings.index = (settings.index + PAGE).min(length - 1),
            KeyCode::Home => settings.index = 0,
            KeyCode::End => settings.index = length - 1,
            KeyCode::F2 if matches!(settings.view, View::Edit) => {
                settings.types = TypePreset::Stepmania.types()
            }
            KeyCode::F3 if matches!(settings.view, View::Edit) => {
                settings.types = TypePreset::Timing.types()
            }
            KeyCode::F4 if matches!(settings.view, View::Edit) => {
                settings.types = TypePreset::TimingAndScroll.types()
            }
            KeyCode::F5 if matches!(settings.view, View::Edit) => {
                settings.types = TypePreset::All.types()
            }
            KeyCode::KeyA if self.control && matches!(settings.view, View::Edit) => {
                settings.types = if self.shift {
                    BTreeSet::new()
                } else {
                    EventType::ALL.into_iter().collect()
                }
            }
            KeyCode::Space | KeyCode::Enter | KeyCode::NumpadEnter => match settings.view {
                View::Source => {
                    let source = settings.sources[settings.index].0;
                    settings.choose_source(source);
                    settings.view = View::Edit;
                    settings.index = 0;
                }
                View::Quick => match settings.index {
                    0..=2 => {
                        settings.types = match settings.index {
                            0 => TypePreset::Timing,
                            1 => TypePreset::TimingAndScroll,
                            _ => TypePreset::Stepmania,
                        }
                        .types();
                        execute = true;
                    }
                    3 => {
                        let focused = self.workspace.active_chart();
                        if let Some(source) = focused {
                            settings.choose_source(source);
                        }
                        settings.view = View::Edit;
                        settings.index = 0;
                        settings.types = TypePreset::Stepmania.types();
                        if focused.is_none() {
                            reset_source = true;
                            picker = true;
                        }
                    }
                    _ => close = true,
                },
                View::Edit => {
                    if settings.index == 0 {
                        picker = true;
                    } else if let Some(kind) = EventType::ALL.get(settings.index - 1) {
                        if !settings.types.remove(kind) {
                            settings.types.insert(*kind);
                        }
                    } else if let Some((chart, _)) = settings
                        .choices
                        .get(settings.index - 1 - EventType::ALL.len())
                    {
                        if !settings.destinations.remove(chart) {
                            settings.destinations.insert(*chart);
                        }
                    } else if settings.index == apply {
                        execute = true;
                    } else {
                        close = true;
                    }
                }
            },
            _ => {}
        }
        if execute {
            if settings.types.is_empty() {
                self.status = i18n::tr("Edition", "ChartEventCopyNoTypes");
            } else if settings.destinations.is_empty() {
                self.status = i18n::tr("Edition", "ChartEventCopyNoDestination");
            } else {
                match self.apply_chart_event_copy_presented(
                    settings.source,
                    settings.destinations.iter().copied(),
                    settings.types.iter().copied(),
                ) {
                    Ok(report) => {
                        let shared = if report.shared_types_unchanged.is_empty() {
                            i18n::tr("Edition", "ChartEventCopyNone").to_string()
                        } else {
                            report
                                .shared_types_unchanged
                                .iter()
                                .map(|kind| i18n::tr("Edition", type_key(*kind)).to_string())
                                .collect::<Vec<_>>()
                                .join(", ")
                        };
                        self.status = i18n::tr_fmt(
                            "Edition",
                            "ChartEventCopyReport",
                            &[
                                ("charts", &report.destinations_changed.to_string()),
                                ("patterns", &report.patterns_skipped.to_string()),
                                ("shared", &shared),
                            ],
                        );
                        close = true;
                    }
                    Err(error) => self.status = i18n::editor_error(&error).into(),
                }
            }
        }
        if close {
            self.close_chart_event_copy(settings.return_song);
            self.refresh();
        } else {
            self.chart_event_copy = Some(settings);
            if picker {
                self.queue_chart_copy_sources(false, reset_source);
            }
        }
    }
    pub(super) fn chart_event_copy_actors(&self, actors: &mut Vec<Actor>) {
        let Some(settings) = &self.chart_event_copy else {
            return;
        };
        let width = (screen_width() - 40.0).min(780.0);
        let left = (screen_width() - width) * 0.5;
        let top = (screen_height() - 390.0).max(10.0) * 0.5;
        actors.push(act!(quad: xy(left+width*0.5,top+195.0): zoomto(width,390.0): diffuse(0.08,0.1,0.15,1.0): z(8020)));
        let label = |actors: &mut Vec<Actor>, text: String, y: f32, color: [f32; 4]| {
            actors.push(act!(text: font("miso"): settext(text): align(0.0,0.0): xy(left+16.0,y): zoom(0.60): maxwidth(width-32.0): diffuse(color[0],color[1],color[2],color[3]): z(8021)));
        };
        label(
            actors,
            i18n::tr(
                "Edition",
                if matches!(settings.view, View::Quick) {
                    "SongTimingApplyTitle"
                } else {
                    "ChartEventCopyTitle"
                },
            )
            .to_string(),
            top + 12.0,
            [1.0; 4],
        );
        label(
            actors,
            i18n::tr_fmt(
                "Edition",
                "ChartEventCopySource",
                &[("chart", &(settings.source + 1).to_string())],
            )
            .to_string(),
            top + 40.0,
            [0.7, 0.8, 0.9, 1.0],
        );
        let apply = EventType::ALL.len() + settings.choices.len() + 1;
        let length = match settings.view {
            View::Quick => 5,
            View::Source => settings.sources.len(),
            View::Edit => apply + 2,
        };
        let start = settings.index / PAGE * PAGE;
        for index in start..(start + PAGE).min(length) {
            let ui_row_start = actors.len();
            let checked;
            let name = if matches!(settings.view, View::Quick) {
                checked = false;
                i18n::tr(
                    "Edition",
                    [
                        "SongTimingApplyTiming",
                        "SongTimingApplyTimingScroll",
                        "SongTimingApplyStepmania",
                        "SongTimingApplyAdvanced",
                        "ChartEventCopyCancel",
                    ][index],
                )
                .to_string()
            } else if matches!(settings.view, View::Source) {
                checked = settings.sources[index].0 == settings.source;
                settings.sources[index].1.clone()
            } else if index == 0 {
                checked = false;
                i18n::tr("Edition", "ChartEventCopyChooseSource").to_string()
            } else if let Some(kind) = EventType::ALL.get(index - 1) {
                checked = settings.types.contains(kind);
                i18n::tr("Edition", type_key(*kind)).to_string()
            } else if let Some((chart, description)) =
                settings.choices.get(index - 1 - EventType::ALL.len())
            {
                checked = settings.destinations.contains(chart);
                i18n::tr_fmt(
                    "Edition",
                    "ChartEventCopyDestination",
                    &[("chart", description)],
                )
                .to_string()
            } else {
                checked = false;
                i18n::tr(
                    "Edition",
                    if index == apply {
                        "ChartEventCopyApply"
                    } else {
                        "ChartEventCopyCancel"
                    },
                )
                .to_string()
            };
            let text = if matches!(settings.view, View::Source)
                || (matches!(settings.view, View::Edit) && index > 0 && index < apply)
            {
                format!("[{}] {name}", if checked { "x" } else { " " })
            } else {
                name
            };
            label(
                actors,
                text,
                top + 80.0 + ((index - start) * 28) as f32,
                if index == settings.index {
                    [0.2, 0.9, 0.9, 1.0]
                } else {
                    [1.0; 4]
                },
            );

            self.ui_row(actors, ui_row_start, index == settings.index, 28.);
        }
        label(
            actors,
            i18n::tr_fmt(
                "Edition",
                "ChartEventCopyPage",
                &[
                    ("page", &(settings.index / PAGE + 1).to_string()),
                    ("pages", &length.div_ceil(PAGE).to_string()),
                ],
            )
            .to_string(),
            top + 315.0,
            [0.7, 0.8, 0.9, 1.0],
        );
        label(
            actors,
            i18n::tr(
                "Edition",
                if matches!(settings.view, View::Quick) {
                    "SongTimingApplyHelp"
                } else if matches!(settings.view, View::Source) {
                    "ChartEventCopySourceHelp"
                } else {
                    "ChartEventCopyHelp"
                },
            )
            .to_string(),
            top + 345.0,
            [0.7, 0.8, 0.9, 1.0],
        );
    }
}
fn type_key(kind: EventType) -> &'static str {
    match kind {
        EventType::Signature => "EventSelectionSignature",
        EventType::Tempo => "EventSelectionTempo",
        EventType::Stop => "EventSelectionStop",
        EventType::Delay => "EventSelectionDelay",
        EventType::Warp => "EventSelectionWarp",
        EventType::Scroll => "EventSelectionScroll",
        EventType::Speed => "EventSelectionSpeed",
        EventType::Fake => "EventSelectionFake",
        EventType::Combo => "EventSelectionCombo",
        EventType::Tick => "EventSelectionTick",
        EventType::Label => "EventSelectionLabel",
        EventType::Attack => "EventSelectionAttack",
        EventType::Pattern => "EventSelectionPattern",
    }
}

#[cfg(test)]
mod source_presets_tests {
    use super::*;
    use deadlib_platform::input::RawKeyboardEvent;
    fn press(state: &mut State, code: KeyCode) {
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
    fn source_open_and_four_presets_are_exact_and_apply_is_one_undo_en_fr() {
        for lang in ["en", "fr"] {
            i18n::init(deadsync_assets::language::load_for_tests(lang));
            i18n::set_locale(deadsync_assets::language::load_for_tests(lang));
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("copy.ssc");
            std::fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=180,4=240;#LABELS:1=source;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=90;#LABELS:3=old;#NOTES:0100;").unwrap();
            let mut state = State::open(&path, 0).unwrap();
            let before = state.workspace.file().session.serialize().unwrap();
            state.open_chart_event_copy();
            let settings = state.chart_event_copy.as_ref().unwrap();
            assert_eq!(settings.types.len(), 12);
            assert!(!settings.types.contains(&EventType::Pattern));
            assert!(settings.types.contains(&EventType::Attack));
            for (key, expected) in [
                (
                    KeyCode::F3,
                    vec![
                        EventType::Signature,
                        EventType::Tempo,
                        EventType::Stop,
                        EventType::Delay,
                        EventType::Warp,
                    ],
                ),
                (
                    KeyCode::F4,
                    vec![
                        EventType::Signature,
                        EventType::Tempo,
                        EventType::Stop,
                        EventType::Delay,
                        EventType::Warp,
                        EventType::Scroll,
                        EventType::Speed,
                    ],
                ),
                (KeyCode::F5, EventType::ALL.to_vec()),
                (
                    KeyCode::F2,
                    EventType::ALL
                        .into_iter()
                        .filter(|k| *k != EventType::Pattern)
                        .collect(),
                ),
            ] {
                press(&mut state, key);
                assert_eq!(
                    state.chart_event_copy.as_ref().unwrap().types,
                    expected.into_iter().collect()
                );
                assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
                assert!(state.workspace.file().session.undo_label().is_none());
            }
            // Navigate to Apply through actual panel keys, retaining the source
            // StepMania default (patterns remain opt-in).
            press(&mut state, KeyCode::End);
            press(&mut state, KeyCode::ArrowUp);
            press(&mut state, KeyCode::Enter);
            assert!(state.panel.is_none());
            assert_eq!(
                state
                    .workspace
                    .file()
                    .session
                    .document()
                    .text(Some(1), "BPMS"),
                Some("0=180,4=240".into())
            );
            assert_eq!(
                state
                    .workspace
                    .file()
                    .session
                    .document()
                    .text(Some(1), "LABELS"),
                Some("1=source".into())
            );
            let after = state.workspace.file().session.serialize().unwrap();
            assert_ne!(before, after);
            state.command(deadsync_editor::workspace::Command::Undo);
            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
            assert!(state.workspace.file().session.undo_label().is_none());
            state.command(deadsync_editor::workspace::Command::Redo);
            assert_eq!(state.workspace.file().session.serialize().unwrap(), after);
        }
    }
}
