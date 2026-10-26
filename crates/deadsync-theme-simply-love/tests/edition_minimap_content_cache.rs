#[path = "../src/screens/edition/minimap_content_cache.rs"]
mod minimap_content_cache;
#[path = "../src/screens/edition/minimap_content_render.rs"]
mod minimap_content_render;
use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    minimap_viewport::Axis,
};
use minimap_content_cache::{Cache, Frame, Key};
use minimap_content_render::Mesh;
use std::{cell::Cell, sync::Arc};
fn session() -> Session {
    Session::new(
        Document::parse(
            b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;".as_slice(),
            Format::Ssc,
        )
        .unwrap(),
    )
}
fn key() -> Key {
    Key {
        axis: Axis::Time,
        projection: [0; 9],
        width: 16,
        height: 16,
        focused: false,
        captured: false,
        selection: None,
        player: 0,
        palette_revision: 0,
        options_revision: 0,
    }
}
fn frame() -> Frame {
    Frame {
        mesh: Mesh::build(16, 16, &vec![0xff123456; 256], 32).unwrap(),
        notes_complete: true,
        markers_complete: true,
        note_count: 4,
        tree_queries: 10,
    }
}
#[test]
fn unchanged_frames_reuse_arc_and_edits_undo_selection_palette_invalidate() {
    let mut session = session();
    let cache = Cache::default();
    let builds = Cell::new(0);
    let make = || {
        builds.set(builds.get() + 1);
        Ok(frame())
    };
    let first = cache.frame(&session, 0, key(), make).unwrap();
    let same = cache.frame(&session, 0, key(), make).unwrap();
    assert!(Arc::ptr_eq(&first, &same));
    assert_eq!(builds.get(), 1);
    assert!(cache.current(&session, 0, &key()).unwrap().is_ok());
    assert!(first.complete());
    assert_eq!((first.note_count, first.tree_queries), (4, 10));
    let mut selected = key();
    selected.selection = Some((0, 48));
    assert!(cache.current(&session, 0, &selected).is_none());
    let changed = cache.frame(&session, 0, selected, make).unwrap();
    assert!(!Arc::ptr_eq(&first, &changed));
    let mut palette = key();
    palette.palette_revision = 1;
    let _ = cache.frame(&session, 0, palette, make).unwrap();
    assert_eq!(builds.get(), 3);
    session.set_text(Some(0), "BPMS", "0=60").unwrap();
    assert!(cache.current(&session, 0, &key()).is_none());
    let _ = cache.frame(&session, 0, key(), make).unwrap();
    assert_eq!(builds.get(), 4);
    session.undo().unwrap();
    let _ = cache.frame(&session, 0, key(), make).unwrap();
    assert_eq!(
        builds.get(),
        5,
        "undo content cannot reuse another content token's frame"
    );
}
#[test]
fn failures_are_cached_per_binding_and_clear_releases_retained_frames() {
    let session = session();
    let mut cache = Cache::default();
    let builds = Cell::new(0);
    for _ in 0..3 {
        assert!(
            cache
                .frame(&session, 0, key(), || {
                    builds.set(builds.get() + 1);
                    Err(deadsync_editor::Error::InvalidEdit("failure".into()))
                })
                .is_err()
        );
    }
    assert_eq!(builds.get(), 1);
    cache.clear();
    let first = cache.frame(&session, 0, key(), || Ok(frame())).unwrap();
    let weak = Arc::downgrade(&first);
    drop(first);
    assert!(weak.upgrade().is_some());
    cache.clear();
    assert!(weak.upgrade().is_none());
    let different = crate::session();
    let _ = cache.frame(&different, 0, key(), || Ok(frame())).unwrap();
}
