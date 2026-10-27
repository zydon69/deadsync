//! Poll completion without scheduling another write while a modal owns file IO.
use super::{AutoRecovery, Error, TryRecvError};
impl AutoRecovery {
    pub fn poll_pending(&mut self) -> Result<bool, Error> {
        let Some(worker) = self.worker.as_ref() else {
            return Ok(false);
        };
        match worker.try_recv() {
            Ok(result) => {
                self.worker = None;
                self.saved = Some(result?);
                Ok(true)
            }
            Err(TryRecvError::Empty) => Ok(false),
            Err(TryRecvError::Disconnected) => {
                self.worker = None;
                Err(Error::InvalidEdit(
                    "Recovery worker stopped unexpectedly".into(),
                ))
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn modal_poll_never_schedules_an_expired_recovery_write() {
        let mut recovery = AutoRecovery::new(std::time::Duration::ZERO);
        for _ in 0..4 {
            assert!(!recovery.poll_pending().unwrap());
            assert!(!recovery.is_pending());
        }
        assert!(recovery.last_attempt.is_none());
    }
}
