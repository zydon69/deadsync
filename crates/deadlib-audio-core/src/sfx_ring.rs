use crate::{QueuedSfx,SfxSchedule};
use std::cell::{Cell,RefCell};
use std::sync::atomic::{AtomicU64,Ordering};
static NEXT_SCHEDULE_TRANSPORT:AtomicU64=AtomicU64::new(1);

use rtrb::{Consumer, Producer, PushError, RingBuffer};

/// Sole-producer endpoint for the bounded sound-effect command queue.
///
/// Applications with multiple producer threads must serialize access outside
/// this realtime core. The audio callback never touches the producer.
pub struct SfxSender {
    producer: Producer<QueuedSfx>,
    retired:RefCell<Consumer<SfxSchedule>>,
    credits:Cell<usize>,
    schedule_capacity:usize,
    identity:u64,
}

/// Audio-callback endpoint for the bounded sound-effect command queue.
///
/// The callback is the sole consumer. Queue storage is allocated once during
/// backend startup, lives for the output-stream session, and is destroyed only
/// after the backend stops. A miss is one bounded atomic pop; a full queue
/// rejects the newest command on the producer thread. There is no callback-time
/// allocation, lock, wait, scan, eviction, or destruction of queue storage.
pub struct SfxReceiver {
    consumer: Consumer<QueuedSfx>,
    retirement:Option<SfxScheduleRetirer>,
}

pub struct SfxDrain<'a> {
    consumer: &'a mut Consumer<QueuedSfx>,
}

impl SfxSender {
    /// Enqueue one effect, returning it unchanged when the fixed queue is full.
    /// Reclamation is control-thread-only. One accepted scheduled message uses
    /// one credit until its owned snapshot returns from the render consumer.
    pub fn reclaim_schedules(&self){
        let mut retired=self.retired.borrow_mut();
        for _ in 0..self.schedule_capacity{let Ok(schedule)=retired.pop()else{break;};if schedule.credit()==Some(self.identity){self.credits.set(self.credits.get().checked_add(1).filter(|n|*n<=self.schedule_capacity).expect("SFX schedule credit conservation"));}drop(schedule);}
    }
    pub fn try_send(&mut self,mut queued:QueuedSfx)->Result<(),QueuedSfx>{
        self.reclaim_schedules();
        if self.producer.is_abandoned(){return Err(queued);}
        let scheduled=queued.schedule.is_some();
        if scheduled&&self.credits.get()==0{return Err(queued);}
        if let Some(schedule)=queued.schedule.as_mut(){schedule.set_credit(Some(self.identity));}
        match self.producer.push(queued){Ok(())=>{if scheduled{self.credits.set(self.credits.get()-1);}Ok(())},Err(PushError::Full(mut queued))=>{if let Some(schedule)=queued.schedule.as_mut(){schedule.set_credit(None);}Err(queued)}}
    }

}

pub struct SfxScheduleRetirer{producer:Producer<SfxSchedule>,identity:u64}
impl SfxScheduleRetirer{
    pub(crate)fn retire(&mut self,schedule:SfxSchedule)->Result<(),SfxSchedule>{
        if schedule.credit().is_some_and(|credit|credit!=self.identity){return Err(schedule);}
        self.producer.push(schedule).map_err(|PushError::Full(schedule)|schedule)
    }
}
impl SfxReceiver {
    /// Transfer before starting callbacks; the receiver remains solely the
    /// ready command consumer. Retirement storage survives either endpoint.
    pub fn take_retirement(&mut self)->Option<SfxScheduleRetirer>{self.retirement.take()}

    /// Drain the commands available at callback entry in FIFO order.
    #[inline(always)]
    pub const fn try_iter(&mut self) -> SfxDrain<'_> {
        SfxDrain {
            consumer: &mut self.consumer,
        }
    }
}

impl Iterator for SfxDrain<'_> {
    type Item = QueuedSfx;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.consumer.pop().ok()
    }
}

