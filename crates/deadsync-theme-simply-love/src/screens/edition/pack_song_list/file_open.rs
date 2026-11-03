//! Direct File/Open and Recent candidate preparation reuse the strict editor
//! loader, preferred-open comparison inputs and application snapshot boundary.
use super::{State, application_preferences};
use crate::screens::edition::pack_song_preference::Prepared;
use deadsync_editor::{
    Error, chart_order::Orderer, chart_views::ActiveChartViews, open_preferences::Preferences,
    recent_songs::Song, storage::FileSession,
};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
};

pub(in crate::screens::edition) struct Job {
    cancelled: Arc<AtomicBool>,
    receiver: Receiver<Result<Prepared, Error>>,
}
impl Job {
    pub(in crate::screens::edition) fn spawn_with_profiles(path:PathBuf,preferences:Preferences,directory:Option<PathBuf>,
    )->Result<Self,Error>{Self::spawn_plan(path,preferences,None,directory)}
    pub(in crate::screens::edition) fn spawn_published_with_profiles(witness:deadsync_editor::storage::PublishedOpenWitness,preferences:Preferences,directory:Option<PathBuf>,
    )->Result<Self,Error>{Self::spawn_plan(witness.path().to_owned(),preferences,Some(witness),directory,
        )}
    fn spawn_plan(path:PathBuf,preferences:Preferences,witness:Option<deadsync_editor::storage::PublishedOpenWitness>,directory:Option<PathBuf>,
    )->Result<Self,Error>{
        let permit = crate::screens::edition::file_open_permit::Permit::acquire()?;
        let cancelled = Arc::new(AtomicBool::new(false));
        let signal = Arc::clone(&cancelled);
        let (sender, receiver) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("editor-file-open".into())
            .spawn(move || {
                let _permit = permit;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match witness{
                        Some(witness)=>{
                            let file=witness.open_bounded_with_context(16*1024*1024,&signal,preferences.load_context(),
                            )?;
                            let prepared=Prepared::new_with_profiles(file,preferences,&signal,directory.as_deref(),
                            )?;
                            witness.validate_opened(&prepared.file,&signal)?;
                            Ok(prepared)
                        }
                        None=> {
                            prepare_with_profiles(&path, preferences, &signal,directory.as_deref())
                        }
                }))
                .unwrap_or_else(|_| {
                        Err(Error::InvalidEdit("Song loading worker failed".into()))
                    });
                let _ = sender.send(result);
            })?;
        Ok(Self {
            cancelled,
            receiver,
        })
    }
    /// Keep this Job in the native I/O slot until poll yields a result, even
    /// when its modal closes. The synchronous parser cannot be interrupted.
    pub(in crate::screens::edition) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
    pub(in crate::screens::edition) fn poll(&mut self) -> Option<Result<Prepared, Error>> {
        match self.receiver.try_recv() {
            Ok(value) if !self.cancelled.load(Ordering::Relaxed) => Some(value),
            Ok(_) | Err(TryRecvError::Disconnected) => {
                Some(Err(Error::InvalidEdit("Song loading cancelled".into())))
            }
            Err(TryRecvError::Empty) => None,
        }
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}
#[cfg(test)]
fn prepare(
    path: &Path,
    preferences: Preferences,
    cancelled: &AtomicBool,
) -> Result<Prepared, Error> {
    prepare_with_profiles(path,preferences,cancelled,deadsync_editor::runtime::configs_dir(),
    )
}
fn prepare_with_profiles(path:&Path,preferences:Preferences,cancelled:&AtomicBool,directory:Option<&Path>,
)->Result<Prepared,Error>{
    if cancelled.load(Ordering::Relaxed) {
        return Err(Error::InvalidEdit("Song loading cancelled".into()));
    }
    let file = FileSession::open_bounded_with_context(path,16*1024*1024,cancelled,preferences.load_context(),
    )?;
    let original = file.require_existing_path()?.to_owned();
    let prepared = Prepared::new_with_profiles(file, preferences, cancelled,directory)?;
    if cancelled.load(Ordering::Relaxed) {
        return Err(Error::InvalidEdit("Song loading cancelled".into()));
    }
    if path.canonicalize()? != original || prepared.file.has_external_change()? {
        return Err(Error::ExternalChange);
    }
    Ok(prepared)
}

