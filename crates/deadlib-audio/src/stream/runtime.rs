use super::Cut;
use super::{MusicDecodeContext, MusicStream, OutputFormat};
use crate::OutputPlan;
use deadlib_audio_core::{MusicBlockWriter, activate_music_track, stop_music_track};
use deadlib_audio_core::{OutputBackendReady, PlayedMapReader, SfxSender};
use log::{error, info};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

/// Commands for one resolved stream; playback policy belongs to the caller.
#[derive(Debug)]
pub enum StreamCommand {
    /// Editor cache publication only: caller already accepted its owner/load
    /// token and invalidated the previous asset epoch. No timeline reset here.
    ReplaceEditorPrepared {
        source: std::sync::Arc<deadlib_audio_decode::prepared::PreparedAudioSource>,
        path: PathBuf, cut: Cut, looping: bool, rate: f32, preserve_pitch: bool,
        generation: u64, asset_epoch: u64, follow_live_phase: bool, preview_phase: Option<(f64,u64)>,
    },
    ReplaceEditorMainPrepared{source:std::sync::Arc<deadlib_audio_decode::prepared::PreparedAudioSource>,path:PathBuf,generation:u64,asset_epoch:u64},
    ReplaceEditorPreviewPrepared{source:std::sync::Arc<deadlib_audio_decode::prepared::PreparedAudioSource>,path:PathBuf,cut:Cut,looping:bool,generation:u64,asset_epoch:u64,preview_phase:Option<(f64,u64)>},
    EditorPreparedFailed { asset_epoch:u64, generation:u64, message:String },
    /// Source editor music starts its logical clock before asynchronous decode.
    StartMusic{path:Option<PathBuf>,seconds:f64,rate:f32,preserve_pitch:bool,decode_options:deadlib_audio_decode::DecodeOptions,generation:u64,asset_epoch:u64},
    ClearMusicAsset{asset_epoch:u64},
    PauseMusic{generation:u64,asset_epoch:u64},
    ResumeMusic{seconds:f64,rate:f32,preserve_pitch:bool,generation:u64,asset_epoch:u64},
    SetMusicRateAt{rate:f32,seconds:f64,generation:u64,asset_epoch:u64},
    SwapMusicAsset { decode_options:deadlib_audio_decode::DecodeOptions,path:PathBuf,rate:f32,preserve_pitch:bool,generation:u64,asset_epoch:u64 },
    PlayMusic {
        asset_epoch:u64,
        decode_options: deadlib_audio_decode::DecodeOptions,
        path: PathBuf,
        cut: Cut,
        looping: bool,
        rate: f32,
        preserve_pitch: bool,
        generation: u64,
    },
    /// Stops the worker after the caller invalidates the shared timeline generation.
    StopMusic {asset_epoch:u64},
    /// Manager-only termination; acknowledgment follows native stop and join.
    Shutdown {
        acknowledged: Sender<Result<(), String>>,
    },
    /// Reposition the retained decoder; rate, pitch policy and track identity remain active.
    /// The caller supplies a fresh invalidated timeline generation, as for PlayMusic.
    SeekMusic {
        seconds: f64,
        generation: u64,
    },
    SetMusicRate {
        rate: f32,
        generation: u64,
    },
    SetPreservePitch {
        enabled: bool,
        generation: u64,
    },
}

/// Manager-thread owner of the decoder and its recyclable block writer.
pub struct MusicStreamRuntime {
    music_stream: Option<MusicStream>,
    decoder_status: super::lifecycle::DecoderStatusMonitor,
    writer: Option<MusicBlockWriter>,
    output: OutputFormat,
}

impl MusicStreamRuntime {
    pub fn new(writer: MusicBlockWriter, output: OutputFormat) -> Self {
        Self {
            music_stream: None,
            decoder_status: super::lifecycle::DecoderStatusMonitor::default(),
            writer: Some(writer),
            output,
        }
    }

    pub fn decoder_status(&self) -> super::lifecycle::DecoderStatusMonitor {
        self.decoder_status.clone()
    }

