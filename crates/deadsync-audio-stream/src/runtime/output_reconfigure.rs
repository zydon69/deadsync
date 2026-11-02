//! One output transaction owns old and replacement managers. A native shutdown
//! failure quarantines output; it never opens another device over an unknown one.
use super::*;
use deadlib_audio_core::{
    BufferCapabilities, BufferControl, BufferNegotiation, BufferRequest, OutputBufferState,
    ReconfigureError, ReconfigureFailure, ReconfigureOutcome,
};
use std::sync::{Arc, Mutex, atomic::AtomicU64};
use std::thread::{self, JoinHandle};

mod transaction;

#[derive(Clone)]
struct Snapshot {
    plan: Arc<deadlib_audio::OutputPlan>,
    config: InitConfig,
    controls: Arc<deadlib_audio_core::MixControls>,
    buffers: OutputBufferState,
    output: OutputFormat,
    backend: &'static str,
    device: String,
}
impl Snapshot {
    fn capture(engine: &AudioEngine) -> Result<Self, ReconfigureError> {
        let live = engine.live.as_ref().ok_or(ReconfigureError::Unavailable)?;
        Ok(Self {
            plan: live.plan.clone(),
            config: live.config,
            controls: live.controls.clone(),
            buffers: live.buffers,
            output: engine.output,
            backend: live.backend,
            device: live.device.clone(),
        })
    }
    fn effective_request(&self) -> BufferRequest {
        // CoreAudio device state can survive a stream. Restore observed frames
        // even when the original requested preference was None (native defaults).
        if self.backend.starts_with("coreaudio-") {
            BufferRequest {
                period_frames: self.buffers.negotiated.period_frames,
                num_buffers: None,
            }
        } else {
            self.buffers.requested
        }
    }
}
struct Lease {
    owner: u64,
    epoch: AtomicU64,
}
#[derive(Clone, Copy)]
enum Guard {
    Lease,
    Exact(u64),
}
#[derive(Clone)]
pub struct OutputRestoreReceipt {
    snapshot: Arc<Snapshot>,
    lease: Arc<Lease>,
    guard: Guard,
}

pub struct ReconfigureAck {
    pub owner: u64,
    pub id: u64,
    pub outcome: ReconfigureOutcome,
    pub clock: Option<MusicClock>,
    pub format: Option<OutputFormat>,
    pub rollback: Option<OutputRestoreReceipt>,
}

