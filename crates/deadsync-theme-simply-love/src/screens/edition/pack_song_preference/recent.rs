//! Shared comparison boundary for direct Open and saved-view restoration.
use super::*;
impl Prepared {
    pub(in crate::screens::edition) fn restoration(
        &self,
        saved: Option<&deadsync_editor::recent_songs::Song>,
        orderer: &mut impl Orderer,
    ) -> Result<deadsync_editor::recent_songs::Restoration, Error> {
        let order = if self.snapshot.source_indices().len() < 2 {
            self.snapshot.source_indices().to_vec()
        } else {
            orderer.order(&self.snapshot)?
        };
        if let Some(saved) = saved {
            saved.resolve(&self.snapshot, &order, self.preferences)
        } else {
            let focused = self.preferences.select(&self.snapshot, &order)?;
            Ok(deadsync_editor::recent_songs::Restoration {
                active: focused.into_iter().collect(),
                focused,
            })
        }
    }
}
