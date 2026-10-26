use super::{Panel, State};
use crate::i18n;
use deadlib_platform::input::KeyCode;
use deadsync_editor::transform::Transform;
#[derive(Default)]
pub(super) struct Settings {
    swap: bool,
    pub a: u32,
    pub b: u32,
}
impl State {
    pub(super) fn note_owners_key(&mut self, index: usize, code: KeyCode) {
        match code {
            KeyCode::Escape => self.panel = Some(Panel::Transforms(10)),
            KeyCode::ArrowUp => self.panel = Some(Panel::NoteOwners((index + 4) % 5)),
            KeyCode::ArrowDown | KeyCode::Tab => {
                self.panel = Some(Panel::NoteOwners((index + 1) % 5))
            }
            KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Enter if index < 3 => {
                if index == 0 {
                    self.note_owners.swap = !self.note_owners.swap;
                } else {
                    let count = self.workspace.grid().players();
                    let value = if index == 1 {
                        &mut self.note_owners.a
                    } else {
                        &mut self.note_owners.b
                    };
                    *value = (*value
                        + if code == KeyCode::ArrowLeft {
                            count - 1
                        } else {
                            1
                        })
                        % count;
                }
            }
            KeyCode::Enter if index == 3 => {
                let settings = &self.note_owners;
                let transform = if settings.swap {
                    Transform::SwapPlayers(settings.a, settings.b)
                } else {
                    Transform::AssignPlayer(settings.a)
                };
                self.apply_dialog_transform(
                    transform,
                    "NoteOwnersApplied",
                    "NoteOwnersNoChange",
                    "NoteOwnersFailed",
                );
            }
            KeyCode::Enter => self.panel = Some(Panel::Transforms(10)),
            _ => {}
        }
    }
    pub(super) fn note_owners_label(&self, index: usize, key: &str) -> std::sync::Arc<str> {
        let value = match index {
            0 => i18n::tr(
                "Edition",
                if self.note_owners.swap {
                    "NoteOwnersSwap"
                } else {
                    "NoteOwnersAssign"
                },
            )
            .to_string(),
            1 => (self.note_owners.a + 1).to_string(),
            2 => (self.note_owners.b + 1).to_string(),
            _ => return i18n::tr("Edition", key),
        };
        i18n::tr_fmt("Edition", key, &[("value", &value)])
    }
}
