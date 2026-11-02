//! Owned SPSC preview descriptor publication and off-callback retirement.
use rtrb::{Consumer,Producer,PushError,RingBuffer};
use std::sync::Arc;
pub(crate) const COMMAND_CAPACITY:usize=2;
pub struct PreviewSource {
    pub editor_main:bool,
    pub samples:Arc<[f32]>,
    pub channels:usize,
    pub sample_rate_hz:u32,
    pub generation:u64,
    pub asset_epoch:u64,
    pub initial_frame:i64,
    pub looping:bool,
}
impl std::fmt::Debug for PreviewSource{
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{f.debug_struct("PreviewSource").field("frames",&(self.samples.len()/self.channels.max(1))).field("generation",&self.generation).field("asset_epoch",&self.asset_epoch).finish()}
}
pub(crate) struct PreviewPublisher{ready:Producer<PreviewSource>,retired:Consumer<PreviewSource>}
pub(crate) struct PreviewConsumer{ready:Consumer<PreviewSource>,retired:Producer<PreviewSource>}
pub(crate) fn channel()->(PreviewPublisher,PreviewConsumer){
    let(ready,reader)=RingBuffer::new(COMMAND_CAPACITY);
    let(retired,reclaimer)=RingBuffer::new(COMMAND_CAPACITY+1);
    (PreviewPublisher{ready,retired:reclaimer},PreviewConsumer{ready:reader,retired})
}
impl PreviewPublisher{
    pub(crate) fn reclaim(&mut self){for _ in 0..COMMAND_CAPACITY+1{if self.retired.pop().is_err(){break;}}}
    /// Producer owns failure and the descriptor; backpressure never drops it
    /// in the callback or waits for audio there.
    pub(crate) fn publish(&mut self,source:PreviewSource)->Result<(),PreviewSource>{self.reclaim();self.ready.push(source).map_err(|PushError::Full(value)|value)}
}
impl PreviewConsumer{
    pub(crate) fn pop(&mut self)->Option<PreviewSource>{self.ready.pop().ok()}
    pub(crate) fn retire(&mut self,value:PreviewSource)->Result<(),PreviewSource>{self.retired.push(value).map_err(|PushError::Full(value)|value)}
}