pub(super) struct OutputReconfigure {
    epoch: u64,
    last_id: Option<u64>,
    recovery_allowed: bool,
    lease: Option<Arc<Lease>>,
    pending: Option<Pending>,
}
impl OutputReconfigure {
    pub(super) const fn new() -> Self {
        Self {
            epoch: 0,
            last_id: None,
            recovery_allowed: false,
            lease: None,
            pending: None,
        }
    }
}
struct Pending {
    owner: u64,
    id: u64,
    epoch: u64,
    lease: Arc<Lease>,
    previous: Arc<Snapshot>,
    cancel: Arc<AtomicBool>,
    thread: Option<JoinHandle<WorkerResult>>,
}
struct ClockInstall {
    played_map: deadlib_audio_core::PlayedMapReader,
    sample_rate_hz: u32,
}
struct WorkerResult {
    engine: Option<AudioEngine>,
    clock: Option<ClockInstall>,
    outcome: ReconfigureOutcome,
    published: bool,
    recovery_allowed: bool,
}
impl Drop for Pending {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        // Forced destruction drains the bounded transaction; no detached manager
        // may outlive AudioControl and later replace a newer host configuration.
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl AudioControl {
    pub fn output_buffer_state(&self) -> OutputBufferState {
        self.engine
            .as_ref()
            .and_then(|engine| engine.live.as_ref())
            .map(|live| live.buffers)
            .unwrap_or(OutputBufferState {
                capabilities: BufferCapabilities {
                    period_frames: BufferControl::Unsupported,
                    num_buffers: BufferControl::Unsupported,
                },
                requested: BufferRequest::default(),
                negotiated: BufferNegotiation::default(),
            })
    }
    pub fn capture_output_lease(
        &mut self,
        owner: u64,
    ) -> Result<OutputRestoreReceipt, ReconfigureError> {
        if self.output_reconfigure.pending.is_some() {
            return Err(ReconfigureError::Busy);
        }
        let snapshot =
            Snapshot::capture(self.engine.as_ref().ok_or(ReconfigureError::Unavailable)?)?;
        let lease = Arc::new(Lease {
            owner,
            epoch: AtomicU64::new(self.output_reconfigure.epoch),
        });
        self.output_reconfigure.lease = Some(lease.clone());
        self.output_reconfigure.last_id = None;
        Ok(OutputRestoreReceipt {
            snapshot: Arc::new(snapshot),
            lease,
            guard: Guard::Lease,
        })
    }
    pub fn begin_output_reconfigure(
        &mut self,
        owner: u64,
        id: u64,
        request: BufferRequest,
    ) -> Result<(), ReconfigureError> {
        if self.output_reconfigure.pending.is_some() {
            return Err(ReconfigureError::Busy);
        }
        let old = Snapshot::capture(self.engine.as_ref().ok_or(ReconfigureError::Unavailable)?)?;
        request.validate(old.buffers.capabilities)?;
        let mut target = old.clone();
        target.buffers.requested = old.buffers.requested.merged(request);
        let effective = target.buffers.requested;
        self.begin_output_transaction(owner, id, old, target, effective)
    }
    pub fn begin_output_restore(
        &mut self,
        owner: u64,
        id: u64,
        receipt: &OutputRestoreReceipt,
    ) -> Result<(), ReconfigureError> {
        if self.output_reconfigure.pending.is_some() {
            return Err(ReconfigureError::Busy);
        }
        if !restore_authorized(
            self.output_reconfigure.epoch,
            self.output_reconfigure.lease.as_ref(),
            owner,
            &receipt.lease,
            receipt.guard,
        ) {
            return Err(ReconfigureError::StaleOwner);
        }
        let old = match self.engine.as_ref() {
            Some(engine) => Snapshot::capture(engine)?,
            None if self.output_reconfigure.recovery_allowed => receipt.snapshot.as_ref().clone(),
            None => return Err(ReconfigureError::Unavailable),
        };
        self.begin_output_transaction(
            owner,
            id,
            old,
            receipt.snapshot.as_ref().clone(),
            receipt.snapshot.effective_request(),
        )
    }
    fn begin_output_transaction(
        &mut self,
        owner: u64,
        id: u64,
        previous: Snapshot,
        target: Snapshot,
        effective: BufferRequest,
    ) -> Result<(), ReconfigureError> {
        if self
            .output_reconfigure
            .last_id
            .is_some_and(|previous| id <= previous)
        {
            return Err(ReconfigureError::StaleOwner);
        }
        let lease = self
            .output_reconfigure
            .lease
            .as_ref()
            .filter(|lease| {
                lease.owner == owner
                    && lease.epoch.load(Ordering::Acquire) == self.output_reconfigure.epoch
            })
            .cloned()
            .ok_or(ReconfigureError::StaleOwner)?;
        let epoch = self
            .output_reconfigure
            .epoch
            .checked_add(1)
            .ok_or(ReconfigureError::Unavailable)?;
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        self.freeze_editor_preview_before_handoff();
        let handoff = Arc::new(Mutex::new(self.engine.take()));
        let worker_handoff = handoff.clone();
        let previous = Arc::new(previous);
        let restore = previous.clone();
        let thread = match thread::Builder::new()
            .name("output-reconfigure".into())
            .spawn(move || {
                let engine = worker_handoff
                    .lock()
                    .expect("output handoff poisoned before worker")
                    .take();
                match engine {
                    Some(engine) => reconfigure(engine, restore, target, effective, &worker_cancel),
                    None => recover_empty(target, effective, &worker_cancel),
                }
            }) {
            Ok(thread) => thread,
            Err(error) => {
                log::error!("Unable to spawn output reconfiguration worker: {error}");
                self.engine = handoff
                    .lock()
                    .map_err(|_| ReconfigureError::Unavailable)?
                    .take();
                self.restore_preview_after_output_handoff();
                return Err(ReconfigureError::Unavailable);
            }
        };
        self.output_reconfigure.last_id = Some(id);
        self.output_reconfigure.pending = Some(Pending {
            owner,
            id,
            epoch,
            lease,
            previous,
            cancel,
            thread: Some(thread),
        });
        Ok(())
    }
    pub fn release_output_lease(&mut self, owner: u64) -> Result<(), ReconfigureError> {
        if self.output_reconfigure.pending.is_some() {
            return Err(ReconfigureError::Busy);
        }
        if !self
            .output_reconfigure
            .lease
            .as_ref()
            .is_some_and(|lease| lease.owner == owner)
        {
            return Err(ReconfigureError::StaleOwner);
        }
        self.output_reconfigure.lease = None;
        self.output_reconfigure.last_id = None;
        Ok(())
    }
    pub fn cancel_output_reconfigure(&mut self, owner: u64, id: u64) {
        if let Some(pending) = self
            .output_reconfigure
            .pending
            .as_ref()
            .filter(|pending| pending.owner == owner && pending.id == id)
        {
            pending.cancel.store(true, Ordering::Release);
        }
    }
    pub fn poll_output_reconfigure(&mut self) -> Option<ReconfigureAck> {
        if !self
            .output_reconfigure
            .pending
            .as_ref()?
            .thread
            .as_ref()?
            .is_finished()
        {
            return None;
        }
        let mut pending = self.output_reconfigure.pending.take()?;
        let result = match pending.thread.take()?.join() {
            Ok(result) => result,
            Err(_) => WorkerResult {
                engine: None,
                clock: None,
                outcome: ReconfigureOutcome::FailedUnavailable {
                    reason: ReconfigureFailure::ShutdownFailed,
                },
                published: false,
                recovery_allowed: false,
            },
        };
        let format = result.engine.as_ref().map(|engine| engine.output);
        let rollback = if result.published {
            self.output_reconfigure.epoch = pending.epoch;
            pending.lease.epoch.store(pending.epoch, Ordering::Release);
            matches!(result.outcome, ReconfigureOutcome::Applied { .. }).then(|| {
                OutputRestoreReceipt {
                    snapshot: pending.previous.clone(),
                    lease: pending.lease.clone(),
                    guard: Guard::Exact(pending.epoch),
                }
            })
        } else {
            None
        };
        self.output_reconfigure.recovery_allowed =
            result.engine.is_none() && result.recovery_allowed;
        self.engine = result.engine;
        self.restore_preview_after_output_handoff();
        Some(ReconfigureAck {
            owner: pending.owner,
            id: pending.id,
            outcome: result.outcome,
            clock: result
                .clock
                .map(|clock| MusicClock::new(clock.played_map, clock.sample_rate_hz)),
            format,
            rollback,
        })
    }
}

fn safe_to_reopen(reason: ReconfigureFailure) -> bool {
    matches!(
        reason,
        ReconfigureFailure::OpenFailed
            | ReconfigureFailure::FormatChanged
            | ReconfigureFailure::RollbackFailed
    )
}

fn restore_authorized(
    epoch: u64,
    current: Option<&Arc<Lease>>,
    owner: u64,
    receipt_lease: &Arc<Lease>,
    guard: Guard,
) -> bool {
    current.is_some_and(|current| {
        current.owner == owner
            && Arc::ptr_eq(current, receipt_lease)
            && current.epoch.load(Ordering::Acquire) == epoch
            && !matches!(guard, Guard::Exact(expected) if expected != epoch)
    })
}

struct NativeHost<'a> {
    engine: &'a mut AudioEngine,
}
impl transaction::Host<Snapshot> for NativeHost<'_> {
    type Ready = stream::StreamReady;
    fn shutdown_current(&mut self) -> Result<(), ReconfigureFailure> {
        self.engine.sfx_cache.stop_owned_voices();
        let mut live = self
            .engine
            .live
            .take()
            .ok_or(ReconfigureFailure::DeviceUnavailable)?;
        live.manager.shutdown().map_err(|error| {
            log::error!("Output shutdown was not confirmed; refusing overlapping reopen: {error}");
            ReconfigureFailure::ShutdownFailed
        })
    }
    fn open(
        &mut self,
        configuration: &Snapshot,
        request: BufferRequest,
    ) -> Result<Self::Ready, ReconfigureFailure> {
        open_matching(configuration, request)
    }
    fn shutdown_replacement(
        &mut self,
        replacement: &mut Self::Ready,
    ) -> Result<(), ReconfigureFailure> {
        replacement.manager.shutdown().map_err(|error| {
            log::error!("Cancelled replacement shutdown failed: {error}");
            ReconfigureFailure::ShutdownFailed
        })
    }
}
fn reconfigure(
    mut engine: AudioEngine,
    previous: Arc<Snapshot>,
    target: Snapshot,
    effective: BufferRequest,
    cancel: &AtomicBool,
) -> WorkerResult {
    let decision = transaction::run(
        &mut NativeHost {
            engine: &mut engine,
        },
        &previous,
        &target,
        previous.effective_request(),
        effective,
        cancel,
    );
    match decision {
        transaction::Decision::UnchangedCancelled => WorkerResult {
            engine: Some(engine),
            clock: None,
            outcome: ReconfigureOutcome::Cancelled,
            published: false,
            recovery_allowed: false,
        },
        transaction::Decision::Applied(ready) => {
            let outcome = ReconfigureOutcome::Applied {
                requested: target.buffers.requested,
                negotiated: ready.buffers.negotiated,
            };
            install(engine, &target, ready, outcome)
        }
        transaction::Decision::Restored { ready, reason } => {
            let outcome = if reason == ReconfigureFailure::Cancelled {
                ReconfigureOutcome::Cancelled
            } else {
                ReconfigureOutcome::FailedRestored { reason }
            };
            install(engine, previous.as_ref(), ready, outcome)
        }
        transaction::Decision::Unavailable(reason) => WorkerResult {
            engine: None,
            clock: None,
            outcome: ReconfigureOutcome::FailedUnavailable { reason },
            published: false,
            recovery_allowed: safe_to_reopen(reason),
        },
    }
}
fn recover_empty(
    snapshot: Snapshot,
    effective: BufferRequest,
    cancel: &AtomicBool,
) -> WorkerResult {
    if cancel.load(Ordering::Acquire) {
        return WorkerResult {
            engine: None,
            clock: None,
            outcome: ReconfigureOutcome::Cancelled,
            published: false,
            recovery_allowed: true,
        };
    }
    let startup_output_devices = snapshot.plan.devices().to_vec();
    let ready = match open_matching(&snapshot, effective) {
        Ok(ready) => ready,
        Err(reason) => {
            return WorkerResult {
                engine: None,
                clock: None,
                outcome: ReconfigureOutcome::FailedUnavailable { reason },
                published: false,
                recovery_allowed: safe_to_reopen(reason),
            };
        }
    };
    let stream::StreamReady {
        manager,
        buffers,
        command_sender,
        backend_ready,
        sfx_sender,
        played_map,
        decoder_status,
    } = ready;
    let outcome = if cancel.load(Ordering::Acquire) {
        ReconfigureOutcome::Cancelled
    } else {
        ReconfigureOutcome::Applied {
            requested: snapshot.buffers.requested,
            negotiated: buffers.negotiated,
        }
    };
    let engine = AudioEngine {
        live: Some(LiveOutput {
            manager,
            plan: snapshot.plan.clone(),
            config: snapshot.config,
            controls: snapshot.controls.clone(),
            buffers: OutputBufferState {
                requested: snapshot.buffers.requested,
                ..buffers
            },
            backend: backend_ready.backend_name,
            device: backend_ready.device_name,
        }),
        command_sender,
        decoder_status,
        sfx_cache: SfxCache::new(snapshot.controls.clone(), sfx_sender),
        output: snapshot.output,
        startup_output_devices,
    };
    WorkerResult {
        engine: Some(engine),
        clock: Some(ClockInstall {
            played_map,
            sample_rate_hz: snapshot.output.sample_rate_hz,
        }),
        outcome,
        published: true,
        recovery_allowed: false,
    }
}