    pub fn handle(&mut self, command: StreamCommand) {
        match command {
            StreamCommand::ReplaceEditorMainPrepared{source,path,generation,asset_epoch}=>{
                // Bytes are immutable asset authority; an intervening seek/rate
                // must not install the command's obsolete origin or readiness.
                if asset_epoch!=deadlib_audio_core::music_asset_epoch()||!deadlib_audio_core::music_track_active(){return;}
                let current=deadlib_audio_core::editor_main_configuration().filter(|c|c.asset_epoch==asset_epoch);
                let Some(current)=current else{return;};let _=generation;
                self.stop_decoder();let Some(writer)=self.writer.take()else{return;};
                self.music_stream=Some(super::spawn_prepared_preview_thread(source,path,Cut::default(),false,writer,MusicDecodeContext{decode_options:deadlib_audio_decode::DecodeOptions::STEP_MANIA_TIMELINE,output:self.output,generation:current.generation},asset_epoch,None,true));
                if let Some(stream)=&self.music_stream{self.decoder_status.set(stream.status_reader());}
            },
            StreamCommand::ReplaceEditorPreviewPrepared{source,path,cut,looping,generation,asset_epoch,preview_phase}=>{
                if asset_epoch!=deadlib_audio_core::music_asset_epoch()||generation!=deadlib_audio_core::music_map_generation()||!deadlib_audio_core::music_track_active(){return;}
                self.stop_decoder();let Some(writer)=self.writer.take()else{return;};
                self.music_stream=Some(super::spawn_prepared_preview_thread(source,path,cut,looping,writer,MusicDecodeContext{decode_options:deadlib_audio_decode::DecodeOptions::STEP_MANIA_TIMELINE,output:self.output,generation},asset_epoch,preview_phase,false));
                if let Some(stream)=&self.music_stream{self.decoder_status.set(stream.status_reader());}
            },
            StreamCommand::EditorPreparedFailed { asset_epoch,generation,message }=>{
                if asset_epoch!=deadlib_audio_core::music_asset_epoch() || !deadlib_audio_core::music_track_active(){return;}
                self.stop_decoder();let status=super::lifecycle::DecoderStatusReader::new_asset(generation,0.,asset_epoch);
                status.publish(generation,super::lifecycle::DecoderPhase::Failed,None,Some(&message));self.decoder_status.set(status);
            },
            StreamCommand::ReplaceEditorPrepared { source, path, cut, looping, rate, preserve_pitch, generation, asset_epoch, follow_live_phase, preview_phase } => {
                if asset_epoch != deadlib_audio_core::music_asset_epoch() || !deadlib_audio_core::music_track_active() { return; }
                self.stop_decoder();
                let Some(writer) = self.writer.take() else { return; };
                self.music_stream = Some(super::spawn_music_decoder_thread_with_source(path, cut, looping, rate, preserve_pitch, writer, MusicDecodeContext { decode_options: deadlib_audio_decode::DecodeOptions::STEP_MANIA_TIMELINE, output: self.output, generation }, true, asset_epoch, follow_live_phase, Some(source), preview_phase));
                if let Some(stream) = &self.music_stream { self.decoder_status.set(stream.status_reader()); }
            },
            StreamCommand::StartMusic{path,seconds,rate,preserve_pitch,decode_options,generation,asset_epoch}=>{
                if asset_epoch!=deadlib_audio_core::music_asset_epoch()||!deadlib_audio_core::music_track_active(){return;}
                self.stop_decoder();
                if let Some(path)=path{let Some(writer)=self.writer.take()else{return;};
                    self.music_stream=Some(super::spawn_music_decoder_thread_for_asset(path,Cut{start_sec:seconds,..Default::default()},false,rate,preserve_pitch,writer,MusicDecodeContext{decode_options,output:self.output,generation},true,asset_epoch,true));
                    if let Some(stream)=&self.music_stream{self.decoder_status.set(stream.status_reader());}
                }
            },
            StreamCommand::ClearMusicAsset{asset_epoch}=>{if asset_epoch==deadlib_audio_core::music_asset_epoch(){self.stop_decoder();}},
            StreamCommand::PauseMusic{generation,asset_epoch}=>{if asset_epoch==deadlib_audio_core::music_asset_epoch(){if let Some(stream)=&self.music_stream{stream.control.paused.store(true,Ordering::Release);stream.control.status.begin(generation,None);stream.control.generation.store(generation,Ordering::SeqCst);stream.wake_command(generation);}}},
            StreamCommand::ResumeMusic{seconds,rate,preserve_pitch,generation,asset_epoch}=>{if asset_epoch==deadlib_audio_core::music_asset_epoch(){if let Some(stream)=&self.music_stream{stream.control.rate_bits.store(rate.to_bits(),Ordering::Release);stream.control.preserve_pitch.store(preserve_pitch,Ordering::Release);stream.control.paused.store(false,Ordering::Release);}self.seek_mode(seconds,generation,true);}},
            StreamCommand::SetMusicRateAt{rate,seconds,generation,asset_epoch}=>{if asset_epoch==deadlib_audio_core::music_asset_epoch(){if let Some(stream)=&self.music_stream{stream.control.rate_bits.store(rate.to_bits(),Ordering::Release);}self.seek_mode(seconds,generation,true);}},
            StreamCommand::SwapMusicAsset{decode_options,path,rate,preserve_pitch,generation,asset_epoch}=>{
                if !deadlib_audio_core::music_track_active() || asset_epoch!=deadlib_audio_core::music_asset_epoch(){return;}
                self.stop_decoder();
                let Some(writer)=self.writer.take()else{return;};
                self.music_stream=Some(super::spawn_music_decoder_thread_for_asset(path,Cut::default(),false,rate,preserve_pitch,writer,MusicDecodeContext{decode_options,output:self.output,generation},true,asset_epoch,true));
                if let Some(stream)=&self.music_stream{self.decoder_status.set(stream.status_reader());}
            },
            StreamCommand::PlayMusic {
                asset_epoch,
                decode_options,
                path,
                cut,
                looping,
                rate,
                preserve_pitch,
                generation,
            } => {if asset_epoch==deadlib_audio_core::music_asset_epoch(){self.play(
                path,
                cut,
                looping,
                rate,
                preserve_pitch,
                generation,
                decode_options,
                asset_epoch,
            )}},
            StreamCommand::StopMusic{asset_epoch}=>{if asset_epoch==deadlib_audio_core::music_asset_epoch(){self.stop_decoder();stop_music_track();}},
            // The outer manager intercepts this variant while it owns the device.
            // A detached decoder runtime cannot acknowledge device shutdown.
            StreamCommand::Shutdown { .. } => self.stop(),
            StreamCommand::SeekMusic {
                seconds,
                generation,
            } => self.seek(seconds, generation),
            StreamCommand::SetMusicRate { rate, generation } => self.set_rate(rate, generation),
            StreamCommand::SetPreservePitch {
                enabled,
                generation,
            } => self.set_preserve_pitch(enabled, generation),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn play(
        &mut self,
        path: PathBuf,
        cut: Cut,
        looping: bool,
        rate: f32,
        preserve_pitch: bool,
        generation: u64,
        decode_options: deadlib_audio_decode::DecodeOptions,
        asset_epoch:u64,
    ) {
        self.stop_decoder();
        activate_music_track();

        let Some(writer) = self.writer.take() else {
            error!("Music decoder writer was lost after a worker panic.");
            stop_music_track();
            return;
        };
        self.music_stream = Some(super::spawn_music_decoder_thread_for_asset(
            path,
            cut,
            looping,
            rate,
            preserve_pitch,
            writer,
            MusicDecodeContext {
                decode_options,
                output: self.output,
                generation,
            },
            true,asset_epoch,false,
        ));
        if let Some(stream) = &self.music_stream {
            self.decoder_status.set(stream.status_reader());
        }
    }

    fn stop(&mut self) {
        self.stop_decoder();
        stop_music_track();
    }

    fn seek(&self,seconds:f64,generation:u64){self.seek_mode(seconds,generation,false)}
    fn seek_mode(&self, seconds: f64, generation: u64,follow_live_phase:bool) {
        if !seconds.is_finite() {
            return;
        }
        if let Some(stream) = &self.music_stream {
            let control = &stream.control;
            control.status.begin(generation, Some(seconds));
            let mut pending = control.seek.lock().unwrap_or_else(|e| e.into_inner());
            *pending = Some(super::SeekRequest {
                follow_live_phase,
                seconds,
                generation,
            });
            control.seek_serial.store(generation, Ordering::Release);
            control.generation.store(generation, Ordering::SeqCst);
            drop(pending);
            stream.wake_command(generation);
        }
    }

    fn set_rate(&self, rate: f32, generation: u64) {
        if let Some(stream) = &self.music_stream {
            let control = &stream.control;
            control.status.begin(generation, None);
            control.rate_bits.store(rate.to_bits(), Ordering::Release);
            control.generation.store(generation, Ordering::SeqCst);
            stream.wake_command(generation);
        }
    }

    fn set_preserve_pitch(&self, enabled: bool, generation: u64) {
        if let Some(stream) = &self.music_stream {
            let control = &stream.control;
            control.status.begin(generation, None);
            control.preserve_pitch.store(enabled, Ordering::Release);
            control.generation.store(generation, Ordering::SeqCst);
            stream.wake_command(generation);
        }
    }

    fn stop_decoder(&mut self) {
        if let Some(old) = self.music_stream.take() {
            old.control.status.publish(
                old.control.generation.load(Ordering::Acquire),
                super::lifecycle::DecoderPhase::Stopped,
                None,
                None,
            );
            old.control.stop_signal.store(true, Ordering::Release);
            old.control.wake.notify();
            match old.thread.join() {
                Ok(writer) => self.writer = Some(writer),
                Err(_) => error!("Music decoder thread panicked; its transport writer was lost."),
            }
        }
    }
}

impl Drop for MusicStreamRuntime {
    fn drop(&mut self) {
        self.stop_decoder();
    }
}

/// Output transports returned once the manager has opened the device.
///
/// The application keeps the command sender and sole SFX producer. The manager
/// owns the output session until all command senders have been dropped.
pub struct StreamReady {
    pub buffers: deadlib_audio_core::OutputBufferState,
    pub manager: StreamManager,
    pub command_sender: Sender<StreamCommand>,
    pub backend_ready: OutputBackendReady,
    pub sfx_sender: SfxSender,
    pub played_map: PlayedMapReader,
    pub decoder_status: super::lifecycle::DecoderStatusMonitor,
}

/// Owns the device-manager join, independently of retained command senders.
/// Shutdown acknowledgment is emitted only after the output and decoder have
/// actually been dropped; a stale sender cannot keep the old device alive.
pub struct StreamManager {
    command_sender: Sender<StreamCommand>,
    thread: Option<thread::JoinHandle<()>>,
}

impl StreamManager {
    pub fn shutdown(&mut self) -> Result<(), String> {
        let Some(thread) = self.thread.take() else {
            return Ok(());
        };
        let (acknowledged, acknowledgment) = channel();
        let sent = self
            .command_sender
            .send(StreamCommand::Shutdown { acknowledged })
            .is_ok();
        let acknowledged = sent && matches!(acknowledgment.recv(), Ok(Ok(())));
        let joined = thread.join().is_ok();
        if acknowledged && joined {
            Ok(())
        } else {
            Err("audio manager did not acknowledge completed shutdown".into())
        }
    }
}

impl Drop for StreamManager {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            error!("{error}");
        }
    }
}