/// Create a fixed-capacity, session-lifetime SFX command transport.
#[must_use]
/// # Panics
///
/// Panics if the function's input or state invariants are violated.
pub fn sfx_transport(capacity: usize) -> (SfxSender, SfxReceiver) {
    assert!(capacity > 0, "SFX queue capacity must be nonzero");
    let schedule_capacity=capacity.checked_add(crate::MAX_ACTIVE_SFX).and_then(|n|n.checked_add(1)).expect("SFX schedule capacity overflow");
    let identity=NEXT_SCHEDULE_TRANSPORT.fetch_update(Ordering::AcqRel,Ordering::Acquire,|n|n.checked_add(1)).expect("SFX transport authority exhausted");
    let (producer, consumer) = RingBuffer::new(capacity);
    let (returner,retired)=RingBuffer::new(schedule_capacity);
    (SfxSender{producer,retired:RefCell::new(retired),credits:Cell::new(schedule_capacity),schedule_capacity,identity},SfxReceiver{consumer,retirement:Some(SfxScheduleRetirer{producer:returner,identity})})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MixBus;
    use std::sync::Arc;

    fn queued(frame: u64) -> QueuedSfx {
        QueuedSfx {
            data: Arc::from([frame as i16]),
            bus: MixBus::new(0),
            generation: frame + 1,
            restart_slot: None,
            pan: crate::SfxPan::Center,
            target_stream_frame: crate::mixer::SfxOnset::At(frame), schedule: None,
            voice: None,
        }
    }

    #[test]
    fn sfx_transport_preserves_fifo_and_reuses_capacity() {
        let (mut sender, mut receiver) = sfx_transport(3);
        for frame in 0..3 {
            assert!(sender.try_send(queued(frame)).is_ok());
        }
        let rejected = sender.try_send(queued(3)).unwrap_err();
        assert_eq!(rejected.target_stream_frame, crate::mixer::SfxOnset::At(3));

        let first: Vec<_> = receiver
            .try_iter()
            .map(|queued| match queued.target_stream_frame { crate::mixer::SfxOnset::At(frame) => frame, crate::mixer::SfxOnset::Immediate => panic!("expected scheduled onset") })
            .collect();
        assert_eq!(first, [0, 1, 2]);

        assert!(sender.try_send(queued(4)).is_ok());
        assert_eq!(
            receiver
                .try_iter()
                .next()
                .map(|queued| match queued.target_stream_frame { crate::mixer::SfxOnset::At(frame) => frame, crate::mixer::SfxOnset::Immediate => panic!("expected scheduled onset") }),
            Some(4)
        );
        assert!(receiver.try_iter().next().is_none());
    }
}
#[cfg(test)]mod schedule_retirement_tests{
 use super::*;use crate::*;use std::sync::{Arc,Weak};
 fn message(controls:&MixControls)->(QueuedSfx,Weak<[i64]>){
  let samples:Arc<[i64]>=(0..100_000i64).collect::<Vec<_>>().into();let weak=Arc::downgrade(&samples);
  let schedule=SfxSchedule::with_source_main(samples,Arc::new(SfxScheduleVoice::new()),1,1).unwrap();let bus=MixBus::new(7);
  (QueuedSfx{data:Arc::from([123i16,123]),bus,generation:controls.bus_generation(bus),restart_slot:Some(0),pan:SfxPan::Center,target_stream_frame:SfxOnset::Immediate,schedule:Some(schedule),voice:None},weak)
 }
 fn callback(render:&mut RenderState,receiver:&mut SfxReceiver){let mut out=[0f32;8];render.render(OutputBufferMut::F32(&mut out),CallbackInfo{anchor_nanos:0,clock:CallbackClockSource::Instant},receiver.try_iter());}
 #[test]fn scheduled_window_replacement_and_stale_discard_return_before_final_drop(){
  let _guard=crate::render::test_global_audio_guard();
  let(_stream,handle)=music_transport(48000,2);let controls=Arc::new(MixControls::new());let(mut sender,mut receiver)=sfx_transport(3);let mut render=RenderState::new(handle,controls.clone(),2);render.attach_sfx_retirement(receiver.take_retirement());
  let(a,wa)=message(&controls);let(b,wb)=message(&controls);let(mut stale,ws)=message(&controls);stale.generation=stale.generation.checked_add(100).unwrap();assert!(sender.try_send(a).is_ok());assert!(sender.try_send(b).is_ok());assert!(sender.try_send(stale).is_ok());callback(&mut render,&mut receiver);
  assert!(wa.upgrade().is_some());assert!(wb.upgrade().is_some());assert!(ws.upgrade().is_some());sender.reclaim_schedules();assert!(wa.upgrade().is_none());assert!(ws.upgrade().is_none());assert!(wb.upgrade().is_some());controls.stop_bus(MixBus::new(7));callback(&mut render,&mut receiver);assert!(wb.upgrade().is_some());sender.reclaim_schedules();assert!(wb.upgrade().is_none());assert_eq!(sender.credits.get(),sender.schedule_capacity);
 }
 #[test]fn no_callback_queue_saturation_is_bounded_and_rejection_stays_on_producer(){
  let controls=MixControls::new();let(mut sender,receiver)=sfx_transport(2);let mut retained=Vec::new();for _ in 0..2{let(q,w)=message(&controls);assert!(sender.try_send(q).is_ok());retained.push(w);}
  for _ in 0..128{let(q,w)=message(&controls);let q=sender.try_send(q).unwrap_err();assert!(w.upgrade().is_some());drop(q);assert!(w.upgrade().is_none());}
  assert_eq!(sender.credits.get(),sender.schedule_capacity-2);assert!(retained.iter().all(|w|w.upgrade().is_some()));drop(receiver);assert!(retained.iter().all(|w|w.upgrade().is_some()));drop(sender);assert!(retained.iter().all(|w|w.upgrade().is_none()));
 }
 #[test]fn producer_closed_first_keeps_retired_window_until_off_callback_renderer_drop(){
  let _guard=crate::render::test_global_audio_guard();
  let(_stream,handle)=music_transport(48000,2);let controls=Arc::new(MixControls::new());let(mut sender,mut receiver)=sfx_transport(2);let mut render=RenderState::new(handle,controls.clone(),2);render.attach_sfx_retirement(receiver.take_retirement());let(q,w)=message(&controls);assert!(sender.try_send(q).is_ok());drop(sender);callback(&mut render,&mut receiver);assert!(w.upgrade().is_some());drop(receiver);assert!(w.upgrade().is_some());drop(render);assert!(w.upgrade().is_none());
 }
 #[test]fn consumer_closed_first_rejects_new_publications_and_drops_on_control_teardown(){
  let _guard=crate::render::test_global_audio_guard();
  let(_stream,handle)=music_transport(48000,2);let controls=Arc::new(MixControls::new());let(mut sender,mut receiver)=sfx_transport(2);let mut render=RenderState::new(handle,controls.clone(),2);render.attach_sfx_retirement(receiver.take_retirement());let(q,w)=message(&controls);assert!(sender.try_send(q).is_ok());drop(render);drop(receiver);assert!(w.upgrade().is_some());let(q,rejected)=message(&controls);drop(sender.try_send(q).unwrap_err());assert!(rejected.upgrade().is_none());drop(sender);assert!(w.upgrade().is_none());
 }
 #[test]fn all_schedule_credits_return_without_overflow_and_are_refunded_only_on_control(){
  let controls=MixControls::new();let(mut sender,mut receiver)=sfx_transport(1);let mut retirer=receiver.take_retirement().unwrap();let capacity=sender.schedule_capacity;let mut held=Vec::new();
  for _ in 0..capacity{let(q,_)=message(&controls);assert!(sender.try_send(q).is_ok());held.push(receiver.try_iter().next().unwrap().schedule.unwrap());}
  assert_eq!(sender.credits.get(),0);let(q,w)=message(&controls);drop(sender.try_send(q).unwrap_err());assert!(w.upgrade().is_none());
  for schedule in held{assert!(retirer.retire(schedule).is_ok());}assert_eq!(sender.credits.get(),0);sender.reclaim_schedules();assert_eq!(sender.credits.get(),capacity);
 }

}

