//! Batch conversion uses the same options and profile chooser as single conversion.
//! Only the source type and result navigation belong to this panel.
use super::*;
#[derive(Default)]
pub(super) struct SourceInventory {
    binding:Option<deadsync_editor::chart_order::SessionBinding>,
    layouts:std::collections::BTreeMap<usize,String>,
    counts:std::collections::BTreeMap<String,usize>,
    #[cfg(test)] rebuilds:usize,
}

impl State {
    pub fn batch_preview(&self) -> Option<(&batch_job::Preview, usize)> {
        match self.preview.as_ref()? {
            GenerationPreview::Batch { preview, entry } => Some((preview, *entry)),
            _ => None,
        }
    }

    pub(super) fn refresh_batch_sources(&mut self){
        let session=&self.workspace.file().session;
        if self.batch_inventory.binding.as_ref().is_some_and(|binding|binding.is_current(session)){return;}
        let mut layouts=std::collections::BTreeMap::new();let mut counts=std::collections::BTreeMap::new();
        for (chart,layout) in batch_job::source_charts(session){layouts.insert(chart,layout.to_owned());*counts.entry(layout.to_owned()).or_insert(0)+=1;}
        self.batch_inventory.layouts=layouts;self.batch_inventory.counts=counts;
        self.batch_inventory.binding=Some(deadsync_editor::chart_order::SessionBinding::capture(session,
        ));
        #[cfg(test)] {self.batch_inventory.rebuilds+=1;}
    }
    fn batch_inventory_current(&self)->bool{
        self.batch_inventory.binding.as_ref().is_some_and(|binding|binding.is_current(&self.workspace.file().session))
    }
    fn batch_source_count(&self,layout:&str)->usize{
        if !self.batch_inventory_current(){return 0;}
        self.batch_inventory.counts.get(layout).copied().unwrap_or(0)
    }
    fn batch_sources(&self) -> Vec<&str> {
        TARGET_LAYOUTS.iter().copied().filter(|layout|self.batch_source_count(layout)>0).collect()
    }

    pub(super) fn open_batch(&mut self) {
        if self.is_generating() {
            return;
        }
        self.refresh_batch_sources();
        let sources = self.batch_sources();
        let current=self.workspace.active_chart().and_then(|chart|self.batch_inventory.layouts.get(&chart)).map(String::as_str);
        let source = sources
            .iter()
            .copied()
            .find(|source| Some(*source) == current)
            .or_else(|| {
                self.batch_source
                    .as_deref()
                    .filter(|source| sources.contains(source))
            })
            .or_else(|| sources.first().copied());
        let Some(source) = source else {
            self.batch_source=None;
            self.status = i18n::tr("Edition", "BatchNoSources");
            return;
        };
        self.batch_source = Some(source.to_owned());
        self.panel = Some(Panel::Batch(0));
        self.status = i18n::tr("Edition", "BatchOptionsHelp");
    }

