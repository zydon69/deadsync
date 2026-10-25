// Compile the actual adapter and run its IO-free lifetime contract. The host's
// clipboard is intentionally neither inspected nor overwritten by this test.
#[path = "../src/editor_clipboard.rs"]
mod editor_clipboard;