fn open_matching(
    snapshot: &Snapshot,
    request: BufferRequest,
) -> Result<stream::StreamReady, ReconfigureFailure> {
    let mut ready = stream::start(snapshot.plan.with_buffers(request)).map_err(|error| {
        log::error!("Output reopen failed: {error}");
        ReconfigureFailure::OpenFailed
    })?;
    if ready.backend_ready.device_sample_rate != snapshot.output.sample_rate_hz
        || ready.backend_ready.device_channels != snapshot.output.channels
        || ready.backend_ready.backend_name != snapshot.backend
        || ready.backend_ready.device_name != snapshot.device
    {
        log::error!("Output identity/format changed during DSP-only reopen");
        ready.manager.shutdown().map_err(|error| {
            log::error!("Mismatched replacement shutdown failed: {error}");
            ReconfigureFailure::ShutdownFailed
        })?;
        return Err(ReconfigureFailure::FormatChanged);
    }
    Ok(ready)
}
fn install(
    mut engine: AudioEngine,
    snapshot: &Snapshot,
    ready: stream::StreamReady,
    mut outcome: ReconfigureOutcome,
) -> WorkerResult {
    let stream::StreamReady {
        manager,
        buffers,
        command_sender,
        backend_ready,
        sfx_sender,
        played_map,
        decoder_status,
    } = ready;
    if let ReconfigureOutcome::Applied { negotiated, .. } = &mut outcome {
        *negotiated = buffers.negotiated;
    }
    engine.sfx_cache.rebind_output(sfx_sender);
    engine.command_sender = command_sender;
    engine.decoder_status = decoder_status;
    engine.live = Some(LiveOutput {
        manager,
        plan: snapshot.plan.clone(),
        config: snapshot.config,
        controls: snapshot.controls.clone(),
        buffers: OutputBufferState {
            requested: snapshot.buffers.requested,
            ..buffers
        },
        backend: backend_ready.backend_name,
        device: backend_ready.device_name,
    });
    let clock = ClockInstall {
        played_map,
        sample_rate_hz: engine.output.sample_rate_hz,
    };
    WorkerResult {
        engine: Some(engine),
        clock: Some(clock),
        outcome,
        published: true,
        recovery_allowed: false,
    }
}

