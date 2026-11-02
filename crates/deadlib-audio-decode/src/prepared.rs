//! Immutable editor source PCM. Gameplay's streaming Reader remains unchanged.
use crate::{DecodeOptions, Reader};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparedCodec {
    Flac,
    Mp3,
    Vorbis,
    Opus,
    Wav,
}
#[derive(Debug)]
pub struct PreparedAudioSource {
    pub codec: PreparedCodec,
    pub channels: usize,
    pub sample_rate_hz: u32,
    samples: Arc<[i16]>,
    editor_samples: Option<Arc<[f32]>>,
}
impl PreparedAudioSource {
    pub fn samples(&self) -> &[i16] {
        &self.samples
    }
    /// Present only for explicit editor preparation. Gameplay load does not
    /// allocate a second PCM view.
    pub fn editor_samples(&self) -> Option<&[f32]> {
        self.editor_samples.as_deref()
    }

    pub fn load_editor(
        path: &Path,
        options: DecodeOptions,
        cancelled: &AtomicBool,
    ) -> Result<Arc<Self>, Box<dyn std::error::Error + Send + Sync>> {
        Self::load_editor_limited(path, options, cancelled, usize::MAX)
    }

    /// Bounds retained raw+compatibility PCM for standalone waveform workers.
    /// Decoder packet scratch remains bounded independently by each codec.
    pub fn load_editor_limited(
        path: &Path,
        options: DecodeOptions,
        cancelled: &AtomicBool,
        max_pcm_bytes: usize,
    ) -> Result<Arc<Self>, Box<dyn std::error::Error + Send + Sync>> {
        let max_samples = max_pcm_bytes / (std::mem::size_of::<f32>() + std::mem::size_of::<i16>());
        let raw = if crate::wav::path_is_wav(path) {
            Some((
                PreparedCodec::Wav,
                crate::wav::load_editor_pcm(path, cancelled, max_samples)?,
            ))
        } else if crate::flac::path_is_flac(path) {
            Some((
                PreparedCodec::Flac,
                crate::flac::load_editor_pcm(path, cancelled, max_samples)?,
            ))
        } else if crate::opus::path_is_opus(path) {
            Some((
                PreparedCodec::Opus,
                crate::opus::load_editor_pcm(path, cancelled, max_samples)?,
            ))
        } else {
            None
        };
        if let Some((codec, (channels, sample_rate_hz, pcm))) = raw {
            if channels == 0 || sample_rate_hz == 0 || pcm.len() % channels != 0 {
                return Err("Invalid editor source PCM".into());
            }
            let mut compatibility = Vec::new();
            compatibility.try_reserve_exact(pcm.len())?;
            for chunk in pcm.chunks(524288) {
                if cancelled.load(Ordering::Acquire) {
                    return Err("Editor source load cancelled".into());
                }
                compatibility.extend(
                    chunk
                        .iter()
                        .map(|sample| (*sample * 32768.).clamp(-32768., 32767.) as i16),
                );
            }
            if cancelled.load(Ordering::Acquire) {
                return Err("Editor source load cancelled".into());
            }
            return Ok(Arc::new(Self {
                codec,
                channels,
                sample_rate_hz,
                samples: compatibility.into(),
                editor_samples: Some(pcm.into()),
            }));
        }
        // FMOD exposes MP3 and Vorbis as PCM16. Preserve the current native
        // decoder's timeline/codec behavior, then apply the source PCM16 parser.
        // This is normalization fidelity, not an assertion of FMOD codec parity.
        let legacy = Self::load_limited(path, options, cancelled, max_samples)?;
        let mut pcm = Vec::new();
        pcm.try_reserve_exact(legacy.samples.len())?;
        for chunk in legacy.samples.chunks(524288) {
            if cancelled.load(Ordering::Acquire) {
                return Err("Editor source load cancelled".into());
            }
            pcm.extend(
                chunk
                    .iter()
                    .map(|sample| f32::from(*sample) * (1.0f32 / 32767.)),
            );
        }
        if cancelled.load(Ordering::Acquire) {
            return Err("Editor source load cancelled".into());
        }
        Ok(Arc::new(Self {
            codec: legacy.codec,
            channels: legacy.channels,
            sample_rate_hz: legacy.sample_rate_hz,
            samples: legacy.samples.clone(),
            editor_samples: Some(pcm.into()),
        }))
    }

