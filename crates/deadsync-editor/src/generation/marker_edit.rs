//! Snapshot-bound editing of saved pattern markers, independent of their notes.
use super::metadata::{ChartData, Definition};
use crate::{
    Error,
    history::{RevisionToken, Session},
};

/// Both variants use the complete editor-action pattern worker. Disk profiles
/// remain loaded on that worker, never by a UI event handler.
pub enum GenerationTask {
    Builtin(super::pattern_job::Task),
    Directory(super::pattern_job::SavedTask),
}
impl GenerationTask {
    pub fn spawn(self) -> Result<super::job::Job<super::pattern_job::Preview>, Error> {
        match self { Self::Builtin(task) => task.spawn(), Self::Directory(task) => task.spawn() }
    }
}

pub struct Draft {
    token: RevisionToken,
    chart: usize,
    original_start: u64,
    original: Definition,
    original_expression: super::metadata::Guid,
    pub expressed_chart_config: super::metadata::Guid,
    profiles: Option<super::profile_catalog::Selection>,
    pub start: u64,
    pub definition: Definition,
}
impl Draft {
    /// Preserve pending values across the immediately following disk-only copy creation.
    pub fn new_profile_plan(&self,session:&Session,kind:super::profiles::Kind)->Result<crate::history::profile_creation::Plan,Error>{
        self.data(session,self.chart)?;
        if let Some(selection)=&self.profiles{selection.check(&self.definition.pattern_config_guid,&self.definition.performed_chart_config_guid,&self.expressed_chart_config)?;}
        crate::history::profile_creation::Plan::marker(session,self.chart,self.original_start,kind)
    }
    pub fn after_inline_profile_creation(&mut self,session:&Session,receipt:&crate::history::profile_creation::Committed)->Result<(),Error>{
        let guid=receipt.rebind(session,&self.token,self.chart,Some(self.original_start))?;
        match receipt.kind(){
            super::profiles::Kind::Pattern=>{self.original.pattern_config_guid=guid.clone();self.definition.pattern_config_guid=guid;},
            super::profiles::Kind::Performed=>{self.original.performed_chart_config_guid=guid.clone();self.definition.performed_chart_config_guid=guid;},
            super::profiles::Kind::Expressed=>{self.original_expression=guid.clone();self.expressed_chart_config=guid;},
        }
        if let Some(selection)=self.profiles.as_mut(){selection.after_inline_creation(receipt);}
        self.token=session.revision_token();Ok(())
    }
    pub fn after_profile_creation(&mut self, session: &Session) -> Result<(), Error> {
        self.token = session.after_profile_creation(&self.token)?;
        Ok(())
    }

    /// Overlapping markers use the same latest-start selection as F5.
    pub fn at_cursor(session: &Session, chart: usize, cursor: u64) -> Result<Self, Error> {
        let data = ChartData::read(session.document(), chart)?
            .ok_or_else(|| invalid("No saved pattern markers in this chart"))?;
        let (start, definition) = data
            .marker_at(cursor)
            .ok_or_else(|| invalid("No saved pattern marker at the cursor"))?;
        Ok(Self {
            token: session.revision_token(),
            chart,
            original_start: start,
            original: definition.clone(),
            original_expression: data.expressed_chart_config.clone(),
            expressed_chart_config: data.expressed_chart_config.clone(),
            profiles: None,
            start,
            definition: definition.clone(),
        })
    }
    fn data(&self, session: &Session, chart: usize) -> Result<ChartData, Error> {
        if chart != self.chart || !session.accepts_token(&self.token) {
            return Err(invalid(
                "Pattern editor belongs to another chart or revision",
            ));
        }
        ChartData::read(session.document(), chart)?
            .ok_or_else(|| invalid("Saved pattern metadata is missing"))
    }
    pub fn has_changes(&self) -> bool {
        self.start != self.original_start
            || self.definition != self.original
            || self.expressed_chart_config != self.original_expression
    }
    pub fn clear_task(
        &self,
        session: &Session,
        chart: usize,
        directory: Option<&std::path::Path>,
    ) -> Result<super::clear_patterns::Task, Error> {
        self.data(session, chart)?;
        if self.has_changes() {
            return Err(invalid(
                "Apply or discard marker edits before clearing its notes",
            ));
        }
        super::clear_patterns::Task::capture(session, chart, [self.original_start], directory)
    }

