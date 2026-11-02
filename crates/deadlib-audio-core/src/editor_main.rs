//! Editor Main's control origin and real callback cursor. No decoder readiness.
use std::sync::atomic::{AtomicBool,AtomicU64,Ordering};
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct EditorMainCursor{pub generation:u64,pub asset_epoch:u64,pub source_frame:i64,pub sample_rate_hz:u32,pub callback_frame:u64}
static CURSOR:crate::rendered_cursor::Cursor=crate::rendered_cursor::Cursor::new();
pub fn editor_main_cursor()->Option<EditorMainCursor>{let(c,(frame,hz))=CURSOR.read_details().and_then(|(c,s)|s.map(|s|(c,s)))?;Some(EditorMainCursor{generation:c.generation,asset_epoch:c.asset_epoch,source_frame:frame,sample_rate_hz:hz,callback_frame:c.rendered_frame})}
#[derive(Clone,Copy,Debug)]pub struct Configuration{pub generation:u64,pub asset_epoch:u64,pub frame:i64,pub rate:f64,pub callback_frame:u64}
struct Control{writer:AtomicBool,sequence:AtomicU64,epoch:AtomicU64,generation:AtomicU64,frame:AtomicU64,rate:AtomicU64,anchor:AtomicU64}
impl Control{
    const fn new()->Self{Self{writer:AtomicBool::new(false),sequence:AtomicU64::new(0),epoch:AtomicU64::new(0),generation:AtomicU64::new(0),frame:AtomicU64::new(0),rate:AtomicU64::new(1f64.to_bits()),anchor:AtomicU64::new(0)}}
    fn available(&self)->bool{let s=self.sequence.load(Ordering::SeqCst);s&1==0&&s.checked_add(2).is_some()&&!self.writer.load(Ordering::Acquire)}
    fn configure(&self,generation:u64,epoch:u64,frame:i64,rate:f64)->bool{
        if !rate.is_finite()||rate<=0.||generation==0||epoch==0{return false;}
        while self.writer.compare_exchange(false,true,Ordering::Acquire,Ordering::Relaxed).is_err(){std::thread::yield_now();}
        let sequence=self.sequence.load(Ordering::SeqCst);let Some(next)=sequence.checked_add(2).filter(|_|sequence&1==0)else{self.writer.store(false,Ordering::Release);return false;};
        self.sequence.store(sequence+1,Ordering::SeqCst);
        #[cfg(test)]if std::ptr::eq(self,&CONTROL){control_test_gate::hold();}
        self.generation.store(generation,Ordering::SeqCst);self.epoch.store(epoch,Ordering::SeqCst);self.frame.store(frame as u64,Ordering::SeqCst);self.rate.store(rate.to_bits(),Ordering::SeqCst);self.anchor.store(crate::music_total_frames(),Ordering::SeqCst);self.sequence.store(next,Ordering::SeqCst);self.writer.store(false,Ordering::Release);true
    }
    fn read(&self)->Option<Configuration>{for _ in 0..2{let s=self.sequence.load(Ordering::SeqCst);if s==0||s&1!=0{continue;}let c=Configuration{generation:self.generation.load(Ordering::SeqCst),asset_epoch:self.epoch.load(Ordering::SeqCst),frame:self.frame.load(Ordering::SeqCst)as i64,rate:f64::from_bits(self.rate.load(Ordering::SeqCst)),callback_frame:self.anchor.load(Ordering::SeqCst)};if self.sequence.load(Ordering::SeqCst)==s{return Some(c);}}None}
}
static CONTROL:Control=Control::new();
/// Control preflight before any transport mutation. AudioControl is !Sync and
/// its single UI owner serializes configuration with this final publication.
pub fn editor_main_can_configure()->bool{CONTROL.available()}
/// Control state only, not a callback, PCM, decoder or lease acknowledgement.
pub fn configure_editor_main(generation:u64,epoch:u64,frame:i64,rate:f64)->bool{CONTROL.configure(generation,epoch,frame,rate)}
pub fn editor_main_configuration()->Option<Configuration>{CONTROL.read()}
/// Reopen only the same frozen authority after its new coherent origin exists.
pub fn thaw_editor_main(epoch:u64){crate::rendered_cursor::thaw_music_render(epoch);}
pub(crate)fn publish(c:Configuration,frame:i64,callback_frame:u64,hz:u32){CURSOR.publish_details(crate::RenderedMusicCursor{generation:c.generation,asset_epoch:c.asset_epoch,next_music_seconds:frame as f64/f64::from(hz),rendered_frame:callback_frame},Some((frame,hz)));}
pub(crate)fn rate_one(rate:f64)->bool{rate.to_bits().abs_diff(1f64.to_bits())<=1}
#[derive(Clone,Copy)]pub(crate)struct Callback{pub control:Configuration,pub initial:i64,pub total_before:u64,pub frames:usize,pub consumed:bool}
impl EditorMainCursor{
    /// Explicit DSP receipt conversion only. A HostNOW Start is a different
    /// caller-selected origin and must not be overwritten by a frozen receipt.
    pub fn source_frame_at_rate(self,hz:u32)->Option<i64>{if self.sample_rate_hz==0||hz==0{return None;}i64::try_from(i128::from(self.source_frame)*i128::from(hz)/i128::from(self.sample_rate_hz)).ok()}
}
#[cfg(test)]mod tests{use super::*;#[test]fn explicit_receipt_conversion_preserves_same_rate_and_signed_rational_phase(){let c=EditorMainCursor{generation:1,asset_epoch:2,source_frame:-32769,sample_rate_hz:44100,callback_frame:10};assert_eq!(c.source_frame_at_rate(44100),Some(-32769));assert_eq!(c.source_frame_at_rate(48000),Some((-32769i128*48000/44100)as i64));assert_eq!(c.source_frame_at_rate(0),None);let c=EditorMainCursor{source_frame:i64::MAX,sample_rate_hz:1,..c};assert_eq!(c.source_frame_at_rate(u32::MAX),None);}}

#[cfg(test)]mod authority_tests{use super::*;#[test]fn invalid_or_exhausted_origin_is_atomic_and_unlocks_without_global_poison(){let c=Control::new();assert!(c.configure(1,2,3,0.3));let before=c.read().unwrap();for rate in [f64::NAN,0.,-1.]{assert!(!c.configure(4,5,6,rate));assert_eq!(c.read().unwrap().frame,before.frame);assert!(!c.writer.load(Ordering::Acquire));}c.sequence.store(u64::MAX-1,Ordering::SeqCst);assert!(!c.available());assert!(!c.configure(4,5,6,1.35));assert_eq!(c.read().unwrap().frame,before.frame);assert_eq!(c.read().unwrap().generation,before.generation);assert_eq!(c.read().unwrap().rate.to_bits(),before.rate.to_bits());assert!(!c.writer.load(Ordering::Acquire));}}

#[cfg(test)]pub(crate)mod control_test_gate{use std::sync::{Arc,Barrier,Mutex};static GATE:Mutex<Option<(Arc<Barrier>,Arc<Barrier>)>>=Mutex::new(None);pub(crate)fn install(a:Arc<Barrier>,b:Arc<Barrier>){*GATE.lock().unwrap()=Some((a,b));}pub(crate)fn hold(){let gate=GATE.lock().unwrap().take();if let Some((a,b))=gate{a.wait();b.wait();}}
}