    /// Worker-only whole-source load. Publication occurs only after successful
    /// decode; cancellation cannot expose a partial immutable PCM allocation.
    pub fn load(
        path: &Path,
        options: DecodeOptions,
        cancelled: &AtomicBool,
    ) -> Result<Arc<Self>, Box<dyn std::error::Error + Send + Sync>> {
        Self::load_limited(path, options, cancelled, usize::MAX)
    }
    fn load_limited(
        path: &Path,
        options: DecodeOptions,
        cancelled: &AtomicBool,
        max_samples: usize,
    ) -> Result<Arc<Self>, Box<dyn std::error::Error + Send + Sync>> {
        let mut opened = crate::open_file(path, options)?;
        let codec = match &opened.reader {
            Reader::Flac(_) => PreparedCodec::Flac,
            Reader::Mp3(_) => PreparedCodec::Mp3,
            Reader::Ogg(_) => PreparedCodec::Vorbis,
            Reader::Opus(_) => PreparedCodec::Opus,
            Reader::Wav(_) => PreparedCodec::Wav,
        };
        if opened.channels == 0 || opened.sample_rate_hz == 0 {
            return Err("Invalid prepared audio format".into());
        }
        let mut samples = Vec::new();
        let mut packet = Vec::new();
        loop {
            if cancelled.load(Ordering::Acquire) {
                return Err("Prepared audio load cancelled".into());
            }
            packet.clear();
            let has_packet = opened.reader.read_dec_packet_into(&mut packet)?;
            if samples
                .len()
                .checked_add(packet.len())
                .is_none_or(|len| len > max_samples)
            {
                return Err("Prepared audio PCM memory limit exceeded".into());
            }
            samples.try_reserve(packet.len())?;
            samples.extend_from_slice(&packet);
            if !has_packet {
                break;
            }
        }
        if cancelled.load(Ordering::Acquire) {
            return Err("Prepared audio load cancelled".into());
        }
        if samples.len() % opened.channels != 0 {
            return Err("Incomplete prepared PCM frame".into());
        }
        Ok(Arc::new(Self {
            codec,
            channels: opened.channels,
            sample_rate_hz: opened.sample_rate_hz,
            samples: samples.into(),
            editor_samples: None,
        }))
    }
    pub fn frames(&self) -> u64 {
        (self.samples.len() / self.channels) as u64
    }
    pub fn reader(self: &Arc<Self>) -> PreparedReader {
        PreparedReader {
            source: Arc::clone(self),
            frame: 0,
        }
    }
}
pub struct PreparedReader {
    source: Arc<PreparedAudioSource>,
    frame: u64,
}
impl PreparedReader {
    pub fn source(&self) -> &Arc<PreparedAudioSource> {
        &self.source
    }
    pub fn seek_frame(&mut self, frame: u64) {
        self.frame = frame.min(self.source.frames());
    }
    pub fn current_frame(&self) -> u64 {
        self.frame
    }
    /// Same packet/EOF convention as the codec readers, with exact frame seeks.
    pub fn read_dec_packet_into(&mut self, out: &mut Vec<i16>) -> bool {
        let start = self.frame as usize * self.source.channels;
        let end = start
            .saturating_add(1024 * self.source.channels)
            .min(self.source.samples.len());
        out.clear();
        out.extend_from_slice(&self.source.samples[start..end]);
        self.frame += ((end - start) / self.source.channels) as u64;
        end > start
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn readers_have_independent_cursors_and_exact_zero_eof_seeks() {
        let source = Arc::new(PreparedAudioSource {
            codec: PreparedCodec::Wav,
            channels: 2,
            sample_rate_hz: 48_000,
            samples: Arc::from([1, 2, 3, 4, 5, 6]),
            editor_samples: None,
        });
        let mut a = source.reader();
        let mut b = source.reader();
        let mut out = Vec::new();
        a.seek_frame(1);
        assert!(a.read_dec_packet_into(&mut out));
        assert_eq!(out, [3, 4, 5, 6]);
        assert_eq!(b.current_frame(), 0);
        b.seek_frame(u64::MAX);
        assert!(!b.read_dec_packet_into(&mut out));
        assert!(out.is_empty());
        a.seek_frame(0);
        assert!(a.read_dec_packet_into(&mut out));
        assert_eq!(out, [1, 2, 3, 4, 5, 6]);
        assert_eq!(source.codec, PreparedCodec::Wav);
    }
    #[test]
    fn real_wav_load_is_immutable_after_file_replacement_and_cancel_is_not_published() {
        let path = std::env::temp_dir().join(format!(
            "prepared-editor-audio-{}-{}.wav",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let wav = |value: i16| {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(b"RIFF");
            bytes.extend_from_slice(&44u32.to_le_bytes());
            bytes.extend_from_slice(b"WAVEfmt ");
            bytes.extend_from_slice(&16u32.to_le_bytes());
            bytes.extend_from_slice(&1u16.to_le_bytes());
            bytes.extend_from_slice(&2u16.to_le_bytes());
            bytes.extend_from_slice(&48_000u32.to_le_bytes());
            bytes.extend_from_slice(&192_000u32.to_le_bytes());
            bytes.extend_from_slice(&4u16.to_le_bytes());
            bytes.extend_from_slice(&16u16.to_le_bytes());
            bytes.extend_from_slice(b"data");
            bytes.extend_from_slice(&8u32.to_le_bytes());
            for _ in 0..4 {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            std::fs::write(&path, bytes).unwrap();
        };
        wav(-1000);
        let cancelled = AtomicBool::new(false);
        let source =
            PreparedAudioSource::load(&path, DecodeOptions::STEP_MANIA_TIMELINE, &cancelled)
                .unwrap();
        assert_eq!(source.frames(), 2);
        assert_eq!(source.codec, PreparedCodec::Wav);
        wav(4000);
        let mut reader = source.reader();
        let mut samples = Vec::new();
        assert!(reader.read_dec_packet_into(&mut samples));
        assert_eq!(samples, [-1000; 4]);
        assert!(!reader.read_dec_packet_into(&mut samples));
        cancelled.store(true, Ordering::Release);
        assert!(
            PreparedAudioSource::load(&path, DecodeOptions::STEP_MANIA_TIMELINE, &cancelled)
                .is_err()
        );
        std::fs::remove_file(path).unwrap();
    }
}

#[cfg(test)]
mod editor_pcm_tests {
    use super::*;
    #[test]
    fn real_source_fmod_float_hashes_match_for_bundled_wav_assets() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/sounds");
        for (name, frames, hash) in [
            ("editor_assist_tick.wav", 18000, 507382426822696679u64),
            ("editor_beat_tick.wav", 5142, 13401652033342896737u64),
        ] {
            let source = PreparedAudioSource::load_editor(
                &root.join(name),
                DecodeOptions::STEP_MANIA_TIMELINE,
                &AtomicBool::new(false),
            )
            .unwrap();
            let pcm =
                crate::preview::resample_prepared(&source, 48000, &AtomicBool::new(false)).unwrap();
            assert_eq!(pcm.len() / source.channels, frames);
            let actual = pcm.iter().fold(1469598103934665603u64, |hash, sample| {
                (hash ^ u64::from(sample.to_bits())).wrapping_mul(1099511628211)
            });
            assert_eq!(
                actual, hash,
                "Original FMOD NOSOUND_NRT + FillSamplesAsync witness"
            );
            let legacy = PreparedAudioSource::load(
                &root.join(name),
                DecodeOptions::STEP_MANIA_TIMELINE,
                &AtomicBool::new(false),
            )
            .unwrap();
            assert!(legacy.editor_samples().is_none());
            let budget = source.frames() as usize * source.channels * 6;
            assert!(
                PreparedAudioSource::load_editor_limited(
                    &root.join(name),
                    DecodeOptions::STEP_MANIA_TIMELINE,
                    &AtomicBool::new(false),
                    budget - 1
                )
                .is_err()
            );
            let bounded = PreparedAudioSource::load_editor_limited(
                &root.join(name),
                DecodeOptions::STEP_MANIA_TIMELINE,
                &AtomicBool::new(false),
                budget,
            )
            .unwrap();
            assert_eq!(bounded.editor_samples(), source.editor_samples());
        }
    }
}