    /// Generate the inspected marker without resolving it again at the cursor.
    pub fn generation_task(
        &self, session: &Session, chart: usize, directory: Option<&std::path::Path>,
    ) -> Result<GenerationTask, Error> {
        self.data(session, chart)?;
        if self.has_changes() {
            return Err(invalid("Apply or discard marker edits before generating its notes"));
        }
        match directory {
            Some(directory) => Ok(GenerationTask::Directory(super::pattern_job::SavedTask::capture(session, chart, self.original_start, directory)?)),
            None => Ok(GenerationTask::Builtin(super::pattern_job::Task::capture_saved(session, chart, self.original_start, &super::metadata::Registry::builtin())?)),
        }
    }

    /// Source random-seed callbacks are immediate, independent actions. A pending
    /// native Apply draft must be resolved before this action can target its event.
    pub(crate) fn reseed(&self, session:&mut Session, chart:usize, seed:i32)->Result<bool,Error>{
        let mut data=self.data(session,chart)?;
        if self.has_changes(){return Err(invalid("Apply or discard marker edits before choosing a new seed"));}
        if !(0..i32::MAX).contains(&seed){return Err(invalid("New random seed is outside System.Random.Next's range"));}
        let definition=data.patterns.as_mut().and_then(|patterns|patterns.get_mut(&self.original_start)).ok_or_else(||invalid("Inspected pattern marker is missing"))?;
        if *definition!=self.original{return Err(invalid("Inspected pattern marker changed"));}
        definition.random_seed=seed;
        if data.save_with_offset(session,chart,None,"Set pattern random seed")?{return Ok(true);}
        // Random.Next may repeat the previous value. Source still enqueues the
        // explicit seed action; preserve history without inventing byte changes.
        session.commit_marker_seed_touch(chart)
    }

    pub fn profile_task(
        &self,
        session: &Session,
        chart: usize,
        directory: Option<&std::path::Path>,
    ) -> Result<super::profile_catalog::Task, Error> {
        self.data(session, chart)?;
        super::profile_catalog::Task::capture(session, chart, directory)
    }
    pub fn select_profiles(
        &mut self,
        session: &Session,
        chart: usize,
        catalog: &super::profile_catalog::Catalog,
        pattern: &super::metadata::Guid,
        performed: &super::metadata::Guid,
    ) -> Result<(), Error> {
        self.data(session, chart)?;
        let selection = catalog.select(session, chart, pattern, performed)?;
        self.definition.pattern_config_guid = pattern.clone();
        self.definition.performed_chart_config_guid = performed.clone();
        self.profiles = Some(selection);
        Ok(())
    }

    /// The expressed profile belongs to the chart, including every saved marker.
    pub fn select_generation_profiles(
        &mut self,
        session: &Session,
        chart: usize,
        catalog: &super::profile_catalog::Catalog,
        pattern: &super::metadata::Guid,
        performed: &super::metadata::Guid,
        expressed: &super::metadata::Guid,
    ) -> Result<(), Error> {
        self.data(session, chart)?;
        let selection = catalog.select_generation(session, chart, pattern, performed, expressed)?;
        self.definition.pattern_config_guid = pattern.clone();
        self.definition.performed_chart_config_guid = performed.clone();
        self.expressed_chart_config = expressed.clone();
        self.profiles = Some(selection);
        Ok(())
    }

    pub fn apply(&self, session: &mut Session, chart: usize) -> Result<bool, Error> {
        let mut data = self.data(session, chart)?;
        if !self.has_changes() {
            return Ok(false);
        }
        let patterns = data
            .patterns
            .as_mut()
            .ok_or_else(|| invalid("Saved pattern metadata is missing"))?;
        if self.start != self.original_start && patterns.contains_key(&self.start) {
            return Err(invalid("A saved pattern already exists at the new start"));
        }
        patterns.remove(&self.original_start);
        patterns.insert(self.start, self.definition.clone());
        data.expressed_chart_config = self.expressed_chart_config.clone();
        data.validate()?;
        if let Some(selection) = &self.profiles {
            selection.check(
                &self.definition.pattern_config_guid,
                &self.definition.performed_chart_config_guid,
                &self.expressed_chart_config,
            )?;
        }
        data.save(session, chart)
    }
    /// Deleting a marker does not delete or regenerate its notes.
    pub fn delete(&self, session: &mut Session, chart: usize) -> Result<bool, Error> {
        let mut data = self.data(session, chart)?;
        data.patterns
            .as_mut()
            .ok_or_else(|| invalid("Saved pattern metadata is missing"))?
            .remove(&self.original_start);
        data.save(session, chart)
    }
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
