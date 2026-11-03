use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

/// Upper bound (in device frames) on how far ahead of the audible write head a
/// scheduled SFX onset may sit before the mixer treats it as stale and drops it.
/// This is a last-resort sanity bound; seek/stop/track-change staleness is
/// handled by the caller's generation guard.
pub const MAX_SCHEDULE_AHEAD_FRAMES: u64 = 192_000;
pub const MAX_ACTIVE_SFX: usize = 32;
pub const MAX_MIX_BUSES: usize = 8;

/// Stable index into the session's fixed mixer-bus table.
///
/// Bus meaning is deliberately supplied by the owning application. The mixer
/// only applies the bus gain and generation associated with this index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MixBus(u8);

impl MixBus {
    #[must_use]
    /// # Panics
    ///
    /// Panics if the function's input or state invariants are violated.
    pub const fn new(index: u8) -> Self {
        assert!(
            (index as usize) < MAX_MIX_BUSES,
            "mixer bus index out of range"
        );
        Self(index)
    }

    #[inline(always)]
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// Session-owned controls shared by producers and the realtime mixer.
///
/// The application thread updates atomic gains and generations. The audio
/// callback only performs bounded atomic loads; it never locks or allocates.
pub struct MixControls {
    editor_mix:crate::editor_mix::Cell,
    editor_mix_owner:AtomicU64,
    stream_gain: AtomicU32,
    override_owner: AtomicU64,
    override_gain: AtomicU32,
    bus_gains: [AtomicU32; MAX_MIX_BUSES],
    bus_generations: [AtomicU64; MAX_MIX_BUSES],
}

impl MixControls {
    #[must_use]
    pub fn new() -> Self {
        Self {
            editor_mix:crate::editor_mix::Cell::new(),editor_mix_owner:AtomicU64::new(0),
            stream_gain: AtomicU32::new(1.0f32.to_bits()),
            override_owner: AtomicU64::new(0),
            override_gain: AtomicU32::new(1.0f32.to_bits()),
            bus_gains: std::array::from_fn(|_| AtomicU32::new(1.0f32.to_bits())),
            bus_generations: std::array::from_fn(|_| AtomicU64::new(0)),
        }
    }

    #[inline(always)]
    pub fn set_stream_gain(&self, gain: f32) {
        self.stream_gain
            .store(valid_gain(gain).to_bits(), Ordering::Release);
    }

    #[inline(always)]
    pub fn stream_gain(&self) -> f32 {
        f32::from_bits(self.stream_gain.load(Ordering::Acquire))
    }

    /// A scoped owner can override music without rewriting global gain. End
    /// reveals the latest global setting, including changes made during scope.
    /// Scoped override writes belong to the sole application audio producer.
    /// Global set_stream_gain remains independent and may change during scope.
    pub fn begin_stream_gain_override(&self, owner: u64, gain: f32) -> bool {
        if owner == 0
            || owner == u64::MAX
            || self
                .override_owner
                .compare_exchange(0, u64::MAX, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return false;
        }
        self.override_gain
            .store(valid_gain(gain).to_bits(), Ordering::Release);
        self.override_owner.store(owner, Ordering::Release);
        true
    }
    pub fn update_stream_gain_override(&self, owner: u64, gain: f32) -> bool {
        if owner == 0 || owner == u64::MAX || self.override_owner.load(Ordering::Acquire) != owner {
            return false;
        }
        self.override_gain
            .store(valid_gain(gain).to_bits(), Ordering::Release);
        true
    }
    pub fn can_begin_editor_gain_override(&self, owner: u64, gains: crate::EditorMixGains) -> bool {
        owner != 0
            && owner != u64::MAX
            && gains.valid()
            && self.editor_mix.can_publish()
            && self.override_owner.load(Ordering::Acquire) == 0
    }
    pub fn can_update_editor_gain_override(
        &self,
        owner: u64,
        gains: crate::EditorMixGains,
    ) -> bool {
        owner != 0
            && owner != u64::MAX
            && self.editor_mix_owner.load(Ordering::Acquire) == owner
            && self.override_owner.load(Ordering::Acquire) == owner
            && gains.valid()
            && self.editor_mix.can_publish()
    }
    pub fn begin_editor_gain_override(&self, owner: u64, gains: crate::EditorMixGains) -> bool {
        if !self.can_begin_editor_gain_override(owner, gains) {
            return false;
        }
        if !self.begin_stream_gain_override(owner, gains.music * gains.main) {
            return false;
        }
        let published = self.editor_mix.publish(owner, gains);
        debug_assert!(published, "sole editor publisher preflight");
        self.editor_mix_owner.store(owner, Ordering::Release);
        published
    }
    pub fn update_editor_gain_override(&self, owner: u64, gains: crate::EditorMixGains) -> bool {
        if !self.can_update_editor_gain_override(owner, gains) {
            return false;
        }
        if !self.editor_mix.publish(owner, gains) {
            return false;
        }
        self.override_gain
            .store((gains.music * gains.main).to_bits(), Ordering::Release);
        true
    }
    #[cfg(test)]pub(crate)fn editor_gain_gate(&self,enter:Arc<std::sync::Barrier>,release:Arc<std::sync::Barrier>){*self.editor_mix.gate.lock().unwrap()=Some((enter,release));}
    pub(crate) fn editor_gain_owner(&self) -> u64 {
        let owner = self.editor_mix_owner.load(Ordering::Acquire);
        if owner != 0 && self.override_owner.load(Ordering::Acquire) == owner {
            owner
        } else {
            0
        }
    }
    pub(crate) fn editor_gain_snapshot(&self) -> Option<crate::editor_mix::Snapshot> {
        let owner = self.editor_gain_owner();
        self.editor_mix
            .read()
            .filter(|s| owner != 0 && s.owner == owner && self.editor_gain_owner() == owner)
    }
    pub fn end_stream_gain_override(&self, owner: u64) -> bool {
        if owner == 0 || owner == u64::MAX {
            return false;
        }
        if self
            .override_owner
            .compare_exchange(owner, 0, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return false;
        }
        self.editor_mix_owner.store(0, Ordering::Release);
        true
    }
    pub fn effective_stream_gain(&self) -> f32 {
        if matches!(self.override_owner.load(Ordering::Acquire), 0 | u64::MAX) {
            self.stream_gain()
        } else {
            f32::from_bits(self.override_gain.load(Ordering::Acquire))
        }
    }
    #[inline(always)]
    pub fn set_bus_gain(&self, bus: MixBus, gain: f32) {
        self.bus_gains[bus.index()].store(valid_gain(gain).to_bits(), Ordering::Release);
    }

