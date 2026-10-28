//! Detached output policy. No path mutation, publication or recovery encoding.
//!
//! The target format survives native DocumentSwap history. Native lossless Save
//! retains its byte-preserving contract; advanced output requires a fresh review.
use crate::{
    Error,
    advanced_export::{self, Issue, Limits, Options},
    document::Format,
    generation::metadata::Guid,
    history::{Session, file_intent::IntentGuard},
};
use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutputPolicy {
    NativeLossless,
    Advanced {
        format: Format,
        options: Options,
        timing_chart: Guid,
    },
}
impl OutputPolicy {
    /// Chart indices are resolved afresh. Presentation sorting never changes the
    /// selected identity; structural deletion cannot silently select a peer.
    pub fn timing_chart(&self, session: &Session) -> Result<Option<usize>, Error> {
        match self {
            Self::NativeLossless => Ok(None),
            Self::Advanced { timing_chart, .. } => session
                .chart_ids()
                .iter()
                .position(|id| id == timing_chart)
                .map(Some)
                .ok_or_else(|| {
                    invalid(
                        "The output timing chart no longer exists; choose another chart explicitly",
                    )
                }),
        }
    }
    pub fn prepare_output(
        &self,
        session: &mut Session,
        limits: Limits,
        cancelled: &AtomicBool,
    ) -> Result<PreparedOutput, Error> {
        check(cancelled)?;
        let guard = IntentGuard::capture(session);
        let prepared = match self {
            Self::NativeLossless => {
                if session.document().source().len() > limits.bytes
                    || session.document().charts().len() > limits.charts
                {
                    return Err(invalid(
                        "Native output exceeds the configured source limits",
                    ));
                }
                // Same established serializer as ordinary Save. This cap is not
                // a peak-memory guarantee for pending grid serialization.
                let bytes = session.serialize()?;
                check(cancelled)?;
                if bytes.len() > limits.bytes {
                    return Err(invalid("Native output byte limit exceeded"));
                }
                PreparedKind::Native {
                    bytes: bytes.into(),
                    format: session.document().format,
                }
            }
            Self::Advanced {
                format, options, ..
            } => {
                let chart = self
                    .timing_chart(session)?
                    .ok_or_else(|| invalid("Advanced output requires a timing chart"))?;
                PreparedKind::Advanced(advanced_export::Plan::prepare(
                    session, *format, chart, *options, limits, cancelled,
                )?)
            }
        };
        check(cancelled)?;
        Ok(PreparedOutput {
            policy: self.clone(),
            guard,
            prepared,
        })
    }
}
pub struct PreparedOutput {
    policy: OutputPolicy,
    guard: IntentGuard,
    prepared: PreparedKind,
}
enum PreparedKind {
    Native { bytes: Arc<[u8]>, format: Format },
    Advanced(advanced_export::Plan),
}
impl PreparedOutput {
    pub fn policy(&self) -> &OutputPolicy {
        &self.policy
    }
    pub fn issues(&self) -> &[Issue] {
        match &self.prepared {
            PreparedKind::Native { .. } => &[],
            PreparedKind::Advanced(plan) => plan.issues(),
        }
    }
    pub fn is_current(&self, session: &Session) -> bool {
        self.guard.is_current(session)
    }
    pub fn generate(
        &self,
        session: &Session,
        acknowledged: &BTreeSet<usize>,
        maximum_bytes: usize,
        cancelled: &AtomicBool,
    ) -> Result<Output, Error> {
        check(cancelled)?;
        if !self.is_current(session) {
            return Err(invalid(
                "Output review is stale or belongs to another session",
            ));
        }
        let kind = match &self.prepared {
            PreparedKind::Native { bytes, format } => {
                if !acknowledged.is_empty() {
                    return Err(invalid("Native output has no loss acknowledgements"));
                }
                if bytes.len() > maximum_bytes {
                    return Err(invalid("Native output byte limit exceeded"));
                }
                OutputKind::Native {
                    bytes: Arc::clone(bytes),
                    format: *format,
                }
            }
            PreparedKind::Advanced(plan) => OutputKind::Advanced(plan.generate(
                session,
                acknowledged,
                maximum_bytes,
                cancelled,
            )?),
        };
        check(cancelled)?;
        Ok(Output {
            policy: self.policy.clone(),
            guard: self.guard.clone(),
            kind,
        })
    }
}
/// Not a publication receipt. Callers cannot replace its bytes or output policy.
pub struct Output {
    policy: OutputPolicy,
    guard: IntentGuard,
    kind: OutputKind,
}
enum OutputKind {
    Native { bytes: Arc<[u8]>, format: Format },
    Advanced(advanced_export::Output),
}
impl Output {
    pub fn policy(&self) -> &OutputPolicy {
        &self.policy
    }
    pub fn bytes(&self) -> &[u8] {
        match &self.kind {
            OutputKind::Native { bytes, .. } => bytes,
            OutputKind::Advanced(output) => output.bytes(),
        }
    }
    pub fn format(&self) -> Format {
        match &self.kind {
            OutputKind::Native { format, .. } => *format,
            OutputKind::Advanced(output) => output.format(),
        }
    }
    pub fn is_current(&self, session: &Session) -> bool {
        self.guard.is_current(session)
            && match &self.kind {
                OutputKind::Native { .. } => true,
                OutputKind::Advanced(output) => output.is_current(session),
            }
    }
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
fn check(cancelled: &AtomicBool) -> Result<(), Error> {
    if cancelled.load(Ordering::Relaxed) {
        Err(invalid("Output preparation was cancelled"))
    } else {
        Ok(())
    }
}
