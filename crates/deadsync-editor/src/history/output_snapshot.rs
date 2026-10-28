//! Detached native snapshots preserve chart ownership without cloning/replaying
//! history or any external preference/profile side effect.
use super::*;
impl Session {
    pub(crate) fn detached_output_snapshot(&self, document: Document) -> Result<Self, Error> {
        let mut snapshot = Self::staged(document);
        snapshot.install_snapshot_chart_ids(self.chart_ids.clone())?;
        Ok(snapshot)
    }
    pub(crate) fn recovered_output_snapshot(
        document: Document,
        identities: Vec<crate::generation::metadata::Guid>,
    ) -> Result<Self, Error> {
        let mut snapshot = Self::recovered(document);
        snapshot.install_snapshot_chart_ids(identities)?;
        Ok(snapshot)
    }
    fn install_snapshot_chart_ids(
        &mut self,
        identities: Vec<crate::generation::metadata::Guid>,
    ) -> Result<(), Error> {
        if identities.len() != self.document.charts().len()
            || identities
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != identities.len()
        {
            return Err(Error::InvalidFormat(
                "Recovery chart identities are incomplete or duplicated".into(),
            ));
        }
        self.chart_ids = identities;
        Ok(())
    }
}
