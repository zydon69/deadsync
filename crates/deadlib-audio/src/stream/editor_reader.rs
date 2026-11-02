//! Editor-only prepared PCM adapter; gameplay retains its codec streaming path.
use super::{decode, lifecycle};
use deadlib_audio_decode::prepared::{PreparedAudioSource, PreparedCodec, PreparedReader};
use std::{path::Path, sync::Arc};
pub(super) enum EditorDecodeReader {
    Streaming(decode::Reader),
    Prepared(PreparedReader),
}
pub(super) struct OpenEditorSource {
    pub reader: EditorDecodeReader,
    pub channels: usize,
    pub sample_rate_hz: u32,
}
impl EditorDecodeReader {
    pub fn open(
        path: &Path,
        options: decode::DecodeOptions,
        prepared: Option<&Arc<PreparedAudioSource>>,
    ) -> Result<OpenEditorSource, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(source) = prepared {
            return Ok(OpenEditorSource {
                reader: Self::Prepared(source.reader()),
                channels: source.channels,
                sample_rate_hz: source.sample_rate_hz,
            });
        }
        let opened = decode::open_file(path, options)?;
        Ok(OpenEditorSource {
            reader: Self::Streaming(opened.reader),
            channels: opened.channels,
            sample_rate_hz: opened.sample_rate_hz,
        })
    }
    pub fn codec(&self) -> lifecycle::DecodedCodec {
        use lifecycle::DecodedCodec as C;
        match self {
            Self::Streaming(reader) => match reader {
                decode::Reader::Flac(_) => C::Flac,
                decode::Reader::Mp3(_) => C::Mp3,
                decode::Reader::Ogg(_) => C::Vorbis,
                decode::Reader::Opus(_) => C::Opus,
                decode::Reader::Wav(_) => C::Wav,
            },
            Self::Prepared(reader) => match reader.source().codec {
                PreparedCodec::Flac => C::Flac,
                PreparedCodec::Mp3 => C::Mp3,
                PreparedCodec::Vorbis => C::Vorbis,
                PreparedCodec::Opus => C::Opus,
                PreparedCodec::Wav => C::Wav,
            },
        }
    }
    pub fn is_streaming_vorbis(&self) -> bool {
        matches!(self, Self::Streaming(decode::Reader::Ogg(_)))
    }
    pub fn read_dec_packet_into(
        &mut self,
        out: &mut Vec<i16>,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        match self {
            Self::Streaming(reader) => reader.read_dec_packet_into(out),
            Self::Prepared(reader) => Ok(reader.read_dec_packet_into(out)),
        }
    }
    pub fn seek_frame(
        &mut self,
        frame: u64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match self {
            Self::Streaming(reader) => reader.seek_frame(frame),
            Self::Prepared(reader) => {
                reader.seek_frame(frame);
                Ok(())
            }
        }
    }
    pub fn current_frame(&self) -> u64 {
        match self {
            Self::Streaming(reader) => reader.current_frame(),
            Self::Prepared(reader) => reader.current_frame(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::{Cut, MusicStreamRuntime, OutputFormat, StreamCommand};
    use deadlib_audio_core::{
        CallbackClockSource, CallbackInfo, MixControls, OutputBufferMut, RenderState,
    };
    use std::{
        sync::atomic::AtomicBool,
        time::{Duration, Instant},
    };
    #[test]
    fn prepared_editor_transport_uses_original_pcm_and_codec_after_unlink() {
        let path =
            std::env::temp_dir().join(format!("prepared-transport-{}.wav", std::process::id()));
        let frames = 48_000u32;
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + frames * 4).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&48_000u32.to_le_bytes());
        wav.extend_from_slice(&192_000u32.to_le_bytes());
        wav.extend_from_slice(&4u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(frames * 4).to_le_bytes());
        for _ in 0..frames * 2 {
            wav.extend_from_slice(&(-3000i16).to_le_bytes());
        }
        std::fs::write(&path, wav).unwrap();
        let source = PreparedAudioSource::load(
            &path,
            decode::DecodeOptions::STEP_MANIA_TIMELINE,
            &AtomicBool::new(false),
        )
        .unwrap();
        std::fs::remove_file(&path).unwrap();
        deadlib_audio_core::resume_music_track();
        deadlib_audio_core::reset_music_stream_clock_state();
        deadlib_audio_core::reset_music_target_gain();
        let generation = deadlib_audio_core::bump_music_map_generation();
        let asset_epoch = deadlib_audio_core::advance_music_asset_epoch();
        deadlib_audio_core::activate_music_track();
        let (stream, handle) = deadlib_audio_core::music_transport(48_000, 2);
        let mut render = RenderState::new(handle, Arc::new(MixControls::new()), 2);
        let mut runtime = MusicStreamRuntime::new(
            stream.writer,
            OutputFormat {
                sample_rate_hz: 48_000,
                channels: 2,
            },
        );
        runtime.handle(StreamCommand::ReplaceEditorPrepared {
            source,
            path,
            cut: Cut::default(),
            looping: false,
            rate: 1.,
            preserve_pitch: false,
            generation,
            asset_epoch,
            follow_live_phase: false,
            preview_phase: None,
        });
        let deadline = Instant::now() + Duration::from_secs(3);
        while !runtime
            .decoder_status()
            .snapshot()
            .is_some_and(|s| s.render_ready_generation == Some(generation))
        {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        let status = runtime.decoder_status().snapshot().unwrap();
        assert_eq!(status.codec, Some(lifecycle::DecodedCodec::Wav));
        assert!(status.opened_successfully);
        let mut output = [0i16; 480];
        render.render(
            OutputBufferMut::I16(&mut output),
            CallbackInfo {
                anchor_nanos: 1_000_000_000,
                clock: CallbackClockSource::Instant,
            },
            std::iter::empty(),
        );
        assert!(output.iter().any(|sample| *sample < -1000));
        assert_eq!(deadlib_audio_core::music_asset_epoch(), asset_epoch);
        drop(runtime);
    }
}

#[cfg(test)]
#[path = "editor_reader_codec_tests.rs"]
mod codec_tests;