/// Returns a detached replacement; caller still owns all dirty/pending review
/// and the old State. No recent entry or filesystem output is published here.
pub(in crate::screens::edition) fn finish(
    source: &State,
    prepared: Prepared,
    saved: Option<&Song>,
    orderer: &mut impl Orderer,
) -> Result<State, Error> {
    prepared.file.validate_generation_profile_context(source.profile_directory.as_deref(),&AtomicBool::new(false),
    )?;
    if prepared.file.has_external_change()? {
        return Err(Error::ExternalChange);
    }
    let culture = crate::i18n::revision();
    let restoration = prepared.restoration(saved, orderer)?;
    if culture != crate::i18n::revision() {
        return Err(Error::InvalidEdit(
            "Editor language changed while ordering the opened song".into(),
        ));
    }
    if prepared.file.has_external_change()? {
        return Err(Error::ExternalChange);
    }
    let (workspace, failures) = prepared.workspace(restoration.focused)?;
    let mut candidate = State::from_workspace(workspace)?;
    application_preferences::Snapshot::capture(source).install(&mut candidate)?;
    candidate.song_open_warnings = failures;
    // DefaultChartListProvider controls only chart choice. Source restores the
    // last presentation for every successful open, including File/Open.
    let recalled=if saved.is_none(){
        let path=candidate.workspace.file().require_existing_path()?;
        if source.workspace.file().existing_path()==Some(path){source.capture_recent_song()?}
        else{source.file_menu.recent_preferences().entries.iter().filter_map(|record|record.song().ok()).find(|song|song.file.as_path()==path)}
    }else{None};
    let presentation=saved.or(recalled.as_ref());
    let row = presentation.map_or(0.0, |saved| saved.chart_position);
    let views =
        ActiveChartViews::restore_recent(&candidate.workspace.file().session, &restoration, row)?;
    if let Some(focused) = views.focused() {
        let position = focused.position().clone();
        candidate
            .workspace
            .seek_preserving_selection(position.edit_row())?;
        candidate.focus_position = Some(position);
    }
    candidate.active_views = Some(views);
    candidate
        .zoom
        .set_zoom(presentation.map_or(1.0, |saved| saved.spacing_zoom))?;
    candidate.refresh();
    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadsync_editor::{chart_order::Snapshot, recent_songs::SavedChart, workspace::Command};
    struct NoSort;
    impl Orderer for NoSort {
        fn order(&mut self, _: &Snapshot) -> Result<Vec<usize>, Error> {
            panic!("one supported chart must not request cultural sorting")
        }
    }
    fn simfile() -> &'static str {
        "#VERSION:0.83;#BPMS:0=120;#WARPS:2=4;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Challenge;#METER:9;#NOTES:1000\n0000\n0000\n0000;"
    }
    #[test]
    fn detached_candidate_restores_warp_row_and_shared_settings_without_losing_old_undo() {
        crate::i18n::init_for_tests();
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("original.ssc");
        let destination = dir.path().join("opened.ssc");
        std::fs::write(&original, simfile()).unwrap();
        std::fs::write(&destination, simfile()).unwrap();
        let mut source = State::open(&original, 0).unwrap();
        source.workspace.execute(Command::Seek(96)).unwrap();
        source.workspace.execute(Command::Place(1)).unwrap();
        let before = source.workspace.file().session.serialize().unwrap();
        let ids = source.workspace.file().session.chart_ids().to_vec();
        let colors = Arc::clone(&source.note_colors);
        let old_undo = source
            .workspace
            .file()
            .session
            .undo_label()
            .map(str::to_owned);
        let saved = Song {
            file: destination.clone(),
            title: None,
            pack: None,
            spacing_zoom: 1.75,
            chart_position: 160.5,
            active: vec![SavedChart {
                steps_type: "dance-single".into(),
                difficulty: 4,
                rating: 9,
                name: Some(String::new()),
                description: Some(String::new()),
            }],
            focused: Some(0),
        };
        let prepared = prepare(
            &destination,
            Preferences::default(),
            &AtomicBool::new(false),
        )
        .unwrap();
        let candidate = finish(&source, prepared, Some(&saved), &mut NoSort).unwrap();
        assert_eq!(candidate.workspace.row(), 160);
        assert_eq!(candidate.focus_position.as_ref().unwrap().row(), 160.5);
        assert_eq!(
            candidate
                .active_views
                .as_ref()
                .unwrap()
                .focused()
                .unwrap()
                .position()
                .row(),
            160.5
        );
        assert_eq!(candidate.zoom.spacing_zoom(), 1.75);
        assert!(Arc::ptr_eq(&candidate.note_colors, &colors));
        assert!(
            candidate
                .active_views
                .as_ref()
                .unwrap()
                .focused()
                .unwrap()
                .dedicated()
        );
        assert_eq!(source.workspace.file().session.serialize().unwrap(), before);
        assert_eq!(source.workspace.file().session.chart_ids(), ids);
        assert_eq!(
            source.workspace.file().session.undo_label(),
            old_undo.as_deref()
        );
        assert!(source.workspace.file().session.is_dirty());
        assert!(
            !candidate
                .active_views
                .as_ref()
                .unwrap()
                .belongs_to(&source.workspace.file().session)
        );
    }
    #[test]
    fn cancelled_and_changed_sources_fail_before_any_candidate_is_published() {
        crate::i18n::init_for_tests();
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("song.ssc");
        std::fs::write(&file, simfile()).unwrap();
        assert!(prepare(&file, Preferences::default(), &AtomicBool::new(true)).is_err());
        let source = State::open(&file, 0).unwrap();
        let prepared = prepare(&file, Preferences::default(), &AtomicBool::new(false)).unwrap();
        std::fs::write(&file, b"external change").unwrap();
        assert!(finish(&source, prepared, None, &mut NoSort).is_err());
        assert_eq!(source.workspace.file().session.document().charts().len(), 1);
    }

    #[test]
    fn cancelled_job_keeps_busy_slot_until_worker_acknowledges_completion() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut job = Job {
            cancelled: Arc::new(AtomicBool::new(false)),
            receiver,
        };
        job.cancel();
        assert!(job.poll().is_none());
        assert!(
            sender
                .send(Err(Error::InvalidEdit("worker acknowledged cancel".into())))
                .is_ok()
        );
        assert!(matches!(job.poll(), Some(Err(_))));
    }
}

