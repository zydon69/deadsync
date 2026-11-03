//! Opaque backend restoration authority stays beside AudioControl, never in UI.
use super::super::OutputRestoreReceipt;
use super::*;
use deadlib_audio_core::output_buffer::{
    BufferRequest, OutputBufferState, ReconfigureError, ReconfigureOutcome,
};
use std::cell::RefCell;
#[derive(Clone, Copy, Debug)]
pub enum EditorOutputEvent {
    Completed(ReconfigureOutcome),
    Rejected(ReconfigureError),
}
#[derive(Clone, Copy, Debug)]
pub struct EditorOutputAck {
    pub owner: u64,
    pub id: u64,
    pub event: EditorOutputEvent,
}
pub struct EditorOutputUpdate {
    pub ack: EditorOutputAck,
    pub clock: Option<MusicClock>,
}
#[derive(Clone, Copy)]
enum Kind {
    Change,
    Release,
}
struct State {
    lease: Option<(u64, OutputRestoreReceipt)>,
    rollback: Option<(u64, u64, OutputRestoreReceipt)>,
    pending: Option<(u64, u64, Kind)>,
    rejected: Option<EditorOutputAck>,
    hardware_changed: bool,
}
pub(super) struct Output(RefCell<State>);
impl Output {
    pub(super) const fn new() -> Self {
        Self(RefCell::new(State {
            lease: None,
            rollback: None,
            pending: None,
            rejected: None,
            hardware_changed: false,
        }))
    }
}
impl AudioControl {
    pub fn editor_output_is_leased(&self, owner: u64) -> bool {
        self.editor
            .output
            .0
            .borrow()
            .lease
            .as_ref()
            .is_some_and(|(actual, _)| *actual == owner)
    }
    fn editor_output_rejected(&self, owner: u64, id: u64, error: ReconfigureError) {
        self.editor.output.0.borrow_mut().rejected = Some(EditorOutputAck {
            owner,
            id,
            event: EditorOutputEvent::Rejected(error),
        });
    }
    pub fn capture_editor_output_lease(&mut self, owner: u64) -> Result<(), ReconfigureError> {
        if owner == 0 || self.editor.state.get().owner != owner {
            return Err(ReconfigureError::StaleOwner);
        }
        if self.editor_output_is_leased(owner) {
            return Ok(());
        }
        let receipt = self.capture_output_lease(owner)?;
        {
            let mut state = self.editor.output.0.borrow_mut();
            state.lease = Some((owner, receipt));
            state.hardware_changed = false;
        }
        Ok(())
    }
    pub fn begin_editor_output_change(&mut self, owner: u64, id: u64, request: BufferRequest) {
        if let Err(error) = self.capture_editor_output_lease(owner) {
            self.editor_output_rejected(owner, id, error);
            return;
        }
        match self.begin_output_reconfigure(owner, id, request) {
            Ok(()) => self.editor.output.0.borrow_mut().pending = Some((owner, id, Kind::Change)),
            Err(error) => self.editor_output_rejected(owner, id, error),
        }
    }
    pub fn begin_editor_output_release(&mut self, owner: u64, id: u64) {
        if !self.editor.output.0.borrow().hardware_changed {
            let buffers = self.output_buffer_state();
            match self.release_output_lease(owner) {
                Ok(()) => {
                    let mut state = self.editor.output.0.borrow_mut();
                    state.lease = None;
                    state.rollback = None;
                    state.rejected = Some(EditorOutputAck {
                        owner,
                        id,
                        event: EditorOutputEvent::Completed(ReconfigureOutcome::Applied {
                            requested: buffers.requested,
                            negotiated: buffers.negotiated,
                        }),
                    });
                }
                Err(error) => {
                    if error == ReconfigureError::StaleOwner {
                        let mut state = self.editor.output.0.borrow_mut();
                        state.lease = None;
                        state.rollback = None;
                    }
                    self.editor_output_rejected(owner, id, error);
                }
            };
            return;
        }

        let receipt = self
            .editor
            .output
            .0
            .borrow()
            .lease
            .as_ref()
            .filter(|(actual, _)| *actual == owner)
            .map(|(_, receipt)| receipt.clone());
        let Some(receipt) = receipt else {
            self.editor_output_rejected(owner, id, ReconfigureError::StaleOwner);
            return;
        };
        match self.begin_output_restore(owner, id, &receipt) {
            Ok(()) => self.editor.output.0.borrow_mut().pending = Some((owner, id, Kind::Release)),
            Err(error) => {
                if error == ReconfigureError::StaleOwner {
                    let mut state = self.editor.output.0.borrow_mut();
                    state.lease = None;
                    state.rollback = None;
                }
                self.editor_output_rejected(owner, id, error);
            }
        }
    }
    /// Poll once on the host thread before constructing its clock snapshot.
    /// Clock replacement also occurs for FailedRestored, since the previous
    /// decoder's played-map reader has been destroyed and replaced.
    pub fn poll_editor_output_change(&mut self) -> Option<EditorOutputUpdate> {
        if let Some(ack) = self.editor.output.0.borrow_mut().rejected.take() {
            return Some(EditorOutputUpdate { ack, clock: None });
        }
        let mut ack = self.poll_output_reconfigure()?;
        let mut state = self.editor.output.0.borrow_mut();
        if restore_required(ack.clock.is_some(),ack.outcome) {
            state.hardware_changed = true;
        }
        let mut release_owner = None;
        if let Some((owner, id, kind)) = state
            .pending
            .filter(|(owner, id, _)| *owner == ack.owner && *id == ack.id)
        {
            state.pending = None;
            if matches!(ack.outcome, ReconfigureOutcome::Applied { .. }) {
                match kind {
                    Kind::Release => {
                        release_owner = Some(owner);
                        state.hardware_changed = false;
                        state.lease = None;
                        state.rollback = None;
                    }
                    Kind::Change => {
                        state.rollback = ack.rollback.take().map(|receipt| (owner, id, receipt));
                    }
                }
            }
        }
        drop(state);
        if let Some(owner) = release_owner {
            let _ = self.release_output_lease(owner);
        }
        Some(EditorOutputUpdate {
            ack: EditorOutputAck {
                owner: ack.owner,
                id: ack.id,
                event: EditorOutputEvent::Completed(ack.outcome),
            },
            clock: ack.clock.take(),
        })
    }
    pub fn editor_output_buffer_state(&self) -> OutputBufferState {
        self.output_buffer_state()
    }
}

