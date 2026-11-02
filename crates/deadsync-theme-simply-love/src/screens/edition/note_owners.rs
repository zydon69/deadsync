use super::{Panel, State};
use crate::i18n;
use deadlib_platform::input::KeyCode;
use deadsync_editor::transform::Transform;
#[derive(Default)]
pub(super) struct Settings {
    mode: usize,
    pub a: u32,
    pub b: u32,
    pub count: u32,
}
impl State {
    pub(super) fn note_owners_key(&mut self, index: usize, code: KeyCode) {
        match code {
            KeyCode::Escape => self.panel = Some(Panel::Transforms(10)),
            KeyCode::ArrowUp => self.panel = Some(Panel::NoteOwners((index + 5) % 6)),
            KeyCode::ArrowDown | KeyCode::Tab => {
                self.panel = Some(Panel::NoteOwners((index + 1) % 6))
            }
            KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Enter if index < 4 => {
                if index == 3 {
                    let layout = &self.workspace.file().session.document().charts()
                        [self.workspace.chart()]
                    .steps_type;
                    if deadsync_editor::notes::supports_variable_players(layout) {
                        let minimum = self.workspace.grid().minimum_players();
                        self.note_owners.count = if code == KeyCode::ArrowLeft {
                            self.note_owners.count.saturating_sub(1).max(minimum)
                        } else {
                            self.note_owners.count.saturating_add(1)
                        };
                        // Source UIChartProperties drag bounds are 2..128. Keep
                        // the existing native non-discard guard for sparse owners.
                        self.note_owners.count =
                            self.note_owners.count.max(2).min(128).max(minimum);
                        self.note_owners.a = self.note_owners.a.min(self.note_owners.count - 1);
                        self.note_owners.b = self.note_owners.b.min(self.note_owners.count - 1);
                    }
                } else if index == 0 {
                    self.note_owners.mode = (self.note_owners.mode
                        + if code == KeyCode::ArrowLeft { 2 } else { 1 })
                        % 3;
                } else {
                    let count = self.note_owners.count;
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
            KeyCode::Enter if index == 4 => {
                let settings = &self.note_owners;
                let count = settings.count;
                if settings.mode == 2 {
                    self.apply_dialog_operation(
                        |workspace| workspace.set_participants(count).map(|_| ()),
                        "NoteOwnersApplied",
                        "NoteOwnersNoChange",
                        "NoteOwnersFailed",
                    );
                } else {
                    let transform = if settings.mode == 1 {
                        Transform::SwapPlayers(settings.a, settings.b)
                    } else {
                        Transform::AssignPlayer(settings.a)
                    };
                    self.apply_dialog_operation(
                        |workspace| workspace.change_note_owners(count, transform).map(|_| ()),
                        "NoteOwnersApplied",
                        "NoteOwnersNoChange",
                        "NoteOwnersFailed",
                    );
                }
            }
            KeyCode::Enter => self.panel = Some(Panel::Transforms(10)),
            _ => {}
        }
    }
    pub(super) fn note_owners_label(&self, index: usize, key: &str) -> std::sync::Arc<str> {
        let value = match index {
            0 => i18n::tr(
                "Edition",
                if self.note_owners.mode == 2 {
                    "NoteOwnersCountOnly"
                } else if self.note_owners.mode == 1 {
                    "NoteOwnersSwap"
                } else {
                    "NoteOwnersAssign"
                },
            )
            .to_string(),
            1 => (self.note_owners.a + 1).to_string(),
            2 => (self.note_owners.b + 1).to_string(),
            3 => self.note_owners.count.to_string(),
            _ => return i18n::tr("Edition", key),
        };
        i18n::tr_fmt("Edition", key, &[("value", &value)])
    }
}

#[cfg(test)]
mod source_property_ui_bounds_tests {
    use super::*;
    #[test]
    fn participant_widget_uses_source_bounds_without_restricting_loaded_owner() {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.ssc");
        std::fs::write(&path,"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-routine;#DIFFICULTY:Easy;#METER:1;#NOTES:00000000;").unwrap();
        let mut state = State::open(&path, 0).unwrap();
        state.command_open_note_owners();
        state.note_owners.count = 2;
        state.note_owners_key(3, KeyCode::ArrowLeft);
        assert_eq!(state.note_owners.count, 2);
        state.note_owners.count = 128;
        state.note_owners_key(3, KeyCode::ArrowRight);
        assert_eq!(state.note_owners.count, 128);
        // Source MaxPlayers setter accepts values above its UI maximum; loading is
        // not a UI edit and must keep that value until the user adjusts it.
        state.workspace.set_participants(129).unwrap();
        state.command_open_note_owners();
        assert_eq!(state.note_owners.count, 129);
        state.note_owners_key(3, KeyCode::ArrowLeft);
        assert_eq!(state.note_owners.count, 128);
    }
}