    #[inline(always)]
    pub fn bus_gain(&self, bus: MixBus) -> f32 {
        f32::from_bits(self.bus_gains[bus.index()].load(Ordering::Acquire))
    }

    #[inline(always)]
    pub fn bus_generation(&self, bus: MixBus) -> u64 {
        self.bus_generations[bus.index()].load(Ordering::Acquire)
    }

    #[inline(always)]
    pub fn stop_bus(&self, bus: MixBus) -> u64 {
        self.bus_generations[bus.index()]
            .fetch_add(1, Ordering::AcqRel)
            .wrapping_add(1)
    }

    #[inline(always)]
    pub fn is_current(&self, bus: MixBus, generation: u64) -> bool {
        generation == self.bus_generation(bus)
    }
}

impl Default for MixControls {
    fn default() -> Self {
        Self::new()
    }
}

/// Stereo balance for a sound. ITGmania pans mine sounds to the owning
/// player when both sides are active; ordinary effects remain centered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SfxPan {
    #[default]
    Center,
    Left,
    Right,
}

#[inline(always)]
fn valid_gain(gain: f32) -> f32 {
    if gain.is_finite() && gain >= 0.0 {
        gain
    } else {
        1.0
    }
}

/// Optional per-voice control, independent of the sound file and mixer bus.
/// Two actors loading the same file must still be stoppable independently.
pub struct SfxVoiceControl {
    playing: AtomicBool,
    paused: AtomicBool,
}

impl SfxVoiceControl {
    pub fn new() -> Self {
        Self {
            playing: AtomicBool::new(true),
            paused: AtomicBool::new(false),
        }
    }
    pub fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Acquire)
    }
    pub fn stop(&self) {
        self.playing.store(false, Ordering::Release);
    }
    pub fn pause(&self, paused: bool) {
        self.paused.store(paused, Ordering::Release);
    }
}

impl Default for SfxVoiceControl {
    fn default() -> Self {
        Self::new()
    }
}

/// Exact output-frame onset. Frame zero is a valid scheduled timestamp.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SfxOnset {
    Immediate,
    At(u64),
}
impl SfxOnset {
    /// Existing gameplay APIs reserve zero for immediate playback.
    pub const fn from_legacy(frame: u64) -> Self {
        if frame == 0 {
            Self::Immediate
        } else {
            Self::At(frame)
        }
    }
    const fn order_key(self) -> Option<u64> {
        match self {
            Self::Immediate => None,
            Self::At(frame) => Some(frame),
        }
    }
}

#[derive(Clone)]
pub struct QueuedSfx {
    pub data: Arc<[i16]>,
    pub bus: MixBus,
    pub generation: u64,
    /// A logical sound instance to stop and restart on another trigger.
    /// Slots are scoped to a bus; `None` preserves overlapping one-shots.
    pub restart_slot: Option<u8>,
    pub pan: SfxPan,
    /// Absolute stream frame at which the first sample should become audible.
    /// `Immediate` starts at the next buffer; `At(0)` retains exact frame zero.
    pub target_stream_frame: SfxOnset,
    /// Authoritative window for a single logical editor tick voice.
    pub schedule: Option<crate::scheduled_sfx::SfxSchedule>,
    pub voice: Option<Arc<SfxVoiceControl>>,
}

/// Active SFX state retained across output callbacks.
pub struct ActiveSfx {
    pub data: Arc<[i16]>,
    pub cursor: usize,
    pub bus: MixBus,
    pub generation: u64,
    pub restart_slot: Option<u8>,
    pub pan: SfxPan,
    pub target_stream_frame: SfxOnset,
    /// Authoritative window for a single logical editor tick voice.
    pub schedule: Option<crate::scheduled_sfx::SfxSchedule>,
    pub voice: Option<Arc<SfxVoiceControl>>,
}

