//! DSP source preferences are deferred until the next editor audio context.
//! File changes retain this context; only actual editor exit restores the game.
use super::*;
use deadsync_editor::{
    audio_preferences::{Audio, Field, FieldValue},
    history::file_intent::IntentGuard,
};
use deadsync_gameplay::{
    EditorBufferControl as Control, EditorOutputOutcome as Outcome,
    EditorOutputSnapshot as Snapshot, GameplayAudioCommand as AudioCommand,
};
pub(super) struct Output {
    snapshot: Snapshot,
    next_id: u64,
    initial_attempted: bool,
    startup: Option<Audio>,
    ready: bool,
    faulted: bool,
    activation_failed: bool,
    pending_init: Option<u64>,
    leaving: Option<Leaving>,
    restored: bool,
    leave_without_audio: bool,
}
impl Default for Output {
    fn default() -> Self {
        Self {
            snapshot: Default::default(),
            next_id: 1,
            initial_attempted: false,
            startup: None,
            ready: false,
            faulted: false,
            activation_failed: false,
            pending_init: None,
            leaving: None,
            restored: false,
            leave_without_audio: false,
        }
    }
}
impl State {
    pub(super) fn audio_output_snapshot(&self) -> Snapshot {
        self.audio_session.output.snapshot
    }
    pub(super) fn audio_output_busy(&self) -> bool {
        self.audio_session.output.pending_init.is_some()
            || self.audio_session.output.leaving.is_some()
    }
    pub(super) fn audio_output_transport_ready(&self) -> bool {
        let state = &self.audio_session.output;
        state.ready && !state.faulted && !self.audio_output_busy()
    }
    pub(super) fn audio_output_restart_required(&self) -> bool {
        let state = &self.audio_session.output;
        state.activation_failed
            || state.startup.is_some_and(|started| {
                [Field::DspBufferSize, Field::DspNumBuffers]
                    .into_iter()
                    .any(|field| started.value(field) != self.audio_preferences.value(field))
            })
    }
    fn next_audio_output_id(&mut self) -> Option<u64> {
        let id = self.audio_session.output.next_id;
        self.audio_session.output.next_id = id.checked_add(1)?;
        Some(id)
    }
    pub(super) fn begin_audio_preferences_apply(&mut self, audio: Audio) {
        match self.commit_audio_preferences(audio) {
            Ok(_) => {
                self.audio_preferences_draft = None;
                self.panel = Some(Panel::SongSample(0));
                self.status = i18n::tr(
                    "Edition",
                    if self.audio_output_restart_required() {
                        "AudioDspRestartRequired"
                    } else {
                        "AudioSettingsSaved"
                    },
                );
            }
            Err(error) => self.status = i18n::tr_fmt("Edition", "AudioSaveFailedReason", &[("message", &i18n::editor_error(&error))]),
        }
    }
    fn prepare_initial_editor_output(&mut self) {
        let state = &self.audio_session.output;
        if state.initial_attempted || self.audio_output_busy() {
            return;
        }
        let output = state.snapshot;
        if !output.available && output.leased {
            return;
        }
        if !output.available {
            let state = &mut self.audio_session.output;
            state.initial_attempted = true;
            state.startup = Some(self.audio_preferences);
            state.ready = true;
            return;
        }
        if !output.leased {
            if output.period_control == Control::Unsupported
                && output.count_control == Control::Unsupported
            {
                let state = &mut self.audio_session.output;
                state.initial_attempted = true;
                state.startup = Some(self.audio_preferences);
                state.ready = true;
            };
            return;
        }
        let target = self.audio_preferences;
        let request = |field, control, requested, negotiated| -> Result<Option<u32>, ()> {
            if control != Control::Adjustable {
                return Ok(None);
            }
            let FieldValue::Int(value) = target.value(field) else {
                return Err(());
            };
            let value = u32::try_from(value).map_err(|_| ())?;
            let valid = if field == Field::DspBufferSize {
                (32..=8192).contains(&value)
            } else {
                (2..=32).contains(&value)
            };
            if !valid {
                return Err(());
            }
            Ok((requested != Some(value) && negotiated != Some(value)).then_some(value))
        };
        let request = request(
            Field::DspBufferSize,
            output.period_control,
            output.requested_period,
            output.negotiated_period,
        )
        .and_then(|period| {
            Ok((
                period,
                request(
                    Field::DspNumBuffers,
                    output.count_control,
                    output.requested_count,
                    output.negotiated_count,
                )?,
            ))
        });
        self.audio_session.output.initial_attempted = true;
        self.audio_session.output.startup = Some(target);
        let Ok((period_frames, num_buffers)) = request else {
            self.audio_session.output.activation_failed = true;
            self.audio_session.output.ready = true;
            self.status = i18n::tr("Edition", "AudioDspFallback");
            return;
        };
        if period_frames.is_none() && num_buffers.is_none() {
            self.audio_session.output.ready = true;
            return;
        }
        let Some(id) = self.next_audio_output_id() else {
            self.audio_session.output.faulted = true;
            self.status = i18n::tr("Edition", "AudioDspUnavailable");
            return;
        };
        self.audio_session.output.pending_init = Some(id);
        self.audio_commands
            .push(AudioCommand::EditorInitializeOutput {
                owner: self.audio_session.scope.owner,
                id,
                period_frames,
                num_buffers,
            });
        self.status = i18n::tr("Edition", "AudioOutputBusy");
    }
    pub(super) fn observe_editor_output(&mut self, state: Snapshot) {
        self.audio_session.output.snapshot = state;
        self.prepare_initial_editor_output();
        let Some(ack) = state
            .ack
            .filter(|ack| ack.owner == self.audio_session.scope.owner)
        else {
            return;
        };
        if self.finish_audio_output_leave(ack) {
            return;
        }
        if self.audio_session.output.pending_init != Some(ack.id) {
            return;
        }
        self.audio_session.output.pending_init = None;
        match ack.outcome {
            Outcome::Applied => {
                self.audio_session.output.ready = true;
                self.status = i18n::tr("Edition", "AudioOutputReady");
            }
            Outcome::FailedRestored
            | Outcome::Unsupported
            | Outcome::InvalidRequest
            | Outcome::Cancelled => {
                self.audio_session.output.ready = true;
                self.audio_session.output.activation_failed = true;
                self.status = i18n::tr("Edition", "AudioDspFallback");
            }
            _ => {
                self.audio_session.output.faulted = true;
                self.audio_session.output.activation_failed = true;
                self.status = i18n::tr("Edition", "AudioDspUnavailable");
            }
        }
    }
}