struct ManagerReady {
    buffers: deadlib_audio_core::OutputBufferState,
    backend_ready: OutputBackendReady,
    sfx_sender: SfxSender,
    played_map: PlayedMapReader,
    decoder_status: super::lifecycle::DecoderStatusMonitor,
}

/// Opens output on its manager thread. Ownership includes an explicit shutdown
/// and join handle so reopening never overlaps an unacknowledged old device.
pub fn start(output_plan: OutputPlan) -> Result<StreamReady, String> {
    let (command_sender, command_receiver) = channel();
    let (ready_sender, ready_receiver) = channel();
    let thread =
        thread::spawn(move || audio_manager_thread(command_receiver, ready_sender, output_plan));
    let ready = match ready_receiver.recv() {
        Ok(Ok(ready)) => ready,
        Ok(Err(error)) => {
            let _ = thread.join();
            return Err(error);
        }
        Err(_) => {
            let _ = thread.join();
            return Err("audio manager thread exited before reporting ready".into());
        }
    };
    let backend = &ready.backend_ready;
    info!(
        "Audio runtime initialized ({} Hz, {} ch, backend={} req={} fallback={} clock={} quality={} device='{}').",
        backend.device_sample_rate,
        backend.device_channels,
        backend.backend_name,
        backend.requested_output_mode.as_str(),
        backend.fallback_from_native,
        backend.timing_clock,
        backend.timing_quality,
        backend.device_name
    );
    deadlib_audio_core::publish_output_backend_ready(backend.clone());
    Ok(StreamReady {
        manager: StreamManager {
            command_sender: command_sender.clone(),
            thread: Some(thread),
        },
        command_sender,
        buffers: ready.buffers,
        backend_ready: ready.backend_ready,
        sfx_sender: ready.sfx_sender,
        played_map: ready.played_map,
        decoder_status: ready.decoder_status,
    })
}