impl ActiveSfx {
    #[inline(always)]
    #[must_use]
    pub fn from_queued(queued: QueuedSfx) -> Self {
        Self {
            data: queued.data,
            cursor: 0,
            bus: queued.bus,
            generation: queued.generation,
            restart_slot: queued.restart_slot,
            pan: queued.pan,
            target_stream_frame: queued.target_stream_frame,
            schedule: queued.schedule,
            voice: queued.voice,
        }
    }
}

impl Drop for ActiveSfx {
    fn drop(&mut self) {
        if let Some(voice) = &self.voice {
            voice.stop();
        }
    }
}

#[inline(always)]
pub fn push_queued_sfx(active:&mut Vec<ActiveSfx>,queued:QueuedSfx,controls:&MixControls){let result=push_queued_sfx_shared(active,queued,controls,None);debug_assert!(result.is_ok());}

pub(crate)fn retire_schedule(schedule:&mut Option<crate::SfxSchedule>,retirement:&mut Option<crate::SfxScheduleRetirer>)->bool{
    let Some(value)=schedule.take()else{return true;};
    let Some(retirement)=retirement.as_mut()else{if value.credit().is_some(){*schedule=Some(value);return false;}drop(value);return true;};
    match retirement.retire(value){Ok(())=>true,Err(value)=>{*schedule=Some(value);false}}
}
pub(crate)fn push_queued_sfx_retiring(active:&mut Vec<ActiveSfx>,queued:QueuedSfx,controls:&MixControls,retirement:&mut Option<crate::SfxScheduleRetirer>)->Result<(),QueuedSfx>{push_queued_sfx_shared(active,queued,controls,Some(retirement))}
fn push_queued_sfx_shared(active:&mut Vec<ActiveSfx>,mut queued:QueuedSfx,controls:&MixControls,mut retirement:Option<&mut Option<crate::SfxScheduleRetirer>>)->Result<(),QueuedSfx>{
    if !controls.is_current(queued.bus, queued.generation) {
        return if retire_optional(&mut queued.schedule,&mut retirement){Ok(())}else{Err(queued)};
    }
    if let Some(slot) = queued.restart_slot
        && queued.target_stream_frame == SfxOnset::Immediate
        && let Some(previous) = active.iter_mut().find(|sfx| {
            sfx.bus == queued.bus
                && sfx.generation == queued.generation
                && sfx.restart_slot == Some(slot)
                && sfx.target_stream_frame == SfxOnset::Immediate
                && sfx.schedule.is_some() == queued.schedule.is_some()
        })
    {
        if !retire_optional(&mut previous.schedule,&mut retirement){return Err(queued);}
        *previous = ActiveSfx::from_queued(queued);
        return Ok(());
    }
    // A bus generation reset can leave stale voices occupying the fixed
    // capacity until the next callback. Make room for current-generation SFX.
    if active.len() == MAX_ACTIVE_SFX {
        active.retain_mut(|sfx| controls.is_current(sfx.bus,sfx.generation)||!retire_optional(&mut sfx.schedule,&mut retirement));
    }
    if active.len() < MAX_ACTIVE_SFX {
        active.push(ActiveSfx::from_queued(queued));
    }else{if !retire_optional(&mut queued.schedule,&mut retirement){return Err(queued);}if let Some(voice)=queued.voice{voice.stop();}}
    Ok(())
}


pub fn mix_active_sfx(active:&mut Vec<ActiveSfx>,output:&mut[f32],before:u64,channels:usize,controls:&MixControls)->bool{
    mix_active_sfx_shared(active,output,before,channels,controls,None,None,SfxMixPass::All,None)
}

