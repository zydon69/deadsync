#[path="../src/screens/edition/minimap_options_draft.rs"]
mod minimap_options_draft;
use minimap_options_draft::{Draft,Field};
#[test]
fn source_width_dependency_and_per_position_offsets_remain_valid() {
    let mut draft=Draft{position:Default::default(),content:Default::default()};
    for _ in 0..100 {draft.adjust(Field::Width,-1,false);}
    assert_eq!(draft.content.width,3);
    assert!(draft.content.note_width<=1);
    draft.adjust(Field::NoteWidth,1,true);
    assert_eq!(draft.content.note_width,1);
    let first=draft.position.mount;
    draft.adjust(Field::Offset,-1,false);
    let saved=draft.position.offset(first);
    draft.adjust(Field::Mount,1,false);
    assert_ne!(draft.position.mount,first);
    draft.adjust(Field::Offset,1,false);
    assert_eq!(draft.position.offset(first),saved);
}
