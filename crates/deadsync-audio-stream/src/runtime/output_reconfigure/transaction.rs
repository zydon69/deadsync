use deadlib_audio_core::{BufferRequest, ReconfigureFailure};
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) trait Host<C> {
    type Ready;
    fn shutdown_current(&mut self) -> Result<(), ReconfigureFailure>;
    fn open(
        &mut self,
        configuration: &C,
        request: BufferRequest,
    ) -> Result<Self::Ready, ReconfigureFailure>;
    fn shutdown_replacement(
        &mut self,
        replacement: &mut Self::Ready,
    ) -> Result<(), ReconfigureFailure>;
}
pub(super) enum Decision<R> {
    UnchangedCancelled,
    Applied(R),
    Restored {
        ready: R,
        reason: ReconfigureFailure,
    },
    Unavailable(ReconfigureFailure),
}

pub(super) fn run<C, H: Host<C>>(
    host: &mut H,
    previous: &C,
    target: &C,
    restore_request: BufferRequest,
    target_request: BufferRequest,
    cancelled: &AtomicBool,
) -> Decision<H::Ready> {
    if cancelled.load(Ordering::Acquire) {
        return Decision::UnchangedCancelled;
    }
    if host.shutdown_current().is_err() {
        return Decision::Unavailable(ReconfigureFailure::ShutdownFailed);
    }
    let failure = if cancelled.load(Ordering::Acquire) {
        ReconfigureFailure::Cancelled
    } else {
        match host.open(target, target_request) {
            Ok(ready) if !cancelled.load(Ordering::Acquire) => return Decision::Applied(ready),
            Ok(mut ready) => {
                if host.shutdown_replacement(&mut ready).is_err() {
                    return Decision::Unavailable(ReconfigureFailure::ShutdownFailed);
                }
                drop(ready);
                ReconfigureFailure::Cancelled
            }
            // A mismatched target that could not shut down is quarantined.
            Err(ReconfigureFailure::ShutdownFailed) => {
                return Decision::Unavailable(ReconfigureFailure::ShutdownFailed);
            }
            Err(reason) => reason,
        }
    };
    match host.open(previous, restore_request) {
        Ok(ready) => Decision::Restored {
            ready,
            reason: failure,
        },
        Err(reason) => Decision::Unavailable(if reason == ReconfigureFailure::ShutdownFailed {
            reason
        } else {
            ReconfigureFailure::RollbackFailed
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake<'a> {
        log: Vec<&'static str>,
        cancel: &'a AtomicBool,
        stop_fail: bool,
        target_fail: bool,
        target_stop_fail: bool,
        restore_fail: bool,
        replacement_stop_fail: bool,
        cancel_on_open: bool,
    }
    impl<'a> Fake<'a> {
        fn new(cancel: &'a AtomicBool) -> Self {
            Self {
                log: vec![],
                cancel,
                stop_fail: false,
                target_fail: false,
                target_stop_fail: false,
                restore_fail: false,
                replacement_stop_fail: false,
                cancel_on_open: false,
            }
        }
    }
    impl Host<u8> for Fake<'_> {
        type Ready = u8;
        fn shutdown_current(&mut self) -> Result<(), ReconfigureFailure> {
            self.log.push("stop-old-ack-join");
            if self.stop_fail {
                Err(ReconfigureFailure::ShutdownFailed)
            } else {
                Ok(())
            }
        }
        fn open(&mut self, config: &u8, _: BufferRequest) -> Result<u8, ReconfigureFailure> {
            self.log.push(if *config == 1 {
                "open-old"
            } else {
                "open-target"
            });
            if *config == 2 && self.cancel_on_open {
                self.cancel.store(true, Ordering::Release);
            }
            if *config == 2 && self.target_stop_fail {
                return Err(ReconfigureFailure::ShutdownFailed);
            }
            if (*config == 2 && self.target_fail) || (*config == 1 && self.restore_fail) {
                Err(ReconfigureFailure::OpenFailed)
            } else {
                Ok(*config)
            }
        }
        fn shutdown_replacement(&mut self, _: &mut u8) -> Result<(), ReconfigureFailure> {
            self.log.push("stop-target-ack-join");
            if self.replacement_stop_fail {
                Err(ReconfigureFailure::ShutdownFailed)
            } else {
                Ok(())
            }
        }
    }
    fn apply<'a>(host: &mut Fake<'a>, cancel: &'a AtomicBool) -> Decision<u8> {
        run(
            host,
            &1,
            &2,
            BufferRequest::default(),
            BufferRequest {
                period_frames: Some(256),
                num_buffers: None,
            },
            cancel,
        )
    }
    #[test]
    fn stop_ack_precedes_reopen_and_target_failure_rolls_back() {
        let cancel = AtomicBool::new(false);
        let mut host = Fake::new(&cancel);
        assert!(matches!(apply(&mut host, &cancel), Decision::Applied(2)));
        assert_eq!(host.log, ["stop-old-ack-join", "open-target"]);
        let mut host = Fake::new(&cancel);
        host.target_fail = true;
        assert!(matches!(
            apply(&mut host, &cancel),
            Decision::Restored {
                ready: 1,
                reason: ReconfigureFailure::OpenFailed
            }
        ));
        assert_eq!(host.log, ["stop-old-ack-join", "open-target", "open-old"]);
    }
    #[test]
    fn unacknowledged_stop_never_opens_a_second_device() {
        let cancel = AtomicBool::new(false);
        let mut host = Fake::new(&cancel);
        host.stop_fail = true;
        assert!(matches!(
            apply(&mut host, &cancel),
            Decision::Unavailable(ReconfigureFailure::ShutdownFailed)
        ));
        assert_eq!(host.log, ["stop-old-ack-join"]);
    }
    #[test]
    fn cancellation_before_start_retains_original_device() {
        let cancel = AtomicBool::new(true);
        let mut host = Fake::new(&cancel);
        assert!(matches!(
            apply(&mut host, &cancel),
            Decision::UnchangedCancelled
        ));
        assert!(host.log.is_empty());
    }
    #[test]
    fn cancellation_after_open_closes_target_before_restore() {
        let cancel = AtomicBool::new(false);
        let mut host = Fake::new(&cancel);
        host.cancel_on_open = true;
        assert!(matches!(
            apply(&mut host, &cancel),
            Decision::Restored {
                ready: 1,
                reason: ReconfigureFailure::Cancelled
            }
        ));
        assert_eq!(
            host.log,
            [
                "stop-old-ack-join",
                "open-target",
                "stop-target-ack-join",
                "open-old"
            ]
        );
    }
    #[test]
    fn cancelled_target_stop_failure_is_quarantined_without_restore() {
        let cancel = AtomicBool::new(false);
        let mut host = Fake::new(&cancel);
        host.cancel_on_open = true;
        host.replacement_stop_fail = true;
        assert!(matches!(
            apply(&mut host, &cancel),
            Decision::Unavailable(ReconfigureFailure::ShutdownFailed)
        ));
        assert_eq!(
            host.log,
            ["stop-old-ack-join", "open-target", "stop-target-ack-join"]
        );
    }
    #[test]
    fn mismatched_target_unconfirmed_stop_never_opens_rollback() {
        let cancel = AtomicBool::new(false);
        let mut host = Fake::new(&cancel);
        host.target_stop_fail = true;
        assert!(matches!(
            apply(&mut host, &cancel),
            Decision::Unavailable(ReconfigureFailure::ShutdownFailed)
        ));
        assert_eq!(host.log, ["stop-old-ack-join", "open-target"]);
    }
    #[test]
    fn rollback_failure_is_unavailable_not_fake_success() {
        let cancel = AtomicBool::new(false);
        let mut host = Fake::new(&cancel);
        host.target_fail = true;
        host.restore_fail = true;
        assert!(matches!(
            apply(&mut host, &cancel),
            Decision::Unavailable(ReconfigureFailure::RollbackFailed)
        ));
    }
}
