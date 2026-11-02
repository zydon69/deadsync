//! GrooveAuthor Editor.PlaybackStopwatch position, independent of audio readiness.
//! A host position is never a decoder-open acknowledgement, PCM map or DSP lease.
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HostPosition {
    pub music_time: f64,
    pub drift_corrected: bool,
}
#[derive(Clone, Debug)]
pub struct SourceVisualClock {
    origin: f64,
    started: Instant,
    last: Instant,
    rate: f64,
}
impl SourceVisualClock {
    pub fn start(music_time: f64, rate: f64, now: Instant) -> Option<Self> {
        (music_time.is_finite() && rate.is_finite() && rate > 0.).then_some(Self {
            origin: music_time,
            started: now,
            last: now,
            rate,
        })
    }
    /// Existing presentation origin of this Play/Resume or explicit time/rate
    /// reanchor. Decoder acknowledgements and clock observations never replace
    /// it; this is not a generated revision or an audio-ready proof.
    pub fn intent_origin(&self) -> Instant {
        self.started
    }
    pub fn advance(
        &mut self,
        now: Instant,
        focused: bool,
        music_manager_time: Option<f64>,
    ) -> Option<HostPosition> {
        if now < self.last {
            return None;
        }
        // Stopwatch.Elapsed is a TimeSpan: truncate host duration to100ns ticks.
        let elapsed = (now.duration_since(self.started).as_nanos() / 100) as f64 / 10_000_000.;
        let mut position = self.origin + elapsed * self.rate;
        let mut corrected = false;
        if focused && let Some(reference) = music_manager_time.filter(|value| value.is_finite()) {
            if position - reference > 0.1 {
                self.origin -= 0.05;
                corrected = true;
            } else if reference - position > 0.1 {
                self.origin += 0.05;
                corrected = true;
            }
            // Pinned Editor.cs1873/1878 omit MusicRate in these two setters.
            // Preserve the source branch literally; this has an observable
            // discontinuity at non-unit rates, covered by the original oracle.
            if corrected {
                position = self.origin + elapsed;
            }
        }
        if !position.is_finite() {
            return None;
        }
        self.last = now;
        Some(HostPosition {
            music_time: position,
            drift_corrected: corrected,
        })
    }
    /// Source OnActiveChartPositionChanged restarts the stopwatch at chosen time.
    pub fn seek(&mut self, music_time: f64, now: Instant) -> bool {
        if !music_time.is_finite() || now < self.last {
            return false;
        }
        self.origin = music_time;
        self.started = now;
        self.last = now;
        true
    }
    /// Source OnMusicRateChanged restarts its timer at the MusicManager time.
    /// If there is no device, use the current host position as an explicit
    /// host-only continuation; never manufacture a MusicManager/DSP reading.
    pub fn set_rate(&mut self, rate: f64, now: Instant, music_manager_time: Option<f64>) -> bool {
        if !rate.is_finite() || rate <= 0. || now < self.last {
            return false;
        }
        let Some(current) = self.advance(now, false, None) else {
            return false;
        };
        let position = music_manager_time
            .filter(|value| value.is_finite())
            .unwrap_or(current.music_time);
        self.rate = rate;
        self.seek(position, now)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    fn at(base: Instant, seconds: f64) -> Instant {
        base + Duration::from_secs_f64(seconds)
    }
    #[test]
    fn no_device_clock_advances_from_zero_without_any_audio_claim() {
        let now = Instant::now();
        let mut clock = SourceVisualClock::start(0., 1., now).unwrap();
        assert_eq!(
            clock.advance(at(now, 0.5), true, None),
            Some(HostPosition {
                music_time: 0.5,
                drift_corrected: false
            })
        );
    }
    #[test]
    fn source_drift_threshold_and_nonunit_correction_are_literal() {
        let now = Instant::now();
        let mut clock = SourceVisualClock::start(0., 2., now).unwrap();
        assert_eq!(
            clock.advance(at(now, 1.), true, Some(0.)).unwrap(),
            HostPosition {
                music_time: 0.95,
                drift_corrected: true
            }
        );
        let mut clock = SourceVisualClock::start(0., 1., now).unwrap();
        assert!(
            !clock
                .advance(at(now, 0.1), true, Some(0.))
                .unwrap()
                .drift_corrected
        );
        assert_eq!(
            clock
                .advance(at(now, 0.2), true, Some(0.))
                .unwrap()
                .music_time,
            0.15000000000000002
        );
    }
    #[test]
    fn source_timespan_truncates_subtick_host_precision() {
        let now = Instant::now();
        let mut clock = SourceVisualClock::start(0., 1., now).unwrap();
        assert_eq!(
            clock
                .advance(now + Duration::from_nanos(149), false, None)
                .unwrap()
                .music_time,
            0.0000001
        );
    }
    #[test]
    fn no_focus_skips_music_manager_correction() {
        let now = Instant::now();
        let mut clock = SourceVisualClock::start(1., 2., now).unwrap();
        assert_eq!(
            clock
                .advance(at(now, 1.), false, Some(-20.))
                .unwrap()
                .music_time,
            3.
        );
    }
    #[test]
    fn rate_and_seek_restart_host_origin_without_audio() {
        let now = Instant::now();
        let mut clock = SourceVisualClock::start(-0.25, 1., now).unwrap();
        assert!(clock.set_rate(2., at(now, 0.5), None));
        assert_eq!(
            clock.advance(at(now, 1.), true, None).unwrap().music_time,
            1.25
        );
        assert!(clock.seek(7.125, at(now, 1.)));
        assert_eq!(
            clock.advance(at(now, 1.25), true, None).unwrap().music_time,
            7.625
        );
    }
    #[test]
    fn stale_time_and_invalid_input_cannot_mutate_origin() {
        let now = Instant::now();
        let mut clock = SourceVisualClock::start(0., 1., now).unwrap();
        clock.advance(at(now, 1.), true, None).unwrap();
        assert!(clock.advance(now, true, None).is_none());
        assert!(!clock.set_rate(f64::NAN, at(now, 2.), None));
        assert!(!clock.seek(f64::INFINITY, at(now, 2.)));
        assert_eq!(
            clock
                .advance(at(now, 2.), true, Some(f64::NAN))
                .unwrap()
                .music_time,
            2.
        );
    }
}

#[cfg(test)]
mod intent_origin_tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn observations_and_drift_keep_origin_while_seek_rate_and_new_play_replace_it() {
        let start = Instant::now();
        let mut clock = SourceVisualClock::start(1.0, 1.0, start).unwrap();
        assert_eq!(clock.intent_origin(), start);
        clock
            .advance(start + Duration::from_millis(20), true, Some(0.0))
            .unwrap();
        assert_eq!(clock.intent_origin(), start);
        clock
            .advance(start + Duration::from_millis(40), false, None)
            .unwrap();
        assert_eq!(clock.intent_origin(), start);
        let seek = start + Duration::from_millis(50);
        assert!(clock.seek(1.0, seek));
        assert_eq!(clock.intent_origin(), seek);
        let rate = start + Duration::from_millis(60);
        assert!(clock.set_rate(2.0, rate, Some(1.0)));
        assert_eq!(clock.intent_origin(), rate);
        let restart = start + Duration::from_millis(70);
        let clock = SourceVisualClock::start(1.0, 2.0, restart).unwrap();
        assert_eq!(clock.intent_origin(), restart);
    }
}