#[cfg(test)]mod default_open_presentation_tests{
 use super::*;use deadsync_editor::recent_songs::SavedChart;
 struct SourceOrder;impl Orderer for SourceOrder{fn order(&mut self,s:&deadsync_editor::chart_order::Snapshot,
        )->Result<Vec<usize>,Error>{Ok(s.source_indices().to_vec())}}
 #[test]fn default_open_recalls_fractional_mru_position_zoom_without_restoring_mru_chart_provider(){
  crate::i18n::init_for_tests();let root=tempfile::tempdir().unwrap();let old=root.path().join("old.ssc");let target=root.path().join("target.ssc");let text="#VERSION:0.83;#BPMS:0=120;#WARPS:2=4;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Challenge;#METER:10;#NOTES:0100;";std::fs::write(&old,text).unwrap();std::fs::write(&target,text).unwrap();let mut source=State::open(&old,0).unwrap();let before=source.workspace.file().session.serialize().unwrap();let target=target.canonicalize().unwrap();let saved=Song{file:target.clone(),title:None,pack:None,spacing_zoom:2.5,chart_position:160.5,active:vec![SavedChart{steps_type:"dance-single".into(),difficulty:3,rating:9,name:None,description:None,
            }],focused:Some(0),
        };let mut recent=source.file_menu.recent_preferences().clone();recent.record(&saved).unwrap();source.file_menu.install_recent(recent);
  let prepared=prepare(&target,Preferences::default(),&AtomicBool::new(false)).unwrap();let candidate=finish(&source,prepared,None,&mut SourceOrder).unwrap();assert_eq!(candidate.workspace.chart(),1,"default Challenge, not MRU Hard chart");assert_eq!(candidate.focus_position.as_ref().unwrap().row(),160.5);assert_eq!(candidate.zoom.spacing_zoom(),2.5);assert_eq!(source.workspace.file().session.serialize().unwrap(),before);
 }
}

