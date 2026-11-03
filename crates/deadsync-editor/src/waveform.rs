//! Multiresolution waveform data, independent of the decoder and renderer.
//!
//! Like GrooveAuthor's SoundMipMap, nodes pack a 9-bit minimum, 9-bit maximum
//! and 14-bit mean sample-to-sample distance. Channels never mix. A range query
//! decomposes its interval into aligned powers of two rather than scanning PCM.
use crate::Error;
use std::sync::atomic::{AtomicBool, Ordering};

const MAX: u16 = 511;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Node(u32);
impl Node {
    fn new(min: u16, max: u16, distance: u16) -> Self {
        Self(u32::from(min) | (u32::from(max) << 9) | (u32::from(distance) << 18))
    }
    fn min(self) -> u16 {
        (self.0 & 511) as u16
    }
    fn max(self) -> u16 {
        ((self.0 >> 9) & 511) as u16
    }
    fn distance(self) -> u16 {
        (self.0 >> 18) as u16
    }
    fn merge(self, other: Self) -> Self {
        Self::new(
            self.min().min(other.min()),
            self.max().max(other.max()),
            ((u32::from(self.distance()) + u32::from(other.distance()) + 1) / 2) as u16,
        )
    }
}

/// Quantized envelope and mean variation in [0, 511] horizontal pixel units.
/// Distance can reach 1022 when adjacent samples jump between full-scale poles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Envelope {
    pub min: u16,
    pub max: u16,
    pub mean_distance: f64,
}