    pub(super) fn batch_key(&mut self, index: usize, code: KeyCode) {
        self.refresh_batch_sources();
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
            self.panel = Some(Panel::Batch(
                (index + if code == KeyCode::ArrowUp { 7 } else { 1 }) % 8,
            ));
        } else if matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight) {
            let forward = code == KeyCode::ArrowRight;
            match index {
                0 => {
                    let sources = self.batch_sources();
                    if !sources.is_empty() {
                        let current = sources
                            .iter()
                            .position(|source| Some(*source) == self.batch_source.as_deref())
                            .unwrap_or(0);
                        self.batch_source = Some(
                            sources[(current + if forward { 1 } else { sources.len() - 1 })
                                % sources.len()]
                            .to_owned(),
                        );
                    }
                }
                1..=3 => self.adjust_generation_setting(index - 1, forward),
                _ => {}
            }
        } else if code == KeyCode::Enter {
            match index {
                4 => self.open_conversion_profiles_for(ProfileTarget::Batch),
                5 => self.start_batch(),
                6 => self.panel = None,
                7 => self.open_autogen_source_expression(Panel::Batch(7)),
                _ => {}
            }
        }
    }

    fn start_batch(&mut self) {
        self.refresh_batch_sources();
        if self.batch_source.as_deref().is_none_or(|source|self.batch_source_count(source)==0){self.status=i18n::tr("Edition","BatchNoSources");return;}
        if !self.current_timing_for_generation() {
            return;
        }
        if self.is_generating() {
            return;
        }
        let Some(source) = self.batch_source.as_deref() else {
            return;
        };
        let options=match self.generation_options(){Ok(value)=>value,Err(_)=>{self.status=i18n::tr_fmt("Edition","AutogenUnsupportedTarget",&[("value",&self.autogen_preferences.target.source_name())],
                );return;}};
        match self
            .workspace
            .batch_generation_task(
                source,
                options,
                self.profile_directory.as_deref(),
                self.generation.profiles.clone(),
            )
            .and_then(|task| task.spawn())
        {
            Ok(job) => {
                self.preview = None;
                self.job = Some(GenerationJob::Batch(job));
                self.panel = Some(Panel::Generating);
                self.status = i18n::tr("Edition", "GenerationPreparing");
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &crate::i18n::editor_error(&error))],
                )
            }
        }
    }

    pub(super) fn change_batch_entry(&mut self, forward: bool) {
        if let Some(GenerationPreview::Batch { preview, entry }) = &mut self.preview {
            let count = preview.entries().len();
            if count > 0 {
                *entry = (*entry + if forward { 1 } else { count - 1 }) % count;
                self.receptor_views.invalidate_preview();
                // Switching the detached result changes its source clock and
                // admitted note tree. Publish those resources before actors
                // can observe the newly selected entry.
                self.refresh();
            }
        }
    }

    pub(super) fn batch_label(&self, index: usize, key: &str) -> Arc<str> {
        match index {
            0 => {
                let source = self.batch_source.as_deref().unwrap_or("");
                let count = self.batch_source_count(source);
                i18n::tr_fmt(
                    "Edition",
                    key,
                    &[("value", source), ("count", &count.to_string())],
                )
            }
            1..=3 => self.generation_label(index - 1, key),
            4 => self.generation_label(5, key),
            7 => self.generation_label(6, key),
            _ => i18n::tr("Edition", key),
        }
    }

    pub(super) fn batch_summary(&self, preview: &batch_job::Preview, index: usize) -> Arc<str> {
        let report = preview.report();
        let mut summary = i18n::tr_fmt(
            "Edition",
            "BatchSummary",
            &[
                ("generated", &report.generated.to_string()),
                ("failed", &report.failed.to_string()),
                ("total", &report.total.to_string()),
                ("layout", preview.target_layout()),
            ],
        )
        .to_string();
        if let Some(entry) = preview.entries().get(index) {
            summary.push('\n');
            summary.push_str(&i18n::tr_fmt(
                "Edition",
                "BatchEntry",
                &[
                    ("index", &(index + 1).to_string()),
                    ("total", &report.total.to_string()),
                    ("source", &(entry.source_chart + 1).to_string()),
                    ("difficulty", &entry.difficulty),
                    ("meter", &entry.meter),
                ],
            ));
            summary.push('\n');
            if let Some(result) = entry.preview() {
                let report = result.report();
                summary.push_str(&i18n::tr_fmt(
                    "Edition",
                    "BatchEntryReady",
                    &[
                        ("notes", &report.notes.to_string()),
                        ("mines", &report.mines.to_string()),
                        ("skipped", &report.skipped_events.to_string()),
                        ("dropped", &report.dropped_mines.to_string()),
                        ("issues", &result.profile_issues().len().to_string()),
                    ],
                ));
            } else if let Some(error) = entry.error() {
                summary.push_str(&i18n::tr_fmt(
                    "Edition",
                    "BatchEntryFailed",
                    &[("message", error)],
                ));
            }
        }
        summary.into()
    }
}

