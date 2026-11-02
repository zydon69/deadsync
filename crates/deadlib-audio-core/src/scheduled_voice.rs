//! One callback-owned source tick voice, shared across producer window snapshots.
use std::sync::atomic::{AtomicU64, Ordering};
#[derive(Debug)]
pub struct SfxScheduleVoice {
    source_receipt: crate::source_tick::ReceiptCell,
    #[cfg(test)]
    gate: std::sync::Mutex<
        Option<(
            std::sync::Arc<std::sync::Barrier>,
            std::sync::Arc<std::sync::Barrier>,
        )>,
    >,
    timeline: AtomicU64,
    schedule: AtomicU64,
    sequence: AtomicU64,
    tail_timeline: AtomicU64,
    tail_frame: AtomicU64,
    tail_skipped: AtomicU64,
}
#[derive(Clone, Copy)]
pub(crate) enum TickData<'a> {
    I16(&'a [i16]),
    Float(&'a [f32], bool),
}
impl TickData<'_> {
    fn len(self) -> usize {
        match self {
            Self::I16(v) => v.len(),
            Self::Float(v, _) => v.len(),
        }
    }
    fn write(self, dst: &mut f32, index: usize, gain: f32) {
        match self {
            Self::Float(v, true) => *dst = v[index] * gain,
            Self::Float(v, false) => *dst += v[index] * gain,
            Self::I16(v) => *dst += crate::i16_to_f32(v[index]) * gain,
        }
    }
}
impl SfxScheduleVoice {
    pub(super) fn mix_source_pcm(
        &self,
        authority: (u64, u64),
        samples: &[i64],
        clock: crate::SourceTickClock,
        data: TickData<'_>,
        output: &mut [f32],
        before: u64,
        channels: usize,
        gain: f32,
    ) -> bool {
        if channels == 0
            || output.len() % channels != 0
            || self.timeline.load(Ordering::Acquire) != authority.0
        {
            return false;
        }
        let allowed = self.schedule.load(Ordering::Acquire) == authority.1;
        let source_frames = data.len() / channels;
        let frames = output.len() / channels;
        let offset =
            usize::try_from(before.saturating_sub(clock.physical_start)).unwrap_or(usize::MAX);
        let source_index = |e: super::ScheduledTrigger, now: u64| {
            now.saturating_sub(e.frame).saturating_add(e.skipped_frames)
        };
        let mut current = self
            .tail(authority.0)
            .filter(|e| source_index(*e, before) < source_frames as u64);
        let limit = samples.partition_point(|s| *s < clock.end());
        let mut next = if offset == 0 {
            samples[..limit].partition_point(|s| *s < clock.source_frame)
        } else {
            samples[..limit].partition_point(|s| *s <= clock.scaled(offset - 1))
        };
        // A rate>1 can leave authored onsets between the last output sample and
        // the callback's integer tracking end. Original UpdateTicks carries
        // these into the next callback even when none was audible before it.
        let gap = self.source_receipt.read().filter(|r| {
            r.timeline == authority.0
                && r.schedule == authority.1
                && r.generation == clock.generation
                && r.asset == clock.asset_epoch
                && r.physical_end == clock.physical_start
        });
        if offset == 0 && allowed && next > 0 {
            let source = samples[next - 1];
            let crossed_gap = gap.is_some_and(|r| source > r.last_scaled);
            if current.is_none() || crossed_gap {
                let skipped = ((clock.source_frame as f64 - source as f64) / clock.rate).trunc();
                if skipped >= 0. && skipped < (source_frames as f64) {
                    let skipped = skipped as u64;
                    let frame = clock.physical_start.saturating_sub(skipped);
                    current = Some(super::ScheduledTrigger {
                        frame,
                        skipped_frames: skipped.saturating_sub(clock.physical_start),
                    });
                }
            }
        }
        if offset > 0 {
            if let Some(r) = self.source_receipt.read().filter(|r| {
                r.timeline == authority.0
                    && r.schedule == authority.1
                    && r.generation == clock.generation
                    && r.asset == clock.asset_epoch
                    && r.physical_end == before
            }) {
                next = r.next;
            }
        }
        let mut mixed = false;
        let mut last = None;
        for frame in 0..frames {
            let now = before.saturating_add(frame as u64);
            let scaled = clock.scaled(offset.saturating_add(frame));
            if allowed {
                if next < limit && samples[next] <= scaled {
                    current = Some(super::ScheduledTrigger {
                        frame: now,
                        skipped_frames: 0,
                    });
                    next += 1;
                }
            }
            if let Some(event) = current {
                let source = usize::try_from(source_index(event, now)).unwrap_or(usize::MAX);
                if source < source_frames {
                    let first = source * channels;
                    for channel in 0..channels {
                        data.write(
                            &mut output[frame * channels + channel],
                            first + channel,
                            gain,
                        );
                    }
                    mixed = true;
                    last = Some(event);
                }
            }
        }
        if let Some(event) = last {
            self.publish(authority.0, event);
        }
        if frames > 0 {
            self.source_receipt.publish(crate::source_tick::Receipt {
                timeline: authority.0,
                schedule: authority.1,
                generation: clock.generation,
                asset: clock.asset_epoch,
                physical_end: before.saturating_add(frames as u64),
                last_scaled: clock.scaled(offset + frames - 1),
                next,
            });
        }
        mixed
    }