/// Incremental interleaved PCM input. Packet boundaries must fall between frames.
/// The builder retains packed samples, not the decoder's uncompressed PCM buffers.
pub struct Builder {
    channels: usize,
    rate: u32,
    previous: Vec<f32>,
    samples: Vec<Node>,
    max_bytes: usize,
}
impl Builder {
    pub fn new(channels: usize, sample_rate: u32) -> Result<Self, Error> {
        Self::with_memory_limit(channels, sample_rate, usize::MAX)
    }
    /// Budget for packed sample allocations, including all pyramid levels.
    /// Decoder buffers and small vector headers are outside this budget.
    pub fn with_memory_limit(
        channels: usize,
        sample_rate: u32,
        max_bytes: usize,
    ) -> Result<Self, Error> {
        if channels == 0 || channels > 32 || sample_rate == 0 {
            return Err(Error::InvalidEdit(
                "Invalid waveform channel count or sample rate".into(),
            ));
        }
        Ok(Self {
            channels,
            rate: sample_rate,
            previous: vec![0.5; channels],
            samples: Vec::new(),
            max_bytes,
        })
    }
    pub fn push_i16(&mut self,pcm:&[i16])->Result<(),Error>{self.push_values(pcm.len(),pcm.iter().map(|sample|f32::from(*sample)/32768.))}
    /// Consume the shared immutable editor PCM without an i16 round trip.
    pub fn push_f32(&mut self,pcm:&[f32])->Result<(),Error>{
        if pcm.iter().any(|sample|!sample.is_finite()){return Err(Error::InvalidFormat("Nonfinite waveform sample".into()));}
        self.push_values(pcm.len(),pcm.iter().copied())
    }
    fn push_values(&mut self,len:usize,pcm:impl Iterator<Item=f32>)->Result<(),Error>{
        if len % self.channels != 0 {
            return Err(Error::InvalidFormat(
                "Incomplete interleaved waveform frame".into(),
            ));
        }
        let wanted = self
            .samples
            .len()
            .checked_add(len)
            .ok_or_else(memory_limit)?;
        if pyramid_bytes(wanted / self.channels, self.channels)
            .is_none_or(|bytes| bytes > self.max_bytes)
        {
            return Err(memory_limit());
        }
        if wanted > self.samples.capacity() {
            let grown = self.samples.capacity().saturating_mul(2).max(wanted);
            let capacity = if pyramid_bytes(grown.div_ceil(self.channels), self.channels)
                .is_some_and(|bytes| bytes <= self.max_bytes)
            {
                grown
            } else {
                wanted
            };
            self.samples
                .try_reserve_exact(capacity - self.samples.len())
                .map_err(|e| Error::InvalidEdit(e.to_string()))?;
        }
        for (index,value) in pcm.enumerate(){
            let channel=index%self.channels;
            let x=(f32::from(MAX)*(value+1.)*0.5)as u16;
            let distance=((value-self.previous[channel]).abs()*f32::from(MAX))as u16;
            self.samples.push(Node::new(x.min(MAX),x.min(MAX),distance));
            self.previous[channel]=value;
        }
        Ok(())
    }
    /// Build independent power-of-two-aligned mip ranges on a bounded CPU pool.
    /// No arithmetic is reassociated: every node merges the same two children.
    pub fn finish_parallelism_cancellable(self,cancelled:&AtomicBool,parallelism:i32)->Result<Option<Waveform>,Error>{
        self.finish_parallelism_observed(cancelled,parallelism,None)
    }
    fn finish_parallelism_observed(self,cancelled:&AtomicBool,parallelism:i32,observer:Option<&(dyn Fn()+Sync)>)->Result<Option<Waveform>,Error>{
        let frames=self.samples.len()/self.channels;
        let mut chosen=source_task_span(frames,parallelism);
        if chosen>=frames || cancelled.load(Ordering::Relaxed){return self.finish_cancellable(cancelled);}
        // Source uses Tasks, not an OS thread per task. Bound both the worker
        // pool and queued task metadata independently of a raw imported value.
        while frames.div_ceil(chosen)>4096 {chosen=chosen.checked_mul(2).unwrap_or(usize::MAX);}
        let depth=chosen.ilog2() as usize;
        let channels=self.channels;let rate=self.rate;
        let mut levels=vec![self.samples];let mut count=frames;
        while count>1 {
            count=count.div_ceil(2);let mut level=Vec::new();
            level.try_reserve_exact(count*channels).map_err(|e|Error::InvalidEdit(e.to_string()))?;
            level.resize(count*channels,Node(0));levels.push(level);
        }
        let tasks=frames.div_ceil(chosen);
        let workers=tasks.min(std::thread::available_parallelism().map_or(1,usize::from)).max(1);
        {
            struct Task<'a>{previous:&'a[Node],levels:Vec<&'a mut[Node]>}
            let (base,upper)=levels.split_first_mut().unwrap();
            let mut jobs:Vec<_>=base.chunks(chosen*channels).map(|previous|Task{previous,levels:Vec::with_capacity(depth)}).collect();
            for (level,output) in upper.iter_mut().take(depth).enumerate(){
                let width=(chosen>>(level+1))*channels;
                for (job,part) in jobs.iter_mut().zip(output.chunks_mut(width)){job.levels.push(part);}
            }
            let jobs=std::sync::Mutex::new(std::collections::VecDeque::from(jobs));
            std::thread::scope(|scope|->Result<(),Error>{
                let mut handles=Vec::with_capacity(workers);
                for _ in 0..workers {
                    let jobs=&jobs;
                    handles.push(std::thread::Builder::new().name("editor-waveform-mip".into()).spawn_scoped(scope,move||{
                        if let Some(observer)=observer{observer();}
                        loop {
                            if cancelled.load(Ordering::Relaxed){break;}
                            let Some(task)=jobs.lock().unwrap().pop_front()else{break;};
                            let mut previous=task.previous;
                            for output in task.levels {
                                fill_level(previous,output,channels,cancelled);
                                previous=output;
                            }
                        }
                    }).map_err(|e|Error::InvalidEdit(e.to_string()))?);
                }
                for handle in handles{handle.join().map_err(|_|Error::InvalidEdit("Waveform worker stopped".into()))?;}
                Ok(())
            })?;
        }
        if cancelled.load(Ordering::Relaxed){return Ok(None);}
        for index in depth+1..levels.len(){let (previous,next)=levels.split_at_mut(index);fill_level(previous.last().unwrap(),&mut next[0],channels,cancelled);}
        if cancelled.load(Ordering::Relaxed){return Ok(None);}
        Ok(Some(Waveform{channels,sample_rate:rate,frames,levels}))
    }
    pub fn finish(self) -> Result<Waveform, Error> {
        self.finish_cancellable(&AtomicBool::new(false))?
            .ok_or_else(|| Error::InvalidEdit("Waveform load cancelled".into()))
    }
    /// Cancellation discards all unpublished data. No partial waveform escapes.
    pub fn finish_cancellable(self, cancelled: &AtomicBool) -> Result<Option<Waveform>, Error> {
        if cancelled.load(Ordering::Relaxed) {
            return Ok(None);
        }
        let frames = self.samples.len() / self.channels;
        let mut levels = vec![self.samples];
        let mut count = frames;
        while count > 1 {
            let previous = levels.last().unwrap();
            let next_count = count.div_ceil(2);
            let mut next = Vec::new();
            next.try_reserve_exact(next_count * self.channels)
                .map_err(|e| Error::InvalidEdit(e.to_string()))?;
            for frame in 0..next_count {
                if frame % 4096 == 0 && cancelled.load(Ordering::Relaxed) {
                    return Ok(None);
                }
                for channel in 0..self.channels {
                    let first = previous[frame * 2 * self.channels + channel];
                    let second = (frame * 2 + 1) * self.channels + channel;
                    // A trailing singleton keeps its actual range and distance.
                    next.push(
                        previous
                            .get(second)
                            .map_or(first, |&other| first.merge(other)),
                    );
                }
            }
            levels.push(next);
            count = next_count;
        }
        if cancelled.load(Ordering::Relaxed) {
            return Ok(None);
        }
        Ok(Some(Waveform {
            channels: self.channels,
            sample_rate: self.rate,
            frames,
            levels,
        }))
    }
}