impl AudioControl {
    /// Explicitly abandon a completed failed restore. Does not reopen output or
    /// report restoration; the backend remains unavailable/quarantined.
    pub fn abandon_editor_output_lease(&mut self,owner:u64) {
        if self.editor.output.0.borrow().pending.is_some(){return;}
        match self.release_output_lease(owner) {
            Ok(())|Err(ReconfigureError::StaleOwner)=>{
                let mut state=self.editor.output.0.borrow_mut();
                if state.lease.as_ref().is_some_and(|(actual,_)|*actual==owner){state.lease=None;state.rollback=None;}
            }
            Err(_)=>{}
        }
    }
}

#[cfg(test)]
mod failed_startup_restore_tests {
    use super::*;
    use deadlib_audio_core::output_buffer::ReconfigureFailure;
    #[test]
    fn failed_unavailable_without_clock_still_requires_real_restore() {
        // A failed shutdown/open may publish no replacement clock. That is not
        // proof that the pre-editor output remained intact.
        for reason in [ReconfigureFailure::ShutdownFailed,ReconfigureFailure::RollbackFailed] {
            let clock:Option<MusicClock>=None;
            let outcome=ReconfigureOutcome::FailedUnavailable{reason};
            let output=Output::new();
            if restore_required(clock.is_some(),outcome) {
                output.0.borrow_mut().hardware_changed=true;
            }
            assert!(output.0.borrow().hardware_changed);
        }
    }
}

fn restore_required(clock_replaced:bool,outcome:ReconfigureOutcome)->bool {
    clock_replaced || matches!(outcome,ReconfigureOutcome::FailedUnavailable{..})
}
