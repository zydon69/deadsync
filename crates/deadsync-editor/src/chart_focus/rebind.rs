use super::Position;
use crate::{
    Error,
    history::{Session, media_relocation::rebind::RelocationReceipt},
};
impl Position {
    pub fn can_rebind_relocation(&self, receipt: &RelocationReceipt) -> bool {
        receipt.accepts_before(&self.token)
    }
    /// Keep fractional/negative/warp row and time independently; no inversion.
    pub fn rebind_relocation(
        &self,
        session: &Session,
        receipt: &RelocationReceipt,
    ) -> Result<Self, Error> {
        if session.chart_ids().get(self.chart) != Some(&self.identity) {
            return Err(Error::InvalidEdit(
                "Relocation changed focus chart identity".into(),
            ));
        }
        let mut result = self.clone();
        result.token = receipt.rebind_token(&self.token, session)?;
        Ok(result)
    }
}
