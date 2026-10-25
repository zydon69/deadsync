//! Multiresolution waveform data, independent of the decoder and renderer.
//!
//! Like GrooveAuthor's SoundMipMap, nodes pack a 9-bit minimum, 9-bit maximum
//! and 14-bit mean sample-to-sample distance. Channels never mix. A range query
//! decomposes its interval into aligned powers of two rather than scanning PCM.
use crate::Error;
use std::sync::atomic::{AtomicBool, Ordering};

const MAX: u16 = 511;

#[derive(Clone, Copy, Debug)]
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
    pub fn push_i16(&mut self, pcm: &[i16]) -> Result<(), Error> {
        if pcm.len() % self.channels != 0 {
            return Err(Error::InvalidFormat(
                "Incomplete interleaved waveform frame".into(),
            ));
        }
        let wanted = self
            .samples
            .len()
            .checked_add(pcm.len())
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
        for frame in pcm.chunks_exact(self.channels) {
            for (channel, &sample) in frame.iter().enumerate() {
                let value = f32::from(sample) / 32768.0;
                let x = (f32::from(MAX) * (value + 1.0) * 0.5) as u16;
                let distance = ((value - self.previous[channel]).abs() * f32::from(MAX)) as u16;
                self.samples
                    .push(Node::new(x.min(MAX), x.min(MAX), distance));
                self.previous[channel] = value;
            }
        }
        Ok(())
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
