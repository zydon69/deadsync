//! Background waveform decoding. No audio device or playback stream is opened.
use deadlib_audio_decode::{DecodeOptions, prepared::PreparedAudioSource};
use deadsync_editor::waveform::{Builder, Waveform};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicI32, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
    thread,
};

/// One worker per editor loader, including workers still cancelling. Replacing
/// a song cannot accumulate concurrent full-song waveform allocations.
pub struct Loader {
    busy: Arc<AtomicBool>,
    max_bytes: usize,
    parallelism: Arc<AtomicI32>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum LoadError {
    Busy,
    Cancelled,
    Decode(String),
    WorkerStopped,
}
impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy => f.write_str("A previous waveform worker is still stopping"),
            Self::Cancelled => f.write_str("Waveform loading cancelled"),
            Self::Decode(message) => f.write_str(message),
            Self::WorkerStopped => f.write_str("Waveform worker stopped unexpectedly"),
        }
    }
}
impl std::error::Error for LoadError {}

pub struct Job {
    cancelled: Arc<AtomicBool>,
    result: Receiver<Result<Waveform, LoadError>>,
    finished: bool,
}
impl Loader {
    pub fn new(max_packed_bytes: usize) -> Self {
        Self {
            busy: Arc::new(AtomicBool::new(false)),
            max_bytes: max_packed_bytes,
            parallelism:Arc::new(AtomicI32::new(deadsync_editor::waveform_preferences::Preferences::default().loading_parallelism())),
        }
    }
    pub fn set_load_parallelism(&self,value:i32){self.parallelism.store(value,Ordering::Release);}
    pub fn start(&self, path: PathBuf) -> Result<Job, LoadError> {
        self.start_input(Input::Path(path))
    }
    pub fn start_prepared(&self, source: Arc<PreparedAudioSource>) -> Result<Job, LoadError> {
        self.start_input(Input::Prepared(source))
    }
    fn start_input(&self, input: Input) -> Result<Job, LoadError> {
        let permit = Permit::acquire(&self.busy)?;
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancelled);
        let budget = self.max_bytes;
        let parallelism=Arc::clone(&self.parallelism);
        let (send, result) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("editor-waveform".into())
            .spawn(move || {
                let value = match input {
                    Input::Path(path) => decode(path, budget, &worker_cancel, &parallelism),
                    Input::Prepared(source) => decode_prepared(source, budget, &worker_cancel, &parallelism),
                };
                // Free the worker slot before publishing completion, so a caller
                // consuming the result may immediately start the next request.
                drop(permit);
                let _ = send.send(value);
            })
            .map_err(|error| LoadError::Decode(error.to_string()))?;
        Ok(Job {
            cancelled,
            result,
            finished: false,
        })
    }
}
enum Input {
    Path(PathBuf),
    Prepared(Arc<PreparedAudioSource>),
}
fn decode_prepared(
    source: Arc<PreparedAudioSource>,
    budget: usize,
    cancelled: &AtomicBool,
    parallelism: &AtomicI32,
) -> Result<Waveform, LoadError> {
    let convert = |error: deadsync_editor::Error| LoadError::Decode(error.to_string());
    let mut builder = Builder::with_memory_limit(source.channels, source.sample_rate_hz, budget)
        .map_err(convert)?;
    if let Some(pcm)=source.editor_samples(){
        for chunk in pcm.chunks(1024*source.channels){
            if cancelled.load(Ordering::Acquire){return Err(LoadError::Cancelled);}
            builder.push_f32(chunk).map_err(convert)?;
        }
        return builder.finish_parallelism_cancellable(cancelled,parallelism.load(Ordering::Acquire)).map_err(convert)?.ok_or(LoadError::Cancelled);
    }
    let mut reader = source.reader();
    let mut packet = Vec::new();
    loop {
        if cancelled.load(Ordering::Acquire) {
            return Err(LoadError::Cancelled);
        }
        if !reader.read_dec_packet_into(&mut packet) {
            break;
        }
        builder.push_i16(&packet).map_err(convert)?;
    }
    builder
        .finish_parallelism_cancellable(cancelled,parallelism.load(Ordering::Acquire))
        .map_err(convert)?
        .ok_or(LoadError::Cancelled)
}
impl Job {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
    /// Consume completion once without blocking the editor update loop.
    pub fn poll(&mut self) -> Option<Result<Waveform, LoadError>> {
        if self.finished {
            return None;
        }
        let value = match self.result.try_recv() {
            Ok(value) => value,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Err(LoadError::WorkerStopped),
        };
        self.finished = true;
        // Even if decoding completed just before cancellation, its result must
        // not become the newly selected song's waveform.
        Some(if self.cancelled.load(Ordering::Relaxed) {
            Err(LoadError::Cancelled)
        } else {
            value
        })
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel();
    }
}
struct Permit(Arc<AtomicBool>);
impl Permit {
    fn acquire(busy: &Arc<AtomicBool>) -> Result<Self, LoadError> {
        busy.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| LoadError::Busy)?;
        Ok(Self(Arc::clone(busy)))
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

fn decode(path:PathBuf,budget:usize,cancelled:&AtomicBool,parallelism:&AtomicI32)->Result<Waveform,LoadError>{
    if cancelled.load(Ordering::Acquire){return Err(LoadError::Cancelled);}
    let source=PreparedAudioSource::load_editor_limited(&path,DecodeOptions::STEP_MANIA_TIMELINE,cancelled,budget).map_err(|error|LoadError::Decode(error.to_string()))?;
    decode_prepared(source,budget,cancelled,parallelism)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_slot_refuses_overlap_and_releases_on_guard_drop() {
        let loader = Loader::new(1024);
        let permit = Permit::acquire(&loader.busy).unwrap();
        assert!(matches!(
            loader.start("unopened.wav".into()),
            Err(LoadError::Busy)
        ));
        drop(permit);
        assert!(Permit::acquire(&loader.busy).is_ok());
    }
    #[test]
    fn cancellation_discards_already_queued_success_and_completion_is_consumed_once() {
        let (send, result) = mpsc::sync_channel(1);
        assert!(
            send.send(Ok(Builder::new(1, 48000).unwrap().finish().unwrap()))
                .is_ok()
        );
        let mut job = Job {
            cancelled: Arc::new(AtomicBool::new(false)),
            result,
            finished: false,
        };
        job.cancel();
        assert!(matches!(job.poll(), Some(Err(LoadError::Cancelled))));
        assert!(job.poll().is_none());
    }
    #[test]
    fn unexpected_worker_disconnect_is_a_single_visible_error() {
        let (send, result) = mpsc::sync_channel(1);
        drop(send);
        let mut job = Job {
            cancelled: Arc::new(AtomicBool::new(false)),
            result,
            finished: false,
        };
        assert!(matches!(job.poll(), Some(Err(LoadError::WorkerStopped))));
        assert!(job.poll().is_none());
    }
    #[test]
    fn prepared_pcm_uses_configured_parallel_builder_after_path_is_unlinked(){
        use std::time::{Duration,Instant};
        let root=tempfile::tempdir().unwrap();let path=root.path().join("parallel.wav");let frames=4801u32;let pcm=(0..frames*2).map(|i|(i.wrapping_mul(7919)as i32%65536-32768)as i16).collect::<Vec<_>>();
        let mut bytes=Vec::new();bytes.extend_from_slice(b"RIFF");bytes.extend_from_slice(&(36+frames*4).to_le_bytes());bytes.extend_from_slice(b"WAVEfmt ");bytes.extend_from_slice(&16u32.to_le_bytes());bytes.extend_from_slice(&1u16.to_le_bytes());bytes.extend_from_slice(&2u16.to_le_bytes());bytes.extend_from_slice(&48000u32.to_le_bytes());bytes.extend_from_slice(&192000u32.to_le_bytes());bytes.extend_from_slice(&4u16.to_le_bytes());bytes.extend_from_slice(&16u16.to_le_bytes());bytes.extend_from_slice(b"data");bytes.extend_from_slice(&(frames*4).to_le_bytes());for sample in &pcm{bytes.extend_from_slice(&sample.to_le_bytes());}std::fs::write(&path,bytes).unwrap();
        let source=PreparedAudioSource::load(&path,DecodeOptions::STEP_MANIA_TIMELINE,&AtomicBool::new(false)).unwrap();std::fs::remove_file(path).unwrap();
        let mut expected=Builder::new(2,48000).unwrap();expected.push_i16(&pcm).unwrap();let expected=expected.finish().unwrap();let loader=Loader::new(16*1024*1024);
        for parallelism in [1,2,8,128]{loader.set_load_parallelism(parallelism);assert_eq!(loader.parallelism.load(Ordering::Acquire),parallelism);let mut job=loader.start_prepared(Arc::clone(&source)).unwrap();let deadline=Instant::now()+Duration::from_secs(5);let actual=loop{if let Some(result)=job.poll(){break result.unwrap();}assert!(Instant::now()<deadline);std::thread::yield_now();};assert_eq!(actual.frames(),frames as usize);for channel in 0..2{for range in [(0,4801),(1,4801),(17,301),(4800,4801)]{assert_eq!(actual.envelope(channel,range.0,range.1),expected.envelope(channel,range.0,range.1));}}}
    }

}
