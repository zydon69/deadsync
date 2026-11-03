//! Last actually consumed music mapping, never a decoded-ahead or host clock.
use std::sync::atomic::{AtomicU64, Ordering};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderedMusicCursor {
    pub generation: u64,
    pub asset_epoch: u64,
    pub next_music_seconds: f64,
    pub rendered_frame: u64,
}
pub(crate) struct Cursor {
    sequence: AtomicU64,
    generation: AtomicU64,
    asset: AtomicU64,
    seconds: AtomicU64,
    frame: AtomicU64,
    source_frame:AtomicU64,
    source_hz:AtomicU64,
}
impl Cursor {
    pub(crate) const fn new() -> Self {
        Self {
            sequence: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            asset: AtomicU64::new(0),
            seconds: AtomicU64::new(0),
            frame: AtomicU64::new(0),
            source_frame:AtomicU64::new(0),source_hz:AtomicU64::new(0),
        }
    }
    pub(crate) fn publish(&self, value: RenderedMusicCursor) {self.publish_details(value,None);}
    pub(crate) fn publish_details(&self,value:RenderedMusicCursor,source:Option<(i64,u32)>){
        if !value.next_music_seconds.is_finite() {
            return;
        }
        let seq = self.sequence.load(Ordering::Acquire);
        let Some(next) = seq.checked_add(2) else {
            return;
        };
        // No blocking, allocation, locks or spin on the callback. A simultaneous old
        // session writer may skip publication; identity filtering still rejects it.
        if seq & 1 != 0
            || self
                .sequence
                .compare_exchange(seq, seq + 1, Ordering::AcqRel, Ordering::Relaxed)
                .is_err()
        {
            return;
        }
        self.generation.store(value.generation, Ordering::Relaxed);
        self.asset.store(value.asset_epoch, Ordering::Relaxed);
        self.seconds
            .store(value.next_music_seconds.to_bits(), Ordering::Relaxed);
        self.frame.store(value.rendered_frame, Ordering::Relaxed);
        self.source_frame.store(source.map_or(0,|value|value.0 as u64),Ordering::Relaxed);
        self.source_hz.store(source.map_or(0,|value|u64::from(value.1)),Ordering::Relaxed,
        );
        self.sequence.store(next, Ordering::Release);
    }
    pub(crate) fn read(&self) -> Option<RenderedMusicCursor> {self.read_details().map(|(cursor,_)|cursor)}
    pub(crate) fn read_details(&self)->Option<(RenderedMusicCursor,Option<(i64,u32)>)>{
        for _ in 0..4 {
            let seq = self.sequence.load(Ordering::Acquire);
            if seq == 0 || seq & 1 != 0 {
                continue;
            }
            let value = RenderedMusicCursor {
                generation: self.generation.load(Ordering::Relaxed),
                asset_epoch: self.asset.load(Ordering::Relaxed),
                next_music_seconds: f64::from_bits(self.seconds.load(Ordering::Relaxed)),
                rendered_frame: self.frame.load(Ordering::Relaxed),
            };
            std::sync::atomic::fence(Ordering::Acquire);
            let source_frame=self.source_frame.load(Ordering::Relaxed)as i64;let source_hz=self.source_hz.load(Ordering::Relaxed)as u32;
            std::sync::atomic::fence(Ordering::Acquire);
            if self.sequence.load(Ordering::Relaxed) == seq {
                return Some((value,(source_hz>0).then_some((source_frame,source_hz))));
            }
        }
        None
    }
}
static CURSOR: Cursor = Cursor::new();
pub fn rendered_music_cursor() -> Option<RenderedMusicCursor> {
    CURSOR.read()
}
pub(crate) fn publish_rendered_music_cursor(value: RenderedMusicCursor) {
    CURSOR.publish(value);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cursor_requires_actual_publication_and_never_extrapolates() {
        let _audio_guard = crate::test_audio_guard();
        let cursor = Cursor::new();
        assert!(cursor.read().is_none());
        let value = RenderedMusicCursor {
            generation: 3,
            asset_epoch: 7,
            next_music_seconds: 1.25,
            rendered_frame: 1024,
        };
        cursor.publish(value);
        for _ in 0..100 {
            assert_eq!(cursor.read(), Some(value));
        }
        cursor.publish(RenderedMusicCursor {
            generation: 4,
            asset_epoch: 8,
            next_music_seconds: 0.,
            rendered_frame: 2048,
        });
        assert_eq!(cursor.read().unwrap().asset_epoch, 8);
    }
}
#[cfg(test)]
mod callback_tests {
    use super::*;
    use crate::*;
    use std::sync::Arc;
    #[test]
    fn actual_callback_publishes_consumed_partial_block_end_not_decoder_ahead() {
        let _audio_guard = crate::test_audio_guard();
        resume_music_track();
        reset_music_stream_clock_state();
        reset_music_target_gain();
        let generation = bump_music_map_generation();
        let epoch = advance_music_preview_asset_epoch();
        activate_music_track();
        let (mut stream, handle) = music_transport(48000, 2);
        let mut render = RenderState::new(handle, Arc::new(MixControls::new()), 2);
        let timing = MusicBlockTiming {
            generation,
            asset_epoch: epoch,
            music_start_sec: 0.25,
            music_sec_per_frame: 1. / 48000.,
        };
        assert_eq!(stream.writer.try_push(&[1234; 200], timing), 200);
        stream.writer.publish_generation_ready(generation);
        let mut output = [0i16; 20];
        render.render(
            OutputBufferMut::I16(&mut output),
            CallbackInfo {
                anchor_nanos: 1000,
                clock: CallbackClockSource::Instant,
            },
            [],
        );
        assert_eq!(output, [1234; 20]);
        let first = rendered_music_cursor().unwrap();
        assert_eq!((first.generation, first.asset_epoch), (generation, epoch));
        assert!((first.next_music_seconds - (0.25 + 10. / 48000.)).abs() < 1e-12);
        assert_eq!(first.rendered_frame, music_total_frames());
        for _ in 0..100 {
            assert_eq!(rendered_music_cursor(), Some(first));
        }
        pause_music_track();
        render.render(
            OutputBufferMut::I16(&mut output),
            CallbackInfo {
                anchor_nanos: 2000,
                clock: CallbackClockSource::Instant,
            },
            [],
        );
        assert_eq!(output, [0; 20]);
        assert_eq!(rendered_music_cursor(), Some(first));
        resume_music_track();
        stop_music_track();
    }
}

