//! Immutable snapshots of one logical source tick voice.
//! Prepared on the application thread, rendered without allocation or locks.
use std::sync::Arc;

/// A scheduled trigger, including the portion preceding output frame zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScheduledTrigger {
    pub frame: u64,
    pub skipped_frames: u64,
}

#[derive(Clone, Debug)]
pub struct SfxSchedule(Arc<[ScheduledTrigger]>,Option<Arc<crate::SfxScheduleVoice>>,(u64,u64),Option<u64>,Option<SourceMain>);
#[derive(Clone,Debug)]struct SourceMain{samples:Arc<[i64]>,generation:u64,asset:u64,pcm:Option<(Arc<[f32]>,usize)>,_legacy_pcm:Option<Arc<[i16]>>}
impl SfxSchedule {
    pub fn with_source_main(samples:impl Into<Arc<[i64]>>,voice:Arc<crate::SfxScheduleVoice>,generation:u64,asset:u64)->Option<Self>{
        let samples=samples.into();if generation==0||asset==0||samples.windows(2).any(|w|w[0]>w[1]){return None;}
        let mut s=Self::with_voice(Vec::<ScheduledTrigger>::new(),voice)?;s.4=Some(SourceMain{samples,generation,asset,pcm:None,_legacy_pcm:None});Some(s)
    }
    pub fn with_source_main_float(samples:impl Into<Arc<[i64]>>,voice:Arc<crate::SfxScheduleVoice>,generation:u64,asset:u64,pcm:Arc<[f32]>,channels:usize)->Option<Self>{
        if channels==0||pcm.len()%channels!=0{return None;}let mut snapshot=Self::with_source_main(samples,voice,generation,asset)?;snapshot.4.as_mut()?.pcm=Some((pcm,channels));Some(snapshot)
    }
    /// Keep any legacy PCM storage used by the same queued voice in the
    /// credited object, so producer-first teardown also retires its last Arc.
    #[doc(hidden)]pub fn retain_pcm_storage(&mut self,pcm:Arc<[i16]>){if let Some(source)=self.4.as_mut(){source._legacy_pcm=Some(pcm);}}
    pub(crate)fn source_tick_bus(&self)->bool{self.4.is_some()}
    pub(crate)fn mix_with_source(&self,data:&[i16],output:&mut[f32],before:u64,channels:usize,gain:f32,clock:Option<crate::SourceTickClock>,isolated_source:bool)->bool{
        if let Some(source)=&self.4{if let Some(voice)=&self.1{
            let data=if let Some((pcm,source_channels))=&source.pcm{if *source_channels!=channels{return false;}crate::scheduled_voice::TickData::Float(pcm,isolated_source)}else{crate::scheduled_voice::TickData::I16(data)};
            if let Some(clock)=clock.filter(|c|c.generation==source.generation&&c.asset_epoch==source.asset){return voice.mix_source_pcm(self.2,&source.samples,clock,data,output,before,channels,gain);}
            return voice.mix_pcm(self.2,&[],data,output,before,channels,gain);
        }return false;}self.mix(data,output,before,channels,gain)
    }

    pub(crate) fn credit(&self)->Option<u64>{self.3}
    pub(crate) fn set_credit(&mut self,credit:Option<u64>){self.3=credit;}