#[derive(Clone,Copy)]pub(crate)enum SfxMixPass{All,Assist,Beat,Others}
impl SfxMixPass{fn accepts(self,source:bool,bus:usize)->bool{match self{Self::All=>true,Self::Assist=>source&&bus==4,Self::Beat=>source&&bus==5,Self::Others=>!source}}}
pub(crate)fn mix_active_sfx_retiring(active:&mut Vec<ActiveSfx>,output:&mut[f32],before:u64,channels:usize,controls:&MixControls,retirement:&mut Option<crate::SfxScheduleRetirer>,clock:Option<crate::SourceTickClock>,pass:SfxMixPass,gain:Option<f32>)->bool{
    mix_active_sfx_shared(active,output,before,channels,controls,Some(retirement),clock,pass,gain)
}
fn retire_optional(schedule:&mut Option<crate::SfxSchedule>,retirement:&mut Option<&mut Option<crate::SfxScheduleRetirer>>)->bool{match retirement.as_deref_mut(){Some(r)=>retire_schedule(schedule,r),None=>{schedule.take();true}}}
fn mix_active_sfx_shared(
    active: &mut Vec<ActiveSfx>,
    mix_f32: &mut [f32],
    total_before: u64,
    device_channels: usize,
    controls: &MixControls,
    mut retirement:Option<&mut Option<crate::SfxScheduleRetirer>>,
    source_clock:Option<crate::SourceTickClock>,
    pass:SfxMixPass,gain_override:Option<f32>,
) -> bool {
    let buf_len = mix_f32.len();
    let mut mixed_sfx = false;
    // Restart only when the next trigger becomes audible. Pending triggers do
    // not stop the existing voice at enqueue time. Fixed stack storage bounds
    // the callback to at most MAX_ACTIVE_SFX squared comparisons and no allocs.
    let mut stop_frames = [u64::MAX; MAX_ACTIVE_SFX];
    for (i, current) in active.iter().take(MAX_ACTIVE_SFX).enumerate() {
        if current.restart_slot.is_none() || current.schedule.is_some() {
            continue;
        }
        for (j, next) in active.iter().take(MAX_ACTIVE_SFX).enumerate() {
            if i == j
                || current.bus != next.bus
                || current.generation != next.generation
                || current.restart_slot != next.restart_slot
                || next.target_stream_frame == SfxOnset::Immediate
            {
                continue;
            }
            if next.target_stream_frame.order_key() > current.target_stream_frame.order_key()
                || (next.target_stream_frame == current.target_stream_frame && j > i)
            {
                stop_frames[i] =
                    stop_frames[i].min(next.target_stream_frame.order_key().unwrap_or(u64::MAX));
            }
        }
    }
    let mut index = 0usize;
    active.retain_mut(|sfx| {
        let keep=(||{
        let slot = index;
        index += 1;
        let source_bus=sfx.schedule.as_ref().is_some_and(|s|s.source_tick_bus())&&matches!(sfx.bus.index(),4|5);
        if !pass.accepts(source_bus,sfx.bus.index()){return true;}
        if slot >= MAX_ACTIVE_SFX {
            return false;
        }
        let stop_frame = stop_frames[slot];
        if stop_frame <= total_before {
            return false;
        }
        if !controls.is_current(sfx.bus, sfx.generation) {
            return false;
        }
        if let Some(voice) = &sfx.voice {
            if !voice.is_playing() {
                return false;
            }
            if voice.paused.load(Ordering::Acquire) {
                return true;
            }
        }
        if let Some(schedule) = &sfx.schedule {
            mixed_sfx |= schedule.mix_with_source(
                &sfx.data,
                mix_f32,
                total_before,
                device_channels,
                gain_override.unwrap_or_else(||controls.bus_gain(sfx.bus)),
                source_clock,
                matches!(pass,SfxMixPass::Assist|SfxMixPass::Beat),
            );
            return true;
        }
        let start_sample = match exact_onset_decision(
            sfx.target_stream_frame,
            total_before,
            device_channels,
            buf_len,
        ) {
            ScheduledOnset::Drop => return false,
            ScheduledOnset::Pending => return true,
            ScheduledOnset::StartAt(offset) => offset,
        };
        if sfx.restart_slot.is_some()
            && let SfxOnset::At(target) = sfx.target_stream_frame
            && target < total_before
        {
            let elapsed = total_before - target;
            sfx.cursor = sfx.cursor.saturating_add(
                usize::try_from(elapsed)
                    .unwrap_or(usize::MAX)
                    .saturating_mul(device_channels),
            );
        }
        sfx.cursor = sfx.cursor.min(sfx.data.len());
        sfx.target_stream_frame = SfxOnset::Immediate;
        let stop_sample = usize::try_from(stop_frame.saturating_sub(total_before))
            .unwrap_or(usize::MAX)
            .saturating_mul(device_channels);
        let n = (sfx.data.len().saturating_sub(sfx.cursor))
            .min(buf_len - start_sample)
            .min(stop_sample.saturating_sub(start_sample));
        mixed_sfx |= n > 0;
        let bus_gain = controls.bus_gain(sfx.bus);
        let src = &sfx.data[sfx.cursor..sfx.cursor + n];
        let dst = &mut mix_f32[start_sample..start_sample + n];
        if device_channels == 2 && sfx.pan != SfxPan::Center {
            mix_sfx_panned_stereo(src, dst, bus_gain, sfx.pan);
        } else {
            mix_sfx_samples(src, dst, bus_gain);
        }
        sfx.cursor += n;
        sfx.cursor < sfx.data.len() && stop_sample >= buf_len

        })();
        if !keep&&!retire_optional(&mut sfx.schedule,&mut retirement){return true;}keep
    });
    mixed_sfx
}

