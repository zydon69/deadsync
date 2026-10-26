use super::{Panel, State};
use crate::{act, i18n};
use deadlib_platform::input::KeyCode;
use deadlib_present::{
    actors::Actor,
    space::{screen_height, screen_width},
};
use deadsync_editor::event_clipboard::{Event, Selection, TimingKind};
const PAGE: usize = 8;
pub(super) struct Settings {
    selection: Selection,
    index: usize,
}
impl State {
    pub(super) fn open_event_selection(&mut self) {
        if self.is_generating() {
            return;
        }
        match self.workspace.event_selection() {
            Ok(selection) => {
                self.event_selection = Some(Settings {
                    selection,
                    index: 0,
                });
                self.panel = Some(Panel::EventSelection);
                self.status = i18n::tr("Edition", "EventSelectionHelp");
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
    }
    pub(super) fn event_selection_key(&mut self, code: KeyCode) {
        if code == KeyCode::Escape {
            self.event_selection = None;
            self.panel = None;
            self.status = i18n::tr("Edition", "Ready");
            return;
        }
        let Some(mut settings) = self.event_selection.take() else {
            return;
        };
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
                KeyCode::PageUp => settings.index = settings.index.saturating_sub(PAGE),
                KeyCode::PageDown => {
                    settings.index = (settings.index + PAGE).min(length.saturating_sub(1))
                }
                KeyCode::Home => settings.index = 0,
                KeyCode::End => settings.index = length.saturating_sub(1),
                KeyCode::Space | KeyCode::Enter if length > 0 => {
                    let checked = !settings.selection.is_selected(settings.index);
                    settings.selection.set_selected(settings.index, checked)?;
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
            Ok("EventSelectionHelp")
        })();
        match result {
            Ok(key) => self.status = i18n::tr("Edition", key),
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
        if close {
            self.panel = None;
            self.refresh();
        } else {
            self.event_selection = Some(settings);
        }
    }
    pub(super) fn event_selection_actors(&self, actors: &mut Vec<Actor>) {
        let Some(settings) = &self.event_selection else {
            return;
        };
        let width = screen_width();
        let height = screen_height();
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
                ("page", &(settings.index / PAGE + 1).to_string()),
                ("pages", &total.div_ceil(PAGE).max(1).to_string()),
            ],
        );
        label(actors, &summary, 114.0, [0.75, 0.8, 0.85, 1.0]);
        let start = settings.index / PAGE * PAGE;
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
            .take(PAGE)
            .enumerate()
        {
            let index = start + offset;
            let y = 146.0 + offset as f32 * 25.0;
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
                Event::Attack { .. } => i18n::tr("Edition", "EventSelectionAttack"),
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
                    ("beat", &format!("{:.3}", event.row() as f64 / 48.0)),
                    ("name", &name),
                ],
            );
            label(actors, &text, y, color);
        }
        label(
            actors,
            &i18n::tr("Edition", "EventSelectionHelp"),
            height - 132.0,
            [0.7, 0.8, 0.9, 1.0],
        );
    }
}
fn label(actors: &mut Vec<Actor>, value: &str, y: f32, color: [f32; 4]) {
    actors.push(act!(text: font("miso"): settext(value.to_owned()): align(0.0, 0.0): xy(42.0, y): zoom(0.60): diffuse(color[0], color[1], color[2], color[3]): z(8012)));
}
fn timing_key(kind: TimingKind) -> &'static str {
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