// Admission covers the full consumption/publication critical section, not only
// cursor stores. Freeze waits on the CONTROL thread; the callback never waits.
static RENDER_READERS:AtomicU64=AtomicU64::new(0);
static FROZEN_ASSET:AtomicU64=AtomicU64::new(0);
pub(crate) struct MusicRenderPermit;
impl Drop for MusicRenderPermit{fn drop(&mut self){RENDER_READERS.fetch_sub(1,Ordering::SeqCst);}}
pub(crate)fn begin_music_render(epoch:u64)->Option<MusicRenderPermit>{
    RENDER_READERS.fetch_add(1,Ordering::SeqCst);
    let permit=MusicRenderPermit;
    if FROZEN_ASSET.load(Ordering::SeqCst)==epoch{return None;}
    #[cfg(test)]callback_barrier::hold();
    Some(permit)
}
/// Linearization: close admissions for this immutable asset, drain admitted
/// renderers, then read their final completed publication. A fresh asset epoch
/// is independently admissible; no old Play can reopen the frozen authority.
pub fn freeze_rendered_music_cursor(generation:u64,asset_epoch:u64,
)->Option<RenderedMusicCursor>{
    FROZEN_ASSET.store(asset_epoch,Ordering::SeqCst);
    while RENDER_READERS.load(Ordering::SeqCst)!=0{std::thread::yield_now();}
    rendered_music_cursor().filter(|cursor|cursor.generation==generation&&cursor.asset_epoch==asset_epoch)
}
#[cfg(test)]mod callback_barrier{
    use std::sync::{Arc,Barrier,Mutex};
    static HOOK:Mutex<Option<(Arc<Barrier>,Arc<Barrier>)>>=Mutex::new(None);
    pub(super)fn install(entered:Arc<Barrier>,release:Arc<Barrier>){*HOOK.lock().unwrap()=Some((entered,release));}
    pub(super)fn hold(){let hook=HOOK.lock().unwrap().take();if let Some((entered,release))=hook{entered.wait();release.wait();}}
}
#[cfg(test)]mod linearization_tests{
 use super::*;use crate::*;use std::sync::{Arc,Barrier,mpsc};
 #[test]fn freeze_waits_for_inflight_callback_and_captures_its_pcm_endpoint(){
        let _audio_guard = crate::test_audio_guard();
        resume_music_track();reset_music_stream_clock_state();reset_music_target_gain();let generation=bump_music_map_generation();let epoch=advance_music_preview_asset_epoch();activate_music_track();let(mut stream,handle)=music_transport(48000,2);let mut renderer=RenderState::new(handle,Arc::new(MixControls::new()),2);assert_eq!(stream.writer.try_push(&[1234;200],MusicBlockTiming{generation,asset_epoch:epoch,music_start_sec:0.25,music_sec_per_frame:1./48000.}),200);stream.writer.publish_generation_ready(generation);
    let entered=Arc::new(Barrier::new(2));let release=Arc::new(Barrier::new(2));callback_barrier::install(entered.clone(),release.clone());let rendering=std::thread::spawn(move||{let mut pcm=[0_i16;20];renderer.render(OutputBufferMut::I16(&mut pcm),CallbackInfo{anchor_nanos:1000,clock:CallbackClockSource::Instant,
                },[],
            );(renderer,pcm)});entered.wait();
    let(done,receipt)=mpsc::channel();let freezing=std::thread::spawn(move||{let cursor=freeze_rendered_music_cursor(generation,epoch);done.send(cursor).unwrap();});
    while FROZEN_ASSET.load(Ordering::SeqCst)!=epoch{std::thread::yield_now();}assert!(receipt.try_recv().is_err(),"freeze must not use a pre-callback snapshot while its PCM is in flight");release.wait();let(mut renderer,pcm)=rendering.join().unwrap();assert_eq!(pcm,[1234;20]);let frozen=receipt.recv().unwrap().unwrap();freezing.join().unwrap();assert!((frozen.next_music_seconds-(0.25+10./48000.)).abs()<1e-12);assert_eq!(frozen.rendered_frame,music_total_frames());
    let mut silent=[i16::MAX;20];renderer.render(OutputBufferMut::I16(&mut silent),CallbackInfo{anchor_nanos:2000,clock:CallbackClockSource::Instant,
            },[],
        );assert_eq!(silent,[0;20]);assert_eq!(rendered_music_cursor(),Some(frozen));let new_epoch=advance_music_preview_asset_epoch();assert_ne!(new_epoch,epoch);assert!(begin_music_render(new_epoch).is_some());stop_music_track();
 }
}