#[inline(never)]
fn mix_sfx_panned_stereo(src: &[i16], dst: &mut [f32], gain: f32, pan: SfxPan) {
    let channel = usize::from(pan == SfxPan::Right);
    // RageSoundUtil::Pan at -1/+1 routes the source's right channel entirely
    // to the owning side. The prepared mine OGG is mono expanded to stereo.
    if gain == 0.0
        || (gain.is_normal()
            && (gain.to_bits() & 0x007f_ffff == 0
                || (gain >= f32::MIN_POSITIVE * 32768.0 && gain.to_bits() & 0x7fff == 0)))
    {
        // A gain with at most nine significant bits multiplies every i16
        // exactly when normalization stays normal. Powers of two are also
        // exact below that bound. Zero preserves signed zeros and NaNs.
        let scale = gain / 32768.0;
        for (source, target) in src
            .as_chunks::<2>()
            .0
            .iter()
            .zip(dst.as_chunks_mut::<2>().0)
        {
            target[channel] += f32::from(source[1]) * scale;
        }
        return;
    }
    for (source, target) in src.chunks_exact(2).zip(dst.chunks_exact_mut(2)) {
        target[channel] = i16_to_f32(source[1]).mul_add(gain, target[channel]);
    }
}

#[inline]
fn mix_sfx_samples(src: &[i16], dst: &mut [f32], gain: f32) {
    if gain == 0.0
        || (gain.is_normal()
            && (gain.to_bits() & 0x007f_ffff == 0
                || (gain >= f32::MIN_POSITIVE * 32768.0 && gain.to_bits() & 0x7fff == 0)))
    {
        // A nine-bit gain significand times any i16 is representable in f32,
        // so the product needs no fused rounding.
        let scale = gain / 32768.0;
        for (dst, &src) in dst.iter_mut().zip(src) {
            *dst += f32::from(src) * scale;
        }
        return;
    }
    for (dst, &src) in dst.iter_mut().zip(src) {
        *dst = i16_to_f32(src).mul_add(gain, *dst);
    }
}

/// Where a scheduled SFX onset lands relative to the buffer currently being
/// filled. See [`scheduled_onset_decision`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduledOnset {
    /// Target frame is implausibly far ahead; drop the entry.
    Drop,
    /// Onset falls in a later buffer; keep pending without mixing.
    Pending,
    /// Mix starting at this sample offset within the current buffer.
    StartAt(usize),
}

/// Decides where a scheduled SFX onset lands within the buffer the mixer is
/// currently filling. `target_stream_frame == 0` means "play immediately".
/// `total_before` is the absolute write-head frame at the start of this buffer;
/// `buf_len` is the buffer length in interleaved samples.
#[inline(always)]
#[must_use]
pub const fn scheduled_onset_decision(
    target_stream_frame: u64,
    total_before: u64,
    device_channels: usize,
    buf_len: usize,
) -> ScheduledOnset {
    exact_onset_decision(
        SfxOnset::from_legacy(target_stream_frame),
        total_before,
        device_channels,
        buf_len,
    )
}

/// Exact onset variant, including absolute frame zero.
#[must_use]
pub const fn exact_onset_decision(
    onset: SfxOnset,
    total_before: u64,
    device_channels: usize,
    buf_len: usize,
) -> ScheduledOnset {
    let target_stream_frame = match onset {
        SfxOnset::Immediate => return ScheduledOnset::StartAt(0),
        SfxOnset::At(frame) => frame,
    };
    let frames_until = target_stream_frame.saturating_sub(total_before);
    if frames_until > MAX_SCHEDULE_AHEAD_FRAMES {
        return ScheduledOnset::Drop;
    }
    let offset = (frames_until as usize) * device_channels;
    if offset >= buf_len {
        return ScheduledOnset::Pending;
    }
    ScheduledOnset::StartAt(offset)
}

#[inline(always)]
#[must_use]
pub fn f32_to_i16(sample: f32) -> i16 {
    // Rust float-to-integer casts saturate and map NaN to zero, which exactly
    // covers the full-scale boundaries after scaling.
    (sample * (f32::from(i16::MAX) + 1.0)) as i16
}