fn audio_manager_thread(
    command_receiver: Receiver<StreamCommand>,
    ready_sender: Sender<Result<ManagerReady, String>>,
    output_plan: OutputPlan,
) {
    let opened = match output_plan.open() {
        Ok(output) => output,
        Err(err) => {
            let _ = ready_sender.send(Err(err));
            return;
        }
    };
    let (mut session, ready, sfx_sender, stream_handle, buffers) = opened.into_parts();
    let deadlib_audio_core::AudioStreamHandle { writer, played_map } = stream_handle;
    let stream_output = OutputFormat {
        sample_rate_hz: ready.device_sample_rate,
        channels: ready.device_channels,
    };
    let mut music_runtime = MusicStreamRuntime::new(writer, stream_output);
    if ready_sender
        .send(Ok(ManagerReady {
            buffers,
            backend_ready: ready,
            sfx_sender,
            played_map,
            decoder_status: music_runtime.decoder_status(),
        }))
        .is_err()
    {
        drop(session);
        drop(music_runtime);
        return;
    }
    let acknowledgment = loop {
        match command_receiver.recv() {
            Ok(StreamCommand::Shutdown { acknowledged }) => break Some(acknowledged),
            Ok(command) => music_runtime.handle(command),
            Err(_) => break None,
        }
    };
    // Invalidate the old shared music generation before replacing its clock.
    stop_music_track();
    // Preserve the recycle pool until the render callback has really stopped.
    let stopped = session.shutdown();
    if let Err(error) = &stopped {
        error!("Native output shutdown failed: {error}");
    }
    drop(session);
    drop(music_runtime);
    if let Some(acknowledgment) = acknowledgment {
        let _ = acknowledgment.send(stopped);
    }
}