    pub fn new() -> Self {
        Self {
            source_receipt: crate::source_tick::ReceiptCell::new(),
            #[cfg(test)]
            gate: std::sync::Mutex::new(None),
            timeline: AtomicU64::new(1),
            schedule: AtomicU64::new(1),
            sequence: AtomicU64::new(0),
            tail_timeline: AtomicU64::new(0),
            tail_frame: AtomicU64::new(0),
            tail_skipped: AtomicU64::new(0),
        }
    }
    /// Future windows are revoked; the callback's current PCM tail survives.
    fn advance(counter: &AtomicU64, message: &str) {
        let mut current = counter.load(Ordering::Acquire);
        loop {
            let next = current.checked_add(1).expect(message);
            match counter.compare_exchange_weak(current, next, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => return,
                Err(observed) => current = observed,
            }
        }
    }
    pub fn cancel_future(&self) {
        Self::advance(&self.schedule, "tick schedule authority exhausted");
    }
    pub fn reset(&self) {
        Self::advance(&self.timeline, "tick voice authority exhausted");
        self.cancel_future();
    }
    pub(super) fn authority(&self) -> (u64, u64) {
        (
            self.timeline.load(Ordering::Acquire),
            self.schedule.load(Ordering::Acquire),
        )
    }
    fn tail(&self, timeline: u64) -> Option<super::ScheduledTrigger> {
        for _ in 0..2 {
            let seq = self.sequence.load(Ordering::SeqCst);
            if seq & 1 != 0 {
                continue;
            }
            let held = self.tail_timeline.load(Ordering::SeqCst);
            let event = super::ScheduledTrigger {
                frame: self.tail_frame.load(Ordering::SeqCst),
                skipped_frames: self.tail_skipped.load(Ordering::SeqCst),
            };
            if self.sequence.load(Ordering::SeqCst) == seq {
                return (held == timeline).then_some(event);
            }
        }
        None
    }
    fn publish(&self, timeline: u64, event: super::ScheduledTrigger) {
        // All snapshots for one bus share a single current render voice.
        self.sequence.fetch_add(1, Ordering::SeqCst);
        self.tail_timeline.store(timeline, Ordering::SeqCst);
        self.tail_frame.store(event.frame, Ordering::SeqCst);
        self.tail_skipped
            .store(event.skipped_frames, Ordering::SeqCst);
        self.sequence.fetch_add(1, Ordering::SeqCst);
    }
    pub(super) fn mix(
        &self,
        authority: (u64, u64),
        triggers: &[super::ScheduledTrigger],
        data: &[i16],
        output: &mut [f32],
        before: u64,
        channels: usize,
        gain: f32,
    ) -> bool {
        self.mix_pcm(
            authority,
            triggers,
            TickData::I16(data),
            output,
            before,
            channels,
            gain,
        )
    }
    pub(super) fn mix_pcm(
        &self,
        authority: (u64, u64),
        triggers: &[super::ScheduledTrigger],
        data: TickData<'_>,
        output: &mut [f32],
        before: u64,
        channels: usize,
        gain: f32,
    ) -> bool {
        if channels == 0
            || output.len() % channels != 0
            || self.timeline.load(Ordering::Acquire) != authority.0
        {
            return false;
        }
        // One callback snapshot: a concurrent cancellation affects the next
        // callback; this callback may finish and publish its actual last onset.
        let allowed = self.schedule.load(Ordering::Acquire) == authority.1;
        #[cfg(test)]
        if let Some((entered, release)) = self.gate.lock().unwrap().clone() {
            entered.wait();
            release.wait();
        }
        let source_index = |event: super::ScheduledTrigger, now: u64| {
            now.saturating_sub(event.frame)
                .saturating_add(event.skipped_frames)
        };
        let source_frames = data.len() / channels;
        let mut current = self
            .tail(authority.0)
            .filter(|event| source_index(*event, before) < source_frames as u64);
        let mut next = triggers.partition_point(|event| event.frame < before);
        if current.is_none() && allowed && next > 0 {
            let event = triggers[next - 1];
            if source_index(event, before) < source_frames as u64 {
                current = Some(event);
            }
        }
        let frames = output.len() / channels;
        let mut frame = 0usize;
        let mut mixed = false;
        let mut last = None;
        while frame < frames {
            let now = before.saturating_add(frame as u64);
            if allowed {
                while next < triggers.len() && triggers[next].frame <= now {
                    current = Some(triggers[next]);
                    next += 1;
                }
            }
            let end = if allowed {
                triggers.get(next).map_or(frames, |event| {
                    usize::try_from(event.frame.saturating_sub(before))
                        .unwrap_or(usize::MAX)
                        .min(frames)
                })
            } else {
                frames
            };
            if let Some(event) = current {
                let start = usize::try_from(source_index(event, now))
                    .unwrap_or(usize::MAX)
                    .saturating_mul(channels);
                let count = data
                    .len()
                    .saturating_sub(start)
                    .min((end - frame).saturating_mul(channels));
                if count > 0 {
                    for (index, dst) in output[frame * channels..frame * channels + count]
                        .iter_mut()
                        .enumerate()
                    {
                        data.write(dst, start + index, gain);
                    }
                    mixed = true;
                    last = Some(event);
                }
            }
            frame = end.max(frame + 1);
        }
        if let Some(event) = last {
            self.publish(authority.0, event);
        }
        mixed
    }
}
impl Default for SfxScheduleVoice {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ScheduledTrigger, SfxSchedule};
    use std::sync::Arc;
    fn window(voice: &Arc<SfxScheduleVoice>, frames: &[u64]) -> SfxSchedule {
        SfxSchedule::with_voice(
            frames
                .iter()
                .map(|frame| ScheduledTrigger {
                    frame: *frame,
                    skipped_frames: 0,
                })
                .collect::<Vec<_>>(),
            voice.clone(),
        )
        .unwrap()
    }
    fn mix(schedule: &SfxSchedule, before: u64, n: usize) -> Vec<i16> {
        let mut output = vec![0.; n];
        schedule.mix(&(1..=12).collect::<Vec<i16>>(), &mut output, before, 1, 1.);
        output
            .into_iter()
            .map(|v| (v * 32768.).round() as i16)
            .collect()
    }
    #[test]
    fn cancel_future_preserves_audible_tail_and_new_empty_window() {
        let voice = Arc::new(SfxScheduleVoice::new());
        let initial = window(&voice, &[0, 8]);
        assert_eq!(mix(&initial, 0, 4), vec![1, 2, 3, 4]);
        voice.cancel_future();
        assert_eq!(mix(&initial, 4, 4), vec![5, 6, 7, 8]);
        let empty = window(&voice, &[]);
        assert_eq!(mix(&empty, 8, 4), vec![9, 10, 11, 12]);
        assert_eq!(mix(&empty, 12, 4), vec![0; 4]);
    }
    #[test]
    fn disabled_gain_is_silent_while_tail_phase_advances_then_reenable_resumes() {
        let voice = Arc::new(SfxScheduleVoice::new());
        let initial = window(&voice, &[0, 8]);
        assert_eq!(mix(&initial, 0, 4), vec![1, 2, 3, 4]);
        voice.cancel_future();
        let empty = window(&voice, &[]);
        let mut silent = vec![0.; 4];
        assert!(empty.mix(&(1..=12).collect::<Vec<i16>>(), &mut silent, 4, 1, 0.));
        assert_eq!(silent, vec![0.; 4]);
        assert_eq!(mix(&window(&voice, &[]), 8, 4), vec![9, 10, 11, 12]);
    }
    #[test]
    fn native_pcm_end_is_zero_not_source_last_value_dc_hold() {
        let voice = Arc::new(SfxScheduleVoice::new());
        let schedule = window(&voice, &[0]);
        let data = (1..=10).collect::<Vec<i16>>();
        let mut before = vec![0.; 8];
        assert!(schedule.mix(&data, &mut before, 0, 1, 1.));
        let mut end = vec![0.; 4];
        assert!(schedule.mix(&data, &mut end, 8, 1, 1.));
        let actual: Vec<_> = end
            .into_iter()
            .map(|value| (value * 32768.).round() as i16)
            .collect();
        assert_eq!(actual, vec![9, 10, 0, 0]);
    }
    #[test]
    fn replacement_past_attack_does_not_rephase_active_voice_but_future_restarts() {
        let voice = Arc::new(SfxScheduleVoice::new());
        let initial = window(&voice, &[0, 8]);
        assert_eq!(mix(&initial, 0, 4), vec![1, 2, 3, 4]);
        voice.cancel_future();
        let updated = window(&voice, &[2, 9]);
        assert_eq!(mix(&updated, 4, 4), vec![5, 6, 7, 8]);
        assert_eq!(mix(&updated, 8, 4), vec![9, 1, 2, 3]);
        voice.reset();
        assert_eq!(mix(&updated, 12, 4), vec![0; 4]);
        assert_eq!(mix(&window(&voice, &[]), 12, 4), vec![0; 4]);
    }
    #[test]
    fn queued_never_audible_window_does_not_start_after_cancellation() {
        let voice = Arc::new(SfxScheduleVoice::new());
        let queued = window(&voice, &[0, 8]);
        voice.cancel_future();
        assert_eq!(mix(&queued, 0, 12), vec![0; 12]);
    }
    #[test]
    fn callback_in_flight_finishes_and_new_window_inherits_its_actual_last_onset() {
        let voice = Arc::new(SfxScheduleVoice::new());
        let old = window(&voice, &[0, 3, 8]);
        let entered = Arc::new(std::sync::Barrier::new(2));
        let release = Arc::new(std::sync::Barrier::new(2));
        *voice.gate.lock().unwrap() = Some((entered.clone(), release.clone()));
        let thread = std::thread::spawn(move || mix(&old, 0, 4));
        entered.wait();
        voice.cancel_future();
        let new = window(&voice, &[]);
        release.wait();
        assert_eq!(thread.join().unwrap(), vec![1, 2, 3, 1]);
        *voice.gate.lock().unwrap() = None;
        assert_eq!(mix(&new, 4, 4), vec![2, 3, 4, 5]);
    }
}
