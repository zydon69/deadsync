//! Editor preview controls are sampled once per callback. Queued PCM stays dry.
use std::sync::atomic::{AtomicBool, AtomicU64,Ordering};
static WRITER:AtomicBool=AtomicBool::new(false);
fn write(update:impl FnOnce()){while WRITER.compare_exchange(false,true,Ordering::Acquire,Ordering::Relaxed).is_err(){std::thread::yield_now();}SEQUENCE.fetch_add(1,Ordering::SeqCst);update();SEQUENCE.fetch_add(1,Ordering::SeqCst);WRITER.store(false,Ordering::Release);}
static SEQUENCE:AtomicU64=AtomicU64::new(0);
static EPOCH:AtomicU64=AtomicU64::new(0);
static START:AtomicU64=AtomicU64::new(0);
static LENGTH:AtomicU64=AtomicU64::new(0);
static FADE_IN:AtomicU64=AtomicU64::new(0);
static FADE_OUT:AtomicU64=AtomicU64::new(0);
static CLOCK_EPOCH:AtomicU64=AtomicU64::new(0);
static CLOCK_GENERATION:AtomicU64=AtomicU64::new(0);
static CLOCK_FRAME:AtomicU64=AtomicU64::new(0);
static CLOCK_ANCHOR:AtomicU64=AtomicU64::new(0);
/// Accepted control state, not a callback, PCM, decoder or lease acknowledgement.
pub fn initialize_editor_preview_clock(generation:u64,epoch:u64,frame:i64,anchor:u64){
    write(||{CLOCK_EPOCH.store(epoch,Ordering::SeqCst);CLOCK_GENERATION.store(generation,Ordering::SeqCst);CLOCK_FRAME.store(frame as u64,Ordering::SeqCst);CLOCK_ANCHOR.store(anchor,Ordering::SeqCst);});
}
#[derive(Clone,Copy)]
pub struct PreviewEnvelope {pub epoch:u64,#[allow(dead_code)]start:f64,length:f64,fade_in:f64,fade_out:f64,clock_epoch:u64,clock_generation:u64,clock_frame:i64,clock_anchor:u64,
}
/// Control writers serialize; render never waits on them.
pub fn set_editor_preview_envelope(epoch:u64,start:f64,length:f64,fade_in:f64,fade_out:f64,
) {
    if epoch==0 || ![start,length,fade_in,fade_out].into_iter().all(f64::is_finite) || length<0. {return;}
    write(||{START.store(start.to_bits(),Ordering::SeqCst);LENGTH.store(length.to_bits(),Ordering::SeqCst);
    FADE_IN.store(fade_in.to_bits(),Ordering::SeqCst);FADE_OUT.store(fade_out.to_bits(),Ordering::SeqCst);
    EPOCH.store(epoch,Ordering::SeqCst);});
}
pub(crate) fn clear(){write(||EPOCH.store(0,Ordering::SeqCst));}
pub fn editor_preview_envelope_enabled(epoch:u64)->bool {epoch!=0 && EPOCH.load(Ordering::SeqCst)==epoch}
pub fn editor_preview_envelope(epoch:u64)->Option<PreviewEnvelope> {
    for _ in 0..2 {
        let before=SEQUENCE.load(Ordering::SeqCst);if before&1!=0 {continue;}
        let value=PreviewEnvelope{epoch:EPOCH.load(Ordering::SeqCst),start:f64::from_bits(START.load(Ordering::SeqCst)),length:f64::from_bits(LENGTH.load(Ordering::SeqCst)),fade_in:f64::from_bits(FADE_IN.load(Ordering::SeqCst)),fade_out:f64::from_bits(FADE_OUT.load(Ordering::SeqCst)),clock_epoch:CLOCK_EPOCH.load(Ordering::SeqCst),clock_generation:CLOCK_GENERATION.load(Ordering::SeqCst),clock_frame:CLOCK_FRAME.load(Ordering::SeqCst)as i64,clock_anchor:CLOCK_ANCHOR.load(Ordering::SeqCst),
        };
        if SEQUENCE.load(Ordering::SeqCst)==before {return (value.epoch==epoch).then_some(value);}
    }
    None
}
/// A bounds edit changes only one coherent callback snapshot, not its cursor.
pub fn set_editor_preview_region(epoch:u64,start:f64,length:f64)->bool{
    if epoch==0||![start,length].into_iter().all(f64::is_finite)||length<0.{return false;}
    let mut accepted=false;
    write(||{if EPOCH.load(Ordering::SeqCst)==epoch{START.store(start.to_bits(),Ordering::SeqCst);LENGTH.store(length.to_bits(),Ordering::SeqCst);accepted=true;}});
    accepted
}
impl PreviewEnvelope {
    pub fn length_seconds(self)->f64{self.length}
    pub(crate) fn initial_clock(self,generation:u64)->Option<(i64,u64)>{(self.clock_epoch==self.epoch&&self.clock_generation==generation).then_some((self.clock_frame,self.clock_anchor))}
    pub(crate) fn region(self,hz:u32)->crate::preview_source::Region{
        let frames=|value:f64|(value*f64::from(hz))as i64;
        crate::preview_source::Region{start:frames(self.start),length:frames(self.length),incoming:frames(self.fade_in),outgoing:frames(self.fade_out),
        }
    }

