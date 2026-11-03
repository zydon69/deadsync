//! Candidate preparation is independent of the active editor. The worker owns
//! its FileSession and builds existing comparison keys; cultural sorting stays
//! on the main thread before publication.
mod recent;
use deadsync_editor::{
    Error,
    chart_order::{Orderer, Snapshot, loadable::Failure},
    open_preferences::Preferences,
    storage::FileSession,
    workspace::Workspace,
};
pub(super) struct Prepared {
    pub(super) file: FileSession,
    snapshot: Snapshot,
    preferences: Preferences,
    failures: Vec<Failure>,
}
impl Prepared {
    pub(super) fn new(
        file: FileSession,
        preferences: Preferences,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<Self, Error> {
        Self::new_with_profiles(
            file,
            preferences,
            cancelled,
            deadsync_editor::runtime::configs_dir(),
        )
    }
    pub(super) fn new_with_profiles(
        mut file: FileSession,
        preferences: Preferences,
        cancelled: &std::sync::atomic::AtomicBool,
        directory: Option<&std::path::Path>,
    ) -> Result<Self, Error> {
        file.resolve_generation_profiles(directory, cancelled)?;
        let report = Snapshot::loadable_checked(&mut file.session, cancelled)?;
        let snapshot = report.snapshot;
        Ok(Self {
            file,
            snapshot,
            preferences,
            failures: report.failures,
        })
    }
    pub(super) fn selected(&self, orderer: &mut impl Orderer) -> Result<Option<usize>, Error> {
        let order = if self.snapshot.source_indices().len() < 2 {
            self.snapshot.source_indices().to_vec()
        } else {
            orderer.order(&self.snapshot)?
        };
        self.preferences.select(&self.snapshot, &order)
    }
    pub(super) fn workspace(
        self,
        chart: Option<usize>,
    ) -> Result<(Workspace, Vec<Failure>), Error> {
        self.file
            .validate_generation_profiles(&std::sync::atomic::AtomicBool::new(false))?;
        // An empty comparison set means no supported editable chart, including
        // a song whose unsupported source blocks are still retained.
        let workspace = if let Some(chart) = chart {
            Workspace::from_file(self.file, chart)?
        } else {
            Workspace::from_file_unfocused(self.file)
        };
        Ok((workspace, self.failures))
    }
}

impl super::State {
    /// Original source indices/GUIDs and failures retained for diagnosis. No
    /// unavailable source block is removed or rewritten during preparation.
    pub fn song_open_warnings(&self) -> &[Failure] {
        &self.song_open_warnings
    }
}