#[cfg(test)]mod float_retirement_tests{use super::*;use crate::*;use std::sync::Arc;
 #[test]fn retained_float_clip_final_destruction_is_not_on_callback_after_producer_closes(){
  let _guard=crate::render::test_global_audio_guard();
  let(_stream,handle)=music_transport(48000,2);let controls=Arc::new(MixControls::new());let(mut sender,mut receiver)=sfx_transport(1);let mut render=RenderState::new(handle,controls.clone(),2);render.attach_sfx_retirement(receiver.take_retirement());
  let pcm:Arc<[f32]>=vec![0.125;200000].into();let weak=Arc::downgrade(&pcm);let raw:Arc<[i16]>=vec![123;200000].into();let raw_weak=Arc::downgrade(&raw);let mut schedule=SfxSchedule::with_source_main_float(vec![0],Arc::new(SfxScheduleVoice::new()),1,1,pcm,2).unwrap();schedule.retain_pcm_storage(raw.clone());assert!(sender.try_send(QueuedSfx{data:raw,bus:MixBus::new(4),generation:99,restart_slot:Some(0),pan:SfxPan::Center,target_stream_frame:SfxOnset::Immediate,schedule:Some(schedule),voice:None}).is_ok());drop(sender);let mut output=[0.;4];render.render(OutputBufferMut::F32(&mut output),CallbackInfo{anchor_nanos:0,clock:CallbackClockSource::Instant},receiver.try_iter());assert!(weak.upgrade().is_some());assert!(raw_weak.upgrade().is_some());drop(receiver);assert!(weak.upgrade().is_some());assert!(raw_weak.upgrade().is_some());drop(render);assert!(weak.upgrade().is_none());assert!(raw_weak.upgrade().is_none());
 }

 #[test]fn unconsumed_float_clips_survive_consumer_first_teardown_until_control_drops_transport(){
  let _guard=crate::render::test_global_audio_guard();
  let(_stream,handle)=music_transport(48000,2);let controls=Arc::new(MixControls::new());let(mut sender,mut receiver)=sfx_transport(1);let mut render=RenderState::new(handle,controls.clone(),2);render.attach_sfx_retirement(receiver.take_retirement());let pcm:Arc<[f32]>=vec![0.125;200000].into();let weak=Arc::downgrade(&pcm);let raw:Arc<[i16]>=vec![123;200000].into();let raw_weak=Arc::downgrade(&raw);let mut schedule=SfxSchedule::with_source_main_float(vec![0],Arc::new(SfxScheduleVoice::new()),1,1,pcm,2).unwrap();schedule.retain_pcm_storage(raw.clone());assert!(sender.try_send(QueuedSfx{data:raw,bus:MixBus::new(4),generation:0,restart_slot:Some(0),pan:SfxPan::Center,target_stream_frame:SfxOnset::Immediate,schedule:Some(schedule),voice:None}).is_ok());drop(render);drop(receiver);assert!(weak.upgrade().is_some());assert!(raw_weak.upgrade().is_some());drop(sender);assert!(weak.upgrade().is_none());assert!(raw_weak.upgrade().is_none());
 }
}
