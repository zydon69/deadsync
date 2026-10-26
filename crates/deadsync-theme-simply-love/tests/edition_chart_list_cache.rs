use deadsync_theme_simply_love::i18n;
#[path = "../src/screens/edition/chart_list.rs"]
mod chart_list;
use chart_list::{Cache, Selection};
use deadsync_editor::{
    Error,
    chart_order::{Orderer, Snapshot},
    workspace::{Command, Workspace},
};
use std::{fs, sync::Mutex};
static SERIAL: Mutex<()> = Mutex::new(());
struct Native {
    calls: usize,
    bad: bool,
    locale_change: bool,
}
impl Orderer for Native {
    fn order(&mut self, snapshot: &Snapshot) -> Result<Vec<usize>, Error> {
        self.calls += 1;
        if self.locale_change {
            i18n::set_locale(deadsync_assets::language::load_for_tests("fr"));
        }
        if self.bad {
            return Ok(vec![0, 0]);
        }
        snapshot.indices_with(str::cmp)
    }
}
fn setup() -> (tempfile::TempDir, Workspace, Native) {
    i18n::init(deadsync_assets::language::load_for_tests("en"));
    i18n::set_locale(deadsync_assets::language::load_for_tests("en"));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("song.ssc");
    fs::write(&path, "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:kb7-single;#NOTES:1000000;#NOTEDATA:;#STEPSTYPE:dance-double;#DIFFICULTY:Hard;#METER:9;#NOTES:10000000;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Easy;#METER:2;#NOTES:1000;").unwrap();
    (
        directory,
        Workspace::open(&path, 1).unwrap(),
        Native {
            calls: 0,
            bad: false,
            locale_change: false,
        },
    )
}
#[test]
fn navigation_reuses_rows_and_keyboard_highlight_does_not_select_or_rewrite_source() {
    let _guard = SERIAL.lock().unwrap();
    let (_directory, mut workspace, mut native) = setup();
    let source = workspace.file().session.serialize().unwrap();
    let mut cache = Cache::default();
    cache
        .refresh(&mut workspace, i18n::revision(), &mut native)
        .unwrap();
    assert!(cache.is_current(&workspace, i18n::revision()));
    assert_eq!(
        cache
            .rows()
            .iter()
            .map(|r| r.source_index)
            .collect::<Vec<_>>(),
        [2, 1]
    );
    let mut selection = Selection::default();
    selection.reconcile(cache.rows(), workspace.chart());
    selection.move_by(cache.rows(), 1);
    assert_eq!(selection.selected_source(cache.rows()), Some(2));
    assert_eq!(workspace.chart(), 1);
    assert_eq!(cache.adjacent(1, false), Some(2));
    assert_eq!(cache.adjacent(2, true), Some(1));
    workspace.execute(Command::SelectChart(2)).unwrap();
    cache
        .refresh(&mut workspace, i18n::revision(), &mut native)
        .unwrap();
    assert_eq!(native.calls, 1);
    assert_eq!(workspace.file().session.serialize().unwrap(), source);
    assert!(!workspace.file().session.is_dirty());
    cache.clear();
    assert!(!cache.is_current(&workspace, i18n::revision()));
    assert!(cache.rows().is_empty());
}
#[test]
fn content_structure_undo_new_sessions_and_language_invalidate_cached_order() {
    let _guard = SERIAL.lock().unwrap();
    let (directory, mut workspace, mut native) = setup();
    let mut cache = Cache::default();
    cache
        .refresh(&mut workspace, i18n::revision(), &mut native)
        .unwrap();
    workspace.execute(Command::DuplicateChart).unwrap();
    cache
        .refresh(&mut workspace, i18n::revision(), &mut native)
        .unwrap();
    assert_eq!(cache.rows().len(), 3);
    let mut selection = Selection::default();
    selection.reconcile(cache.rows(), workspace.chart());
    workspace.execute(Command::Undo).unwrap();
    cache
        .refresh(&mut workspace, i18n::revision(), &mut native)
        .unwrap();
    selection.reconcile(cache.rows(), workspace.chart());
    assert_eq!(cache.rows().len(), 2);
    assert!(selection.selected_source(cache.rows()).is_some());
    workspace.execute(Command::Redo).unwrap();
    cache
        .refresh(&mut workspace, i18n::revision(), &mut native)
        .unwrap();
    assert_eq!(native.calls, 4);
    i18n::set_locale(deadsync_assets::language::load_for_tests("fr"));
    cache
        .refresh(&mut workspace, i18n::revision(), &mut native)
        .unwrap();
    assert_eq!(native.calls, 5);
    let mut reopened = Workspace::open(&directory.path().join("song.ssc"), 1).unwrap();
    cache
        .refresh(&mut reopened, i18n::revision(), &mut native)
        .unwrap();
    assert_eq!(native.calls, 6);
    assert_eq!(cache.rows().len(), 2);
}
#[test]
fn failed_permutations_and_reentrant_language_changes_never_leave_stale_rows() {
    let _guard = SERIAL.lock().unwrap();
    let (_directory, mut workspace, mut native) = setup();
    let mut cache = Cache::default();
    cache
        .refresh(&mut workspace, i18n::revision(), &mut native)
        .unwrap();
    workspace.execute(Command::Place(1)).unwrap();
    native.bad = true;
    assert!(
        cache
            .refresh(&mut workspace, i18n::revision(), &mut native)
            .is_err()
    );
    assert!(cache.rows().is_empty());
    assert_eq!(cache.adjacent(1, false), None);
    native.bad = false;
    native.locale_change = true;
    assert!(
        cache
            .refresh(&mut workspace, i18n::revision(), &mut native)
            .is_err()
    );
    assert!(cache.rows().is_empty());
    native.locale_change = false;
    cache
        .refresh(&mut workspace, i18n::revision(), &mut native)
        .unwrap();
    assert_eq!(native.calls, 4);
}

#[test]
fn empty_and_single_editable_lists_skip_native_work_and_stale_revision_rejects_cache_hit() {
    let _guard = SERIAL.lock().unwrap();
    let (directory, _workspace, mut native) = setup();
    for charts in [
        "#NOTEDATA:;#STEPSTYPE:kb7-single;#NOTES:1000000;",
        "#NOTEDATA:;#STEPSTYPE:kb7-single;#NOTES:1000000;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
    ] {
        i18n::set_locale(deadsync_assets::language::load_for_tests("en"));
        let path = directory.path().join("few.ssc");
        fs::write(&path, format!("#VERSION:0.83;#BPMS:0=120;{charts}")).unwrap();
        let mut workspace = Workspace::open(
            &path,
            if charts.contains("dance-single") {
                1
            } else {
                0
            },
        )
        .unwrap();
        let mut cache = Cache::default();
        let revision = i18n::revision();
        cache
            .refresh(&mut workspace, revision, &mut native)
            .unwrap();
        let mut selection = Selection::default();
        selection.reconcile(cache.rows(), workspace.chart());
        selection.move_by(cache.rows(), isize::MIN);
        assert_eq!(native.calls, 0);
        assert_eq!(
            selection.selected_source(cache.rows()),
            if charts.contains("dance-single") {
                Some(1)
            } else {
                None
            }
        );
        i18n::set_locale(deadsync_assets::language::load_for_tests("fr"));
        assert!(
            cache
                .refresh(&mut workspace, revision, &mut native)
                .is_err()
        );
        assert!(cache.rows().is_empty());
    }
}
