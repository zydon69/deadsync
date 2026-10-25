//! Snapshot analysis on one process-wide bounded worker. RSSP has no cooperative
//! cancellation hook; cancellation discards its result after the call returns.
use crate::{
    Error,
    document::Format,
    generation::job::Job,
    history::{RevisionToken, Session},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
static RUNNING: AtomicBool = AtomicBool::new(false);
struct Permit;
impl Drop for Permit {
    fn drop(&mut self) {
        RUNNING.store(false, Ordering::Release);
    }
}
pub fn worker_busy() -> bool {
    RUNNING.load(Ordering::Acquire)
}
pub struct Task {
    bytes: Arc<[u8]>,
    format: Format,
    token: RevisionToken,
    chart: usize,
}
pub struct Report {
    token: RevisionToken,
    chart: usize,
    summary: rssp::ChartSummary,
    columns: Vec<DensityColumn>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DensityColumn {
    pub first: usize,
    pub end: usize,
    pub steps: usize,
}
impl Task {
    pub fn capture(session: &Session, chart: usize) -> Result<Self, Error> {
        if chart >= session.document().charts().len() {
            return Err(invalid("Analysis chart does not exist"));
        }
        Ok(Self {
            bytes: session.serialize()?.into(),
            format: session.document().format,
            token: session.revision_token(),
            chart,
        })
    }
    pub fn spawn(self) -> Result<Job<Report>, Error> {
        RUNNING
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| invalid("An editor analysis is already running"))?;
        let permit = Permit;
        Job::spawn_with("editor-analysis", move |cancelled, _progress| {
            let _permit = permit;
            self.run(cancelled)
        })
    }
    pub fn run(self, cancelled: &AtomicBool) -> Result<Report, Error> {
        check(cancelled)?;
        let summary = super::chart_bytes(
            &self.bytes,
            self.format,
            self.chart,
            &rssp::AnalysisOptions::default(),
        )?;
        check(cancelled)?;
        let columns = density_columns(&summary.measure_densities);
        Ok(Report {
            token: self.token,
            chart: self.chart,
            summary,
            columns,
        })
    }
}
impl Report {
    pub fn is_current(&self, session: &Session, chart: usize) -> bool {
        self.chart == chart && session.accepts_token(&self.token)
    }
    pub fn summary(&self) -> &rssp::ChartSummary {
        &self.summary
    }
    pub fn columns(&self) -> &[DensityColumn] {
        &self.columns
    }
    /// RSSP density bins are four quarter-note beats, independent of signatures.
    pub fn row_for_bin(&self, index: usize) -> Result<u64, Error> {
        if index >= self.summary.measure_densities.len() {
            return Err(invalid("Density interval does not exist"));
        }
        let row = (index as u64)
            .checked_mul(192)
            .filter(|row| *row <= i32::MAX as u64)
            .ok_or_else(|| invalid("Density interval exceeds the reference row limit"))?;
        Ok(row)
    }
}
fn density_columns(values: &[usize]) -> Vec<DensityColumn> {
    let stride = values.len().div_ceil(80).max(1);
    values
        .chunks(stride)
        .enumerate()
        .map(|(index, chunk)| DensityColumn {
            first: index * stride,
            end: index * stride + chunk.len(),
            steps: chunk.iter().copied().max().unwrap_or(0),
        })
        .collect()
}
fn check(cancelled: &AtomicBool) -> Result<(), Error> {
    if cancelled.load(Ordering::Relaxed) {
        Err(invalid("Analysis cancelled"))
    } else {
        Ok(())
    }
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn density_columns_bound_work_and_keep_every_peak() {
        assert!(density_columns(&[]).is_empty());
        for length in [1, 79, 80, 81, 159, 160, 10001] {
            let values = (0..length)
                .map(|i| if i % 7 == 0 { 100 } else { i % 13 })
                .collect::<Vec<_>>();
            let columns = density_columns(&values);
            assert!(columns.len() <= 80);
            assert_eq!(columns[0].first, 0);
            assert_eq!(columns.last().unwrap().end, length);
            for (index, column) in columns.iter().enumerate() {
                assert_eq!(
                    column.steps,
                    *values[column.first..column.end].iter().max().unwrap()
                );
                if index > 0 {
                    assert_eq!(columns[index - 1].end, column.first);
                }
            }
        }
    }
}

#[cfg(test)]
mod worker_tests {
    use super::*;
    use crate::document::Document;
    #[test]
    fn permit_rejects_overlapping_workers_and_cancellation_releases_it() {
        let session = Session::new(
            Document::parse(
                b"#BPMS:0=120;#NOTES:dance-single::Hard:9::1000;".as_slice(),
                Format::Sm,
            )
            .unwrap(),
        );
        RUNNING
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .unwrap();
        let permit = Permit;
        assert!(worker_busy());
        assert!(Task::capture(&session, 0).unwrap().spawn().is_err());
        drop(permit);
        let mut job = Task::capture(&session, 0).unwrap().spawn().unwrap();
        job.cancel();
        let limit = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            if let Some(result) = job.poll() {
                assert!(result.is_err());
                break;
            }
            assert!(std::time::Instant::now() < limit);
            std::thread::yield_now();
        }
        assert!(!worker_busy());
        assert!(job.poll().is_none());
    }
}
