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
                self.position.set_offset(mount, value.clamp(-1024, 1024));
            }
            Field::Width => {
                self.content.width = uint(self.content.width, default.content.width, 2, 128);
            }
            Field::NoteWidth => {
                self.content.note_width = uint(
                    self.content.note_width,
                    default.content.note_width,
                    1,
                    32,
                )
            }
            Field::NoteSpacing => {
                self.content.note_spacing = uint(
                    self.content.note_spacing,
                    default.content.note_spacing,
                    0,
                    32,
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
                    1,
                    128,
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
                    1,
                    128,
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

#[cfg(test)]
mod source_ui_bounds_tests {
    use super::*;
    #[test]
    fn source_slider_bounds_are_independent_and_shrinking_map_never_rewrites_note_width(){
        let mut draft=Draft{position:Default::default(),content:Default::default()};
        for (field,min,max) in [(Field::Width,2,128),(Field::NoteWidth,1,32),(Field::NoteSpacing,0,32),(Field::PatternsWidth,1,128),(Field::PreviewWidth,1,128)]{
            for _ in 0..2048{draft.adjust(field,-1,false);}
            let value=|d:Draft|match field{Field::Width=>d.content.width,Field::NoteWidth=>d.content.note_width,Field::NoteSpacing=>d.content.note_spacing,Field::PatternsWidth=>d.content.patterns_width,Field::PreviewWidth=>d.content.preview_width,_=>unreachable!()};assert_eq!(value(draft),min);
            for _ in 0..2048{draft.adjust(field,1,false);}assert_eq!(value(draft),max);
        }
        draft.content.note_width=32;draft.content.width=3;draft.adjust(Field::Width,-1,false);assert_eq!(draft.content.width,2);assert_eq!(draft.content.note_width,32);
        for _ in 0..4096{draft.adjust(Field::Offset,-1,false);}assert_eq!(draft.position.offset(draft.position.mount),-1024);
        for _ in 0..4096{draft.adjust(Field::Offset,1,false);}assert_eq!(draft.position.offset(draft.position.mount),1024);
    }
}