#[inline(always)]
#[must_use]
pub fn i16_to_f32(sample: i16) -> f32 {
    f32::from(sample) / (f32::from(i16::MAX) + 1.0)
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_ACTIVE_SFX, MAX_SCHEDULE_AHEAD_FRAMES, MixBus, MixControls, QueuedSfx, ScheduledOnset,
        SfxPan, f32_to_i16, i16_to_f32, mix_active_sfx, push_queued_sfx, scheduled_onset_decision,
    };
    use std::sync::Arc;

    fn sample_play(data: &Arc<[i16]>, bus: MixBus, restart_slot: Option<u8>) -> QueuedSfx {
        QueuedSfx {
            data: Arc::clone(data),
            bus,
            generation: 0,
            restart_slot,
            pan: SfxPan::Center,
            target_stream_frame: super::SfxOnset::Immediate,
            schedule: None,
            voice: None,
        }
    }

    #[test]
    fn mine_retrigger_rewinds_one_player_without_stopping_other_players() {
        let controls = MixControls::new();
        let bus = MixBus::new(0);
        let data: Arc<[i16]> = Arc::from([1000, 2000, 3000, 4000, 5000, 6000]);
        let mut active = Vec::new();

        // Two mines on one frame are one voice per player, as in ITGmania.
        push_queued_sfx(&mut active, sample_play(&data, bus, Some(0)), &controls);
        push_queued_sfx(&mut active, sample_play(&data, bus, Some(0)), &controls);
        assert_eq!(active.len(), 1);
        push_queued_sfx(&mut active, sample_play(&data, bus, Some(1)), &controls);
        assert_eq!(active.len(), 2);

        let mut out = [0.0; 2];
        assert!(mix_active_sfx(&mut active, &mut out, 0, 1, &controls));
        assert_eq!(out, [i16_to_f32(2000), i16_to_f32(4000)]);
        assert_eq!(active[0].cursor, 2);
        assert_eq!(active[1].cursor, 2);

        push_queued_sfx(&mut active, sample_play(&data, bus, Some(0)), &controls);
        assert_eq!(active.len(), 2);
        assert_eq!(active[0].cursor, 0);
        assert_eq!(active[1].cursor, 2);

        out.fill(0.0);
        assert!(mix_active_sfx(&mut active, &mut out, 2, 1, &controls));
        assert_eq!(out, [i16_to_f32(4000), i16_to_f32(6000)]);
    }

    #[test]
    fn menu_sounds_keep_independent_overlapping_voices() {
        let controls = MixControls::new();
        let data: Arc<[i16]> = Arc::from([1000, 1000, 1000, 1000]);
        let mut active = Vec::new();
        let bus = MixBus::new(1);
        push_queued_sfx(&mut active, sample_play(&data, bus, None), &controls);
        push_queued_sfx(&mut active, sample_play(&data, bus, None), &controls);
        assert_eq!(active.len(), 2);
        let mut out = [0.0; 4];
        assert!(mix_active_sfx(&mut active, &mut out, 0, 1, &controls));
        assert_eq!(out, [i16_to_f32(2000); 4]);
    }

    #[test]
    fn two_player_mines_pan_to_separate_sides_using_native_balance() {
        let controls = MixControls::new();
        let samples: Arc<[i16]> = Arc::from([1000, 2000, 3000, 4000]);
        let bus = MixBus::new(3);
        for (pan, expected) in [
            (SfxPan::Left, [i16_to_f32(2000), 0.0, i16_to_f32(4000), 0.0]),
            (
                SfxPan::Right,
                [0.0, i16_to_f32(2000), 0.0, i16_to_f32(4000)],
            ),
            (
                SfxPan::Center,
                [
                    i16_to_f32(1000),
                    i16_to_f32(2000),
                    i16_to_f32(3000),
                    i16_to_f32(4000),
                ],
            ),
        ] {
            let mut active = Vec::new();
            let mut queued = sample_play(&samples, bus, Some(0));
            queued.pan = pan;
            push_queued_sfx(&mut active, queued, &controls);
            let mut out = [0.0; 4];
            assert!(mix_active_sfx(&mut active, &mut out, 0, 2, &controls));
            assert_eq!(out, expected);
        }
    }

    #[test]
    fn mine_retrigger_works_when_32_voice_pool_is_full() {
        let controls = MixControls::new();
        let bus = MixBus::new(0);
        let first: Arc<[i16]> = Arc::from([1000, 2000]);
        let replacement: Arc<[i16]> = Arc::from([3000, 4000]);
        let mut active = Vec::with_capacity(MAX_ACTIVE_SFX);
        push_queued_sfx(&mut active, sample_play(&first, bus, Some(0)), &controls);
        for _ in 1..MAX_ACTIVE_SFX {
            push_queued_sfx(&mut active, sample_play(&first, bus, None), &controls);
        }
        assert_eq!(active.len(), MAX_ACTIVE_SFX);
        push_queued_sfx(
            &mut active,
            sample_play(&replacement, bus, Some(0)),
            &controls,
        );
        assert_eq!(active.len(), MAX_ACTIVE_SFX);
        assert!(Arc::ptr_eq(&active[0].data, &replacement));
    }

    #[test]
    fn reset_bus_reclaims_stale_voices_before_accepting_new_sfx() {
        let controls = MixControls::new();
        let bus = MixBus::new(0);
        let data: Arc<[i16]> = Arc::from([1000, 2000]);
        let mut active = Vec::with_capacity(MAX_ACTIVE_SFX);
        for _ in 0..MAX_ACTIVE_SFX {
            push_queued_sfx(&mut active, sample_play(&data, bus, None), &controls);
        }
        assert_eq!(active.len(), MAX_ACTIVE_SFX);
        controls.stop_bus(bus);
        let mut next = sample_play(&data, bus, Some(0));
        next.generation = controls.bus_generation(bus);
        push_queued_sfx(&mut active, next, &controls);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].restart_slot, Some(0));
        assert_eq!(active[0].generation, controls.bus_generation(bus));
    }

    #[test]
    fn scheduled_onset_immediate_when_target_zero() {
        assert_eq!(
            scheduled_onset_decision(0, 10_000, 2, 1_024),
            ScheduledOnset::StartAt(0)
        );
    }

    #[test]
    fn scheduled_onset_starts_at_offset_within_buffer() {
        assert_eq!(
            scheduled_onset_decision(10_100, 10_000, 2, 1_024),
            ScheduledOnset::StartAt(200)
        );
    }

    #[test]
    fn scheduled_onset_pending_when_beyond_buffer() {
        assert_eq!(
            scheduled_onset_decision(10_600, 10_000, 2, 1_024),
            ScheduledOnset::Pending
        );
    }

    #[test]
    fn scheduled_onset_drops_when_implausibly_far_ahead() {
        assert_eq!(
            scheduled_onset_decision(MAX_SCHEDULE_AHEAD_FRAMES + 10_001, 10_000, 2, 1_024),
            ScheduledOnset::Drop
        );
    }

    #[test]
    fn scheduled_onset_fires_when_target_already_passed() {
        assert_eq!(
            scheduled_onset_decision(9_000, 10_000, 2, 1_024),
            ScheduledOnset::StartAt(0)
        );
    }

    #[test]
    fn f32_to_i16_clamps_full_scale() {
        assert_eq!(f32_to_i16(2.0), i16::MAX);
        assert_eq!(f32_to_i16(1.0), i16::MAX);
        assert_eq!(f32_to_i16(-1.0), i16::MIN);
        assert_eq!(f32_to_i16(-2.0), i16::MIN);
        assert_eq!(f32_to_i16(f32::INFINITY), i16::MAX);
        assert_eq!(f32_to_i16(f32::NEG_INFINITY), i16::MIN);
        assert_eq!(f32_to_i16(f32::NAN), 0);
    }

    #[test]
    fn f32_to_i16_maps_midpoint_samples() {
        assert_eq!(f32_to_i16(0.0), 0);
        assert_eq!(f32_to_i16(0.5), 16_384);
        assert_eq!(f32_to_i16(-0.5), -16_384);
    }

    #[test]
    fn i16_to_f32_maps_full_range() {
        assert_eq!(i16_to_f32(i16::MIN), -1.0);
        assert_eq!(i16_to_f32(0), 0.0);
        assert!((i16_to_f32(i16::MAX) - 0.999_969_5).abs() <= f32::EPSILON);
    }

    #[test]
    fn mix_controls_keep_bus_policy_outside_the_mixer() {
        let controls = MixControls::new();
        let bus = MixBus::new(3);
        controls.set_stream_gain(0.75);
        controls.set_bus_gain(bus, 0.25);
        assert_eq!(controls.stream_gain(), 0.75);
        assert_eq!(controls.bus_gain(bus), 0.25);
    }

    #[test]
    fn stopping_one_bus_only_invalidates_that_bus() {
        let controls = MixControls::new();
        let first = MixBus::new(1);
        let second = MixBus::new(2);
        let first_generation = controls.bus_generation(first);
        let second_generation = controls.bus_generation(second);
        controls.stop_bus(first);
        assert!(!controls.is_current(first, first_generation));
        assert!(controls.is_current(second, second_generation));
    }
}