    /// Duplicate frame timestamps are allowed: the final trigger wins, matching
    /// consecutive restarts in source MusicManager. An empty snapshot silences.
    pub fn new(triggers: impl Into<Arc<[ScheduledTrigger]>>) -> Option<Self> {
        let triggers: Arc<[ScheduledTrigger]> = triggers.into();
        if triggers.windows(2).any(|w| w[0].frame > w[1].frame) {
            return None;
        }
        // Quantization to the output PCM grid makes repeated onsets in the
        // same frame the same audible restart. Keep the final one. This is
        // producer-only preparation; callback visits at most one trigger per
        // frame, independent of authored event density.
        let mut unique: Vec<ScheduledTrigger> = Vec::with_capacity(triggers.len());
        for trigger in triggers.iter().copied() {
            if let Some(last) = unique.last_mut() {
                if last.frame == trigger.frame {
                    *last = trigger;
                    continue;
                }
            }
            unique.push(trigger);
        }
        Some(Self(Arc::from(unique),None,(0,0),None,None))
    }
    pub fn with_voice(triggers:impl Into<Arc<[ScheduledTrigger]>>,voice:Arc<crate::SfxScheduleVoice>)->Option<Self>{
        let mut snapshot=Self::new(triggers)?;snapshot.2=voice.authority();snapshot.1=Some(voice);Some(snapshot)
    }
    /// Mix one authoritative snapshot. The producer retains the most recent
    /// past trigger when replacing a lookahead window, so an ongoing voice is
    /// reconstructed at exactly the same sample without restarting its attack.
    /// Returns whether any samples were rendered.
    pub fn mix(
        &self,
        data: &[i16],
        output: &mut [f32],
        before: u64,
        channels: usize,
        gain: f32,
    ) -> bool {
        if let Some(voice)=&self.1{return voice.mix(self.2,&self.0,data,output,before,channels,gain);}
        if channels == 0 || output.len() % channels != 0 {
            return false;
        }
        let frames = output.len() / channels;
        let mut trigger = self.0.partition_point(|event| event.frame <= before);
        let mut mixed = false;
        let mut frame = 0usize;
        while frame < frames {
            let absolute = before.saturating_add(frame as u64);
            while trigger < self.0.len() && self.0[trigger].frame <= absolute {
                trigger += 1;
            }
            let end = self.0.get(trigger).map_or(frames, |next| {
                usize::try_from(next.frame.saturating_sub(before))
                    .unwrap_or(usize::MAX)
                    .min(frames)
            });
            if trigger > 0 {
                let event = self.0[trigger - 1];
                let source_frame = absolute
                    .saturating_sub(event.frame)
                    .saturating_add(event.skipped_frames);
                let source_start = usize::try_from(source_frame)
                    .unwrap_or(usize::MAX)
                    .saturating_mul(channels);
                let count = data
                    .len()
                    .saturating_sub(source_start)
                    .min((end - frame).saturating_mul(channels));
                if count > 0 {
                    for (dst, src) in output[frame * channels..frame * channels + count]
                        .iter_mut()
                        .zip(&data[source_start..source_start + count])
                    {
                        *dst += crate::mixer::i16_to_f32(*src) * gain;
                    }
                    mixed = true;
                }
            }
            // A timestamp beyond u64::MAX is not representable; saturating
            // arithmetic still guarantees progress for the finite buffer.
            frame = end.max(frame + 1);
        }
        mixed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn schedule(frames: &[u64]) -> SfxSchedule {
        SfxSchedule::new(
            frames
                .iter()
                .map(|frame| ScheduledTrigger {
                    frame: *frame,
                    skipped_frames: 0,
                })
                .collect::<Vec<_>>(),
        )
        .unwrap()
    }
    #[test]
    fn dense_snapshot_uses_one_voice_and_restarts_at_every_onset() {
        let s = schedule(&(0..1024).collect::<Vec<_>>());
        let mut output = [0.; 1024];
        assert!(s.mix(&[1000, 2000, 3000], &mut output, 0, 1, 1.));
        assert!(
            output
                .iter()
                .all(|sample| *sample == crate::mixer::i16_to_f32(1000))
        );
    }
    #[test]
    fn replacement_snapshot_preserves_partial_voice_and_last_duplicate_wins() {
        let old = schedule(&[100]);
        let new = schedule(&[100, 108, 108]);
        let data = [1000, 2000, 3000, 4000, 5000, 6000, 7000, 8000, 9000, 10000];
        let mut before = [0.; 4];
        let mut after = [0.; 8];
        old.mix(&data, &mut before, 102, 1, 1.);
        new.mix(&data, &mut after, 104, 1, 1.);
        assert_eq!(before, std::array::from_fn::<_,4,_>(|i|crate::mixer::i16_to_f32(data[i+2])));
        assert_eq!(
            after,
            [5000, 6000, 7000, 8000, 1000, 2000, 3000, 4000].map(crate::mixer::i16_to_f32)
        );
    }
    #[test]
    fn negative_absolute_start_and_stereo_partial_fragment_are_exact() {
        let s = SfxSchedule::new(vec![ScheduledTrigger {
            frame: 0,
            skipped_frames: 1,
        }])
        .unwrap();
        let mut out = [0.; 8];
        s.mix(&[1000, 1100, 2000, 2100, 3000, 3100], &mut out, 1, 2, 0.5);
        assert_eq!(
            out,
            [
                crate::mixer::i16_to_f32(3000) * 0.5,
                crate::mixer::i16_to_f32(3100) * 0.5,
                0.,
                0.,
                0.,
                0.,
                0.,
                0.
            ]
        );
    }
    #[test]
    fn invalid_order_refused_and_dense_pcm_windows_remain_complete() {
        assert!(
            SfxSchedule::new(vec![
                ScheduledTrigger {
                    frame: 2,
                    skipped_frames: 0
                },
                ScheduledTrigger {
                    frame: 1,
                    skipped_frames: 0
                }
            ])
            .is_none()
        );
        let dense = schedule(&(0..8193).collect::<Vec<_>>());
        let mut output = vec![0.; 8193];
        assert!(dense.mix(&[1000, 2000], &mut output, 0, 1, 1.));
        assert!(
            output
                .iter()
                .all(|sample| *sample == crate::mixer::i16_to_f32(1000))
        );
        let same = SfxSchedule::new(vec![
            ScheduledTrigger {
                frame: 0,
                skipped_frames: 0
            };
            100_000
        ])
        .unwrap();
        assert_eq!(same.0.len(), 1);
    }
}
