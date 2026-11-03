//! Editor-only silent continuation from acknowledged decoder and stream clocks.
//! This model never reads a system clock or infers decode success from elapsed time.
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Pending,
    Opened,
    Producing,
    Exhausted,
    Stopped,
    Failed,
}
#[derive(Clone, Copy, Debug)]
pub struct Ack {
    pub generation: u64,
    pub phase: Phase,
    pub opened: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct Clock {
    /// Supplied by the command owner after checking the backend generation.
    pub generation: u64,
    /// Ordering only; no wall-time differences are ever calculated from this.
    pub valid_at: Instant,
    pub stream_seconds: f64,
    pub music_seconds: Option<f64>,
    pub rate: f64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Mapped,
    Silent,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    Wait,
    Position { music_seconds: f64, source: Source },
    Stopped,
    Failed,
}
#[derive(Clone, Copy)]
struct Anchor {
    stream: f64,
    music: f64,
    rate: f64,
}

#[derive(Default)]
pub struct Transport {
    generation: Option<u64>,
    requested_start: f64,
    latest: Option<(Instant, f64)>,
    anchor: Option<Anchor>,
}
impl Transport {
    /// Invoke synchronously for each issued Play, Seek or Rate command. The start
    /// is its exact requested/resumed position, not an integral note cursor.
    pub fn issue(&mut self, generation: u64, start: f64) -> bool {
        if !start.is_finite() {
            return false;
        }
        self.generation = Some(generation);
        self.requested_start = start;
        self.latest = None;
        self.anchor = None;
        true
    }
    pub fn issue_asset_generation(&mut self,generation:u64,start:f64,rate:f64)->bool{
        if !start.is_finite()||!rate.is_finite()||rate<=0.0{return false;}
        let latest=self.latest;
        if !self.issue(generation,start){return false;}
        self.latest=latest;
        self.anchor=latest.map(|(_,stream)|Anchor{stream,music:start,rate});
        true
    }
    /// A source asset replacement keeps the already established logical DSP
    /// clock even while the new file is Pending or Failed. This does not
    /// fabricate an opened decoder acknowledgement.
    pub fn observe_asset_phase(&mut self,clock:Clock)->Step{self.observe_inner(None,clock,true)}
    pub fn observe(&mut self, ack: Option<Ack>, clock: Clock) -> Step {self.observe_inner(ack,clock,false)}
    fn observe_inner(&mut self,ack:Option<Ack>,clock:Clock,asset_phase:bool)->Step {
        if self.generation != Some(clock.generation)
            || !clock.stream_seconds.is_finite()
            || !clock.rate.is_finite()
            || clock.rate <= 0.0
            || clock.music_seconds.is_some_and(|music| !music.is_finite())
        {
            return Step::Wait;
        }
        if self
            .latest
            .is_some_and(|(stamp, stream)| clock.valid_at < stamp || clock.stream_seconds < stream)
        {
            return Step::Wait;
        }
        self.latest = Some((clock.valid_at, clock.stream_seconds));
        // Actual mapped audio is authoritative even if its decoder is already
        // exhausted: the producer can finish before buffered PCM is heard.
        if let Some(music) = clock.music_seconds {
            self.anchor = Some(Anchor {
                stream: clock.stream_seconds,
                music,
                rate: clock.rate,
            });
            return Step::Position {
                music_seconds: music,
                source: Source::Mapped,
            };
        }
        if asset_phase {
            let anchor=self.anchor.get_or_insert(Anchor{stream:clock.stream_seconds,music:self.requested_start,rate:clock.rate});
            let music=(clock.stream_seconds-anchor.stream).mul_add(anchor.rate,anchor.music);
            return if music.is_finite(){Step::Position{music_seconds:music,source:Source::Silent}}else{Step::Wait};
        }
        let Some(ack) = ack.filter(|ack| self.generation == Some(ack.generation)) else {
            return Step::Wait;
        };
        match ack.phase {
            Phase::Stopped => {
                self.anchor = None;
                return Step::Stopped;
            }
            Phase::Failed => {
                self.anchor = None;
                return Step::Failed;
            }
            Phase::Pending => return Step::Wait,
            Phase::Opened | Phase::Producing | Phase::Exhausted if !ack.opened => {
                return Step::Wait;
            }
            _ => {}
        }
        let anchor = self.anchor.get_or_insert(Anchor {
            stream: clock.stream_seconds,
            music: self.requested_start,
            rate: clock.rate,
        });
        // The most recent actual mapping owns its exact slope. A nominal
        // preference may differ through f32 conversion or DSP chunk ratios.
        // Real speed changes are issued under a fresh command generation.
        let music = (clock.stream_seconds - anchor.stream).mul_add(anchor.rate, anchor.music);
        if !music.is_finite() {
            return Step::Wait;
        }
        Step::Position {
            music_seconds: music,
            source: Source::Silent,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    fn clock(t: Instant, stream: f64, music: Option<f64>) -> Clock {
        Clock {
            generation: 1,
            valid_at: t,
            stream_seconds: stream,
            music_seconds: music,
            rate: 1.0,
        }
    }
    fn ack(phase: Phase) -> Ack {
        Ack {
            generation: 1,
            phase,
            opened: true,
        }
    }
    fn position(step: Step, expected: f64, source: Source) {
        match step {
            Step::Position {
                music_seconds,
                source: actual,
            } => {
                assert!((music_seconds - expected).abs() < 1e-12);
                assert_eq!(actual, source);
            }
            _ => panic!("expected position, got {step:?}"),
        }
    }
    #[test]
    fn no_ack_pending_failed_or_stopped_never_invents_silent_success() {
        let mut transport = Transport::default();
        transport.issue(1, 20.0);
        let t = Instant::now();
        assert_eq!(transport.observe(None, clock(t, 200.0, None)), Step::Wait);
        assert_eq!(
            transport.observe(Some(ack(Phase::Pending)), clock(t, 200.0, None)),
            Step::Wait
        );
        assert_eq!(
            transport.observe(
                Some(Ack {
                    opened: false,
                    ..ack(Phase::Exhausted)
                }),
                clock(t, 200.0, None)
            ),
            Step::Wait
        );
        assert_eq!(
            transport.observe(Some(ack(Phase::Failed)), clock(t, 200.0, None)),
            Step::Failed
        );
        assert_eq!(
            transport.observe(Some(ack(Phase::Stopped)), clock(t, 200.0, None)),
            Step::Stopped
        );
    }
    #[test]
    fn start_beyond_eof_anchors_at_first_acknowledged_stream_observation() {
        let mut transport = Transport::default();
        transport.issue(1, 20.125);
        let t = Instant::now();
        transport.observe(None, clock(t, 100.0, None));
        position(
            transport.observe(
                Some(ack(Phase::Exhausted)),
                clock(t + Duration::from_secs(10), 110.0, None),
            ),
            20.125,
            Source::Silent,
        );
        position(
            transport.observe(
                Some(ack(Phase::Exhausted)),
                clock(t + Duration::from_secs(20), 110.25, None),
            ),
            20.375,
            Source::Silent,
        );
    }
    #[test]
    fn mapped_pcm_wins_and_silent_tail_continues_exact_last_map() {
        let mut transport = Transport::default();
        transport.issue(1, -1.0);
        let t = Instant::now();
        position(
            transport.observe(Some(ack(Phase::Exhausted)), clock(t, 2.0, Some(0.126))),
            0.126,
            Source::Mapped,
        );
        position(
            transport.observe(
                Some(ack(Phase::Exhausted)),
                clock(t + Duration::from_millis(1), 2.25, None),
            ),
            0.376,
            Source::Silent,
        );
        position(
            transport.observe(
                Some(ack(Phase::Opened)),
                clock(t + Duration::from_millis(2), 2.5, Some(0.777)),
            ),
            0.777,
            Source::Mapped,
        );
    }
    #[test]
    fn backend_float_slopes_survive_differently_rounded_nominal_preferences() {
        for nominal in [1.1_f64, 0.8_f64] {
            let mut transport = Transport::default();
            transport.issue(1, 0.0);
            let t = Instant::now();
            let mut mapped = clock(t, 10.0, Some(0.126));
            mapped.rate = f64::from(nominal as f32);
            assert_ne!(mapped.rate, nominal);
            position(
                transport.observe(Some(ack(Phase::Producing)), mapped),
                0.126,
                Source::Mapped,
            );
            let mut silent = clock(t, 10.5, None);
            silent.rate = nominal;
            position(
                transport.observe(Some(ack(Phase::Exhausted)), silent),
                0.126 + 0.5 * mapped.rate,
                Source::Silent,
            );
        }
    }
    #[test]
    fn generations_stale_stamps_nonfinite_and_nominal_rate_hints_are_handled() {
        let mut transport = Transport::default();
        transport.issue(1, -0.5);
        let t = Instant::now();
        position(
            transport.observe(Some(ack(Phase::Producing)), clock(t, 3.0, None)),
            -0.5,
            Source::Silent,
        );
        assert_eq!(
            transport.observe(
                Some(ack(Phase::Opened)),
                clock(t - Duration::from_millis(1), 3.1, None)
            ),
            Step::Wait
        );
        assert_eq!(
            transport.observe(Some(ack(Phase::Opened)), clock(t, 2.9, None)),
            Step::Wait
        );
        for value in [f64::NAN, f64::INFINITY] {
            assert_eq!(
                transport.observe(Some(ack(Phase::Opened)), clock(t, value, None)),
                Step::Wait
            );
        }
        let mut changed = clock(t, 3.1, None);
        changed.rate = 0.25;
        position(
            transport.observe(Some(ack(Phase::Opened)), changed),
            -0.4,
            Source::Silent,
        );
        transport.issue(2, -0.4);
        assert_eq!(
            transport.observe(Some(ack(Phase::Opened)), clock(t, 3.2, None)),
            Step::Wait
        );
        changed.generation = 2;
        assert_eq!(
            transport.observe(Some(ack(Phase::Opened)), changed),
            Step::Wait
        );
        let next_ack = Ack {
            generation: 2,
            ..ack(Phase::Opened)
        };
        position(
            transport.observe(Some(next_ack), changed),
            -0.4,
            Source::Silent,
        );
        changed.stream_seconds += 1.0;
        position(
            transport.observe(Some(next_ack), changed),
            -0.15,
            Source::Silent,
        );
    }
}

#[cfg(test)]
mod asset_phase_tests {
 use super::*;
 #[test]fn replacement_continues_only_matching_finite_dsp_clock_without_inventing_open_ack(){
  let mut transport=Transport::default();assert!(transport.issue(11,1.25));let at=Instant::now();let clock=Clock{generation:11,valid_at:at,stream_seconds:10.,music_seconds:Some(1.25),rate:1.};assert!(matches!(transport.observe(None,clock),Step::Position{music_seconds:1.25,source:Source::Mapped}));
  let next=Clock{valid_at:at+std::time::Duration::from_millis(500),stream_seconds:10.5,music_seconds:None,..clock};assert_eq!(transport.observe_asset_phase(next),Step::Position{music_seconds:1.75,source:Source::Silent});assert_eq!(transport.observe_asset_phase(Clock{generation:12,stream_seconds:11.,..next}),Step::Wait);assert_eq!(transport.observe_asset_phase(Clock{stream_seconds:f64::NAN,..next}),Step::Wait);assert_eq!(transport.observe_asset_phase(Clock{valid_at:at,..next}),Step::Wait);
  assert!(transport.issue_asset_generation(12,4.0,1.5));let later=Clock{generation:12,valid_at:at+std::time::Duration::from_secs(1),stream_seconds:11.,music_seconds:None,rate:1.5};assert_eq!(transport.observe_asset_phase(later),Step::Position{music_seconds:4.75,source:Source::Silent});
 }
}
