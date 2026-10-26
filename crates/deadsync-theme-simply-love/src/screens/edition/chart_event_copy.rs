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
pub(super) struct Settings {
    binding: Binding,
    source: usize,
    index: usize,
    types: BTreeSet<EventType>,
    destinations: BTreeSet<usize>,
    choices: Vec<(usize, String)>,
}
impl State {
    pub(super) fn open_chart_event_copy(&mut self) {
        if self.is_generating() || !self.workspace.has_chart() {
            return;
        }
        let source = self.workspace.chart();
        let choices: Vec<_> = self
            .workspace
            .file()
            .session
            .document()
            .charts()
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != source)
            .map(|(index, chart)| {
                (
                    index,
                    format!(
                        "#{} {} | {} {}",
                        index + 1,
                        chart.steps_type,
                        chart.difficulty,
                        chart.meter
                    ),
                )
            })
            .collect();
        if choices.is_empty() {
            self.status = i18n::tr("Edition", "ChartEventCopyNoDestination");
            return;
        }
        let binding = match Binding::capture(&self.workspace.file().session, source) {
            Ok(binding) => binding,
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())]);
                return;
            }
        };
        self.stop_audio();
        self.chart_event_copy = Some(Settings {
            binding,
            source,
            index: 0,
            types: BTreeSet::new(),
            destinations: choices.iter().map(|(index, _)| *index).collect(),
            choices,
        });
        self.panel = Some(Panel::ChartEventCopy);
        self.status = i18n::tr("Edition", "ChartEventCopyHelp");
    }
    pub(super) fn chart_event_copy_key(&mut self, code: KeyCode) {
        if code == KeyCode::Escape {
            self.chart_event_copy = None;
            self.panel = None;
            self.status = i18n::tr("Edition", "Ready");
            return;
        }
        let Some(mut settings) = self.chart_event_copy.take() else {
            return;
        };
        if !settings
            .binding
            .is_current(&self.workspace.file().session, self.workspace.chart())
        {
            self.panel = None;
            self.status = i18n::tr("Edition", "ChartEventCopySourceChanged");
            return;
        }
        let apply = EventType::ALL.len() + settings.choices.len();
        let length = apply + 2;
        let mut close = false;
        match code {
            KeyCode::ArrowUp => settings.index = settings.index.saturating_sub(1),
            KeyCode::ArrowDown => settings.index = (settings.index + 1).min(length - 1),
            KeyCode::PageUp => settings.index = settings.index.saturating_sub(PAGE),
            KeyCode::PageDown => settings.index = (settings.index + PAGE).min(length - 1),
            KeyCode::Home => settings.index = 0,
            KeyCode::End => settings.index = length - 1,
            KeyCode::KeyA if self.control => {
                settings.types = if self.shift {
                    BTreeSet::new()
                } else {
                    EventType::ALL.into_iter().collect()
                }
            }
            KeyCode::Space | KeyCode::Enter | KeyCode::NumpadEnter => {
                if let Some(kind) = EventType::ALL.get(settings.index) {
                    if !settings.types.remove(kind) {
                        settings.types.insert(*kind);
                    }
                } else if let Some((chart, _)) =
                    settings.choices.get(settings.index - EventType::ALL.len())
                {
                    if !settings.destinations.remove(chart) {
                        settings.destinations.insert(*chart);
                    }
                } else if settings.index == apply {
                    if settings.types.is_empty() {
                        self.status = i18n::tr("Edition", "ChartEventCopyNoTypes");
                    } else if settings.destinations.is_empty() {
                        self.status = i18n::tr("Edition", "ChartEventCopyNoDestination");
                    } else if self.workspace.chart() != settings.source {
                        self.status = i18n::tr("Edition", "ChartEventCopySourceChanged");
                        close = true;
                    } else {
                        match self.workspace.copy_chart_events(
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
                                        .map(|kind| {
                                            i18n::tr("Edition", type_key(*kind)).to_string()
                                        })
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
                            Err(error) => {
                                self.status = i18n::tr_fmt(
                                    "Edition",
                                    "Error",
                                    &[("message", &error.to_string())],
                                )
                            }
                        }
                    }
                } else {
                    close = true;
                    self.status = i18n::tr("Edition", "Ready");
                }
            }
            _ => {}
        }
        if close {
            self.panel = None;
            self.refresh();
        } else {
            self.chart_event_copy = Some(settings);
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
            i18n::tr("Edition", "ChartEventCopyTitle").to_string(),
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
        let apply = EventType::ALL.len() + settings.choices.len();
        let length = apply + 2;
        let start = settings.index / PAGE * PAGE;
        for index in start..(start + PAGE).min(length) {
            let checked;
            let name = if let Some(kind) = EventType::ALL.get(index) {
                checked = settings.types.contains(kind);
                i18n::tr("Edition", type_key(*kind)).to_string()
            } else if let Some((chart, description)) =
                settings.choices.get(index - EventType::ALL.len())
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
            let text = if index < apply {
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
            i18n::tr("Edition", "ChartEventCopyHelp").to_string(),
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