#[cfg(test)]
mod effective_source_tests {
    use super::*;
    fn fixture(mixed:bool)->(tempfile::TempDir,State){
        crate::i18n::init_for_tests();let root=tempfile::tempdir().unwrap();let path=root.path().join("batch.ssc");
        let mut bytes=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:pump-double;#DIFFICULTY:Hard;#METER:9;#NOTES:X000000000;".to_vec();
        if mixed {bytes.extend_from_slice(b"#NOTEDATA:;#STEPSTYPE:pump-double;#DIFFICULTY:Hard;#METER:9;#NOTES:1000000000;",
            );}
        std::fs::write(&path,bytes).unwrap();let mut state=State::open(&path,0).unwrap();state.profile_directory=None;(root,state)
    }
    fn press(state:&mut State,code:KeyCode){for pressed in [true,false]{state.raw_key(&RawKeyboardEvent{code,pressed,repeat:false,timestamp:std::time::Instant::now(),host_nanos:0,
            });}}
    #[test]
    fn all_coop_does_not_advertise_an_unstartable_double_batch_or_rewrite_source(){
        let (_root,mut state)=fixture(false);let before=state.workspace.file().session.serialize().unwrap();let revision=state.workspace.file().session.revision();
        state.open_batch();assert!(state.batch_sources().is_empty());assert!(state.batch_source.is_none());assert!(!matches!(state.panel,Some(Panel::Batch(_))));assert_eq!(state.status,i18n::tr("Edition","BatchNoSources"));
        state.start_batch();assert!(state.job.is_none());assert_eq!(state.workspace.file().session.serialize().unwrap(),before);assert_eq!(state.workspace.file().session.revision(),revision);assert_eq!(state.workspace.file().session.document().charts()[0].steps_type,"pump-double");
    }
    #[test]
    fn mixed_coop_double_count_matches_real_worker_source_and_labels_do_not_rescan(){
        let (_root,mut state)=fixture(true);let before=state.workspace.file().session.serialize().unwrap();let revision=state.workspace.file().session.revision();
        state.open_batch();assert_eq!(state.batch_sources(),vec!["pump-double"]);assert_eq!(state.batch_source_count("pump-double"),1);assert!(!state.batch_inventory.layouts.contains_key(&0));assert_eq!(state.batch_inventory.layouts.get(&1).map(String::as_str),Some("pump-double"));
        let builds=state.batch_inventory.rebuilds;
        for _ in 0..100 {let _=state.batch_label(0,"BatchSource");let _=state.batch_sources();state.refresh();}
        assert_eq!(state.batch_inventory.rebuilds,builds,"unchanged content never re-enumerates note encodings per frame");
        state.generation.target=TARGET_LAYOUTS.iter().position(|layout|*layout=="dance-single").unwrap();
        state.panel=Some(Panel::Batch(5));press(&mut state,KeyCode::Enter);assert!(state.job.is_some(),"{}",state.status);
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(10);
        while state.job.is_some(){state.update();assert!(std::time::Instant::now()<deadline,"{}",state.status);std::thread::yield_now();}
        let Some(GenerationPreview::Batch{preview,..})=state.preview.as_ref()else{panic!("{}",state.status);};
        assert_eq!(preview.report().total,1);assert_eq!(preview.entries()[0].source_chart,1);assert_eq!(state.workspace.file().session.serialize().unwrap(),before);assert_eq!(state.workspace.file().session.revision(),revision);assert_eq!(state.workspace.chart(),0);
    }
    #[test]
    fn cache_is_invalidated_by_content_history_and_another_session_with_equal_bytes(){
        let (root,mut state)=fixture(true);state.open_batch();let builds=state.batch_inventory.rebuilds;
        state.workspace.execute(Command::Place(1)).unwrap();assert!(!state.batch_inventory_current());assert_eq!(state.batch_source_count("pump-double"),0);
        state.refresh();assert_eq!(state.batch_inventory.rebuilds,builds+1);assert_eq!(state.batch_source_count("pump-double"),1);
        state.workspace.execute(Command::Undo).unwrap();state.refresh();assert_eq!(state.batch_inventory.rebuilds,builds+2);
        let bytes=state.workspace.file().session.serialize().unwrap();let other=root.path().join("other.ssc");std::fs::write(&other,&bytes).unwrap();state.workspace=Workspace::open(&other,0).unwrap();assert!(!state.batch_inventory_current());state.refresh();assert_eq!(state.batch_inventory.rebuilds,builds+3);assert_eq!(state.batch_source_count("pump-double"),1);
    }
}