#[cfg(test)]
mod profile_resolution_tests {
    use super::*;
    use deadsync_editor::{
        chart_order::Snapshot,
        generation::{metadata::{ChartData, DYNAMIC, Guid, PATTERN_BALANCED, Registry, SIXTEENTHS},profiles::{Config,Kind},
        },
    };
    const UNKNOWN:&str="12345678-9abc-4def-8123-456789abcdef";
    struct NoSort;
    impl Orderer for NoSort {fn order(&mut self,_:&Snapshot)->Result<Vec<usize>,Error>{panic!("single chart")}}
    fn input(path:&Path){
        let json=serde_json::json!({"ExpressedChartConfig":UNKNOWN,"Future":{"keep":true},"Patterns":{"96":{"PatternConfigGuid":UNKNOWN,"PerformedChartConfigGuid":UNKNOWN,"Length":48,"RandomSeed":42,"StartPositionInclusive":true,"EndPositionInclusive":true,"FutureMarker":"keep"}}});
        std::fs::write(path,format!("#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;#GAChartDataVersion:1;#GAChartData:{};",String::from_utf8(deadsync_editor::document::escape_text(&json.to_string())).unwrap())).unwrap();
    }
    fn profiles(path:&Path){
        std::fs::create_dir(path).unwrap();let registry=Registry::builtin();
        for(kind,config)in[(Kind::Expressed,Config::Expressed(registry.expressed[&DYNAMIC.parse::<Guid>().unwrap()].clone()),
            ),(Kind::Pattern,Config::Pattern(registry.patterns[&SIXTEENTHS.parse::<Guid>().unwrap()].clone()),
            ),(Kind::Performed,Config::Performed(registry.performed[&PATTERN_BALANCED.parse::<Guid>().unwrap()].clone(),
                ),
            ),
        ]{std::fs::write(path.join(format!("{}custom.json",kind.prefix())),serde_json::to_vec(&serde_json::json!({"Guid":UNKNOWN,"Name":"Custom","Config":config.json().unwrap()})).unwrap()).unwrap();}
    }
    fn wait(job:&mut Job)->Result<Prepared,Error>{let deadline=std::time::Instant::now()+std::time::Duration::from_secs(10);loop{if let Some(result)=job.poll(){return result;}assert!(std::time::Instant::now()<deadline);std::thread::yield_now();}}
    #[test]
    fn direct_and_actual_file_open_use_configured_profiles_before_publish_and_never_default_first(){
        crate::i18n::init_for_tests();let root=tempfile::tempdir().unwrap();let path=root.path().join("shared.ssc");input(&path);let directory=root.path().join("profiles");profiles(&directory);let raw=std::fs::read(&path).unwrap();
        let direct=State::open_with_profiles(&path,0,&directory).unwrap();let data=ChartData::read(direct.workspace.file().session.document(),0).unwrap().unwrap();assert_eq!(data.expressed_chart_config.to_string(),UNKNOWN);assert_eq!(data.patterns.unwrap()[&96].pattern_config_guid.to_string(),UNKNOWN);assert!(!direct.workspace.file().session.is_dirty());assert!(direct.workspace.file().session.undo_label().is_none());
        let mut source=State::without_song(root.path()).unwrap();source.profile_directory=Some(directory);source.begin_entry_open(path.clone(),None);let deadline=std::time::Instant::now()+std::time::Duration::from_secs(10);
        while !source.has_open_song(){source.process_file_menu(&mut NoSort);assert!(std::time::Instant::now()<deadline,"{}",source.status);std::thread::yield_now();}
        let data=ChartData::read(source.workspace.file().session.document(),0).unwrap().unwrap();assert_eq!(data.expressed_chart_config.to_string(),UNKNOWN);assert!(!source.workspace.file().session.is_dirty());assert!(source.workspace.file().session.undo_label().is_none());assert_eq!(std::fs::read(path).unwrap(),raw);
    }
    #[test]
    fn missing_profiles_publish_one_dirty_baseline_and_changed_or_cancelled_catalog_never_replaces_source(){
        crate::i18n::init_for_tests();let root=tempfile::tempdir().unwrap();let path=root.path().join("shared.ssc");input(&path);let directory=root.path().join("profiles");std::fs::create_dir(&directory).unwrap();let raw=std::fs::read(&path).unwrap();let mut source=State::without_song(root.path()).unwrap();source.profile_directory=Some(directory.clone());let before=source.workspace.file().session.serialize().unwrap();
        let mut job=Job::spawn_with_profiles(path.clone(),Preferences::default(),Some(directory.clone()),
        ).unwrap();let prepared=wait(&mut job).unwrap();assert!(prepared.file.session.is_dirty());assert!(prepared.file.session.undo_label().is_none());std::fs::write(directory.join("ecc-new.json"),b"{}").unwrap();assert!(finish(&source,prepared,None,&mut NoSort).is_err());assert_eq!(source.workspace.file().session.serialize().unwrap(),before);assert!(!source.has_open_song());
        let mut cancelled=Job::spawn_with_profiles(path.clone(),Preferences::default(),Some(directory.clone()),
        ).unwrap();cancelled.cancel();assert!(wait(&mut cancelled).is_err());assert_eq!(source.workspace.file().session.serialize().unwrap(),before);
        let mut job=Job::spawn_with_profiles(path.clone(),Preferences::default(),Some(directory.clone()),
        ).unwrap();let prepared=wait(&mut job).unwrap();source.profile_directory=None;assert!(finish(&source,prepared,None,&mut NoSort).is_err());source.profile_directory=Some(directory);
        source.begin_entry_open(path.clone(),None);let deadline=std::time::Instant::now()+std::time::Duration::from_secs(10);while !source.has_open_song(){source.process_file_menu(&mut NoSort);assert!(std::time::Instant::now()<deadline,"{}",source.status);std::thread::yield_now();}
        assert!(source.workspace.file().session.is_dirty());assert!(source.workspace.file().session.undo_label().is_none());assert_eq!(source.workspace.file().generation_profile_repairs().len(),2);assert_eq!(ChartData::read(source.workspace.file().session.document(),0).unwrap().unwrap().expressed_chart_config.to_string(),DYNAMIC);assert_eq!(std::fs::read(path).unwrap(),raw);
    }
}