#[cfg(test)]
mod ownership_tests {
    use super::*;
    #[test]
    fn empty_output_recovery_requires_confirmed_quiescence() {
        for reason in [
            ReconfigureFailure::OpenFailed,
            ReconfigureFailure::FormatChanged,
            ReconfigureFailure::RollbackFailed,
        ] {
            assert!(safe_to_reopen(reason));
        }
        for reason in [
            ReconfigureFailure::ShutdownFailed,
            ReconfigureFailure::DeviceUnavailable,
            ReconfigureFailure::InvalidRequest,
            ReconfigureFailure::Cancelled,
        ] {
            assert!(!safe_to_reopen(reason));
        }
    }
    #[test]
    fn identical_owner_and_epoch_never_renew_an_old_lease_token() {
        let old = Arc::new(Lease {
            owner: 7,
            epoch: AtomicU64::new(2),
        });
        let replacement = Arc::new(Lease {
            owner: 7,
            epoch: AtomicU64::new(2),
        });
        assert!(restore_authorized(
            2,
            Some(&old),
            7,
            &old.clone(),
            Guard::Lease
        ));
        assert!(!restore_authorized(
            2,
            Some(&replacement),
            7,
            &old,
            Guard::Lease
        ));
        assert!(!restore_authorized(2, None, 7, &old, Guard::Lease));
        assert!(!restore_authorized(2, Some(&old), 8, &old, Guard::Lease));
    }
    #[test]
    fn lease_restoration_tracks_own_publications_but_exact_rollback_expires() {
        let lease = Arc::new(Lease {
            owner: 7,
            epoch: AtomicU64::new(2),
        });
        assert!(restore_authorized(
            2,
            Some(&lease),
            7,
            &lease,
            Guard::Exact(2)
        ));
        lease.epoch.store(3, Ordering::Release);
        assert!(!restore_authorized(
            2,
            Some(&lease),
            7,
            &lease,
            Guard::Lease
        ));
        assert!(restore_authorized(3, Some(&lease), 7, &lease, Guard::Lease));
        assert!(!restore_authorized(
            3,
            Some(&lease),
            7,
            &lease,
            Guard::Exact(2)
        ));
    }
}