#[cfg(test)]mod preview_frame_snapshot_tests{
    use super::*;
    #[test]fn exact_preview_frame_rate_and_authority_are_one_coherent_snapshot(){
        let _audio_guard = crate::test_audio_guard();
        use std::sync::{Arc,atomic::AtomicBool};
        let cursor=Arc::new(Cursor::new());let done=Arc::new(AtomicBool::new(false));let worker_cursor=cursor.clone();let worker_done=done.clone();
        let writer=std::thread::spawn(move||{
            for n in 1..100000u64{let frame=n as i64-50000;let hz=if n%2==0{44100}else{48000};worker_cursor.publish_details(RenderedMusicCursor{generation:n,asset_epoch:n+1000,next_music_seconds:frame as f64/f64::from(hz),rendered_frame:n,
                    },Some((frame,hz)),
                );}
            worker_done.store(true,Ordering::Release);
        });
        let check=|value:(RenderedMusicCursor,Option<(i64,u32)>)|{if let(cursor,Some((frame,hz)))=value{let n=cursor.generation;assert_eq!(cursor.asset_epoch,n+1000);assert_eq!(cursor.rendered_frame,n);assert_eq!(frame,n as i64-50000);assert_eq!(hz,if n%2==0{44100}else{48000});assert_eq!(cursor.next_music_seconds.to_bits(),(frame as f64/f64::from(hz)).to_bits());}};
        while !done.load(Ordering::Acquire){if let Some(value)=cursor.read_details(){check(value);}}
        writer.join().unwrap();check(cursor.read_details().unwrap());
    }
}

pub(crate) fn thaw_music_render(epoch:u64){let _=FROZEN_ASSET.compare_exchange(epoch,0,Ordering::SeqCst,Ordering::SeqCst);}
