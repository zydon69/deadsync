use deadsync_editor::{
    Error,
    document::Format,
    pack::song_list::{
        Catalog, HeaderReader, IssueKind, Limits, Metadata, OrderKey, Orderer, Reader, Selection,
        Task,
    },
};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
fn decode(s: &str) -> Vec<u8> {
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let (mut bits, mut count, mut out) = (0u32, 0, Vec::new());
    for b in s.bytes().take_while(|v| *v != b'=') {
        bits = (bits << 6) | alphabet.iter().position(|v| *v == b).unwrap() as u32;
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
            bits &= (1 << count) - 1;
        }
    }
    out
}
fn fixtures() -> (Value, Value) {
    (
        serde_json::from_str(include_str!(
            "fixtures/grooveauthor/pack-songs.request.json"
        ))
        .unwrap(),
        serde_json::from_str(include_str!(
            "fixtures/grooveauthor/pack-songs.expected.json"
        ))
        .unwrap(),
    )
}
fn project(metadata: &Metadata) -> Value {
    json!({"Title":metadata.title,"TitleTranslit":metadata.title_transliteration,"Artist":metadata.artist,"Credit":metadata.credit,"Ratings":metadata.ratings})
}
fn expected_projection(row: &Value) -> Value {
    json!({"Title":row["Title"],"TitleTranslit":row["TitleTranslit"],"Artist":row["Artist"],"Credit":row["Credit"],"Ratings":row["Ratings"]})
}
fn sm(title: &str) -> String {
    format!("#TITLE:{title};#ARTIST:Artist;#BPMS:0=120;#NOTES:dance-single::Hard:9::1000;")
}
fn tree() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let song = dir.path().join("Current");
    fs::create_dir(&song).unwrap();
    let path = song.join("song.sm");
    fs::write(&path, sm("Current")).unwrap();
    (dir, path)
}
fn run(path: &Path, limits: Limits) -> Result<Catalog, Error> {
    Task::from_song_file(path, limits)?.run(
        &HeaderReader::default(),
        &AtomicBool::new(false),
        &mut |_| {},
    )
}
fn write_fixtures(root: &Path, request: &Value) {
    for song in request["Songs"].as_array().unwrap() {
        let dir = root.join(song["Name"].as_str().unwrap());
        fs::create_dir(&dir).unwrap();
        for file in song["Files"].as_array().unwrap() {
            fs::write(
                dir.join(file["Name"].as_str().unwrap()),
                decode(file["Bytes"].as_str().unwrap()),
            )
            .unwrap();
        }
    }
}
#[test]
fn real_original_metadata_cases_match_including_header_only_ssc_and_numeric_boundaries() {
    let (request, expected) = fixtures();
    assert_eq!(request["Songs"].as_array().unwrap().len(), 116);
    let reader = HeaderReader::default();
    let mut compared = 0;
    for (input, output) in request["Songs"]
        .as_array()
        .unwrap()
        .iter()
        .zip(expected["Songs"].as_array().unwrap())
    {
        if !output["Loaded"].as_bool().unwrap() {
            continue;
        }
        let file = input["Files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["Name"] == output["Selected"])
            .unwrap();
        let name = file["Name"].as_str().unwrap();
        let metadata = reader
            .read(
                &decode(file["Bytes"].as_str().unwrap()),
                if name.ends_with(".ssc") {
                    Format::Ssc
                } else {
                    Format::Sm
                },
                deadsync_editor::document::metadata::Limits::default(),
            )
            .unwrap();
        assert_eq!(
            project(&metadata),
            expected_projection(output),
            "{}",
            input["Name"]
        );
        assert!(output["Cancelled"].as_bool().unwrap());
        compared += 1;
    }
    assert_eq!(compared, 115);
}
#[test]
fn direct_discovery_matches_source_formats_and_reports_the_explicit_ambiguity_policy() {
    let (request, expected) = fixtures();
    let root = tempfile::tempdir().unwrap();
    write_fixtures(root.path(), &request);
    let source = root.path().join("single-sm/song.sm");
    let catalog = run(&source, Limits::default()).unwrap();
    assert_eq!(catalog.rows.len(), 115);
    assert!(!catalog.rows.iter().any(|r| r.folder == "uppercase-only"));
    for row in &catalog.rows {
        if row.folder == "multiple-same-extension" {
            assert!(row.path.ends_with("z.ssc"));
            assert_eq!(row.metadata.title.as_deref(), Some("Last"));
            continue;
        }
        let output = expected["Songs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["Name"].as_str() == Some(row.folder.as_str()))
            .unwrap();
        assert_eq!(
            project(&row.metadata),
            expected_projection(output),
            "{}",
            row.folder
        );
    }
    assert!(
        catalog
            .issues
            .iter()
            .any(|issue| matches!(issue.kind, IssueKind::Ambiguous { candidates: 2 }))
    );
    assert!(
        catalog
            .rows
            .iter()
            .any(|r| r.folder == "bad-ssc-with-sm" && r.metadata.ratings == "0")
    );
    assert!(!source.parent().unwrap().join("Pack.ini").exists());
}
#[test]
fn only_immediate_song_folders_are_read_and_notes_are_not_interpreted() {
    let (root, source) = tree();
    fs::write(root.path().join("root.sm"), sm("Ignore root")).unwrap();
    let nested = root.path().join("Nested");
    fs::create_dir_all(nested.join("Grandchild")).unwrap();
    fs::write(nested.join("Grandchild/nested.sm"), sm("Ignore nested")).unwrap();
    let bad = root.path().join("BadGrid");
    fs::create_dir(&bad).unwrap();
    fs::write(
        bad.join("song.ssc"),
        "#TITLE:Header;#NOTEDATA:;#STEPSTYPE:dance-single;#METER:9;#NOTES:garbage;",
    )
    .unwrap();
    let catalog = run(&source, Limits::default()).unwrap();
    assert_eq!(catalog.rows.len(), 2);
    assert!(
        catalog
            .rows
            .iter()
            .any(|r| r.metadata.title.as_deref() == Some("Header") && r.metadata.ratings == "9")
    );
}
#[test]
fn selected_ssc_reader_failure_never_falls_back_to_sm() {
    let (root, source) = tree();
    let dir = root.path().join("Broken");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("song.sm"), sm("Must not be shown")).unwrap();
    fs::write(dir.join("song.ssc"), "#TITLE").unwrap();
    let catalog = run(&source, Limits::default()).unwrap();
    assert_eq!(catalog.rows.len(), 1);
    assert!(catalog.issues.iter().any(|v|matches!(v.kind,IssueKind::MetadataFailed(_))&&v.path.ends_with("song.ssc")));
}
#[test]
fn cancellation_before_during_and_after_metadata_never_publishes_a_catalog() {
    let (_root, source) = tree();
    let cancelled = AtomicBool::new(true);
    assert!(
        Task::from_song_file(&source, Limits::default())
            .unwrap()
            .run(&HeaderReader::default(), &cancelled, &mut |_| {})
            .unwrap_err()
            .to_string()
            .contains("cancelled")
    );
    struct Cancelling(Arc<AtomicBool>);
    impl Reader for Cancelling {
        fn read(
            &self,
            bytes: &[u8],
            format: Format,
            limits: deadsync_editor::document::metadata::Limits,
        ) -> Result<Metadata, Error> {
            let result = HeaderReader::default().read(bytes, format, limits);
            self.0.store(true, Ordering::Relaxed);
            result
        }
    }
    let cancelled = Arc::new(AtomicBool::new(false));
    assert!(
        Task::from_song_file(&source, Limits::default())
            .unwrap()
            .run(&Cancelling(cancelled.clone()), &cancelled, &mut |_| {})
            .unwrap_err()
            .to_string()
            .contains("cancelled")
    );
    let cancelled = AtomicBool::new(false);
    let mut progress = |_| cancelled.store(true, Ordering::Relaxed);
    assert!(
        Task::from_song_file(&source, Limits::default())
            .unwrap()
            .run(&HeaderReader::default(), &cancelled, &mut progress)
            .unwrap_err()
            .to_string()
            .contains("cancelled")
    );
}
#[test]
fn directory_entry_byte_and_output_limits_are_explicit_and_do_not_write() {
    let (root, source) = tree();
    let before = fs::read(&source).unwrap();
    for limits in [
        Limits {
            directories: 0,
            ..Limits::default()
        },
        Limits {
            entries: 0,
            ..Limits::default()
        },
        Limits {
            files_per_directory: 0,
            ..Limits::default()
        },
        Limits {
            total_bytes: 1,
            ..Limits::default()
        },
        Limits {
            metadata_bytes: 0,
            ..Limits::default()
        },
    ] {
        assert!(matches!(run(&source, limits), Err(Error::Unsupported(_))));
    }
    for limits in [
        Limits {
            charts_per_song: 0,
            ..Limits::default()
        },
        Limits {
            tags_per_song: 0,
            ..Limits::default()
        },
    ] {
        let catalog = run(&source, limits).unwrap();
        assert!(catalog.rows.is_empty());
        assert!(
            catalog
                .issues
                .iter()
                .any(|issue| matches!(issue.kind, IssueKind::MetadataFailed(_)))
        );
    }
    let catalog = run(
        &source,
        Limits {
            file_bytes: 1,
            ..Limits::default()
        },
    )
    .unwrap();
    assert!(catalog.rows.is_empty());
    assert!(
        catalog
            .issues
            .iter()
            .any(|v| v.kind == IssueKind::FileTooLarge)
    );
    assert_eq!(fs::read(&source).unwrap(), before);
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}
struct Lexical;
impl Orderer for Lexical {
    fn order(&mut self, keys: &[OrderKey]) -> Result<Vec<usize>, Error> {
        let mut indices: Vec<_> = (0..keys.len()).collect();
        indices.sort_by(|&a, &b| {
            keys[a]
                .title
                .cmp(&keys[b].title)
                .then_with(|| keys[a].folder.cmp(&keys[b].folder))
        });
        Ok(indices)
    }
}
#[test]
fn cultural_order_is_injected_and_invalid_permutations_are_rejected_atomically() {
    let (root, source) = tree();
    let dir = root.path().join("Translit");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("song.sm"), sm("Zulu") + "#TITLETRANSLIT:Alpha;").unwrap();
    let catalog = run(&source, Limits::default()).unwrap();
    let order = catalog.ordered(&mut Lexical).unwrap();
    assert_eq!(catalog.rows[order[0]].folder, "Translit");
    struct Broken(Vec<usize>);
    impl Orderer for Broken {
        fn order(&mut self, _: &[OrderKey]) -> Result<Vec<usize>, Error> {
            Ok(self.0.clone())
        }
    }
    let before: Vec<_> = catalog.rows.iter().map(|r| r.path.clone()).collect();
    for bad in [vec![], vec![0, 0], vec![0, 2]] {
        assert!(catalog.ordered(&mut Broken(bad)).is_err());
    }
    assert_eq!(
        catalog
            .rows
            .iter()
            .map(|r| r.path.clone())
            .collect::<Vec<_>>(),
        before
    );
    // This mock verifies injection/permutation only; the .NET fixture retains
    // real en-US/fr-FR order for the forthcoming native ICU adapter test.
}
#[test]
fn selection_reconciles_stable_path_and_open_guard_detects_a_changed_file() {
    let (root, source) = tree();
    let other = root.path().join("Other");
    fs::create_dir(&other).unwrap();
    fs::write(other.join("song.sm"), sm("A")).unwrap();
    let catalog = run(&source, Limits::default()).unwrap();
    let order = catalog.ordered(&mut Lexical).unwrap();
    let mut selection = Selection::default();
    selection.reconcile(&catalog, &order);
    selection.move_by(1, &catalog, &order);
    let selected = selection.selected().unwrap().to_path_buf();
    let mut reverse = order.clone();
    reverse.reverse();
    selection.reconcile(&catalog, &reverse);
    assert_eq!(selection.selected(), Some(selected.as_path()));
    let row = catalog.rows.iter().find(|r| r.path == selected).unwrap();
    assert!(row.is_current().unwrap());
    fs::write(&selected, sm("Changed title length")).unwrap();
    assert!(!row.is_current().unwrap());
    selection.move_by(isize::MAX, &catalog, &reverse);
    assert_eq!(selection.position(), 1);
    selection.reconcile(&catalog, &[]);
    assert!(selection.selected().is_none());
}
#[cfg(unix)]
#[test]
fn user_song_directory_symlinks_are_read_without_recursion_and_aliases_are_diagnosed() {
    use std::os::unix::fs::symlink;
    let (root, source) = tree();
    symlink(source.parent().unwrap(), root.path().join("Alias")).unwrap();
    let catalog = run(&source, Limits::default()).unwrap();
    assert_eq!(catalog.rows.len(), 1);
    assert!(
        catalog
            .issues
            .iter()
            .any(|v| v.kind == IssueKind::DuplicateTarget)
    );
    let original = fs::read(&source).unwrap();
    assert_eq!(fs::read(&source).unwrap(), original);
}
