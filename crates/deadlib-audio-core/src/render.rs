use crate::ring::{AudioRenderHandle, MusicBlock};
use crate::{
    ActiveSfx, CallbackClockSource, MAX_ACTIVE_SFX, MixControls, MusicMapSeg, QueuedSfx,
    f32_to_i16, i16_to_f32, mark_music_track_started, music_asset_epoch,
    music_gain_snap_generation, music_map_generation, music_pause_epoch, music_target_gain, music_total_frames, music_track_active_relaxed,
    music_track_has_started, music_track_paused, music_track_start_frame, note_timing_diag_callback_gap,
    publish_callback_window_end, publish_callback_window_start_nanos, seeded_music_position,
};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderReport {
    pub output_underrun: bool,
    pub callback_gap_ns: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CallbackInfo {
    pub anchor_nanos: u64,
    pub clock: CallbackClockSource,
}

pub enum OutputBufferMut<'a> {
    I16(&'a mut [i16]),
    F32(&'a mut [f32]),
}

pub struct RenderState {
    #[cfg(test)]gain_update_gate:Option<(std::sync::Arc<std::sync::Barrier>,std::sync::Arc<std::sync::Barrier>,
    )>,
    editor_gains:Option<crate::editor_mix::Snapshot>,
    callback_editor_gains:Option<crate::EditorMixGains>,
    assist_mix:Vec<f32>,beat_mix:Vec<f32>,
    sfx_retirement:Option<crate::SfxScheduleRetirer>,
    pending_sfx:Option<QueuedSfx>,
    main_clock:Option<(u64,u64,i64)>,
    main_configuration:Option<crate::editor_main::Configuration>,
    main_callback:Option<crate::editor_main::Callback>,
    preview_clock:Option<(u64,u64,i64)>,
    preview_player:Option<crate::preview_source::PreviewPlayer>,
    preview_retire_pending:Option<crate::PreviewSource>,
    transport: AudioRenderHandle,
    controls: Arc<MixControls>,
    device_channels: usize,
    mix_f32: Vec<f32>,
    active_sfx: Vec<ActiveSfx>,
    active_music: Option<MusicBlock>,
    active_sample: usize,
    music_map_generation: u64,
    music_asset_epoch:u64,
    asset_alignment_pending:bool,
    observed_pause_epoch:u64,
    pause_generation_pending:bool,
    acknowledged_music_generation: Option<u64>,
    stale_blocks_left: usize,
    /// Current music gain as seen by the mixer. Ramps toward the shared music
    /// gain target over [`MUSIC_GAIN_RAMP_FRAMES`] frames so asynchronous
    /// `ReplayGain` results do not produce an audible step.
    music_gain_current: f32,
    preview_envelope:Option<crate::preview_envelope::PreviewEnvelope>,
    /// Gain snap generation last observed; when it changes, the mixer snaps
    /// `music_gain_current` straight to the target instead of ramping.
    music_gain_snap_seen: u64,
}

/// Number of device frames over which the music gain ramps when the target
/// changes. 4000 frames is about 83 ms at 48 kHz and about 91 ms at 44.1 kHz:
/// fast enough to feel instantaneous, slow enough to eliminate the click/step
/// that an atomic gain swap would produce.
const MUSIC_GAIN_RAMP_FRAMES: f32 = 4000.0;
const MUSIC_GAIN_MAX_STEP: f32 = 1.0 / MUSIC_GAIN_RAMP_FRAMES;
const MIX_CHUNK_FRAMES: usize = 2048;
#[inline(always)]
fn advance_gain(current: &mut f32, target: f32) -> bool {
    let diff = target - *current;
    if diff.abs() <= MUSIC_GAIN_MAX_STEP {
        *current = target;
        true
    } else {
        *current = diff.signum().mul_add(MUSIC_GAIN_MAX_STEP, *current);
        false
    }
}

fn convert_music_samples(
    src: &[i16],
    dst: &mut [f32],
    channels: usize,
    music_vol: f32,
    target_gain: f32,
    current_gain: &mut f32,
) {
    if channels == 2
        && *current_gain != target_gain
        && (target_gain - *current_gain).abs()
            <= MUSIC_GAIN_MAX_STEP * (src.len().min(dst.len()) / 2) as f32
    {
        // Only split callbacks whose ramp is close to settling. Check every 64
        // frames, then convert the settled tail in one constant-gain chunk.
        let samples = src.len().min(dst.len()) / 2 * 2;
        let mut start = 0;
        while start < samples {
            let end = if *current_gain == target_gain {
                samples
            } else {
                (start + 128).min(samples)
            };
            convert_music_chunk(
                &src[start..end],
                &mut dst[start..end],
                channels,
                music_vol,
                target_gain,
                current_gain,
            );
            start = end;
        }
    } else {
        convert_music_chunk(src, dst, channels, music_vol, target_gain, current_gain);
    }
}

fn convert_music_chunk(
    src: &[i16],
    dst: &mut [f32],
    channels: usize,
    music_vol: f32,
    target_gain: f32,
    current_gain: &mut f32,
) {
    if *current_gain == target_gain {
        let scale = music_vol * target_gain;
        if scale == 0.0 {
            // Multiplication by signed zero preserves the exact sign bit that
            // the general `(sample / 32768) * scale` path produced, while
            // skipping its division for muted output.
            for (dst, &src) in dst.iter_mut().zip(src) {
                *dst = f32::from(src) * scale;
            }
            return;
        }
        if scale == 1.0 {
            for (dst, &src) in dst.iter_mut().zip(src) {
                *dst = i16_to_f32(src);
            }
            return;
        }
        for (dst, &src) in dst.iter_mut().zip(src) {
            *dst = i16_to_f32(src) * scale;
        }
        return;
    }
    if channels == 2 {
        let src_frames = src.as_chunks::<2>().0;
        let dst_frames = dst.as_chunks_mut::<2>().0;
        for (src_frame, dst_frame) in src_frames.iter().zip(dst_frames) {
            advance_gain(current_gain, target_gain);
            let scale = music_vol * *current_gain;
            dst_frame[0] = i16_to_f32(src_frame[0]) * scale;
            dst_frame[1] = i16_to_f32(src_frame[1]) * scale;
        }
        return;
    }
    for (src_frame, dst_frame) in src
        .chunks_exact(channels)
        .zip(dst.chunks_exact_mut(channels))
    {
        advance_gain(current_gain, target_gain);
        let scale = music_vol * *current_gain;
        for (dst, &src) in dst_frame.iter_mut().zip(src_frame) {
            *dst = i16_to_f32(src) * scale;
        }
    }
}

#[inline]
fn write_i16_samples(src: &[f32], dst: &mut [i16], has_audio: bool) {
    if !has_audio {
        dst.fill(0);
        return;
    }
    for (dst, &src) in dst.iter_mut().zip(src) {
        *dst = f32_to_i16(src);
    }
}

impl RenderState {
    pub fn new(
        transport: AudioRenderHandle,
        controls: Arc<MixControls>,
        device_channels: usize,
    ) -> Self {
        let capacity_blocks = transport.capacity_blocks();
        Self {
            #[cfg(test)]gain_update_gate:None,
            editor_gains:None,callback_editor_gains:None,assist_mix:vec![0.;MIX_CHUNK_FRAMES*device_channels.max(1)],beat_mix:vec![0.;MIX_CHUNK_FRAMES*device_channels.max(1)],
            sfx_retirement:None,pending_sfx:None,
            main_clock:None,main_callback:None,main_configuration:None,
            preview_clock:None,preview_player:None,preview_retire_pending:None,
            transport,
            controls,
            device_channels,
            mix_f32: vec![0.0; MIX_CHUNK_FRAMES * device_channels.max(1)],
            active_sfx: Vec::with_capacity(MAX_ACTIVE_SFX),
            active_music: None,
            active_sample: 0,
            music_map_generation: music_map_generation(),
            music_asset_epoch:music_asset_epoch(),
            asset_alignment_pending:false,
            observed_pause_epoch:music_pause_epoch(),
            pause_generation_pending:music_track_paused(),
            acknowledged_music_generation: None,
            stale_blocks_left: capacity_blocks,
            music_gain_current: music_target_gain(),
            preview_envelope:None,
            music_gain_snap_seen: music_gain_snap_generation(),
        }
    }

    /// Attach the sole callback retirement producer before starting output.
    pub fn attach_sfx_retirement(&mut self,retirement:Option<crate::SfxScheduleRetirer>){self.sfx_retirement=retirement;}
    #[inline(always)]
    fn begin_callback_nanos(
        &mut self,
        anchor_nanos: u64,
        source: CallbackClockSource,
    ) -> (u64, u64) {
        self.stale_blocks_left = self.transport.capacity_blocks();
        let track_active = music_track_active_relaxed();
        self.refresh_music_generation(!track_active);
        if !track_active {
            let _ = self.recycle_active();
        }
        let total_before = music_total_frames();
        let callback_gap_ns =
            note_timing_diag_callback_gap(anchor_nanos, source).unwrap_or_default();
        publish_callback_window_start_nanos(total_before, anchor_nanos, source);
        (total_before, callback_gap_ns)
    }

    #[inline(always)]
    fn recycle_active(&mut self) -> bool {
        let Some(block) = self.active_music.take() else {
            self.active_sample = 0;
            return true;
        };
        match self.transport.recycle_block(block) {
            Ok(()) => {
                self.active_sample = 0;
                true
            }
            Err(block) => {
                // Never free a pooled allocation on the realtime thread. A
                // full recycle queue violates ownership conservation, so keep
                // the block and retry on the next callback.
                self.active_music = Some(block);
                false
            }
        }
    }

    #[inline(always)]
    fn refresh_music_generation(&mut self, allow_unpublished: bool) {
        let target_generation = music_map_generation();
        if target_generation != self.music_map_generation
            && (allow_unpublished || self.transport.published_generation() == target_generation)
        {
            self.music_map_generation = target_generation;
        }
    }

    #[inline(always)]
    fn observe_asset_epoch(&mut self){
        let pause_epoch=music_pause_epoch();
        if self.observed_pause_epoch!=pause_epoch{self.observed_pause_epoch=pause_epoch;self.pause_generation_pending=true;self.asset_alignment_pending=true;}
        let (epoch,asset_phase)=crate::position::music_asset_state();
        if self.music_asset_epoch!=epoch{self.music_asset_epoch=epoch;self.asset_alignment_pending=asset_phase==crate::position::MusicAssetPhase::SourceClock;}
    }

    fn load_music_block(&mut self) -> bool {
        loop {
            self.observe_asset_epoch();
            self.refresh_music_generation(false);
            if let Some(block) = &self.active_music {
                let exhausted = self.active_sample >= block.samples().len();
                let current_asset=block.timing().asset_epoch==self.music_asset_epoch
                    && (!(self.asset_alignment_pending || self.pause_generation_pending) || block.timing().generation==music_map_generation());
                if !exhausted && current_asset && self.transport.published_asset_epoch()!=self.music_asset_epoch{return false;}
                if !exhausted && current_asset && block.timing().generation == self.music_map_generation {
                    if music_track_paused(){return false;}
                    self.pause_generation_pending=false;
                    let generation = self.music_map_generation;
                    if self.acknowledged_music_generation != Some(generation) {
                        // All preceding stale blocks were recycled before this
                        // matching block was reached. Publish that fact after
                        // the recycle queue ownership transfers.
                        self.transport.acknowledge_generation_drained(generation);
                        self.acknowledged_music_generation = Some(generation);
                    }
                    return true;
                }
                let target_generation = music_map_generation();
                if !exhausted && current_asset && block.timing().generation == target_generation {
                    // The producer may still be filling the transition batch.
                    // Retain its first block until readiness is published; do
                    // not recycle usable target audio as if it were stale.
                    return false;
                }
                if !exhausted {
                    if self.stale_blocks_left == 0 {
                        return false;
                    }
                    self.stale_blocks_left -= 1;
                }
                if !self.recycle_active() {
                    return false;
                }
            }
            let Some(block) = self.transport.pop_block() else {
                return false;
            };
            self.active_music = Some(block);
            self.active_sample = 0;
        }
    }

    fn retire_preview_source(&mut self,source:crate::PreviewSource)->bool{
        match self.transport.preview.retire(source){Ok(())=>true,Err(source)=>{self.preview_retire_pending=Some(source);false}}
    }
    fn accept_preview_sources(&mut self,epoch:u64){
        if let Some(source)=self.preview_retire_pending.take(){if !self.retire_preview_source(source){return;}}
        let generation=music_map_generation();
        if self.preview_player.as_ref().is_some_and(|player| {
            player.source.asset_epoch!=epoch||(!player.source.editor_main&&player.source.generation!=generation)
        }){
            let player=self.preview_player.take().unwrap();if !self.retire_preview_source(player.source){return;}
        }
        for _ in 0..crate::preview_source::COMMAND_CAPACITY{
            let Some(source)=self.transport.preview.pop()else{break;};
            if source.asset_epoch!=epoch||(!source.editor_main&&source.generation!=generation)||source.channels!=self.device_channels.max(1)||source.sample_rate_hz!=self.transport.sample_rate_hz(){if !self.retire_preview_source(source){return;}continue;}
            if let Some(player)=self.preview_player.take(){if !self.retire_preview_source(player.source){
                // Reclamation can race publication. Keep one pending retirement
                // and stop draining ready descriptors until its credit returns;
                // the usable replacement remains owned here, never destroyed.
                self.preview_player=Some(crate::preview_source::PreviewPlayer::new(source));return;
            }}
            self.preview_player=Some(crate::preview_source::PreviewPlayer::new(source));
        }
    }
    fn mix_prepared_preview(&mut self,dst:&mut[f32],total_before:u64,music_vol:f32,epoch:u64,
    )->usize{
        dst.fill(0.);
        // Return obsolete streamed blocks too. Direct preview never consumes
        // their samples and a future main stream must regain every pool credit.
        if self.active_music.is_some()&&!self.recycle_active(){return 0;}
        for _ in 0..self.transport.capacity_blocks(){let Some(block)=self.transport.pop_block()else{break;};if let Err(block)=self.transport.recycle_block(block){self.active_music=Some(block);return 0;}}
        if music_track_paused()||!music_track_active_relaxed(){return 0;}
        let Some(envelope)=self.preview_envelope else{return 0;};
        let hz=self.transport.sample_rate_hz();let region=envelope.region(hz);
        #[cfg(test)]crate::preview_source::callback_test_gate::hold();
        if region.length<=0{return 0;}
        let channels=self.device_channels.max(1);let frames=dst.len()/channels;
        let generation=music_map_generation();
        let clock_frame=match self.preview_clock.filter(|(clock_generation,clock_epoch,_)| {
                *clock_generation==generation&&*clock_epoch==epoch
            }){
            Some((_,_,frame))=>frame,
            None=>{
                let elapsed=total_before.saturating_sub(music_track_start_frame())as f64/f64::from(hz);
                if let Some((frame,anchor))=envelope.initial_clock(generation){
                    let elapsed=total_before.saturating_sub(anchor);let target=i128::from(frame)+i128::from(elapsed);
                    if elapsed>0&&target>=i128::from(region.start)+i128::from(region.length){(i128::from(region.start)+(target-i128::from(region.start)).rem_euclid(i128::from(region.length)))as i64}else{target.min(i128::from(i64::MAX))as i64}
                }else{seeded_music_position(elapsed).map(|(time,_)|((time*f64::from(hz))as i64).max(0)).or_else(||self.preview_player.as_ref().map(|player|player.frame)).unwrap_or(region.start.max(0))}
            }
        };
        let ready=self.transport.published_generation()==generation&&self.transport.published_asset_epoch()==epoch;
        let player=self.preview_player.as_mut().filter(|player| {
            player.source.asset_epoch==epoch&&player.source.generation==generation
        });
        if !ready||player.is_none(){
            let frame=region.final_frame(clock_frame,frames,true);
            self.preview_clock=Some((generation,epoch,frame));
            crate::preview_source::publish_callback_cursor(generation,epoch,frame,total_before.saturating_add(frames as u64),hz,
            );
            return 0;
        }
        let player=player.unwrap();player.frame=clock_frame;let callback_initial=clock_frame;
        let target_gain=music_target_gain();
        let mut segment_first=0usize;let mut segment_music=player.frame;
        let mut rendered=0usize;
        let end=i128::from(region.start)+i128::from(region.length);
        for frame in 0..frames{
            if !player.source.looping&&i128::from(player.frame)>=end{break;}
            if rendered==0{mark_music_track_started(total_before);}
            advance_gain(&mut self.music_gain_current,target_gain);
            let gain=region.gain(player.frame)*music_vol*self.music_gain_current;
            for channel in 0..channels{dst[frame*channels+channel]=player.sample(channel)*gain;}
            let previous=player.frame;player.frame=region.advance(previous,player.source.looping);rendered+=1;
            if player.frame!=previous.saturating_add(1)||frame+1==frames{
                let length=frame+1-segment_first;
                self.transport.push_played(generation,epoch,MusicMapSeg{stream_frame_start:total_before.saturating_sub(music_track_start_frame())as i64+segment_first as i64,frames:length as i64,music_start_sec:segment_music as f64/f64::from(hz),music_sec_per_frame:1./f64::from(hz),
                    },
                );
                segment_first=frame+1;segment_music=player.frame;
            }
        }
        if rendered>segment_first{
            self.transport.push_played(generation,epoch,MusicMapSeg{stream_frame_start:total_before.saturating_sub(music_track_start_frame())as i64+segment_first as i64,frames:(rendered-segment_first)as i64,music_start_sec:segment_music as f64/f64::from(hz),music_sec_per_frame:1./f64::from(hz),
                },
            );
        }
        player.frame=region.final_frame(callback_initial,rendered,player.source.looping);
        self.preview_clock=Some((generation,epoch,player.frame));
        if rendered>0{
            crate::preview_source::publish_callback_cursor(generation,epoch,player.frame,total_before.saturating_add(rendered as u64),hz,
            );
            crate::rendered_cursor::publish_rendered_music_cursor(crate::RenderedMusicCursor{generation,asset_epoch:epoch,next_music_seconds:player.frame as f64/f64::from(hz),rendered_frame:total_before.saturating_add(rendered as u64),
            });
        } else if frames > 0
            && crate::preview_callback_cursor()
                .is_none_or(|cursor| cursor.generation != generation || cursor.asset_epoch != epoch)
        {
            // A ready source beginning at EOF still received a real callback.
            // Publish its first silent receipt at this physical callback end;
            // keep any existing endpoint receipt stable on later silence.
            crate::preview_source::publish_callback_cursor(
                generation,
                epoch,
                player.frame,
                total_before.saturating_add(frames as u64),
                hz,
            );
        }
        rendered*channels
    }

    fn mix_editor_main(&mut self,dst:&mut[f32],total_before:u64,music_vol:f32)->usize{
        dst.fill(0.);let Some(mut callback)=self.main_callback else{return 0;};
        if self.active_music.is_some()&&!self.recycle_active(){return 0;}
        for _ in 0..self.transport.capacity_blocks(){let Some(block)=self.transport.pop_block()else{break;};if let Err(block)=self.transport.recycle_block(block){self.active_music=Some(block);return 0;}}
        let Some(player)=self.preview_player.as_ref().filter(|p| {
            p.source.editor_main&&p.source.asset_epoch==callback.control.asset_epoch
        })else{return 0;};
        let channels=self.device_channels.max(1);let hz=self.transport.sample_rate_hz();let offset=total_before.saturating_sub(callback.total_before)as usize;let frames=dst.len()/channels;let source_frames=player.source.samples.len()/channels;
        let mut consumed=false;
        if crate::editor_main::rate_one(callback.control.rate){
            for (frame,out)in dst.chunks_exact_mut(channels).enumerate(){let index=i128::from(callback.initial)+(offset+frame)as i128;if index>=0&&index<source_frames as i128{consumed=true;let start=index as usize*channels;out.copy_from_slice(&player.source.samples[start..start+channels]);}}
        }else{
            if crate::editor_time_stretch::process_range(callback.initial,&player.source.samples,hz,channels,dst,callback.control.rate,offset,
            ).is_err(){dst.fill(0.);return 0;}
            consumed=offset<source_frames&&frames>0;
            // Source's stretched buffer is still gated by original source frame count.
            for (frame,out)in dst.chunks_exact_mut(channels).enumerate(){if offset+frame>=source_frames{out.fill(0.);}}
        }
        let target=music_target_gain();for out in dst.chunks_exact_mut(channels){advance_gain(&mut self.music_gain_current,target);let gain=music_vol*self.music_gain_current;for value in out{*value*=gain;}}
        if !consumed{return 0;}
        callback.consumed=true;self.main_callback=Some(callback);mark_music_track_started(callback.total_before);
        self.transport.push_played(callback.control.generation,callback.control.asset_epoch,MusicMapSeg{stream_frame_start:total_before.saturating_sub(music_track_start_frame())as i64,frames:frames as i64,music_start_sec:(callback.initial as f64+offset as f64*callback.control.rate)/f64::from(hz),music_sec_per_frame:callback.control.rate/f64::from(hz),
            },
        );
        dst.len()
    }

    fn mix_music(&mut self, mix_f32: &mut [f32], total_before: u64, music_vol: f32) -> usize {
        let _permit=if self.main_callback.is_some(){None}else{let Some(permit)=crate::rendered_cursor::begin_music_render(music_asset_epoch())else{mix_f32.fill(0.);return 0;};Some(permit)};
        let epoch=self.main_callback.map_or_else(music_asset_epoch,|c|c.control.asset_epoch);
        self.preview_envelope=if crate::editor_preview_envelope_enabled(epoch){crate::editor_preview_envelope(epoch).or_else(||self.preview_envelope.filter(|value|value.epoch==epoch))}else{None};
        if crate::editor_preview_envelope_enabled(epoch)&&self.preview_envelope.is_none(){mix_f32.fill(0.);return 0;}
        self.accept_preview_sources(epoch);
        if crate::editor_preview_envelope_enabled(epoch){return self.mix_prepared_preview(mix_f32,total_before,music_vol,epoch);}
        if self.main_callback.is_some(){return self.mix_editor_main(mix_f32,total_before,music_vol);}
        let len = mix_f32.len();
        let channels = self.device_channels.max(1);
        let frames = len / channels;
        let target_gain = music_target_gain();
        let mut track_frame_start = None;
        let mut frame = 0;
        while frame < frames && self.load_music_block() {
            let Some(active) = self.active_music.as_ref() else {
                break;
            };
            let timing = active.timing();
            if self.asset_alignment_pending {
                if music_track_has_started() {
                    let hz=f64::from(self.transport.sample_rate_hz());
                    let elapsed=total_before.saturating_sub(music_track_start_frame()).saturating_add(frame as u64) as f64/hz;
                    if let Some((desired,_))=seeded_music_position(elapsed){
                        if !timing.music_sec_per_frame.is_finite() || timing.music_sec_per_frame<=0.0 || !timing.music_start_sec.is_finite(){break;}
                        let offset=(desired-timing.music_start_sec)/timing.music_sec_per_frame;
                        // An asset primed ahead of the live cursor must wait;
                        // do not consume future PCM early to fill a gap.
                        if offset < -0.5 {break;}
                        let target_frame=offset.max(0.0).round() as usize;
                        self.active_sample=self.active_sample.max(target_frame.saturating_mul(channels).min(active.samples().len()),
                        );
                        if self.active_sample>=active.samples().len(){
                            if self.stale_blocks_left==0{break;}
                            self.stale_blocks_left-=1;
                            if !self.recycle_active(){break;}
                            continue;
                        }
                    }
                }
                self.asset_alignment_pending=false;
            }
            let active=self.active_music.as_ref().expect("retained current asset block");
            let available_frames = (active.samples().len() - self.active_sample) / channels;
            let take = available_frames.min(frames - frame);
            let track_frame_start = *track_frame_start.get_or_insert_with(|| {
                mark_music_track_started(total_before);
                total_before.saturating_sub(music_track_start_frame()) as i64
            });
            let block_frame = self.active_sample / channels;
            let consumed = take * channels;
            let dst_start = frame * channels;
            let src = &active.samples()[self.active_sample..self.active_sample + consumed];
            let dst = &mut mix_f32[dst_start..dst_start + consumed];
            convert_music_samples(
                src,
                dst,
                channels,
                music_vol,
                target_gain,
                &mut self.music_gain_current,
            );
            if let Some(envelope)=self.preview_envelope.filter(|value|value.epoch==timing.asset_epoch) {
                if let Some(start)=active.preview_frame(){envelope.apply_frames(dst,channels,start.saturating_add(block_frame as u64),self.transport.sample_rate_hz(),
                    );}
            }
            self.transport.push_played(
                timing.generation,
                timing.asset_epoch,
                MusicMapSeg {
                    stream_frame_start: track_frame_start + frame as i64,
                    frames: take as i64,
                    music_start_sec: (block_frame as f64)
                        .mul_add(timing.music_sec_per_frame, timing.music_start_sec),
                    music_sec_per_frame: timing.music_sec_per_frame,
                },
            );
            crate::rendered_cursor::publish_rendered_music_cursor(crate::RenderedMusicCursor{
                generation:timing.generation,asset_epoch:timing.asset_epoch,
                next_music_seconds:((block_frame+take)as f64).mul_add(timing.music_sec_per_frame,timing.music_start_sec),
                rendered_frame:total_before.saturating_add(frame as u64).saturating_add(take as u64),
            });
            self.active_sample += consumed;
            frame += take;
            let exhausted = self.active_sample == active.samples().len();
            if exhausted && !self.recycle_active() {
                break;
            }
        }
        if self.music_gain_current != target_gain {
            for _ in frame..frames {
                if advance_gain(&mut self.music_gain_current, target_gain) {
                    break;
                }
            }
        }
        let popped_samples = frame * channels;
        mix_f32[popped_samples..].fill(0.0);
        popped_samples
    }

    fn queue_sfx<I>(&mut self,queued_sfx:I)where I:IntoIterator<Item=QueuedSfx>,
    {
        if let Some(queued)=self.pending_sfx.take(){if let Err(queued)=crate::mixer::push_queued_sfx_retiring(&mut self.active_sfx,queued,&self.controls,&mut self.sfx_retirement,
            ){self.pending_sfx=Some(queued);return;}}
        for queued in queued_sfx{if let Err(queued)=crate::mixer::push_queued_sfx_retiring(&mut self.active_sfx,queued,&self.controls,&mut self.sfx_retirement,
            ){self.pending_sfx=Some(queued);break;}}
    }

    fn mix_f32_buffer(&mut self, total_before: u64, len: usize) -> (usize, bool) {
        debug_assert!(len <= self.mix_f32.len());
        let mut mix_f32 = std::mem::take(&mut self.mix_f32);
        let result = self.mix_f32_buffer_into(total_before, &mut mix_f32[..len]);
        self.mix_f32 = mix_f32;
        result
    }

    fn mix_f32_buffer_into(&mut self, total_before: u64, mix_f32: &mut [f32]) -> (usize, bool) {
        let gains = self.callback_editor_gains;
        let main = self.main_callback.is_some();
        let stream_gain = if main {
            gains.map_or_else(|| self.controls.effective_stream_gain(), |g| g.music)
        } else {
            gains.map_or_else(
                || self.controls.effective_stream_gain(),
                |g| g.music * g.main,
            )
        };
        let snap_gen = music_gain_snap_generation();
        if snap_gen != self.music_gain_snap_seen {
            self.music_gain_current = music_target_gain();
            self.music_gain_snap_seen = snap_gen;
        }
        let popped = self.mix_music(mix_f32, total_before, stream_gain);
        let clock = self.main_callback.map(|c| crate::SourceTickClock {
            generation: c.control.generation,
            asset_epoch: c.control.asset_epoch,
            source_frame: c.initial,
            rate: c.control.rate,
            physical_start: c.total_before,
            frames: c.frames,
        });
        let channels = self.device_channels.max(1);
        let mut mixed = false;
        if main {
            let len = mix_f32.len();
            self.assist_mix[..len].fill(0.);
            self.beat_mix[..len].fill(0.);
            mixed |= crate::mixer::mix_active_sfx_retiring(
                &mut self.active_sfx,
                &mut self.assist_mix[..len],
                total_before,
                channels,
                &self.controls,
                &mut self.sfx_retirement,
                clock,
                crate::mixer::SfxMixPass::Assist,
                gains.map(|g| g.assist),
            );
            mixed |= crate::mixer::mix_active_sfx_retiring(
                &mut self.active_sfx,
                &mut self.beat_mix[..len],
                total_before,
                channels,
                &self.controls,
                &mut self.sfx_retirement,
                clock,
                crate::mixer::SfxMixPass::Beat,
                gains.map(|g| g.beat),
            );
            let master = gains.map_or(1., |g| g.main);
            for ((music, assist), beat) in mix_f32
                .iter_mut()
                .zip(&self.assist_mix[..len])
                .zip(&self.beat_mix[..len])
            {
                *music = ((*music + *assist) + *beat) * master;
            }
        }
        mixed |= crate::mixer::mix_active_sfx_retiring(
            &mut self.active_sfx,
            mix_f32,
            total_before,
            channels,
            &self.controls,
            &mut self.sfx_retirement,
            clock,
            if main {
                crate::mixer::SfxMixPass::Others
            } else {
                crate::mixer::SfxMixPass::All
            },
            None,
        );
        (popped, mixed)
    }

    fn render_f32_chunks<I>(&mut self, out: &mut [f32], total_before: u64, queued_sfx: I) -> usize
    where
        I: IntoIterator<Item = QueuedSfx>,
    {
        self.queue_sfx(queued_sfx);
        let chunk_samples = self.mix_f32.len();
        let mut popped_samples = 0;
        let mut mixed_sfx = false;
        for (chunk, out_chunk) in out.chunks_mut(chunk_samples).enumerate() {
            let chunk_total = total_before + (chunk * MIX_CHUNK_FRAMES) as u64;
            let (popped, chunk_mixed_sfx) = self.mix_f32_buffer_into(chunk_total, out_chunk);
            #[cfg(test)]if let Some((enter,release))=self.gain_update_gate.take(){enter.wait();release.wait();}
            mixed_sfx |= chunk_mixed_sfx;
            popped_samples += popped;
        }
        if mixed_sfx {
            for sample in out {
                *sample = sample.clamp(-1.0, 1.0);
            }
        }
        popped_samples
    }

    fn render_i16_chunks<I>(&mut self, out: &mut [i16], total_before: u64, queued_sfx: I) -> usize
    where
        I: IntoIterator<Item = QueuedSfx>,
    {
        self.queue_sfx(queued_sfx);
        let chunk_samples = self.mix_f32.len();
        let mut popped_samples = 0;
        for (chunk, out_chunk) in out.chunks_mut(chunk_samples).enumerate() {
            let chunk_total = total_before + (chunk * MIX_CHUNK_FRAMES) as u64;
            let chunk_len = out_chunk.len();
            let (popped, mixed_sfx) = self.mix_f32_buffer(chunk_total, chunk_len);
            write_i16_samples(
                &self.mix_f32[..chunk_len],
                out_chunk,
                popped > 0 || mixed_sfx,
            );
            popped_samples += popped;
        }
        popped_samples
    }

    #[inline(always)]
    fn finish_callback(
        &self,
        total_before: u64,
        emitted_samples: usize,
        popped_samples: usize,
    ) -> bool {
        let frames = emitted_samples
            .checked_div(self.device_channels)
            .unwrap_or(0);
        let output_underrun = music_track_active_relaxed() && !music_track_paused()
            && music_track_has_started()
            && popped_samples < frames * self.device_channels;
        if frames > 0 {
            publish_callback_window_end(total_before, frames as u64);
        }
        output_underrun
    }

    pub fn render<I>(
        &mut self,
        output: OutputBufferMut<'_>,
        callback: CallbackInfo,
        queued_sfx: I,
    ) -> RenderReport
    where
        I: IntoIterator<Item = QueuedSfx>,
    {
        let (total_before, callback_gap_ns) =
            self.begin_callback_nanos(callback.anchor_nanos, callback.clock);
        let output_len=match &output{OutputBufferMut::I16(out)=>out.len(),OutputBufferMut::F32(out)=>out.len(),
        };
        if let Some(gains)=self.controls.editor_gain_snapshot(){self.editor_gains=Some(gains);}
        self.callback_editor_gains=self.editor_gains.filter(|held|held.owner!=0&&held.owner==self.controls.editor_gain_owner()).map(|s|s.gains);
        let fresh=crate::editor_main_configuration();
        if let Some(configuration)=fresh{self.main_configuration=Some(configuration);}
        let main_control=fresh.or(self.main_configuration).filter(|c| {
            c.asset_epoch==music_asset_epoch()&&c.generation==music_map_generation()&&!crate::editor_preview_envelope_enabled(c.asset_epoch)&&music_track_active_relaxed()&&!music_track_paused()
        });
        let _main_permit=main_control.and_then(|c|crate::rendered_cursor::begin_music_render(c.asset_epoch));
        self.main_callback=if _main_permit.is_some(){main_control.map(|control|{
            let initial=self.main_clock.filter(|(g,e,_)|*g==control.generation&&*e==control.asset_epoch).map_or(control.frame,|(_,_,frame)|frame);
            crate::editor_main::Callback{control,initial,total_before,frames:output_len/self.device_channels.max(1),consumed:false,
                }
        })}else{None};
        let (emitted_samples, popped_samples) = match output {
            OutputBufferMut::I16(out) => {
                let len = out.len();
                (len, self.render_i16_chunks(out, total_before, queued_sfx))
            }
            OutputBufferMut::F32(out) => {
                let len = out.len();
                (len, self.render_f32_chunks(out, total_before, queued_sfx))
            }
        };
        if let Some(callback)=self.main_callback.take(){
            // Original Int32 conversion and one integer addition per PHYSICAL callback.
            let advance=(callback.frames as f64*callback.control.rate)as i32;
            let next=callback.initial.saturating_add(i64::from(advance));
            self.main_clock=Some((callback.control.generation,callback.control.asset_epoch,next,
            ));
            crate::editor_main::publish(callback.control,next,total_before.saturating_add(callback.frames as u64),self.transport.sample_rate_hz(),
            );
            if callback.consumed{crate::rendered_cursor::publish_rendered_music_cursor(crate::RenderedMusicCursor{generation:callback.control.generation,asset_epoch:callback.control.asset_epoch,next_music_seconds:next as f64/f64::from(self.transport.sample_rate_hz()),rendered_frame:total_before.saturating_add(callback.frames as u64),
                });}
        }
        RenderReport {
            output_underrun: self.finish_callback(total_before, emitted_samples, popped_samples),
            callback_gap_ns,
        }
    }
}

impl Drop for RenderState {
    fn drop(&mut self) {
        // Native session teardown stops/joins callbacks before dropping its
        // manager writer. Return descriptors now; manager reclaims final Arcs.
        for sfx in &mut self.active_sfx{let _=crate::mixer::retire_schedule(&mut sfx.schedule,&mut self.sfx_retirement);}
        if let Some(mut queued)=self.pending_sfx.take(){let _=crate::mixer::retire_schedule(&mut queued.schedule,&mut self.sfx_retirement);}
        self.accept_preview_sources(0);
        if let Some(player)=self.preview_player.take(){let _=self.retire_preview_source(player.source);}
        let _ = self.recycle_active();
    }
}

#[cfg(test)]
mod tests {
    use super::{CallbackInfo, MIX_CHUNK_FRAMES, MUSIC_GAIN_MAX_STEP, OutputBufferMut, RenderState, write_i16_samples,
    };
    use crate::ring::{
        AudioRenderHandle, MusicBlockTiming, MusicBlockWriter, PlayedMapReader, music_transport,
    };
    use crate::{
        CallbackClockSource, MixBus, MixControls, QueuedSfx, activate_music_track,
        bump_music_map_generation, i16_to_f32, music_map_generation, music_track_start_frame,
        reset_music_target_gain, set_music_target_gain, snap_music_gain_generation,
        stop_music_track,
    };
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    const CHANNELS: usize = 2;
    const SAMPLE_RATE: u32 = 48_000;
    const EFFECT_BUS: MixBus = MixBus::new(0);
    const SEC_PER_FRAME: f64 = 1.0 / 48_000.0;
    static GLOBAL_AUDIO_STATE_BUSY: AtomicBool = AtomicBool::new(false);

    fn configure_main_test(frame:i64,rate:f64)->(u64,u64){
        reset_levels();crate::reset_music_stream_clock_state();activate_music_track();crate::resume_music_track();
        let generation=crate::bump_music_map_generation();let epoch=crate::advance_music_asset_epoch();
        crate::configure_editor_main(generation,epoch,frame,rate);(generation,epoch)
    }
    fn main_packet(generation:u64,epoch:u64)->crate::PreviewSource{
        let samples:Vec<f32>=(0..20000).flat_map(|i| {
                [(i as f32*0.001).sin()*0.2,(i as f32*0.003).cos()*0.1,
                ]
            }).collect();
        crate::PreviewSource{editor_main:true,samples:samples.into(),channels:2,sample_rate_hz:48000,generation,asset_epoch:epoch,initial_frame:0,looping:false,
        }
    }
    #[test]fn editor_main_physical_callback_grid_and_single_truncation_match_original_kernel(){
        let _audio_guard = crate::test_audio_guard();
        let _guard=GlobalAudioGuard::acquire();
        for frames in [1usize,257,2049,4097]{for rate in [0.3,1.0,1.35]{
            let (generation,epoch)=configure_main_test(37,rate);let(mut stream,handle)=music_transport(48000,2);let mut renderer=RenderState::new(handle,Arc::new(MixControls::new()),2);
            let packet=main_packet(generation,epoch);let reference=packet.samples.clone();stream.writer.try_publish_preview_source(packet).unwrap();stream.writer.publish_preview_generation_ready(generation,epoch);
            for callback in 0..2{
                let initial=37+(callback as f64*(frames as f64*rate).trunc())as i64;
                let mut expected=vec![0.;frames*2];
                if rate==1.{expected.copy_from_slice(&reference[initial as usize*2..initial as usize*2+frames*2],
                        );}else{crate::editor_time_stretch::process(initial,&reference,48000,2,&mut expected,rate,
                        ).unwrap();}
                let mut actual=vec![0.;frames*2];renderer.render(OutputBufferMut::F32(&mut actual),CallbackInfo{anchor_nanos:1000+callback as u64,clock:CallbackClockSource::Instant,
                        },[],
                    );
                assert_eq!(actual.iter().map(|v|v.to_bits()).collect::<Vec<_>>(),expected.iter().map(|v|v.to_bits()).collect::<Vec<_>>(),"frames={frames} rate={rate} callback={callback}");
                let cursor=crate::editor_main_cursor().unwrap();assert_eq!(cursor.source_frame,initial+(frames as f64*rate)as i64);assert_eq!(cursor.generation,generation);assert_eq!(cursor.asset_epoch,epoch);
            }
            stop_music_track();
        }}
    }
    #[test]fn editor_main_pending_rate_and_seek_reuse_asset_pcm_without_phase_ack_before_callback(){
        let _audio_guard = crate::test_audio_guard();
        let _guard=GlobalAudioGuard::acquire();let(generation,epoch)=configure_main_test(11,0.3);let(mut stream,handle)=music_transport(48000,2);let mut renderer=RenderState::new(handle,Arc::new(MixControls::new()),2);
        let mut output=[9.;514];renderer.render(OutputBufferMut::F32(&mut output),CallbackInfo{anchor_nanos:1000,clock:CallbackClockSource::Instant,
            },[],
        );assert_eq!(output,[0.;514]);assert_eq!(crate::editor_main_cursor().unwrap().source_frame,88);assert!(!crate::rendered_music_cursor().is_some_and(|c|c.generation==generation&&c.asset_epoch==epoch));
        let next=crate::bump_music_map_generation();crate::configure_editor_main(next,epoch,88,1.35);
        // Completion began under old timeline, but immutable bytes remain this asset.
        let packet=main_packet(generation,epoch);let source=packet.samples.clone();stream.writer.try_publish_preview_source(packet).unwrap();
        // Descriptor ownership is actual PCM readiness even before the worker status flags.
        let mut expected=[0.;514];crate::editor_time_stretch::process(88,&source,48000,2,&mut expected,1.35).unwrap();renderer.render(OutputBufferMut::F32(&mut output),CallbackInfo{anchor_nanos:2000,clock:CallbackClockSource::Instant,
            },[],
        );assert_eq!(output,expected);assert_eq!(crate::rendered_music_cursor().unwrap().generation,next);
        let seek_generation=crate::bump_music_map_generation();crate::configure_editor_main(seek_generation,epoch,-3,1.);
        renderer.render(OutputBufferMut::F32(&mut output),CallbackInfo{anchor_nanos:3000,clock:CallbackClockSource::Instant,
            },[],
        );assert_eq!(&output[..6],&[0.;6]);assert_eq!(&output[6..],&source[..508]);assert_eq!(crate::editor_main_cursor().unwrap().source_frame,254);
        let stale_epoch=epoch;let epoch=crate::advance_music_asset_epoch();crate::configure_editor_main(seek_generation,epoch,5,1.);stream.writer.try_publish_preview_source(main_packet(generation,stale_epoch)).unwrap();renderer.render(OutputBufferMut::F32(&mut output),CallbackInfo{anchor_nanos:4000,clock:CallbackClockSource::Instant,
            },[],
        );assert_eq!(output,[0.;514]);assert_eq!(crate::editor_main_cursor().unwrap().asset_epoch,epoch);stop_music_track();
    }

    #[test]fn editor_main_writer_pending_uses_only_complete_same_authority_configuration(){
        let _audio_guard = crate::test_audio_guard();
        let _guard=GlobalAudioGuard::acquire();let(generation,epoch)=configure_main_test(37,0.3);let(mut stream,handle)=music_transport(48000,2);let mut renderer=RenderState::new(handle,Arc::new(MixControls::new()),2);let packet=main_packet(generation,epoch);let source=packet.samples.clone();stream.writer.try_publish_preview_source(packet).unwrap();let mut actual=vec![0.;514];renderer.render(OutputBufferMut::F32(&mut actual),CallbackInfo{anchor_nanos:1000,clock:CallbackClockSource::Instant,
            },[],
        );
        let entered=Arc::new(std::sync::Barrier::new(2));let release=Arc::new(std::sync::Barrier::new(2));crate::editor_main::control_test_gate::install(entered.clone(),release.clone());let writer=std::thread::spawn(move|| {
            assert!(crate::configure_editor_main(generation,epoch,114,1.35))
        });entered.wait();assert!(crate::editor_main_configuration().is_none());let mut expected=vec![0.;514];crate::editor_time_stretch::process(114,&source,48000,2,&mut expected,0.3).unwrap();renderer.render(OutputBufferMut::F32(&mut actual),CallbackInfo{anchor_nanos:2000,clock:CallbackClockSource::Instant,
            },[],
        );assert_eq!(actual,expected);assert_eq!(crate::editor_main_cursor().unwrap().source_frame,191);release.wait();writer.join().unwrap();crate::editor_time_stretch::process(191,&source,48000,2,&mut expected,1.35).unwrap();renderer.render(OutputBufferMut::F32(&mut actual),CallbackInfo{anchor_nanos:3000,clock:CallbackClockSource::Instant,
            },[],
        );assert_eq!(actual,expected);stop_music_track();
    }
    fn preview_packet(generation:u64,epoch:u64,initial:i64,looping:bool,
    )->crate::PreviewSource{
        let samples=(0..64).flat_map(|frame|[i16_to_f32(1000+frame);2]).collect::<Vec<_>>();
        crate::PreviewSource{editor_main:false,samples:samples.into(),channels:2,sample_rate_hz:48000,generation,asset_epoch:epoch,initial_frame:initial,looping,
        }
    }
    #[test]
    fn prepared_preview_source_live_bounds_keep_real_cursor_and_segment_loop_maps(){
        let _audio_guard = crate::test_audio_guard();
        let _guard=GlobalAudioGuard::acquire();reset_levels();crate::reset_music_stream_clock_state();activate_music_track();crate::resume_music_track();
        let generation=crate::bump_music_map_generation();let epoch=crate::advance_music_preview_asset_epoch();
        crate::set_editor_preview_envelope(epoch,10./48000.,6./48000.,0.,0.);
        let(mut stream,handle)=music_transport(48000,2);assert!(stream.writer.try_publish_preview_source(preview_packet(generation,epoch,10,true)).is_ok());
        let mut render=test_render(handle);let start=music_track_start_frame();
        // Queue ownership alone is not readiness. No PCM/cursor acknowledgement.
        assert_eq!(render.mix_f32_buffer(start,4), (0,false));
        stream.writer.publish_preview_generation_ready(generation,epoch);
        assert_eq!(render.mix_f32_buffer(start+2,40).0,40);
        for (n,frame)in render.mix_f32[..40].chunks_exact(2).enumerate(){assert_eq!(frame[0],i16_to_f32(1010+((n+2)%6)as i16));}
        let maps=(0..4).map(|_|stream.played_map.pop().unwrap()).collect::<Vec<_>>();
        assert_eq!(maps.iter().map(|(_,_,map)|map.frames).collect::<Vec<_>>(),[4,6,6,4]);
        for(index,(tag,asset,map))in maps.into_iter().enumerate(){assert_eq!((tag,asset),(generation,epoch));assert_eq!(map.music_start_sec,if index==0{12./48000.}else{10./48000.});}
        assert_eq!(crate::rendered_music_cursor().unwrap().next_music_seconds,14./48000.);
        assert!(crate::set_editor_preview_region(epoch,30./48000.,6./48000.));
        assert_eq!(render.mix_f32_buffer(start+22,44).0,44);
        assert!(render.mix_f32[..32].iter().all(|value|*value==0.));
        assert_eq!(render.mix_f32[32],i16_to_f32(1030));
        assert_eq!(crate::rendered_music_cursor().unwrap().next_music_seconds,30./48000.);
        assert!(crate::set_editor_preview_region(epoch,0.,0.));
        assert_eq!(render.mix_f32_buffer(start+44,8).0,0);
        assert_eq!(crate::rendered_music_cursor().unwrap().next_music_seconds,30./48000.);
    }
    #[test]
    fn prepared_preview_source_retirement_and_stale_publication_are_bounded(){
        let _audio_guard = crate::test_audio_guard();
        let _guard=GlobalAudioGuard::acquire();reset_levels();crate::reset_music_stream_clock_state();activate_music_track();crate::resume_music_track();
        let generation=crate::bump_music_map_generation();let epoch=crate::advance_music_preview_asset_epoch();
        crate::set_editor_preview_envelope(epoch,0.,10./48000.,0.,0.);
        let(mut stream,handle)=music_transport(48000,2);let packet=preview_packet(generation,epoch,0,true);let weak=Arc::downgrade(&packet.samples);
        assert!(stream.writer.try_publish_preview_source(packet).is_ok());stream.writer.publish_preview_generation_ready(generation,epoch);let mut render=test_render(handle);
        assert_eq!(render.mix_f32_buffer(music_track_start_frame(),4).0,4);
        let next=crate::advance_music_preview_asset_epoch();crate::set_editor_preview_envelope(next,0.,10./48000.,0.,0.);
        // Old queued descriptors, including an ABA-like same path, cannot gain
        // a new identity at callback time.
        assert!(stream.writer.try_publish_preview_source(preview_packet(generation,epoch,0,true)).is_ok());
        assert!(stream.writer.try_publish_preview_source(preview_packet(generation,next,0,true)).is_ok());
        let returned=stream.writer.try_publish_preview_source(preview_packet(generation,next,0,true));assert!(returned.is_err());drop(returned);
        stream.writer.publish_preview_generation_ready(generation,next);
        assert_eq!(render.mix_f32_buffer(music_track_start_frame()+2,4).0,4);
        assert!(weak.upgrade().is_some(),"callback must retire, not destroy source PCM");
        stream.writer.reclaim_preview_sources();assert!(weak.upgrade().is_none(),"producer performs final PCM destruction");
        assert_eq!(crate::rendered_music_cursor().unwrap().asset_epoch,next);
    }
    #[test]
    fn prepared_preview_source_nonloop_eof_and_paused_callbacks_do_not_advance(){
        let _audio_guard = crate::test_audio_guard();
        let _guard=GlobalAudioGuard::acquire();reset_levels();crate::reset_music_stream_clock_state();activate_music_track();crate::resume_music_track();
        let generation=crate::bump_music_map_generation();let epoch=crate::advance_music_preview_asset_epoch();crate::set_editor_preview_envelope(epoch,10./48000.,4./48000.,0.,0.);
        let(mut stream,handle)=music_transport(48000,2);assert!(stream.writer.try_publish_preview_source(preview_packet(generation,epoch,10,false)).is_ok());stream.writer.publish_preview_generation_ready(generation,epoch);let mut render=test_render(handle);
        crate::pause_music_track();assert_eq!(render.mix_f32_buffer(music_track_start_frame(),12).0,0);crate::resume_music_track();
        assert_eq!(render.mix_f32_buffer(music_track_start_frame(),12).0,8);assert!(render.mix_f32[8..12].iter().all(|value|*value==0.));
        assert_eq!(crate::rendered_music_cursor().unwrap().next_music_seconds,14./48000.);
        assert_eq!(stream.played_map.pop().unwrap().2.frames,4);
        assert_eq!(render.mix_f32_buffer(music_track_start_frame()+6,12).0,0);
    }
    #[test]
    fn prepared_preview_starting_at_eof_publishes_real_silent_callback_without_audible_receipt(){
        let _audio_guard = crate::test_audio_guard();
        let _guard=GlobalAudioGuard::acquire();reset_levels();crate::reset_music_stream_clock_state();activate_music_track();crate::resume_music_track();
        let generation=crate::bump_music_map_generation();let epoch=crate::advance_music_preview_asset_epoch();
        let anchor = music_track_start_frame();
        crate::initialize_editor_preview_clock(generation, epoch, 14, anchor);crate::set_editor_preview_envelope(epoch,10./48000., 4. / 48000., 0., 0.);
        let (mut stream, handle) = music_transport(48000, 2);
        stream
            .writer
            .try_publish_preview_source(preview_packet(generation, epoch, 14, false))
            .unwrap();
        stream
            .writer
            .publish_preview_generation_ready(generation, epoch);
        let audible = crate::rendered_music_cursor();
        assert!(crate::preview_callback_cursor().is_none_or(|cursor| cursor.asset_epoch != epoch));
        let mut render = test_render(handle);
        for n in 0..2 {
            assert_eq!(render.mix_f32_buffer(anchor + n * 6, 12).0, 0);
            assert!(render.mix_f32[..12].iter().all(|value| *value == 0.));
            let callback = crate::preview_callback_cursor().unwrap();
            assert_eq!(
                (callback.generation, callback.asset_epoch),
                (generation, epoch)
            );
            assert_eq!(callback.source_frame, 14);
            assert_eq!(callback.next_music_seconds, 14. / 48000.);
            assert_eq!(callback.callback_frame, anchor + 6);
            assert_eq!(crate::rendered_music_cursor(), audible);
            assert!(stream.played_map.pop().is_none());
        }
    }
    #[test]
    fn prepared_preview_source_callback_snapshot_and_suspend_linearization_are_exact(){
        let _audio_guard = crate::test_audio_guard();
        let _guard=GlobalAudioGuard::acquire();reset_levels();crate::reset_music_stream_clock_state();activate_music_track();crate::resume_music_track();
        let generation=crate::bump_music_map_generation();let epoch=crate::advance_music_preview_asset_epoch();crate::set_editor_preview_envelope(epoch,10./48000.,6./48000.,0.,0.);
        let(mut stream,handle)=music_transport(48000,2);assert!(stream.writer.try_publish_preview_source(preview_packet(generation,epoch,10,true)).is_ok());stream.writer.publish_preview_generation_ready(generation,epoch);let mut render=test_render(handle);
        let entered=Arc::new(std::sync::Barrier::new(2));let release=Arc::new(std::sync::Barrier::new(2));crate::preview_source::callback_test_gate::install(entered.clone(),release.clone());
        let rendering=std::thread::spawn(move||{let mut pcm=[0i16;20];render.render(crate::OutputBufferMut::I16(&mut pcm),crate::CallbackInfo{anchor_nanos:1000,clock:crate::CallbackClockSource::Instant,
                },[],
            );(render,pcm)});
        entered.wait();assert!(crate::set_editor_preview_region(epoch,30./48000.,6./48000.));
        let (sender,receipt)=std::sync::mpsc::channel();let freezing=std::thread::spawn(move||{sender.send(crate::freeze_rendered_music_cursor(generation,epoch)).unwrap();});
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(3);
        while crate::rendered_cursor::begin_music_render(epoch).is_some(){assert!(std::time::Instant::now()<deadline);std::thread::yield_now();}
        // Deterministic callback barrier ensures the consumed endpoint does not
        // exist yet. Freeze may not publish a pre-callback or host-time guess.
        assert!(receipt.try_recv().is_err());release.wait();let(mut render,pcm)=rendering.join().unwrap();
        for(index,frame)in pcm.chunks_exact(2).enumerate(){assert_eq!(frame,[1010+(index%6)as i16;2]);}
        let frozen=receipt.recv().unwrap().unwrap();freezing.join().unwrap();assert_eq!(frozen.next_music_seconds,14./48000.);
        let mut pcm=[1i16;20];render.render(crate::OutputBufferMut::I16(&mut pcm),crate::CallbackInfo{anchor_nanos:2000,clock:crate::CallbackClockSource::Instant,
            },[],
        );assert_eq!(pcm,[0;20]);
        let new_epoch=crate::advance_music_preview_asset_epoch();crate::set_editor_preview_envelope(new_epoch,30./48000.,6./48000.,0.,0.);
        assert!(stream.writer.try_publish_preview_source(preview_packet(generation,new_epoch,14,true)).is_ok());stream.writer.publish_preview_generation_ready(generation,new_epoch);
        render.render(crate::OutputBufferMut::I16(&mut pcm),crate::CallbackInfo{anchor_nanos:3000,clock:crate::CallbackClockSource::Instant,
            },[],
        );assert_eq!(pcm,[0;20]);assert_eq!(crate::rendered_music_cursor().unwrap().next_music_seconds,24./48000.);
    }
    #[test]
    fn prepared_preview_source_pending_callbacks_have_separate_exact_clock_without_pcm_ack(){
        let _audio_guard = crate::test_audio_guard();
        let _guard=GlobalAudioGuard::acquire();reset_levels();crate::reset_music_stream_clock_state();activate_music_track();crate::resume_music_track();
        let generation=crate::bump_music_map_generation();let epoch=crate::advance_music_preview_asset_epoch();let anchor=crate::music_total_frames();
        crate::initialize_editor_preview_clock(generation,epoch,10,anchor);crate::set_editor_preview_envelope(epoch,10./48000.,6./48000.,0.,0.);
        let(mut stream,handle)=music_transport(48000,2);let mut render=test_render(handle);let mut pcm=[i16::MAX;8];
        render.render(crate::OutputBufferMut::I16(&mut pcm),crate::CallbackInfo{anchor_nanos:1000,clock:crate::CallbackClockSource::Instant,
            },[],
        );assert_eq!(pcm,[0;8]);
        assert!(crate::rendered_music_cursor().filter(|cursor|cursor.generation==generation&&cursor.asset_epoch==epoch).is_none());
        let first=crate::preview_callback_cursor().unwrap();assert_eq!((first.source_frame,first.sample_rate_hz),(14,48000));
        assert!(crate::set_editor_preview_region(epoch,30./48000.,6./48000.));
        render.render(crate::OutputBufferMut::I16(&mut pcm),crate::CallbackInfo{anchor_nanos:2000,clock:crate::CallbackClockSource::Instant,
            },[],
        );assert_eq!(pcm,[0;8]);
        let pending=crate::preview_callback_cursor().unwrap();assert_eq!(pending.source_frame,18);
        assert!(crate::freeze_rendered_music_cursor(generation,epoch).is_none(),"logical callback progress is never a PCM acknowledgement");
        let next=crate::advance_music_preview_asset_epoch();let next_generation=crate::bump_music_map_generation();crate::initialize_editor_preview_clock(next_generation,next,18,crate::music_total_frames(),
        );crate::set_editor_preview_envelope(next,30./48000.,6./48000.,0.,0.);
        // Old prepared work may complete now. Captured authority is discarded.
        assert!(stream.writer.try_publish_preview_source(preview_packet(generation,epoch,10,true)).is_ok());
        assert!(stream.writer.try_publish_preview_source(preview_packet(next_generation,next,10,true)).is_ok());stream.writer.publish_preview_generation_ready(next_generation,next);
        render.render(crate::OutputBufferMut::I16(&mut pcm),crate::CallbackInfo{anchor_nanos:3000,clock:crate::CallbackClockSource::Instant,
            },[],
        );assert_eq!(pcm,[0;8]);
        let actual=crate::rendered_music_cursor().unwrap();assert_eq!((actual.generation,actual.asset_epoch),(next_generation,next));assert_eq!(actual.next_music_seconds,22./48000.);
        assert_eq!(stream.played_map.pop().unwrap().2.music_start_sec,18./48000.);
    }
    #[test]
    fn prepared_preview_source_endpoint_teardown_never_destroys_final_pcm_in_callback(){
        let _audio_guard = crate::test_audio_guard();
        let _guard=GlobalAudioGuard::acquire();
        for close_writer_first in [false,true]{
            reset_levels();crate::reset_music_stream_clock_state();activate_music_track();crate::resume_music_track();let generation=crate::bump_music_map_generation();let epoch=crate::advance_music_preview_asset_epoch();crate::set_editor_preview_envelope(epoch,0.,10./48000.,0.,0.);
            let(mut stream,handle)=music_transport(48000,2);let packet=preview_packet(generation,epoch,0,true);let weak=Arc::downgrade(&packet.samples);assert!(stream.writer.try_publish_preview_source(packet).is_ok());stream.writer.publish_preview_generation_ready(generation,epoch);let mut render=test_render(handle);assert_eq!(render.mix_f32_buffer(music_track_start_frame(),8).0,8);
            if close_writer_first{
                drop(stream);let next=crate::advance_music_preview_asset_epoch();crate::set_editor_preview_envelope(next,0.,10./48000.,0.,0.);assert_eq!(render.mix_f32_buffer(music_track_start_frame()+4,8).0,0);
                assert!(weak.upgrade().is_some(),"closed reclaimer cannot force final PCM destruction in callback");
                drop(render);assert!(weak.upgrade().is_none(),"stopped render endpoint releases on this control thread");
            }else{
                drop(render);assert!(weak.upgrade().is_some(),"native session shutdown returns descriptor before manager destruction");stream.writer.reclaim_preview_sources();assert!(weak.upgrade().is_none());
            }
        }
    }
    #[test]
    fn prepared_preview_source_raw_signed_fades_match_original_live_bounds_witness(){
        let _audio_guard = crate::test_audio_guard();
        let _guard=GlobalAudioGuard::acquire();
        for incoming in [true,false]{for raw in [-8.,0.,8.]{
            reset_levels();crate::reset_music_stream_clock_state();activate_music_track();crate::resume_music_track();let generation=crate::bump_music_map_generation();let epoch=crate::advance_music_preview_asset_epoch();
            let(start,length)=if incoming{(30.,6.)}else{(10.,2.)};
            crate::set_editor_preview_envelope(epoch,start/48000.,length/48000.,if incoming{raw/48000.}else{0.},if incoming{0.}else{raw/48000.},
                );
            let(mut stream,handle)=music_transport(48000,2);assert!(stream.writer.try_publish_preview_source(preview_packet(generation,epoch,14,true)).is_ok());stream.writer.publish_preview_generation_ready(generation,epoch);let mut render=test_render(handle);
            assert_eq!(render.mix_f32_buffer(music_track_start_frame(),2).0,2);
            let expected=if raw<0.{i16_to_f32(1014)}else{0.};assert_eq!(render.mix_f32[0].to_bits(),expected.to_bits());
            if !incoming{assert_eq!(crate::preview_callback_cursor().unwrap().source_frame,11);}
        }}
        reset_levels();crate::reset_music_stream_clock_state();activate_music_track();crate::resume_music_track();let generation=crate::bump_music_map_generation();let epoch=crate::advance_music_preview_asset_epoch();crate::set_editor_preview_envelope(epoch,10./48000.,2./48000.,0.,-8./48000.);
        let(mut stream,handle)=music_transport(48000,2);assert!(stream.writer.try_publish_preview_source(preview_packet(generation,epoch,14,true)).is_ok());stream.writer.publish_preview_generation_ready(generation,epoch);let mut render=test_render(handle);
        assert_eq!(render.mix_f32_buffer(music_track_start_frame(),8).0,8);for(frame,expected)in render.mix_f32[..8].chunks_exact(2).zip([1014,1013,1012,1011]){assert_eq!(frame[0],i16_to_f32(expected));}
        assert_eq!(crate::preview_callback_cursor().unwrap().source_frame,10);
    }
    #[test]
    fn prepared_preview_source_full_retirement_queue_retains_pcm_without_callback_wait_or_drop(){
        let _audio_guard = crate::test_audio_guard();
        let _guard=GlobalAudioGuard::acquire();reset_levels();crate::reset_music_stream_clock_state();activate_music_track();crate::resume_music_track();let generation=crate::bump_music_map_generation();let epoch=crate::advance_music_preview_asset_epoch();crate::set_editor_preview_envelope(epoch,0.,10./48000.,0.,0.);
        let(mut stream,handle)=music_transport(48000,2);let old=preview_packet(generation,epoch,0,true);let weak=Arc::downgrade(&old.samples);assert!(stream.writer.try_publish_preview_source(old).is_ok());stream.writer.publish_preview_generation_ready(generation,epoch);let mut render=test_render(handle);assert_eq!(render.mix_f32_buffer(music_track_start_frame(),2).0,2);
        assert!(stream.writer.try_publish_preview_source(preview_packet(generation,epoch,9,true)).is_ok());
        // Model retirements racing after the producer's last reclaim check.
        for _ in 0..3{assert!(render.transport.preview.retire(preview_packet(generation,epoch,0,true)).is_ok());}
        assert_eq!(render.mix_f32_buffer(music_track_start_frame()+1,2).0,2);assert_eq!(render.mix_f32[0],i16_to_f32(1001));assert!(render.preview_retire_pending.is_some());assert!(weak.upgrade().is_some());
        stream.writer.reclaim_preview_sources();assert_eq!(render.mix_f32_buffer(music_track_start_frame()+2,2).0,2);assert!(render.preview_retire_pending.is_none());assert!(weak.upgrade().is_some());
        stream.writer.reclaim_preview_sources();assert!(weak.upgrade().is_none());
    }

    #[test]
    fn silent_i16_fast_path_matches_sample_conversion() {
        let _audio_guard = crate::test_audio_guard();
        let src = [0.0f32; 33];
        let mut expected = [i16::MIN; 33];
        for (dst, &src) in expected.iter_mut().zip(&src) {
            *dst = crate::f32_to_i16(src);
        }
        let mut actual = [i16::MAX; 33];
        write_i16_samples(&src, &mut actual, false);
        assert_eq!(actual, expected);
    }

    #[test]
    fn silent_i16_callback_overwrites_the_whole_device_buffer() {
        let _audio_guard = crate::test_audio_guard();
        let _guard = GlobalAudioGuard::acquire();
        reset_levels();
        let (_stream, render_handle) = music_transport(SAMPLE_RATE, CHANNELS);
        let mut render = test_render(render_handle);
        let mut output = vec![i16::MAX; MIX_CHUNK_FRAMES * CHANNELS + 17];

        let popped = render.render_i16_chunks(
            &mut output,
            music_track_start_frame(),
            std::iter::empty::<QueuedSfx>(),
        );

        assert_eq!(popped, 0);
        assert!(output.iter().all(|&sample| sample == 0));
    }

    pub(super) struct GlobalAudioGuard;

    impl GlobalAudioGuard {
        pub(super) fn acquire() -> Self {
            while GLOBAL_AUDIO_STATE_BUSY
                .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_err()
            {
                std::thread::yield_now();
            }
            Self
        }
    }

    impl Drop for GlobalAudioGuard {
        fn drop(&mut self) {
            GLOBAL_AUDIO_STATE_BUSY.store(false, Ordering::Release);
        }
    }

    fn reset_levels() {
        // Each test models a fresh player; pause is singleton state too.
        crate::resume_music_track();
        reset_music_target_gain();
    }

    fn test_render(render_handle: AudioRenderHandle) -> RenderState {
        RenderState::new(render_handle, Arc::new(MixControls::new()), CHANNELS)
    }

    fn push_all(
        writer: &mut MusicBlockWriter,
        samples: &[i16],
        generation: u64,
        start_frame: usize,
    ) {
        let mut offset = 0;
        while offset < samples.len() {
            let frame = start_frame + offset / CHANNELS;
            let pushed = writer.try_push(
                &samples[offset..],
                MusicBlockTiming {
                    asset_epoch:crate::music_asset_epoch(),
                    generation,
                    music_start_sec: frame as f64 * SEC_PER_FRAME,
                    music_sec_per_frame: SEC_PER_FRAME,
                },
            );
            assert!(pushed > 0, "test transport has enough recycled blocks");
            offset += pushed;
        }
        writer.publish_asset_epoch_ready(crate::music_asset_epoch());
    }

    fn pop_map(played: &mut PlayedMapReader) -> (u64, crate::MusicMapSeg) {
        let(generation,asset_epoch,seg)=played.pop().expect("played timing span");
        assert_eq!(asset_epoch,crate::music_asset_epoch());
        (generation,seg)
    }

    #[test]
    fn mixes_across_callback_and_block_boundaries() {
        let _audio_guard = crate::test_audio_guard();
        let _guard = GlobalAudioGuard::acquire();
        reset_levels();
        let generation = music_map_generation();
        let (mut stream, render_handle) = music_transport(SAMPLE_RATE, CHANNELS);
        let input: Vec<i16> = (0..300 * CHANNELS)
            .map(|sample| (sample as i32 * 97 % 60_000 - 30_000) as i16)
            .collect();
        push_all(&mut stream.writer, &input, generation, 0);
        let mut render = test_render(render_handle);
        let track_start = music_track_start_frame();

        let (popped, _) = render.mix_f32_buffer(track_start, 100 * CHANNELS);
        assert_eq!(popped, 100 * CHANNELS);
        for (&actual, &expected) in render.mix_f32.iter().zip(&input[..popped]) {
            assert_eq!(actual.to_bits(), i16_to_f32(expected).to_bits());
        }

        let (popped, _) = render.mix_f32_buffer(track_start + 100, 200 * CHANNELS);
        assert_eq!(popped, 200 * CHANNELS);
        for (&actual, &expected) in render.mix_f32.iter().zip(&input[200..600]) {
            assert_eq!(actual.to_bits(), i16_to_f32(expected).to_bits());
        }

        let expected = [(0, 100), (100, 156), (256, 44)];
        for (start, frames) in expected {
            let (tag, seg) = pop_map(&mut stream.played_map);
            assert_eq!(tag, generation);
            assert_eq!(seg.stream_frame_start, start);
            assert_eq!(seg.frames, frames);
            assert_eq!(
                seg.music_start_sec.to_bits(),
                (start as f64 * SEC_PER_FRAME).to_bits()
            );
            assert_eq!(seg.music_sec_per_frame.to_bits(), SEC_PER_FRAME.to_bits());
        }
        assert!(stream.played_map.pop().is_none());
    }

    #[test]
    fn callback_larger_than_scratch_buffer_is_chunked_without_data_loss() {
        let _audio_guard = crate::test_audio_guard();
        let _guard = GlobalAudioGuard::acquire();
        reset_levels();
        let generation = music_map_generation();
        let (mut stream, render_handle) = music_transport(SAMPLE_RATE, CHANNELS);
        let input: Vec<i16> = (0..3000 * CHANNELS)
            .map(|sample| (sample as i32 * 193 % 60_000 - 30_000) as i16)
            .collect();
        push_all(&mut stream.writer, &input, generation, 0);
        let mut render = test_render(render_handle);
        let mut output = vec![f32::NAN; input.len()];

        let popped = render.render_f32_chunks(
            &mut output,
            music_track_start_frame(),
            std::iter::empty::<crate::QueuedSfx>(),
        );

        assert_eq!(popped, input.len());
        for (&actual, &expected) in output.iter().zip(&input) {
            assert_eq!(actual.to_bits(), i16_to_f32(expected).to_bits());
        }
    }

    #[test]
    fn sfx_in_later_chunk_clamps_the_whole_f32_callback() {
        let _audio_guard = crate::test_audio_guard();
        let _guard = GlobalAudioGuard::acquire();
        reset_levels();
        set_music_target_gain(2.0);
        snap_music_gain_generation();
        let generation = music_map_generation();
        let (mut stream, render_handle) = music_transport(SAMPLE_RATE, CHANNELS);
        let input = vec![i16::MAX; 3000 * CHANNELS];
        push_all(&mut stream.writer, &input, generation, 0);
        let mut render = test_render(render_handle);
        let total_before = music_track_start_frame();
        let sfx = QueuedSfx {
            data: Arc::from([0, 0]),
            bus: EFFECT_BUS,
            generation: 0,
            restart_slot: None,
            pan: crate::SfxPan::Center,
            target_stream_frame: crate::mixer::SfxOnset::At(total_before + MIX_CHUNK_FRAMES as u64 + 5,
            ), schedule: None,
            voice: None,
        };
        let mut output = vec![f32::NAN; input.len()];

        let popped = render.render_f32_chunks(&mut output, total_before, [sfx]);

        assert_eq!(popped, input.len());
        assert!(output.iter().all(|&sample| sample == 1.0));
        reset_levels();
    }

    #[test]
    fn generation_reset_discards_active_and_queued_old_audio() {
        let _audio_guard = crate::test_audio_guard();
        let _guard = GlobalAudioGuard::acquire();
        reset_levels();
        let old_generation = music_map_generation();
        let (mut stream, render_handle) = music_transport(SAMPLE_RATE, CHANNELS);
        let old = vec![11_000; 256 * CHANNELS];
        push_all(&mut stream.writer, &old, old_generation, 0);
        let mut render = test_render(render_handle);
        let track_start = music_track_start_frame();
        assert_eq!(
            render.mix_f32_buffer(track_start, 20 * CHANNELS).0,
            20 * CHANNELS
        );

        let new_generation = bump_music_map_generation();
        push_all(&mut stream.writer, &old, old_generation, 256);
        let new = vec![-7_000; 64 * CHANNELS];
        push_all(&mut stream.writer, &new, new_generation, 0);
        stream.writer.publish_generation_ready(new_generation);
        let (popped, _) = render.mix_f32_buffer(track_start + 20, 32 * CHANNELS);
        assert_eq!(popped, 32 * CHANNELS);
        for &actual in &render.mix_f32[..32 * CHANNELS] {
            assert_eq!(actual.to_bits(), i16_to_f32(-7_000).to_bits());
        }

        let (old_tag, old_seg) = pop_map(&mut stream.played_map);
        assert_eq!((old_tag, old_seg.frames), (old_generation, 20));
        let (new_tag, new_seg) = pop_map(&mut stream.played_map);
        assert_eq!((new_tag, new_seg.frames), (new_generation, 32));
        assert!(stream.played_map.pop().is_none());
    }

    #[test]
    fn generation_reset_keeps_old_audio_until_replacement_is_published() {
        let _audio_guard = crate::test_audio_guard();
        let _guard = GlobalAudioGuard::acquire();
        reset_levels();
        let old_generation = music_map_generation();
        let (mut stream, render_handle) = music_transport(SAMPLE_RATE, CHANNELS);
        let old = vec![11_000; 128 * CHANNELS];
        push_all(&mut stream.writer, &old, old_generation, 0);
        let mut render = test_render(render_handle);
        let track_start = music_track_start_frame();

        let new_generation = bump_music_map_generation();
        let (popped, _) = render.mix_f32_buffer(track_start, 32 * CHANNELS);
        assert_eq!(popped, 32 * CHANNELS);
        assert!(
            render.mix_f32[..popped]
                .iter()
                .all(|sample| sample.to_bits() == i16_to_f32(11_000).to_bits())
        );

        let new = vec![-7_000; 64 * CHANNELS];
        push_all(&mut stream.writer, &new, new_generation, 0);
        stream.writer.publish_generation_ready(new_generation);
        let (popped, _) = render.mix_f32_buffer(track_start + 32, 32 * CHANNELS);
        assert_eq!(popped, 32 * CHANNELS);
        assert_eq!(stream.writer.render_drained_generation(), new_generation);
        assert!(
            render.mix_f32[..popped]
                .iter()
                .all(|sample| sample.to_bits() == i16_to_f32(-7_000).to_bits())
        );
    }

    #[test]
    fn target_block_is_retained_while_transition_batch_is_primed() {
        let _audio_guard = crate::test_audio_guard();
        let _guard = GlobalAudioGuard::acquire();
        reset_levels();
        let (mut stream, render_handle) = music_transport(SAMPLE_RATE, CHANNELS);
        let mut render = test_render(render_handle);
        let new_generation = bump_music_map_generation();
        push_all(&mut stream.writer, &[-7_000, 7_000], new_generation, 0);

        assert_eq!(
            render.mix_f32_buffer(music_track_start_frame(), CHANNELS).0,
            0
        );
        assert!(render.active_music.is_some());

        stream.writer.publish_generation_ready(new_generation);
        let (popped, _) = render.mix_f32_buffer(music_track_start_frame(), CHANNELS);
        assert_eq!(popped, CHANNELS);
        assert_eq!(render.mix_f32[0].to_bits(), i16_to_f32(-7_000).to_bits());
        assert_eq!(render.mix_f32[1].to_bits(), i16_to_f32(7_000).to_bits());
    }

    #[test]
    fn stopped_track_does_not_render_an_unpublished_stale_generation() {
        let _audio_guard = crate::test_audio_guard();
        let _guard = GlobalAudioGuard::acquire();
        reset_levels();
        activate_music_track();
        let generation = music_map_generation();
        let (mut stream, render_handle) = music_transport(SAMPLE_RATE, CHANNELS);
        push_all(&mut stream.writer, &[12_000, -12_000], generation, 0);
        let mut render = test_render(render_handle);

        bump_music_map_generation();
        stop_music_track();
        let (total_before, _) = render.begin_callback_nanos(0, CallbackClockSource::Instant);
        let (popped, _) = render.mix_f32_buffer(total_before, CHANNELS);

        assert_eq!(popped, 0);
        assert_eq!(&render.mix_f32[..CHANNELS], &[0.0, 0.0]);
    }

    #[test]
    fn underrun_zeroes_whole_frames_and_unaligned_tail() {
        let _audio_guard = crate::test_audio_guard();
        let _guard = GlobalAudioGuard::acquire();
        reset_levels();
        let generation = music_map_generation();
        let (mut stream, render_handle) = music_transport(SAMPLE_RATE, CHANNELS);
        push_all(&mut stream.writer, &[1234, -1234], generation, 0);
        let mut render = test_render(render_handle);
        let (popped, _) = render.mix_f32_buffer(music_track_start_frame(), 5);

        assert_eq!(popped, 2);
        assert_eq!(render.mix_f32[0].to_bits(), i16_to_f32(1234).to_bits());
        assert_eq!(render.mix_f32[1].to_bits(), i16_to_f32(-1234).to_bits());
        assert_eq!(&render.mix_f32[2..5], &[0.0, 0.0, 0.0]);
    }

    #[test]
    fn underrun_reporting_counts_only_complete_frames() {
        let _audio_guard = crate::test_audio_guard();
        let _guard = GlobalAudioGuard::acquire();
        let (_stream, render_handle) = music_transport(SAMPLE_RATE, CHANNELS);
        let mut render = test_render(render_handle);
        activate_music_track();
        crate::mark_music_track_started(0);

        for (channels, emitted, popped, underrun) in [
            (0, 5, 0, false),
            (1, 5, 4, true),
            (2, 1, 0, false),
            (2, 5, 4, false),
            (2, 5, 3, true),
            (2, 5, 6, false),
            (6, 17, 12, false),
            (6, 17, 11, true),
        ] {
            render.device_channels = channels;
            assert_eq!(
                render.finish_callback(0, emitted, popped),
                underrun,
                "channels={channels}, emitted={emitted}, popped={popped}"
            );
        }
        stop_music_track();
    }

    #[test]
    fn gain_ramp_still_advances_once_per_device_frame() {
        let _audio_guard = crate::test_audio_guard();
        let _guard = GlobalAudioGuard::acquire();
        reset_levels();
        let generation = music_map_generation();
        let (mut stream, render_handle) = music_transport(SAMPLE_RATE, CHANNELS);
        let input = [10_000, -10_000, 20_000, -20_000, 30_000, -30_000];
        push_all(&mut stream.writer, &input, generation, 0);
        let mut render = test_render(render_handle);
        set_music_target_gain(0.5);

        let (popped, _) = render.mix_f32_buffer(music_track_start_frame(), input.len());
        assert_eq!(popped, input.len());
        let mut gain = 1.0;
        for (frame, samples) in input.as_chunks::<CHANNELS>().0.iter().enumerate() {
            gain -= MUSIC_GAIN_MAX_STEP;
            for (channel, &sample) in samples.iter().enumerate() {
                let expected = i16_to_f32(sample) * gain;
                assert_eq!(
                    render.mix_f32[frame * CHANNELS + channel].to_bits(),
                    expected.to_bits()
                );
            }
        }
        reset_music_target_gain();
    }

    #[test]
    fn first_played_span_starts_at_zero_after_decode_delay() {
        let _audio_guard = crate::test_audio_guard();
        let _guard = GlobalAudioGuard::acquire();
        reset_levels();
        let generation = music_map_generation();
        let (mut stream, render_handle) = music_transport(SAMPLE_RATE, CHANNELS);
        push_all(&mut stream.writer, &[1000, -1000], generation, 0);
        let mut render = test_render(render_handle);
        let delayed_callback = music_track_start_frame() + 123;
        activate_music_track();

        let (popped, _) = render.mix_f32_buffer(delayed_callback, CHANNELS);
        assert_eq!(popped, CHANNELS);
        let (_, seg) = pop_map(&mut stream.played_map);
        assert_eq!(seg.stream_frame_start, 0);
        stop_music_track();
    }

    #[test]
    fn reset_reaches_current_audio_behind_a_full_stale_backlog() {
        let _audio_guard = crate::test_audio_guard();
        let _guard = GlobalAudioGuard::acquire();
        reset_levels();
        let old_generation = music_map_generation();
        let (mut stream, render_handle) = music_transport(SAMPLE_RATE, CHANNELS);
        let block = vec![500; crate::ring::MUSIC_BLOCK_FRAMES * CHANNELS];
        let pool_blocks = stream.writer.capacity_blocks();
        for block_index in 0..pool_blocks - 1 {
            push_all(
                &mut stream.writer,
                &block,
                old_generation,
                block_index * crate::ring::MUSIC_BLOCK_FRAMES,
            );
        }
        let new_generation = bump_music_map_generation();
        push_all(&mut stream.writer, &[700, -700], new_generation, 0);
        stream.writer.publish_generation_ready(new_generation);
        let mut render = test_render(render_handle);

        let (popped, _) = render.mix_f32_buffer(music_track_start_frame(), CHANNELS);
        assert_eq!(popped, CHANNELS);
        assert_eq!(render.mix_f32[0].to_bits(), i16_to_f32(700).to_bits());
        assert_eq!(render.mix_f32[1].to_bits(), i16_to_f32(-700).to_bits());
        assert_eq!(render.stale_blocks_left, pool_blocks - (pool_blocks - 1));
    }
    #[test]
    fn asset_epoch_rejects_active_and_queued_old_pcm_before_new_asset_is_ready() {
        let _audio_guard = crate::test_audio_guard();
        let _guard=GlobalAudioGuard::acquire();reset_levels();crate::reset_music_stream_clock_state();activate_music_track();crate::seed_music_asset_clock(0.0,1.0);
        let generation=music_map_generation();let old_epoch=crate::music_asset_epoch();
        let(mut stream,handle)=music_transport(SAMPLE_RATE,CHANNELS);let mut render=test_render(handle);
        push_all(&mut stream.writer,&vec![111;600*CHANNELS],generation,0,
        );stream.writer.publish_generation_ready(generation);
        let start=music_track_start_frame();let mut first=[0_i16;8];render.render(OutputBufferMut::I16(&mut first),CallbackInfo{anchor_nanos:1,clock:CallbackClockSource::Instant,
            },[],
        );assert!(first.iter().all(|x|*x==111));
        let epoch=crate::advance_music_asset_epoch();assert_ne!(epoch,old_epoch);assert_eq!(music_map_generation(),generation);
        let mut pending=[i16::MAX;40];render.render(OutputBufferMut::I16(&mut pending),CallbackInfo{anchor_nanos:2,clock:CallbackClockSource::Instant,
            },[],
        );assert!(pending.iter().all(|x|*x==0));assert_eq!(music_track_start_frame(),start);
        // A delayed replacement was decoded from zero, but the logical cursor
        // has advanced24frames. Trim the new ramp to NOW rather than replaying0.
        let ramp:Vec<i16>=(0..100).flat_map(|frame|[500+frame,500+frame]).collect();push_all(&mut stream.writer,&ramp,generation,0);stream.writer.publish_generation_ready(generation);
        let mut output=[0_i16;4];render.render(OutputBufferMut::I16(&mut output),CallbackInfo{anchor_nanos:3,clock:CallbackClockSource::Instant,
            },[],
        );assert_eq!(output,[524,524,525,525]);
        assert_eq!(music_track_start_frame(),start);assert_eq!(music_map_generation(),generation);assert!(crate::music_track_active());
        let mut tags=Vec::new();while let Some((tag,asset,map))=stream.played_map.pop(){assert_eq!(tag,generation);tags.push(asset);if asset==epoch{assert!((map.music_start_sec-24.0/48000.0).abs()<1e-12);}}
        assert_eq!(tags,[old_epoch,epoch]);stop_music_track();crate::clear_music_stream_clock_seed();
    }
    #[test]
    fn asset_epoch_aba_never_authenticates_old_asset_or_its_played_map() {
        let _audio_guard = crate::test_audio_guard();
        let _guard=GlobalAudioGuard::acquire();reset_levels();crate::reset_music_stream_clock_state();activate_music_track();crate::seed_music_asset_clock(0.0,1.0);
        let generation=music_map_generation();let a1=crate::music_asset_epoch();let(mut stream,handle)=music_transport(SAMPLE_RATE,CHANNELS);let mut render=test_render(handle);
        let b=crate::advance_music_asset_epoch();let a2=crate::advance_music_asset_epoch();assert!(a1<b&&b<a2);
        for(epoch,value)in[(a1,100),(b,200),(a2,300)]{assert_eq!(stream.writer.try_push(&[value,value],MusicBlockTiming{generation,asset_epoch:epoch,music_start_sec:0.0,music_sec_per_frame:SEC_PER_FRAME}),CHANNELS);}
        stream.writer.publish_generation_ready(generation);let mut output=[0_i16;2];render.render(OutputBufferMut::I16(&mut output),CallbackInfo{anchor_nanos:4,clock:CallbackClockSource::Instant,
            },[],
        );assert_eq!(output,[300,300]);
        let(tag,asset,_)=stream.played_map.pop().unwrap();assert_eq!((tag,asset),(generation,a2));assert!(stream.played_map.pop().is_none());stop_music_track();crate::clear_music_stream_clock_seed();
    }
    #[test]
    fn full_old_asset_pool_recycles_without_waiting_for_replacement_readiness() {
        let _audio_guard = crate::test_audio_guard();
        let _guard=GlobalAudioGuard::acquire();reset_levels();crate::reset_music_stream_clock_state();activate_music_track();crate::seed_music_asset_clock(0.0,1.0);
        let generation=music_map_generation();let(mut stream,handle)=music_transport(SAMPLE_RATE,CHANNELS);let mut render=test_render(handle);let capacity=stream.writer.capacity_blocks();
        let old=MusicBlockTiming{generation,asset_epoch:crate::music_asset_epoch(),music_start_sec:0.0,music_sec_per_frame:SEC_PER_FRAME,
        };let block=vec![100;crate::ring::MUSIC_BLOCK_FRAMES*CHANNELS];for _ in 0..capacity{assert_eq!(stream.writer.try_push(&block,old),block.len());}stream.writer.publish_generation_ready(generation);
        let epoch=crate::advance_music_asset_epoch();let mut silence=[i16::MAX;2];render.render(OutputBufferMut::I16(&mut silence),CallbackInfo{anchor_nanos:5,clock:CallbackClockSource::Instant,
            },[],
        );assert_eq!(silence,[0,0]);assert_eq!(stream.writer.outstanding_blocks(),0);
        assert_eq!(stream.writer.try_push(&[700,700],MusicBlockTiming{asset_epoch:epoch,..old}),CHANNELS);
        // Same timeline readiness from A cannot expose B's unprimed block.
        render.render(OutputBufferMut::I16(&mut silence),CallbackInfo{anchor_nanos:6,clock:CallbackClockSource::Instant,
            },[],
        );assert_eq!(silence,[0,0]);
        stream.writer.publish_generation_ready(generation);render.render(OutputBufferMut::I16(&mut silence),CallbackInfo{anchor_nanos:7,clock:CallbackClockSource::Instant,
            },[],
        );assert_eq!(silence,[700,700]);stop_music_track();crate::clear_music_stream_clock_seed();
    }
    #[test]
    fn replacement_rejects_its_old_timeline_pcm_while_queued_rate_or_seek_is_pending(){
        let _audio_guard = crate::test_audio_guard();
        let _guard=GlobalAudioGuard::acquire();reset_levels();crate::reset_music_stream_clock_state();activate_music_track();crate::seed_music_asset_clock(0.0,1.0);crate::mark_music_track_started(music_track_start_frame());
        let old=music_map_generation();let(mut stream,handle)=music_transport(SAMPLE_RATE,CHANNELS);let mut render=test_render(handle);
        let epoch=crate::advance_music_asset_epoch();let current=bump_music_map_generation();
        let timing=MusicBlockTiming{generation:old,asset_epoch:epoch,music_start_sec:0.0,music_sec_per_frame:SEC_PER_FRAME,
        };
        assert_eq!(stream.writer.try_push(&[100,100],timing),CHANNELS);stream.writer.publish_generation_ready(old);
        let mut output=[i16::MAX;2];render.render(OutputBufferMut::I16(&mut output),CallbackInfo{anchor_nanos:8,clock:CallbackClockSource::Instant,
            },[],
        );assert_eq!(output,[0,0]);assert!(stream.played_map.pop().is_none());
        assert_eq!(stream.writer.try_push(&[700,700,700,700],MusicBlockTiming{generation:current,..timing}),CHANNELS*2);stream.writer.publish_generation_ready(current);
        render.render(OutputBufferMut::I16(&mut output),CallbackInfo{anchor_nanos:9,clock:CallbackClockSource::Instant,
            },[],
        );assert_eq!(output,[700,700]);let(tag,asset,_)=stream.played_map.pop().unwrap();assert_eq!((tag,asset),(current,epoch));stop_music_track();crate::clear_music_stream_clock_seed();
    }

    #[test]
    fn pause_mutes_and_recycles_old_pcm_without_resetting_asset_origin_or_gain(){
        let _audio_guard = crate::test_audio_guard();
        let _guard=GlobalAudioGuard::acquire();crate::resume_music_track();reset_levels();crate::reset_music_stream_clock_state();activate_music_track();
        let generation=music_map_generation();let epoch=crate::music_asset_epoch();let gain=crate::music_target_gain();let(mut stream,handle)=music_transport(SAMPLE_RATE,CHANNELS);let mut render=test_render(handle);
        push_all(&mut stream.writer,&vec![111;600*CHANNELS],generation,0,
        );stream.writer.publish_generation_ready(generation);
        let mut first=[0_i16;8];render.render(OutputBufferMut::I16(&mut first),CallbackInfo{anchor_nanos:11,clock:CallbackClockSource::Instant,
            },[],
        );assert_eq!(first,[111;8]);let start=music_track_start_frame();while stream.played_map.pop().is_some(){}
        let before=crate::music_pause_epoch();assert!(crate::pause_music_track()>before);bump_music_map_generation();let mut silence=[i16::MAX;16];
        let report=render.render(OutputBufferMut::I16(&mut silence),CallbackInfo{anchor_nanos:12,clock:CallbackClockSource::Instant,
            },[],
        );assert_eq!(silence,[0;16]);assert!(!report.output_underrun);assert!(stream.played_map.pop().is_none());assert_eq!(stream.writer.outstanding_blocks(),0);
        assert!(crate::music_track_paused());assert_eq!(crate::music_asset_epoch(),epoch);assert_eq!(music_track_start_frame(),start);assert_eq!(crate::music_target_gain(),gain);assert!(crate::music_track_active());crate::resume_music_track();stop_music_track();
    }
    #[test]
    fn pause_resume_before_any_callback_cannot_replay_old_generation_active_or_queued_pcm(){
        let _audio_guard = crate::test_audio_guard();
        let _guard=GlobalAudioGuard::acquire();crate::resume_music_track();reset_levels();crate::reset_music_stream_clock_state();activate_music_track();
        let generation=music_map_generation();let epoch=crate::music_asset_epoch();let(mut stream,handle)=music_transport(SAMPLE_RATE,CHANNELS);let mut render=test_render(handle);
        push_all(&mut stream.writer,&vec![111;600*CHANNELS],generation,0,
        );stream.writer.publish_generation_ready(generation);
        let mut first=[0_i16;8];render.render(OutputBufferMut::I16(&mut first),CallbackInfo{anchor_nanos:13,clock:CallbackClockSource::Instant,
            },[],
        );let start=music_track_start_frame();while stream.played_map.pop().is_some(){}
        crate::pause_music_track();bump_music_map_generation();let resumed=bump_music_map_generation();crate::resume_music_track();
        let mut silence=[i16::MAX;8];render.render(OutputBufferMut::I16(&mut silence),CallbackInfo{anchor_nanos:14,clock:CallbackClockSource::Instant,
            },[],
        );assert_eq!(silence,[0;8]);assert_eq!(stream.writer.outstanding_blocks(),0);assert!(stream.played_map.pop().is_none());
        assert_eq!(stream.writer.try_push(&[700,700],MusicBlockTiming{generation:resumed,asset_epoch:epoch,music_start_sec:3.0,music_sec_per_frame:SEC_PER_FRAME}),CHANNELS);
        // Old generation readiness is insufficient after a fast Pause/Resume.
        let mut output=[i16::MAX;2];render.render(OutputBufferMut::I16(&mut output),CallbackInfo{anchor_nanos:15,clock:CallbackClockSource::Instant,
            },[],
        );assert_eq!(output,[0,0]);
        stream.writer.publish_generation_ready(resumed);render.render(OutputBufferMut::I16(&mut output),CallbackInfo{anchor_nanos:16,clock:CallbackClockSource::Instant,
            },[],
        );assert_eq!(output,[700,700]);let(tag,asset,map)=stream.played_map.pop().unwrap();assert_eq!((tag,asset),(resumed,epoch));assert_eq!(map.music_start_sec,3.0);assert_eq!(music_track_start_frame(),start);assert!(!crate::music_track_paused());stop_music_track();
    }
    #[test]
    fn pause_keeps_current_ready_pcm_owned_until_resume_without_consuming_it(){
        let _audio_guard = crate::test_audio_guard();
        let _guard=GlobalAudioGuard::acquire();crate::resume_music_track();reset_levels();crate::reset_music_stream_clock_state();activate_music_track();
        let epoch=crate::music_asset_epoch();let(mut stream,handle)=music_transport(SAMPLE_RATE,CHANNELS);let mut render=test_render(handle);crate::pause_music_track();let generation=bump_music_map_generation();
        assert_eq!(stream.writer.try_push(&[812,812],MusicBlockTiming{generation,asset_epoch:epoch,music_start_sec:2.0,music_sec_per_frame:SEC_PER_FRAME}),CHANNELS);stream.writer.publish_generation_ready(generation);
        let mut output=[i16::MAX;2];render.render(OutputBufferMut::I16(&mut output),CallbackInfo{anchor_nanos:17,clock:CallbackClockSource::Instant,
            },[],
        );assert_eq!(output,[0,0]);assert_eq!(stream.writer.outstanding_blocks(),1);assert!(stream.played_map.pop().is_none());
        // Core flag release itself preserves this block. The backend's normal
        // Resume also publishes a fresh Seek generation, tested separately.
        crate::resume_music_track();render.render(OutputBufferMut::I16(&mut output),CallbackInfo{anchor_nanos:18,clock:CallbackClockSource::Instant,
            },[],
        );assert_eq!(output,[812,812]);assert_eq!(crate::music_asset_epoch(),epoch);stop_music_track();
    }
    #[test]
    fn resumed_retained_asset_aligns_first_new_pcm_to_live_source_clock_after_prime_delay(){
        let _audio_guard = crate::test_audio_guard();
        let _guard=GlobalAudioGuard::acquire();crate::resume_music_track();reset_levels();crate::reset_music_stream_clock_state();activate_music_track();
        let generation=music_map_generation();let epoch=crate::music_asset_epoch();let(mut stream,handle)=music_transport(SAMPLE_RATE,CHANNELS);let mut render=test_render(handle);
        push_all(&mut stream.writer,&vec![111;600*CHANNELS],generation,0,
        );stream.writer.publish_generation_ready(generation);let mut first=[0_i16;8];render.render(OutputBufferMut::I16(&mut first),CallbackInfo{anchor_nanos:19,clock:CallbackClockSource::Instant,
            },[],
        );
        crate::pause_music_track();bump_music_map_generation();let mut silence=vec![i16::MAX;960*CHANNELS];render.render(OutputBufferMut::I16(&mut silence),CallbackInfo{anchor_nanos:20,clock:CallbackClockSource::Instant,
            },[],
        );assert!(silence.iter().all(|v|*v==0));while stream.played_map.pop().is_some(){}
        // Backend Resume establishes its new logical clock immediately. The
        // retained decoder then needs100ms of real callback time to prime.
        crate::reset_music_stream_clock_state();activate_music_track();crate::seed_music_asset_clock(3.0,1.0);crate::mark_music_track_started(music_track_start_frame());let resumed=bump_music_map_generation();crate::resume_music_track();
        for at in 21..26{render.render(OutputBufferMut::I16(&mut silence),CallbackInfo{anchor_nanos:at,clock:CallbackClockSource::Instant,
                },[],
            );assert!(silence.iter().all(|v|*v==0));}
        let ramp:Vec<i16>=(0..6000).flat_map(|frame|[800+(frame/16)as i16;CHANNELS]).collect();let mut offset=0;
        while offset<ramp.len(){let pushed=stream.writer.try_push(&ramp[offset..],MusicBlockTiming{generation:resumed,asset_epoch:epoch,music_start_sec:3.0+(offset/CHANNELS)as f64/48000.0,music_sec_per_frame:SEC_PER_FRAME,
                },
            );assert!(pushed>0);offset+=pushed;}stream.writer.publish_generation_ready(resumed);
        let mut output=[0_i16;2];render.render(OutputBufferMut::I16(&mut output),CallbackInfo{anchor_nanos:26,clock:CallbackClockSource::Instant,
            },[],
        );assert_eq!(output,[1100,1100]);let(tag,asset,map)=stream.played_map.pop().unwrap();assert_eq!((tag,asset),(resumed,epoch));assert!((map.music_start_sec-3.1).abs()<1e-12);assert_eq!(crate::music_asset_epoch(),epoch);crate::resume_music_track();stop_music_track();crate::clear_music_stream_clock_seed();
    }

    #[test]fn coherent_editor_gains_are_frozen_for_callback_and_contention_uses_same_owner_tuple(){
        let _audio_guard = crate::test_audio_guard();
        let _guard=GlobalAudioGuard::acquire();let(generation,epoch)=configure_main_test(0,1.35);let(mut stream,handle)=music_transport(48000,2);let controls=Arc::new(MixControls::new());
        let old=crate::EditorMixGains{main:0.61,music:0.43,assist:0.71,beat:0.79,
        };let new=crate::EditorMixGains{main:0.37,music:0.83,assist:0.29,beat:0.53,
        };assert!(controls.begin_editor_gain_override(73,old));
        stream.writer.try_publish_preview_source(crate::PreviewSource{editor_main:true,samples:vec![0.5;80000].into(),channels:2,sample_rate_hz:48000,generation,asset_epoch:epoch,initial_frame:0,looping:false,
            }).unwrap();stream.writer.publish_preview_generation_ready(generation,epoch);
        let mut render=RenderState::new(handle,controls.clone(),2);let(mut sender,mut receiver)=crate::sfx_transport(8);render.attach_sfx_retirement(receiver.take_retirement());
        for(bus,value)in[(4,-0.25f32),(5,0.125)]{let schedule=crate::SfxSchedule::with_source_main_float(vec![0],Arc::new(crate::SfxScheduleVoice::new()),generation,epoch,vec![value;40000].into(),2,
            ).unwrap();assert!(sender.try_send(QueuedSfx{data:Arc::from([]),bus:MixBus::new(bus),generation:0,restart_slot:Some(0),pan:crate::SfxPan::Center,target_stream_frame:crate::SfxOnset::Immediate,schedule:Some(schedule),voice:None}).is_ok());}
        let expected=|g:crate::EditorMixGains| {
            ((0.5*g.music+(-0.25*g.assist))+(0.125*g.beat))*g.main
        };
        let callback=|render:&mut RenderState,receiver:&mut crate::SfxReceiver,g:crate::EditorMixGains|{let mut output=vec![0.;4097*2];render.render(OutputBufferMut::F32(&mut output),CallbackInfo{anchor_nanos:0,clock:CallbackClockSource::Instant,
                },receiver.try_iter(),
            );assert!(output.iter().all(|v|v.to_bits()==expected(g).to_bits()));};callback(&mut render,&mut receiver,old);
        let enter=Arc::new(std::sync::Barrier::new(2));let release=Arc::new(std::sync::Barrier::new(2));controls.editor_gain_gate(enter.clone(),release.clone());let c=controls.clone();let writer=std::thread::spawn(move||c.update_editor_gain_override(73,new));enter.wait();callback(&mut render,&mut receiver,old);release.wait();assert!(writer.join().unwrap());callback(&mut render,&mut receiver,new);
        let enter=Arc::new(std::sync::Barrier::new(2));let release=Arc::new(std::sync::Barrier::new(2));render.gain_update_gate=Some((enter.clone(),release.clone()));let c=controls.clone();let update=std::thread::spawn(move||{enter.wait();assert!(c.update_editor_gain_override(73,old));release.wait();});callback(&mut render,&mut receiver,new);update.join().unwrap();assert_eq!(controls.editor_gain_snapshot().unwrap().gains,old);

        assert!(controls.end_stream_gain_override(73));assert_eq!(controls.editor_gain_owner(),0);assert!(controls.editor_gain_snapshot().is_none());stop_music_track();
    }

    #[test]fn preview_nonunit_master_retains_source_default_volume_product_before_pcm(){
        let _audio_guard = crate::test_audio_guard();
        let _guard=GlobalAudioGuard::acquire();reset_levels();crate::reset_music_stream_clock_state();activate_music_track();crate::resume_music_track();let generation=crate::bump_music_map_generation();let epoch=crate::advance_music_preview_asset_epoch();crate::set_editor_preview_envelope(epoch,5./48000.,50./48000.,0.,0.);
        let(mut stream,handle)=music_transport(48000,2);let controls=Arc::new(MixControls::new());assert!(controls.begin_editor_gain_override(75,crate::EditorMixGains{main:0.61,music:0.43,assist:0.71,beat:0.79}));stream.writer.try_publish_preview_source(crate::PreviewSource{editor_main:false,samples:(0..100).flat_map(|_|[0.25,-0.125]).collect::<Vec<_>>().into(),channels:2,sample_rate_hz:48000,generation,asset_epoch:epoch,initial_frame:5,looping:true,
            }).unwrap();stream.writer.publish_preview_generation_ready(generation,epoch);
        let mut render=RenderState::new(handle,controls,2);let mut output=vec![0.;34];render.render(OutputBufferMut::F32(&mut output),CallbackInfo{anchor_nanos:0,clock:CallbackClockSource::Instant,
            },[],
        );let volume=0.43f32*0.61;for frame in output.chunks_exact(2){assert_eq!(frame[0].to_bits(),(0.25f32*volume).to_bits());assert_eq!(frame[1].to_bits(),(-0.125f32*volume).to_bits());}stop_music_track();
    }

}

#[cfg(test)]
#[path = "render_perf_tests.rs"]
mod perf_tests;

#[cfg(test)]pub(crate)fn test_global_audio_guard()->impl Drop{tests::GlobalAudioGuard::acquire()}