#[cfg(test)]
mod editor_scoped_mix_tests {
    use super::*;
    #[test]
    fn music_override_end_reveals_latest_global_level_and_rejects_stale_owners() {
        let c = MixControls::new();
        c.set_stream_gain(0.9);
        assert!(c.begin_stream_gain_override(11, 0.4));
        assert_eq!(c.stream_gain(), 0.9);
        assert_eq!(c.effective_stream_gain(), 0.4);
        c.set_stream_gain(0.6);
        assert_eq!(c.effective_stream_gain(), 0.4);
        assert!(!c.begin_stream_gain_override(12, 0.2));
        assert!(!c.end_stream_gain_override(12));
        assert!(c.update_stream_gain_override(11, 0.3));
        assert!(c.end_stream_gain_override(11));
        assert_eq!(c.effective_stream_gain(), 0.6);
        assert!(c.begin_stream_gain_override(12, 0.2));
        assert!(!c.update_stream_gain_override(11, 0.1));
        assert!(!c.end_stream_gain_override(11));
        assert_eq!(c.effective_stream_gain(), 0.2);
        assert!(c.end_stream_gain_override(12));
        assert_eq!(c.effective_stream_gain(), 0.6);
    }
    fn queued(data: Arc<[i16]>, bus: MixBus, target: u64) -> QueuedSfx {
        QueuedSfx {
            data,
            bus,
            generation: 0,
            restart_slot: Some(0),
            pan: SfxPan::Center,
            target_stream_frame: SfxOnset::from_legacy(target),
            schedule: None,
            voice: None,
        }
    }
    #[test]
    fn future_tick_restart_is_sample_accurate_and_does_not_cut_voice_at_enqueue() {
        let c = MixControls::new();
        let b = MixBus::new(4);
        let mut active = Vec::new();
        push_queued_sfx(&mut active, queued(Arc::from([1000i16; 8]), b, 0), &c);
        push_queued_sfx(&mut active, queued(Arc::from([2000i16; 8]), b, 102), &c);
        assert_eq!(active.len(), 2);
        let mut output = [0.; 4];
        mix_active_sfx(&mut active, &mut output, 100, 1, &c);
        assert_eq!(
            output,
            [
                i16_to_f32(1000),
                i16_to_f32(1000),
                i16_to_f32(2000),
                i16_to_f32(2000)
            ]
        );
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].cursor, 2);
    }
    #[test]
    fn scheduled_tick_started_in_the_past_renders_only_remaining_fragment() {
        let c = MixControls::new();
        let mut active = Vec::new();
        let b = MixBus::new(4);
        push_queued_sfx(
            &mut active,
            queued(Arc::from([1000i16, 2000, 3000, 4000]), b, 98),
            &c,
        );
        let mut output = [0.; 4];
        mix_active_sfx(&mut active, &mut output, 100, 1, &c);
        assert_eq!(output, [i16_to_f32(3000), i16_to_f32(4000), 0., 0.]);
        assert!(active.is_empty());
        push_queued_sfx(&mut active, queued(Arc::from([1000i16; 4]), b, 1), &c);
        mix_active_sfx(&mut active, &mut output, 100, 1, &c);
        assert!(active.is_empty());
    }
    #[test]
    fn separate_tick_buses_have_independent_volume_and_generation() {
        let c = MixControls::new();
        let a = MixBus::new(4);
        let b = MixBus::new(5);
        c.set_bus_gain(a, 0.25);
        c.set_bus_gain(b, 0.5);
        let mut active = Vec::new();
        push_queued_sfx(&mut active, queued(Arc::from([1000i16; 4]), a, 0), &c);
        push_queued_sfx(&mut active, queued(Arc::from([1000i16; 4]), b, 0), &c);
        c.stop_bus(a);
        let mut out = [0.; 2];
        mix_active_sfx(&mut active, &mut out, 100, 1, &c);
        assert_eq!(out, [i16_to_f32(1000) * 0.5; 2]);
    }
}

