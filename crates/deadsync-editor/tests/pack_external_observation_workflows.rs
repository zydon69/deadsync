use deadsync_editor::pack::{Data,storage::{FileSession,Error}};
use std::sync::atomic::AtomicBool;
#[test]
fn passive_snapshot_distinguishes_external_versions_cancellation_and_own_publication(){
 let dir=tempfile::tempdir().unwrap();let mut file=FileSession::create(dir.path(),Data::new("Pack","",2026,None,0.009)).unwrap();file.save().unwrap();let watch=file.external_snapshot();let cancel=AtomicBool::new(false);assert!(watch.probe(&cancel).unwrap().is_none());assert!(watch.is_current(&file));
 file.set_year(2027).unwrap();assert!(watch.probe(&cancel).unwrap().is_none());file.save().unwrap();assert!(!watch.is_current(&file));let published=file.external_snapshot();assert!(published.probe(&cancel).unwrap().is_none());assert!(!published.same_capture(&watch));
 std::fs::write(file.path(),b"[Group]\nTitle=External A\n").unwrap();let first=published.probe(&cancel).unwrap().unwrap();let rebuilt=published.rebuild_observer_baseline().unwrap().unwrap();assert!(published.is_current(&rebuilt));assert_eq!(rebuilt.draft().data().year,2027);assert!(matches!(rebuilt.has_external_change(),Ok(true)));std::fs::write(file.path(),b"[Group]\nTitle=External B\n").unwrap();let second=published.probe(&cancel).unwrap().unwrap();assert_ne!(first,second);assert!(matches!(file.save(),Err(Error::ExternalChange)));
 cancel.store(true,std::sync::atomic::Ordering::Release);assert!(matches!(published.probe(&cancel),Err(Error::StaleSnapshot)));
}
#[test]
fn missing_and_oversized_pack_never_become_unchanged(){
 let dir=tempfile::tempdir().unwrap();let mut file=FileSession::create(dir.path(),Data::new("Pack","",2026,None,0.009)).unwrap();file.save().unwrap();let watch=file.external_snapshot();std::fs::remove_file(file.path()).unwrap();let cancelled=AtomicBool::new(false);assert!(watch.probe(&cancelled).unwrap().is_some());std::fs::write(file.path(),vec![b'x';deadsync_editor::pack::MAX_BYTES+1]).unwrap();assert!(watch.probe(&cancelled).is_err());
}
#[cfg(unix)]
#[test]
fn retargeted_logical_symlink_is_changed_even_if_the_previous_target_remains_identical(){
 let dir=tempfile::tempdir().unwrap();let a=dir.path().join("a.ini");let b=dir.path().join("b.ini");let bytes=b"[Group]\nVersion=1\nTitle=Native\n";std::fs::write(&a,bytes).unwrap();std::fs::write(&b,bytes).unwrap();let logical=dir.path().join("Pack.ini");std::os::unix::fs::symlink(&a,&logical).unwrap();let file=FileSession::open(dir.path()).unwrap().unwrap();let watch=file.external_snapshot();assert!(watch.probe(&AtomicBool::new(false)).unwrap().is_none());std::fs::remove_file(&logical).unwrap();std::os::unix::fs::symlink(&b,&logical).unwrap();assert!(watch.probe(&AtomicBool::new(false)).unwrap().is_some());assert!(matches!(file.has_external_change(),Ok(true)));
}
