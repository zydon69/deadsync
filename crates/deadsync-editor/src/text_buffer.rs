//! Single-line Unicode editing state. Positions are byte offsets at grapheme boundaries.
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;
#[derive(Clone, Debug, PartialEq, Eq)]
struct Snapshot {
    value: String,
    cursor: usize,
    anchor: usize,
}
pub struct Buffer {
    current: Snapshot,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}
#[derive(Clone, Copy)]
pub enum Movement {
    Left,
    Right,
    Start,
    End,
    WordLeft,
    WordRight,
}
impl Buffer {
    pub fn new(value: &str, select_all: bool) -> Self {
        Self {
            current: Snapshot {
                value: value.to_owned(),
                cursor: value.len(),
                anchor: if select_all { 0 } else { value.len() },
            },
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }
    pub fn value(&self) -> &str {
        &self.current.value
    }
    pub fn cursor(&self) -> usize {
        self.current.cursor
    }
    pub fn selection(&self) -> Range<usize> {
        self.current.cursor.min(self.current.anchor)..self.current.cursor.max(self.current.anchor)
    }
    pub fn select_all(&mut self) {
        self.current.anchor = 0;
        self.current.cursor = self.current.value.len();
    }
    pub fn move_cursor(&mut self, movement: Movement, extend: bool) {
        let range = self.selection();
        let position = match movement {
            Movement::Left if !extend && !range.is_empty() => range.start,
            Movement::Right if !extend && !range.is_empty() => range.end,
            Movement::Left => self
                .boundaries()
                .into_iter()
                .rev()
                .find(|p| *p < self.current.cursor)
                .unwrap_or(0),
            Movement::Right => self
                .boundaries()
                .into_iter()
                .find(|p| *p > self.current.cursor)
                .unwrap_or(self.current.value.len()),
            Movement::Start => 0,
            Movement::End => self.current.value.len(),
            Movement::WordLeft => self
                .current
                .value
                .unicode_word_indices()
                .map(|(p, _)| p)
                .rev()
                .find(|p| *p < self.current.cursor)
                .unwrap_or(0),
            Movement::WordRight => self
                .current
                .value
                .unicode_word_indices()
                .map(|(p, w)| p + w.len())
                .find(|p| *p > self.current.cursor)
                .unwrap_or(self.current.value.len()),
        };
        self.current.cursor = position;
        if !extend {
            self.current.anchor = position;
        }
    }
    pub fn insert(&mut self, text: &str) -> bool {
        let value: String = text.chars().filter(|c| !c.is_control()).collect();
        if value.is_empty() {
            return false;
        }
        self.replace(self.selection(), &value)
    }
    pub fn backspace(&mut self, word: bool) -> bool {
        let mut range = self.selection();
        if range.is_empty() {
            let end = self.current.cursor;
            self.move_cursor(
                if word {
                    Movement::WordLeft
                } else {
                    Movement::Left
                },
                true,
            );
            range = self.selection();
            self.current.cursor = end;
        }
        self.replace(range, "")
    }
    pub fn delete(&mut self, word: bool) -> bool {
        let mut range = self.selection();
        if range.is_empty() {
            let start = self.current.cursor;
            self.move_cursor(
                if word {
                    Movement::WordRight
                } else {
                    Movement::Right
                },
                true,
            );
            range = self.selection();
            self.current.cursor = start;
        }
        self.replace(range, "")
    }
    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.redo
            .push(std::mem::replace(&mut self.current, previous));
        true
    }
    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(std::mem::replace(&mut self.current, next));
        true
    }
    /// Preedit is shown over the selection without changing value or history.
    /// The viewport follows the caret and keeps long field labels bounded.
    pub fn display(&self, preedit: &str, limit: usize) -> String {
        let range = self.selection();
        let (text, caret) = if preedit.is_empty() {
            (self.current.value.clone(), self.current.cursor)
        } else {
            let prefix = &self.current.value[..range.start];
            (
                format!("{prefix}{preedit}{}", &self.current.value[range.end..]),
                prefix.len() + preedit.len(),
            )
        };
        let points: Vec<_> = text
            .grapheme_indices(true)
            .map(|(p, _)| p)
            .chain(std::iter::once(text.len()))
            .collect();
        let caret_index = points.partition_point(|p| *p < caret).min(points.len() - 1);
        let count = points.len() - 1;
        let limit = limit.max(1);
        let start = caret_index
            .saturating_sub(limit.saturating_sub(1))
            .min(count.saturating_sub(limit));
        let end = (start + limit).min(count);
        let left = points[start];
        let right = points[end];
        let selected = if preedit.is_empty() {
            self.selection()
        } else {
            caret..caret
        };
        let mut result = String::new();
        if start > 0 {
            result.push_str("...");
        }
        for (offset, ch) in text[left..right]
            .char_indices()
            .map(|(i, c)| (left + i, Some(c)))
            .chain(std::iter::once((right, None)))
        {
            if !selected.is_empty() && offset == selected.end.min(right) && offset > selected.start
            {
                result.push(']');
            }
            if offset == caret {
                result.push('|');
            }
            if !selected.is_empty() && offset == selected.start.max(left) && offset < selected.end {
                result.push('[');
            }
            if let Some(ch) = ch {
                if ch.is_control() {
                    result.extend(ch.escape_default());
                } else {
                    result.push(ch);
                }
            }
        }
        if end < count {
            result.push_str("...");
        }
        result
    }

    fn boundaries(&self) -> Vec<usize> {
        self.current
            .value
            .grapheme_indices(true)
            .map(|(p, _)| p)
            .chain(std::iter::once(self.current.value.len()))
            .collect()
    }
    fn replace(&mut self, range: Range<usize>, value: &str) -> bool {
        if range.is_empty() && value.is_empty() {
            return false;
        }
        if self.current.value[range.clone()] == *value {
            self.current.cursor = range.start + value.len();
            self.current.anchor = self.current.cursor;
            return false;
        }
        if self.undo.len() == 256 {
            self.undo.remove(0);
        }
        self.undo.push(self.current.clone());
        self.redo.clear();
        self.current.value.replace_range(range.clone(), value);
        let desired = range.start + value.len();
        self.current.cursor = self
            .boundaries()
            .into_iter()
            .find(|p| *p >= desired)
            .unwrap_or(self.current.value.len());
        self.current.anchor = self.current.cursor;
        true
    }
}