#[cfg(test)]
mod editor_absolute_onset_tests {
    use super::*;
    #[test]
    fn absolute_frame_zero_is_distinct_from_immediate_for_partial_tick() {
        let c = MixControls::new();
        let mut active = Vec::new();
        push_queued_sfx(
            &mut active,
            QueuedSfx {
                data: Arc::from([1000i16, 2000, 3000, 4000]),
                bus: MixBus::new(4),
                generation: 0,
                restart_slot: Some(0),
                pan: SfxPan::Center,
                target_stream_frame: SfxOnset::At(0),
                schedule: None,
                voice: None,
            },
            &c,
        );
        let mut output = [0.; 4];
        mix_active_sfx(&mut active, &mut output, 2, 1, &c);
        assert_eq!(output, [i16_to_f32(3000), i16_to_f32(4000), 0., 0.]);
        assert!(active.is_empty());
    }
}
#[cfg(test)]
mod editor_tick_snapshot_tests {
    use super::*;
    #[test]
    fn dense_tick_windows_replace_two_logical_voices_without_consuming_one_voice_per_tick() {
        let controls = MixControls::new();
        let mut active = Vec::new();
        for refresh in 0..100 {
            for bus in [MixBus::new(4), MixBus::new(5)] {
                let schedule = crate::scheduled_sfx::SfxSchedule::new(
                    (0..1024)
                        .map(|frame| crate::scheduled_sfx::ScheduledTrigger {
                            frame,
                            skipped_frames: 0,
                        })
                        .collect::<Vec<_>>(),
                )
                .unwrap();
                push_queued_sfx(
                    &mut active,
                    QueuedSfx {
                        data: Arc::from([1000i16; 2]),
                        bus,
                        generation: 0,
                        restart_slot: Some(0),
                        pan: SfxPan::Center,
                        target_stream_frame: SfxOnset::Immediate,
                        schedule: Some(schedule),
                        voice: None,
                    },
                    &controls,
                );
            }
            assert_eq!(active.len(), 2, "refresh {refresh}");
        }
        let mut output = [0.; 1024];
        assert!(mix_active_sfx(&mut active, &mut output, 0, 1, &controls));
        assert!(output.iter().all(|sample| *sample == i16_to_f32(1000) * 2.));
        controls.stop_bus(MixBus::new(4));
        controls.stop_bus(MixBus::new(5));
        assert!(!mix_active_sfx(
            &mut active,
            &mut output,
            1024,
            1,
            &controls
        ));
        assert!(active.is_empty());
    }
    #[test]fn checked_editor_gain_publication_refuses_atomically_but_terminal_release_still_cleans_owner(){
        let controls=MixControls::new();controls.set_stream_gain(0.8);let old=crate::EditorMixGains{main:0.61,music:0.43,assist:0.71,beat:0.79};assert!(controls.begin_editor_gain_override(79,old));let before=controls.effective_stream_gain();controls.editor_mix.exhaust_for_test();assert!(!controls.can_update_editor_gain_override(79,crate::EditorMixGains{main:1.,..old}));assert!(!controls.update_editor_gain_override(79,crate::EditorMixGains{main:1.,..old}));assert_eq!(controls.effective_stream_gain(),before);assert_eq!(controls.editor_gain_snapshot().unwrap().gains,old);assert!(controls.end_stream_gain_override(79));assert_eq!(controls.editor_gain_owner(),0);assert_eq!(controls.effective_stream_gain(),0.8);assert!(!controls.begin_editor_gain_override(80,old));assert_eq!(controls.override_owner.load(std::sync::atomic::Ordering::Acquire),0);
    }

}