fn source_task_span(frames:usize,parallelism:i32)->usize{
    let mut chunk=1usize;let mut previous=0usize;let mut chosen=frames.max(1);
    while chunk<frames {
        let count=frames.div_ceil(chunk);
        if parallelism>0 && previous>parallelism as usize && count<=parallelism as usize{chosen=chunk;}
        previous=count;chunk=chunk.checked_mul(2).unwrap_or(usize::MAX);
    }
    chosen
}

fn fill_level(previous:&[Node],output:&mut[Node],channels:usize,cancelled:&AtomicBool){
    for (frame,row) in output.chunks_mut(channels).enumerate(){
        if frame%4096==0 && cancelled.load(Ordering::Relaxed){return;}
        for (channel,node) in row.iter_mut().enumerate(){let first=previous[frame*2*channels+channel];let second=(frame*2+1)*channels+channel;*node=previous.get(second).map_or(first,|&other|first.merge(other));}
    }
}

pub struct Waveform {
    channels: usize,
    sample_rate: u32,
    frames: usize,
    levels: Vec<Vec<Node>>,
}
impl Waveform {
    pub fn channels(&self) -> usize {
        self.channels
    }
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    pub fn frames(&self) -> usize {
        self.frames
    }
    pub fn duration_seconds(&self) -> f64 {
        self.frames as f64 / f64::from(self.sample_rate)
    }
    /// Half-open frame interval. Empty/outside intervals return None. Partial
    /// intervals clamp to the source. Negative-time silence belongs to the view.
    pub fn envelope(&self, channel: usize, start: usize, end: usize) -> Option<Envelope> {
        let end = end.min(self.frames);
        if channel >= self.channels || start >= end {
            return None;
        }
        let mut cursor = start;
        let mut min = MAX;
        let mut max = 0;
        let mut distance = 0.0;
        while cursor < end {
            let remaining = end - cursor;
            let fit = (usize::BITS - 1 - remaining.leading_zeros()) as usize;
            let alignment = if cursor == 0 {
                fit
            } else {
                cursor.trailing_zeros() as usize
            };
            let level = fit.min(alignment);
            let width = 1usize << level;
            let node = self.levels[level][(cursor >> level) * self.channels + channel];
            min = min.min(node.min());
            max = max.max(node.max());
            distance += f64::from(node.distance()) * width as f64;
            cursor += width;
        }
        Some(Envelope {
            min,
            max,
            mean_distance: distance / (end - start) as f64,
        })
    }
}

