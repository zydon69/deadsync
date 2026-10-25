use deadsync_editor::text_buffer::{Buffer, Movement};
use unicode_segmentation::UnicodeSegmentation;
fn invariant(buffer: &Buffer) {
    let mut boundaries: Vec<_> = buffer
        .value()
        .grapheme_indices(true)
        .map(|(i, _)| i)
        .collect();
    boundaries.push(buffer.value().len());
    let selected = buffer.selection();
    assert!(boundaries.contains(&buffer.cursor()));
    assert!(boundaries.contains(&selected.start));
    assert!(boundaries.contains(&selected.end));
}
#[test]
fn unicode_cursor_selection_replacement_and_deletion_never_split_graphemes() {
    let mut b = Buffer::new("e\u{301}👩‍👩‍👧‍👦🇫🇷日本語", false);
    b.move_cursor(Movement::Start, false);
    b.move_cursor(Movement::Right, true);
    assert_eq!(&b.value()[b.selection()], "e\u{301}");
    b.insert("É");
    invariant(&b);
    b.move_cursor(Movement::Right, true);
    assert_eq!(&b.value()[b.selection()], "👩‍👩‍👧‍👦");
    b.delete(false);
    assert_eq!(b.value(), "É🇫🇷日本語");
    b.undo();
    assert_eq!(b.value(), "É👩‍👩‍👧‍👦🇫🇷日本語");
    assert_eq!(&b.value()[b.selection()], "👩‍👩‍👧‍👦");
    b.move_cursor(Movement::End, false);
    b.backspace(false);
    assert_eq!(b.value(), "É👩‍👩‍👧‍👦🇫🇷日本");
    for text in ["a\u{301}", "👩‍👩‍👧‍👦", "🇫🇷", "日本語", "one... two", "\u{301}a"]
    {
        for movement in [
            Movement::Left,
            Movement::Right,
            Movement::Start,
            Movement::End,
            Movement::WordLeft,
            Movement::WordRight,
        ] {
            for extend in [true, false] {
                let mut b = Buffer::new(text, true);
                for _ in 0..8 {
                    b.move_cursor(movement, extend);
                    invariant(&b);
                }
                b.insert("\u{301}👩‍👩‍👧‍👦");
                invariant(&b);
                b.backspace(true);
                invariant(&b);
                b.delete(true);
                invariant(&b);
                b.undo();
                invariant(&b);
                b.redo();
                invariant(&b);
            }
        }
    }
}
#[test]
fn movement_collapses_selection_word_operations_and_middle_insertion_are_reversible() {
    let mut b = Buffer::new("one two three", false);
    b.move_cursor(Movement::WordLeft, false);
    assert_eq!(b.cursor(), 8);
    b.backspace(true);
    assert_eq!(b.value(), "one three");
    b.undo();
    assert_eq!(b.cursor(), 8);
    b.delete(true);
    assert_eq!(b.value(), "one two ");
    b.undo();
    b.move_cursor(Movement::WordLeft, true);
    assert_eq!(&b.value()[b.selection()], "two ");
    b.move_cursor(Movement::Left, false);
    assert_eq!(b.cursor(), 4);
    assert!(b.selection().is_empty());
    b.insert("new ");
    assert_eq!(b.value(), "one new two three");
    b.select_all();
    b.move_cursor(Movement::Right, false);
    assert_eq!(b.cursor(), b.value().len());
    b.move_cursor(Movement::Start, true);
    assert_eq!(&b.value()[b.selection()], b.value());
}
#[test]
fn noops_keep_redo_controls_are_ignored_and_history_is_bounded() {
    let mut b = Buffer::new("Old", true);
    assert!(!b.insert("\n\t\0"));
    assert_eq!(b.value(), "Old");
    b.insert("New");
    b.undo();
    let range = b.selection();
    assert_eq!(&b.value()[range], "Old");
    b.insert("Old");
    assert!(b.redo());
    assert_eq!(b.value(), "New");
    b.move_cursor(Movement::End, false);
    assert!(!b.delete(false));
    b.move_cursor(Movement::Start, false);
    assert!(!b.backspace(false));
    let mut b = Buffer::new("", false);
    for _ in 0..300 {
        b.insert("a");
    }
    let mut undos = 0;
    while b.undo() {
        undos += 1;
    }
    assert_eq!(undos, 256);
    assert_eq!(b.value().len(), 44);
    let mut redos = 0;
    while b.redo() {
        redos += 1;
    }
    assert_eq!(redos, 256);
    assert_eq!(b.value().len(), 300);
}
#[test]
fn viewport_follows_cursor_and_preedit_without_changing_value_selection_or_history() {
    assert_eq!(Buffer::new("a\nb\0", false).display("", 12), r"a\nb\u{0}|");
    let value = "日本語".repeat(40);
    let mut b = Buffer::new(&value, true);
    let selected = b.selection();
    let preedit = b.display("Été", 12);
    assert_eq!(preedit, "Été|");
    assert_eq!(b.value(), value);
    assert_eq!(b.selection(), selected);
    assert!(!b.undo());
    b.move_cursor(Movement::Start, false);
    let start = b.display("", 12);
    assert!(start.starts_with('|'));
    assert!(start.ends_with("..."));
    b.move_cursor(Movement::End, false);
    let end = b.display("", 12);
    assert!(end.starts_with("..."));
    assert!(end.ends_with('|'));
    assert!(end.graphemes(true).count() <= 16);
    b.move_cursor(Movement::Left, true);
    assert!(b.display("", 12).contains("[語]"));
    assert_eq!(b.value(), value);
    for limit in [0, 1, 12] {
        let display = Buffer::new("", false).display("", limit);
        assert_eq!(display, "|");
    }
}
