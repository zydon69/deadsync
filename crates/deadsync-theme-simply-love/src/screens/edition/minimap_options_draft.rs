//! Optional-panel draft: source groups remain distinct and song history untouched.
use deadsync_editor::{
    minimap_content_preferences::Preferences as Content,
    minimap_position_preferences::{Mount, Preferences as Position, SelectMode},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Draft {
    pub position: Position,
    pub content: Content,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Field {
    Visible,
    Mount,
    SelectMode,
    Offset,
    Width,
    NoteWidth,
    NoteSpacing,
    Patterns,
    PatternsWidth,
    Preview,
    PreviewWidth,
    Labels,
    MaxNotes,
}
impl Field {
    pub const ALL: [Self; 13] = [
        Self::Visible,
        Self::Mount,
        Self::SelectMode,
        Self::Offset,
        Self::Width,
        Self::NoteWidth,
        Self::NoteSpacing,
        Self::Patterns,
        Self::PatternsWidth,
        Self::Preview,
        Self::PreviewWidth,
        Self::Labels,
        Self::MaxNotes,
    ];
}
impl Draft {
    pub fn adjust(&mut self, field: Field, direction: i32, reset: bool) {
        let default = Self {
            position: Position::default(),
            content: Content::default(),
        };
        let step = if direction < 0 { -1i64 } else { 1 };
        let uint = |value: u32, original: u32, min: u32, max: u32| {
            if reset {
                original.clamp(min, max)
            } else {
                (i64::from(value) + step).clamp(i64::from(min), i64::from(max)) as u32
            }
        };
        match field {
            Field::Visible => {
                self.content.visible = if reset {
                    default.content.visible
                } else {
                    !self.content.visible
                }
            }
            Field::Mount => {
                self.position.mount = if reset {
                    default.position.mount
                } else {
                    let index = Mount::ALL
                        .iter()
                        .position(|mount| *mount == self.position.mount)
                        .unwrap();
                    Mount::ALL[(index as i64 + step).rem_euclid(4) as usize]
                };
            }
            Field::SelectMode => {
                self.position.select_mode =
                    if reset || self.position.select_mode == SelectMode::MoveToSelectedArea {
                        SelectMode::MoveToCursor
                    } else {
                        SelectMode::MoveToSelectedArea
                    }
            }
            Field::Offset => {
                let mount = self.position.mount;
                let value = if reset {
                    default.position.offset(mount)
                } else {
                    self.position
                        .offset(mount)
                        .saturating_add(direction.signum())
                };
                self.position.set_offset(mount, value);
            }
            Field::Width => {
                self.content.width = uint(self.content.width, default.content.width, 3, 1024);
                self.content.note_width = self.content.note_width.min(self.content.width - 2);
            }
            Field::NoteWidth => {
                self.content.note_width = uint(
                    self.content.note_width,
                    default.content.note_width,
                    0,
                    self.content.width - 2,
                )
            }
            Field::NoteSpacing => {
                self.content.note_spacing = uint(
                    self.content.note_spacing,
                    default.content.note_spacing,
                    0,
                    1024,
                )
            }
            Field::Patterns => {
                self.content.patterns = if reset {
                    default.content.patterns
                } else {
                    !self.content.patterns
                }
            }
            Field::PatternsWidth => {
                self.content.patterns_width = uint(
                    self.content.patterns_width,
                    default.content.patterns_width,
                    0,
                    1024,
                )
            }
            Field::Preview => {
                self.content.preview = if reset {
                    default.content.preview
                } else {
                    !self.content.preview
                }
            }
            Field::PreviewWidth => {
                self.content.preview_width = uint(
                    self.content.preview_width,
                    default.content.preview_width,
                    0,
                    1024,
                )
            }
            Field::Labels => {
                self.content.labels = if reset {
                    default.content.labels
                } else {
                    !self.content.labels
                }
            }
            Field::MaxNotes => {
                self.content.max_notes = if reset {
                    default.content.max_notes
                } else {
                    (self.content.max_notes as i64 + step * 128).clamp(0, 65535) as usize
                }
            }
        }
    }
}