    pub(crate) fn apply_frames(self,samples:&mut[f32],channels:usize,start:u64,hz:u32){
        let incoming=(self.fade_in*f64::from(hz))as u64;let outgoing=(self.fade_out*f64::from(hz))as u64;let length=(self.length*f64::from(hz))as u64;
        for(index,frame)in samples.chunks_exact_mut(channels.max(1)).enumerate(){
            let local=start.saturating_add(index as u64);let out_start=i128::from(length)-i128::from(outgoing);
            let gain=if outgoing>0&&i128::from(local)>out_start{(1.-(i128::from(local)-out_start)as f64/outgoing as f64).clamp(0.,1.)as f32}
            else if incoming>0&&local<incoming{(local as f64/incoming as f64)as f32}else{1.};for sample in frame{*sample*=gain;}
        }
    }

}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepted_preview_clock_is_scoped_exact_and_does_not_publish_callback_progress() {
        let _audio_guard = crate::test_audio_guard();
        let before = crate::rendered_music_cursor();
        let frame = i64::MAX - 1024;
        initialize_editor_preview_clock(801, 802, frame, 803);
        set_editor_preview_envelope(802, 0., 2., 0., 0.);
        assert_eq!(editor_preview_initial_clock(801, 802), Some((frame, 803)));
        assert_eq!(editor_preview_initial_clock(800, 802), None);
        assert_eq!(editor_preview_initial_clock(801, 804), None);
        assert_eq!(editor_preview_initial_frame(801, 802), Some(frame));
        assert_eq!(crate::rendered_music_cursor(), before);
        clear();
    }
    #[test]
    fn source_strict_outgoing_overlap_and_device_sample_units(){
        let _audio_guard = crate::test_audio_guard();
        let value=PreviewEnvelope{epoch:9,start:0.,length:10./48000.,fade_in:8./48000.,fade_out:8./48000.,clock_epoch:0,clock_generation:0,clock_frame:0,clock_anchor:0,
        };
        let mut samples=vec![1.;10];value.apply_frames(&mut samples,1,0,48000);
        assert_eq!(samples,vec![0.,0.125,0.25,0.875,0.75,0.625,0.5,0.375,0.25,0.125]);
        let mut partial=vec![1.;5];value.apply_frames(&mut partial,1,5,48000);assert_eq!(partial,&samples[5..]);
    }
    #[test]
    fn fractional_origin_and_partial_phase_quantize_before_subtraction(){
        let _audio_guard = crate::test_audio_guard();
        let start=0.005012345;let phase=start+2.5/48000.;
        let offset=(phase*48000.)as u64-(start*48000.)as u64;assert_eq!(offset,3);
        let value=PreviewEnvelope{epoch:9,start,length:10./48000.,fade_in:8./48000.,fade_out:8./48000.,clock_epoch:0,clock_generation:0,clock_frame:0,clock_anchor:0,
        };
        let mut partial=vec![1.;3];value.apply_frames(&mut partial,1,offset,48000);
        assert_eq!(partial,vec![0.875,0.75,0.625]);
    }
}

/// Accepted initial frame and physical anchor, scoped to one transport intent.
/// This is not a callback, decoder or lease acknowledgement.
pub fn editor_preview_initial_clock(generation: u64, epoch: u64) -> Option<(i64, u64)> {
    editor_preview_envelope(epoch)?.initial_clock(generation)
}

/// Accepted initial frame only; not a callback, decoder or lease acknowledgement.
pub fn editor_preview_initial_frame(generation:u64,epoch:u64)->Option<i64>{
    editor_preview_initial_clock(generation, epoch).map(|value|value.0)}
