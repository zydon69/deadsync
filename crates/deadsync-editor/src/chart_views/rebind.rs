use super::ActiveChartViews;
use crate::{
    Error,
    history::{Session, media_relocation::rebind::RelocationReceipt},
};
impl ActiveChartViews {
    pub fn can_rebind_relocation(&self, receipt: &RelocationReceipt) -> bool {
        self.entries.iter().all(|entry| {
            receipt.accepts_before(&entry.clock_token)
                && entry.position.can_rebind_relocation(receipt)
        })
    }
    pub fn rebind_relocation(
        &mut self,
        session: &Session,
        receipt: &RelocationReceipt,
    ) -> Result<(), Error> {
        receipt.validate_after(session)?;
        // Validate all entries before changing any token. Clock has only timing,
        // which this proof guarantees unchanged; retain its Arc and exact row.
        if !self.belongs_to(session)
            || !self.can_rebind_relocation(receipt)
            || self
                .entries
                .iter()
                .any(|entry| session.chart_ids().get(entry.source_index()) != Some(&entry.identity))
        {
            return Err(Error::InvalidEdit("Relocation view rebind is stale".into()));
        }
        for entry in &self.entries {
            receipt.rebind_token(&entry.clock_token, session)?;
        }
        for entry in &mut self.entries {
            entry.position = entry.position.rebind_relocation(session, receipt)?;
            entry.clock_token = receipt.rebind_token(&entry.clock_token, session)?;
        }
        self.owner = session.content_token();
        Ok(())
    }
}