fn memory_limit() -> Error {
    Error::InvalidEdit("Waveform exceeds its packed-data memory budget".into())
}
fn pyramid_bytes(mut frames: usize, channels: usize) -> Option<usize> {
    let mut nodes = frames;
    while frames > 1 {
        frames = frames.div_ceil(2);
        nodes = nodes.checked_add(frames)?;
    }
    nodes
        .checked_mul(channels)?
        .checked_mul(std::mem::size_of::<Node>())
}

#[cfg(test)]
mod parallel_tests {
    use super::*;
    fn builder(frames:usize,channels:usize)->Builder{
        let pcm=(0..frames*channels).map(|index|((index.wrapping_mul(7919)%65536)as i32-32768)as i16).collect::<Vec<_>>();
        let mut builder=Builder::new(channels,48000).unwrap();builder.push_i16(&pcm).unwrap();builder
    }
    #[test]
    fn source_task_partition_matches_executed_original_sound_mipmap_method(){
        let rows:&[(usize,i32,usize,usize)]=include!("../tests/fixtures/waveform_parallelism_source.expected.inc");
        for &(frames,parallelism,span,tasks)in rows{assert_eq!(source_task_span(frames,parallelism),span);assert_eq!(frames.div_ceil(span),tasks);}
    }
    #[test]
    fn real_parallel_mip_nodes_are_bit_identical_including_partial_tails_and_negative_raw_settings(){
        for channels in [1,2,6] {for frames in [3,5,257,4097,10003]{
            let reference=builder(frames,channels).finish().unwrap();
            for parallelism in [-3,0,1,2,8,128,i32::MAX] {
                let result=builder(frames,channels).finish_parallelism_cancellable(&AtomicBool::new(false),parallelism).unwrap().unwrap();
                assert_eq!(result.levels,reference.levels,"frames={frames} channels={channels} parallelism={parallelism}");
                for channel in 0..channels{assert_eq!(result.envelope(channel,1,frames),reference.envelope(channel,1,frames));}
            }
        }}
    }
    #[test]
    fn cancellation_while_real_pool_is_entered_joins_workers_and_discards_every_level(){
        use std::sync::{Arc,Condvar,Mutex,mpsc};
        let cancelled=Arc::new(AtomicBool::new(false));let gate=Arc::new((Mutex::new(false),Condvar::new()));let (send,entered)=mpsc::sync_channel(1);
        let worker_cancel=Arc::clone(&cancelled);let worker_gate=Arc::clone(&gate);
        let worker=std::thread::spawn(move||{
            let observer=||{let _=send.try_send(());let(lock,changed)=&*worker_gate;let mut release=lock.lock().unwrap();while !*release{release=changed.wait(release).unwrap();}};
            builder(100003,2).finish_parallelism_observed(&worker_cancel,8,Some(&observer)).unwrap()
        });
        entered.recv_timeout(std::time::Duration::from_secs(5)).unwrap();cancelled.store(true,Ordering::Release);let(lock,changed)=&*gate;*lock.lock().unwrap()=true;changed.notify_all();assert!(worker.join().unwrap().is_none());
    }
}

#[cfg(test)]
mod shared_float_pcm_tests {
    use super::*;
    #[test]
    fn float_builder_keeps_source_sample_bits_and_rejects_nonfinite_atomically(){
        let mut builder=Builder::with_memory_limit(2,48000,1<<20).unwrap();
        let pcm=[f32::from_bits(0x3f000001),-1.0000305];builder.push_f32(&pcm).unwrap();
        assert_eq!(builder.previous.iter().map(|v|v.to_bits()).collect::<Vec<_>>(),pcm.map(f32::to_bits));
        let before=builder.samples.clone();let previous=builder.previous.clone();assert!(builder.push_f32(&[0.5,f32::NAN]).is_err());assert_eq!(builder.samples,before);assert_eq!(builder.previous,previous);assert!(builder.push_f32(&[0.]).is_err());assert_eq!(builder.samples,before);
        assert_eq!(builder.finish().unwrap().frames(),1);
    }
}