#[cfg(test)]
mod manager_shutdown_tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn shutdown_waits_for_ack_and_join_despite_retained_senders() {
        let (sender, receiver) = channel();
        let retained = sender.clone();
        let trace = Arc::new(Mutex::new(Vec::new()));
        let worker_trace = trace.clone();
        let thread = thread::spawn(move || {
            let StreamCommand::Shutdown { acknowledged } = receiver.recv().unwrap() else {
                panic!("expected shutdown");
            };
            worker_trace.lock().unwrap().push("native-stopped");
            worker_trace.lock().unwrap().push("decoder-dropped");
            acknowledged.send(Ok(())).unwrap();
            worker_trace.lock().unwrap().push("thread-return");
        });
        let mut manager = StreamManager {
            command_sender: sender,
            thread: Some(thread),
        };
        manager.shutdown().unwrap();
        assert_eq!(
            *trace.lock().unwrap(),
            ["native-stopped", "decoder-dropped", "thread-return"]
        );
        assert!(retained.send(StreamCommand::StopMusic{asset_epoch:deadlib_audio_core::music_asset_epoch()}).is_err());
        manager.shutdown().unwrap();
    }

    #[test]
    fn acknowledgment_does_not_hide_manager_panic() {
        let (sender, receiver) = channel();
        let thread = thread::spawn(move || {
            let StreamCommand::Shutdown { acknowledged } = receiver.recv().unwrap() else {
                panic!("expected shutdown");
            };
            acknowledged.send(Ok(())).unwrap();
            panic!("injected host shutdown fault");
        });
        let mut manager = StreamManager {
            command_sender: sender,
            thread: Some(thread),
        };
        assert!(manager.shutdown().is_err());
        assert!(manager.thread.is_none());
    }

    #[test]
    fn native_shutdown_error_is_not_an_acknowledged_success() {
        let (sender, receiver) = channel();
        let thread = thread::spawn(move || {
            let StreamCommand::Shutdown { acknowledged } = receiver.recv().unwrap() else {
                panic!("expected shutdown");
            };
            acknowledged
                .send(Err("injected native stop failure".into()))
                .unwrap();
        });
        let mut manager = StreamManager {
            command_sender: sender,
            thread: Some(thread),
        };
        assert!(manager.shutdown().is_err());
    }

    #[test]
    fn vanished_manager_is_not_reported_as_completed_shutdown() {
        let (sender, receiver) = channel();
        let thread = thread::spawn(move || drop(receiver));
        let mut manager = StreamManager {
            command_sender: sender,
            thread: Some(thread),
        };
        assert!(manager.shutdown().is_err());
        assert!(manager.thread.is_none());
    }
}
