use deadsync_editor::{
    document::{Document, Format},
    event_clipboard::{Event, Selection},
    generation::metadata::ChartData,
    history::Session,
    quick_add::{Kind, Plan},
};
fn session() -> Session {
    Session::new(
        Document::parse(
            b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;".as_slice(),
            Format::Ssc,
        )
        .unwrap(),
    )
}
fn add(s: &mut Session, row: u64) {
    Plan::prepare(s, 0, Kind::Pattern, row as f64)
        .unwrap()
        .apply(s, 0)
        .unwrap();
}
fn additions(s: &mut Session) -> Vec<u64> {
    s.take_pattern_added_update()
        .map(|effect| {
            assert!(effect.is_current(s));
            effect.additions().map(|(_, row)| row).collect()
        })
        .unwrap_or_default()
}
fn clipboard(s: &mut Session, start: u64, end: u64) -> deadsync_editor::event_clipboard::Clipboard {
    let mut selected = Selection::capture(s, 0, start, end).unwrap();
    selected.select_all(false);
    for i in 0..selected.events().len() {
        if matches!(selected.events()[i], Event::Pattern { .. }) {
            selected.set_selected(i, true).unwrap();
        }
    }
    selected.copy(s).unwrap().unwrap()
}
#[test]
fn add_undo_redo_and_existing_definition_edit_publish_only_actual_readds() {
    let mut s = session();
    let plan = Plan::prepare(&s, 0, Kind::Pattern, 48.).unwrap();
    assert!(additions(&mut s).is_empty());
    plan.apply(&mut s, 0).unwrap();
    assert_eq!(additions(&mut s), [48]);
    s.undo().unwrap();
    assert!(additions(&mut s).is_empty());
    s.redo().unwrap();
    assert_eq!(additions(&mut s), [48]);
    let mut data = ChartData::read(s.document(), 0).unwrap().unwrap();
    data.patterns
        .as_mut()
        .unwrap()
        .get_mut(&48)
        .unwrap()
        .random_seed += 1;
    data.save(&mut s, 0).unwrap();
    assert!(additions(&mut s).is_empty());
    s.undo().unwrap();
    assert!(additions(&mut s).is_empty());
}
#[test]
fn multiple_paste_same_value_and_overwrite_history_retain_explicit_add_order() {
    let mut s = session();
    add(&mut s, 48);
    add(&mut s, 96);
    additions(&mut s);
    let clip = clipboard(&mut s, 48, 96);
    clip.paste_with_report(&mut s, 0, 144, 0).unwrap();
    assert_eq!(additions(&mut s), [144, 192]);
    s.undo().unwrap();
    assert!(additions(&mut s).is_empty());
    s.redo().unwrap();
    assert_eq!(additions(&mut s), [144, 192]);
    let revision = s.revision();
    let bytes = s.serialize().unwrap();
    let report = clip.paste_with_report(&mut s, 0, 144, 0).unwrap();
    assert!(report.changed);
    assert_eq!(additions(&mut s), [144, 192]);
    assert_ne!(s.revision(), revision);
    assert_eq!(s.serialize().unwrap(), bytes);
    s.undo().unwrap();
    assert_eq!(additions(&mut s), [144, 192]);
    assert_eq!(s.revision(), revision);
    assert_eq!(s.serialize().unwrap(), bytes);
    s.redo().unwrap();
    assert_eq!(additions(&mut s), [144, 192]);
    assert_eq!(s.serialize().unwrap(), bytes);
    let mut data = ChartData::read(s.document(), 0).unwrap().unwrap();
    data.patterns
        .as_mut()
        .unwrap()
        .get_mut(&48)
        .unwrap()
        .random_seed += 1;
    data.save(&mut s, 0).unwrap();
    assert!(additions(&mut s).is_empty());
    let newer = clipboard(&mut s, 48, 96);
    newer.paste_with_report(&mut s, 0, 144, 0).unwrap();
    assert_eq!(additions(&mut s), [144, 192]);
    s.undo().unwrap();
    assert_eq!(additions(&mut s), [144, 192]);
    s.redo().unwrap();
    assert_eq!(additions(&mut s), [144, 192]);
}
#[test]
fn atomic_order_is_operation_order_and_failed_outer_actions_publish_nothing() {
    let mut s = session();
    s.edit_chart_atomic(0, "Ordered batch", |staged| {
        add(staged, 96);
        add(staged, 48);
        Ok(())
    })
    .unwrap();
    assert_eq!(additions(&mut s), [96, 48]);
    let bytes = s.serialize().unwrap();
    let revision = s.revision();
    assert!(
        s.edit_chart_atomic(0, "Refused", |staged| {
            add(staged, 144);
            Err(deadsync_editor::Error::InvalidEdit("refused".into()))
        })
        .is_err()
    );
    assert!(additions(&mut s).is_empty());
    assert_eq!(s.serialize().unwrap(), bytes);
    assert_eq!(s.revision(), revision);
    s.undo().unwrap();
    assert!(additions(&mut s).is_empty());
    s.redo().unwrap();
    assert_eq!(additions(&mut s), [96, 48]);
}
#[test]
fn delete_then_undo_readds_deleted_pattern_and_invalid_quick_add_has_no_callback() {
    let mut s = session();
    add(&mut s, 48);
    additions(&mut s);
    assert!(Plan::prepare(&s, 0, Kind::Pattern, 48.).is_err());
    assert!(additions(&mut s).is_empty());
    let mut data = ChartData::read(s.document(), 0).unwrap().unwrap();
    data.patterns.as_mut().unwrap().remove(&48);
    data.save(&mut s, 0).unwrap();
    assert!(additions(&mut s).is_empty());
    s.undo().unwrap();
    assert_eq!(additions(&mut s), [48]);
}
