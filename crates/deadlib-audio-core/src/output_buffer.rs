//! Device requests and observations are deliberately distinct: an unknown
//! negotiated value is never reconstructed from a requested latency hint.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BufferRequest {
    pub period_frames: Option<u32>,
    pub num_buffers: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufferControl {
    Adjustable,
    ServerControlled,
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferCapabilities {
    pub period_frames: BufferControl,
    pub num_buffers: BufferControl,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BufferNegotiation {
    pub period_frames: Option<u32>,
    pub num_buffers: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputBufferState {
    pub capabilities: BufferCapabilities,
    pub requested: BufferRequest,
    pub negotiated: BufferNegotiation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReconfigureError {
    Busy,
    InvalidRequest,
    Unsupported,
    Unavailable,
    StaleOwner,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReconfigureFailure {
    InvalidRequest,
    DeviceUnavailable,
    FormatChanged,
    ShutdownFailed,
    OpenFailed,
    RollbackFailed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReconfigureOutcome {
    Applied {
        requested: BufferRequest,
        negotiated: BufferNegotiation,
    },
    FailedRestored {
        reason: ReconfigureFailure,
    },
    FailedUnavailable {
        reason: ReconfigureFailure,
    },
    Unsupported,
    Cancelled,
}

impl BufferCapabilities {
    pub fn for_backend(name: &str) -> Self {
        use BufferControl::*;
        if name.starts_with("alsa-") {
            Self {
                period_frames: Adjustable,
                num_buffers: Adjustable,
            }
        } else if name.starts_with("coreaudio-")
            || name.starts_with("wasapi-")
            || name.starts_with("pulse-")
        {
            Self {
                period_frames: Adjustable,
                num_buffers: Unsupported,
            }
        } else if name.starts_with("jack-") || name.starts_with("pipewire-") {
            Self {
                period_frames: ServerControlled,
                num_buffers: Unsupported,
            }
        } else {
            Self {
                period_frames: Unsupported,
                num_buffers: Unsupported,
            }
        }
    }
}

impl BufferRequest {
    pub fn validate(self, capabilities: BufferCapabilities) -> Result<(), ReconfigureError> {
        if self
            .period_frames
            .is_some_and(|value| !(32..=8192).contains(&value))
            || self
                .num_buffers
                .is_some_and(|value| !(2..=32).contains(&value))
            || (self.period_frames.is_none() && self.num_buffers.is_none())
        {
            return Err(ReconfigureError::InvalidRequest);
        }
        if (self.period_frames.is_some() && capabilities.period_frames != BufferControl::Adjustable)
            || (self.num_buffers.is_some() && capabilities.num_buffers != BufferControl::Adjustable)
        {
            return Err(ReconfigureError::Unsupported);
        }
        Ok(())
    }

    /// Apply only explicitly requested fields; a frames-only device never
    /// accidentally acquires a fabricated buffer-count preference.
    pub fn merged(self, request: Self) -> Self {
        Self {
            period_frames: request.period_frames.or(self.period_frames),
            num_buffers: request.num_buffers.or(self.num_buffers),
        }
    }
}

impl BufferNegotiation {
    /// A buffer total is a count only when the host reports an exact multiple
    /// of its period. Rounding would invent an unsupported negotiation result.
    pub fn from_period_and_total(period_frames: u32, total_frames: u32) -> Self {
        Self {
            period_frames: (period_frames != 0).then_some(period_frames),
            num_buffers: if period_frames != 0 && total_frames % period_frames == 0 {
                Some(total_frames / period_frames)
            } else {
                None
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const FRAMES_ONLY: BufferCapabilities = BufferCapabilities {
        period_frames: BufferControl::Adjustable,
        num_buffers: BufferControl::Unsupported,
    };

    #[test]
    fn six_backend_contracts_never_invent_buffer_count_controls() {
        use BufferControl::*;
        assert_eq!(
            BufferCapabilities::for_backend("alsa-shared"),
            BufferCapabilities {
                period_frames: Adjustable,
                num_buffers: Adjustable
            }
        );
        for name in ["coreaudio-shared", "wasapi-shared", "pulse-shared"] {
            assert_eq!(
                BufferCapabilities::for_backend(name),
                BufferCapabilities {
                    period_frames: Adjustable,
                    num_buffers: Unsupported
                }
            );
        }
        for name in ["jack-shared", "pipewire-shared"] {
            assert_eq!(
                BufferCapabilities::for_backend(name),
                BufferCapabilities {
                    period_frames: ServerControlled,
                    num_buffers: Unsupported
                }
            );
        }
        assert_eq!(
            BufferCapabilities::for_backend("unknown"),
            BufferCapabilities {
                period_frames: Unsupported,
                num_buffers: Unsupported
            }
        );
    }

    #[test]
    fn requests_validate_source_limits_without_clamping_imports() {
        for frames in [32, 8192] {
            assert_eq!(
                BufferRequest {
                    period_frames: Some(frames),
                    num_buffers: None
                }
                .validate(FRAMES_ONLY),
                Ok(())
            );
        }
        for frames in [0, 31, 8193, u32::MAX] {
            assert_eq!(
                BufferRequest {
                    period_frames: Some(frames),
                    num_buffers: None
                }
                .validate(FRAMES_ONLY),
                Err(ReconfigureError::InvalidRequest)
            );
        }
        assert_eq!(
            BufferRequest::default().validate(FRAMES_ONLY),
            Err(ReconfigureError::InvalidRequest)
        );
    }

    #[test]
    fn unsupported_count_and_server_control_refuse_before_device_shutdown() {
        assert_eq!(
            BufferRequest {
                period_frames: None,
                num_buffers: Some(2)
            }
            .validate(FRAMES_ONLY),
            Err(ReconfigureError::Unsupported)
        );
        let server = BufferCapabilities {
            period_frames: BufferControl::ServerControlled,
            num_buffers: BufferControl::Unsupported,
        };
        assert_eq!(
            BufferRequest {
                period_frames: Some(256),
                num_buffers: None
            }
            .validate(server),
            Err(ReconfigureError::Unsupported)
        );
    }

    #[test]
    fn partial_request_preserves_requested_fields_and_actual_is_independent() {
        let old = BufferRequest {
            period_frames: Some(256),
            num_buffers: Some(4),
        };
        assert_eq!(
            old.merged(BufferRequest {
                period_frames: Some(512),
                num_buffers: None
            }),
            BufferRequest {
                period_frames: Some(512),
                num_buffers: Some(4)
            }
        );
        assert_eq!(
            BufferNegotiation::from_period_and_total(256, 1024).num_buffers,
            Some(4)
        );
        assert_eq!(
            BufferNegotiation::from_period_and_total(256, 1000).num_buffers,
            None
        );
        assert_eq!(
            BufferNegotiation::from_period_and_total(0, 1024),
            BufferNegotiation::default()
        );
    }
}