struct Leaving {
    id: u64,
    guard: IntentGuard,
    revision: u64,
    policy: u64,
    pack: Option<super::pack_properties::CouplingGuard>,
}
impl State {
    pub(super) fn prepare_audio_output_close(&mut self) -> bool {
        if self.audio_output_busy() {
            self.status = i18n::tr("Edition", "AudioOutputBusy");
            return false;
        }
        if self.audio_session.output.leave_without_audio {
            self.audio_commands.push(AudioCommand::EditorAbandonOutputLease {owner:self.audio_session.scope.owner});
            self.audio_session.output.ready=false;
            self.audio_session.output.faulted=true;
            return true;
        }
        let output = &self.audio_session.output;
        if output.restored || !output.snapshot.leased {
            return true;
        }
        let Some(id) = self.next_audio_output_id() else {
            self.status = i18n::tr("Edition", "AudioOutputRestoreFailed");
            return false;
        };
        self.stop_audio();
        self.audio_session.output.leaving = Some(Leaving {
            id,
            guard: IntentGuard::capture(&self.workspace.file().session),
            revision: self.workspace.file().session.revision(),
            policy: self.workspace.file().output_policy_revision(),
            pack: self.capture_pack_coupling(),
        });
        self.audio_commands
            .push(AudioCommand::EditorRestoreOutputLease {
                owner: self.audio_session.scope.owner,
                id,
            });
        self.status = i18n::tr("Edition", "AudioOutputRestoring");
        false
    }
    fn finish_audio_output_leave(&mut self, ack: deadsync_gameplay::EditorOutputAck) -> bool {
        let Some(leaving) = self.audio_session.output.leaving.take() else {
            return false;
        };
        if ack.id != leaving.id {
            self.audio_session.output.leaving = Some(leaving);
            return false;
        }
        if !matches!(ack.outcome, Outcome::Applied | Outcome::StaleOwner) {
            if ack.outcome==Outcome::FailedUnavailable {
                self.audio_session.output.leave_without_audio=true;
                self.audio_session.output.faulted=true;
                self.audio_session.output.ready=false;
                self.status=i18n::tr("Edition","AudioOutputCloseWithoutAudio");
            } else {self.status = i18n::tr("Edition", "AudioOutputRestoreFailed");}
            return true;
        }
        self.audio_session.output.restored = true;
        self.audio_session.output.ready = false;
        let current = leaving.guard.is_current(&self.workspace.file().session)
            && leaving.revision == self.workspace.file().session.revision()
            && leaving.policy == self.workspace.file().output_policy_revision()
            && self.pack_coupling_current(&leaving.pack);
        if !current {
            self.status = i18n::tr("Edition", "AudioOutputIntentStale");
            return true;
        }
        self.finish_authorized_audio_close();
        true
    }
    pub(super) fn restart_source_output_after_device_return(&mut self){
        let output=&mut self.audio_session.output;output.ready=false;output.initial_attempted=false;output.faulted=false;output.activation_failed=false;output.pending_init=None;output.startup=None;
    }
    pub(super) fn rearm_audio_output_for_transport(&mut self) -> bool {
        if !self.audio_session.output.restored {
            return false;
        }
        let output = &mut self.audio_session.output;
        output.restored = false;
        output.ready = false;
        output.initial_attempted = false;
        output.faulted = false;
        true
    }
}

impl State {
    /// Only an authorized close awaiting backend restoration counts as a host
    /// departure. Startup DSP initialization is not a queued close request.
    pub(super) fn audio_output_close_pending(&self)->bool {
        self.audio_session.output.leaving.is_some()
    }
}
