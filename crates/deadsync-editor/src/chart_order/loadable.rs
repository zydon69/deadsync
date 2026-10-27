//! Opening may skip one unsupported editor grid/clock without losing its source
//! section. Ordinary Snapshot::resident remains strict for existing callers.
use super::Snapshot;
use crate::{Error, generation::metadata::Guid, history::Session};
#[derive(Clone, Debug)]
pub struct Failure {
    pub source_index: usize,
    pub identity: Guid,
    pub message: String,
}
pub struct Report {
    pub snapshot: Snapshot,
    pub failures: Vec<Failure>,
}
impl Snapshot {
    pub fn loadable(session: &mut Session) -> Result<Report, Error> {
        let identities = session.chart_ids().to_vec();
        let (snapshot, failures) = Self::read_mode(session, &identities, true, || false)?;
        Ok(Report { snapshot, failures })
    }
    /// Check cancellation between charts and after each existing grid/clock
    /// parser. Cancellation returns no partial Report and never becomes a chart
    /// failure that could accidentally authorize a fallback.
    pub fn loadable_checked(
        session: &mut Session,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<Report, Error> {
        let identities = session.chart_ids().to_vec();
        let (snapshot, failures) = Self::read_mode(session, &identities, true, || {
            cancelled.load(std::sync::atomic::Ordering::Relaxed)
        })?;
        Ok(Report { snapshot, failures })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Document, Format};
    #[test]
    fn cancellation_between_charts_never_returns_partial_candidates_or_failure_report() {
        let bytes=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-double;#NOTES:10000000;";
        let mut session = Session::new(Document::parse(bytes.as_slice(), Format::Ssc).unwrap());
        let identities = session.chart_ids().to_vec();
        let token = session.content_token();
        let mut calls = 0;
        let result = Snapshot::read_mode(&mut session, &identities, true, || {
            calls += 1;
            calls >= 3
        });
        assert!(result.is_err());
        assert_eq!(calls, 3);
        assert_eq!(session.serialize().unwrap(), bytes);
        assert!(session.accepts_content_token(&token));
        assert!(!session.is_dirty());
    }
}
