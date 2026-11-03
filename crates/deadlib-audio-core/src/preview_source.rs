//! Immutable editor preview PCM, prepared completely outside the callback.
//! The callback transfers retired descriptors back to the unique producer;
//! it never destroys a potentially final PCM Arc while rendering.
use crate::PreviewSource;
pub(crate) use crate::preview_transport::{
    COMMAND_CAPACITY, PreviewConsumer, PreviewPublisher, channel,
};
/// Callback-progress provenance, deliberately separate from audible PCM's
/// RenderedMusicCursor. This never claims opened/ready or an audio lease.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreviewCallbackCursor {
    pub generation: u64,
    pub asset_epoch: u64,
    pub next_music_seconds: f64,
    pub callback_frame: u64,
    pub source_frame: i64,
    pub sample_rate_hz: u32,
}
static CALLBACK_CURSOR: crate::rendered_cursor::Cursor = crate::rendered_cursor::Cursor::new();
pub fn preview_callback_cursor() -> Option<PreviewCallbackCursor> {
    let (cursor, (source_frame, sample_rate_hz)) = CALLBACK_CURSOR
        .read_details()
        .and_then(|(cursor, source)| source.map(|source| (cursor, source)))?;
    Some(PreviewCallbackCursor {
        generation: cursor.generation,
        asset_epoch: cursor.asset_epoch,
        next_music_seconds: cursor.next_music_seconds,
        callback_frame: cursor.rendered_frame,
        source_frame,
        sample_rate_hz,
    })
}
pub(crate) fn publish_callback_cursor(
    generation: u64,
    asset_epoch: u64,
    frame: i64,
    callback_frame: u64,
    hz: u32,
) {
    CALLBACK_CURSOR.publish_details(
        crate::RenderedMusicCursor {
            generation,
            asset_epoch,
            next_music_seconds: frame as f64 / f64::from(hz),
            rendered_frame: callback_frame,
        },
        Some((frame, hz)),
    );
}
/// One callback's coherent frame snapshot. Signed values mirror source longs.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Region {
    pub start: i64,
    pub length: i64,
    pub incoming: i64,
    pub outgoing: i64,
}
impl Region {
    pub(crate) fn gain(self, frame: i64) -> f32 {
        let frame = i128::from(frame);
        let start = i128::from(self.start);
        let length = i128::from(self.length);
        let outgoing = i128::from(self.outgoing);
        let incoming = i128::from(self.incoming);
        let end = start + length;
        if frame > end - outgoing {
            if outgoing == 0 {
                return 0.;
            }
            return (1. - (frame - (end - outgoing)) as f64 / outgoing as f64).clamp(0., 1.) as f32;
        }
        if frame < start + incoming {
            if incoming == 0 {
                return 0.;
            }
            return ((frame - start) as f64 / incoming as f64).clamp(0., 1.) as f32;
        }
        1.
    }
    /// Source RenderPreview's PCM-local cursor subtracts exactly once/frame.
    pub(crate) fn advance(self, frame: i64, looping: bool) -> i64 {
        let next = i128::from(frame) + 1;
        let start = i128::from(self.start);
        let length = i128::from(self.length);
        let next = if looping && length > 0 && next >= start + length {
            next - length
        } else {
            next
        };
        next.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
    }
    /// PreviewData tracking is computed separately before the source PCM loop
    /// and fully wraps. Arithmetic replaces its unbounded while loop safely.
    pub(crate) fn final_frame(self, frame: i64, frames: usize, looping: bool) -> i64 {
        let next = i128::from(frame) + frames as i128;
        let start = i128::from(self.start);
        let length = i128::from(self.length);
        let next = if looping && length > 0 && next >= start + length {
            start + (next - start).rem_euclid(length)
        } else {
            next
        };
        next.clamp(0, i128::from(i64::MAX)) as i64
    }
}
pub(crate) struct PreviewPlayer {
    pub source: PreviewSource,
    pub frame: i64,
}
impl PreviewPlayer {
    pub(crate) fn new(source: PreviewSource) -> Self {
        let frame = source.initial_frame;
        Self { source, frame }
    }
    pub(crate) fn sample(&self, channel: usize) -> f32 {
        if self.frame < 0 {
            return 0.;
        }
        let Some(index) = (self.frame as usize)
            .checked_mul(self.source.channels)
            .and_then(|index| index.checked_add(channel))
        else {
            return 0.;
        };
        self.source.samples.get(index).copied().unwrap_or(0.)
    }
}

#[cfg(test)]
pub(crate) mod callback_test_gate {
    use std::sync::{Arc, Barrier, Mutex};
    static GATE: Mutex<Option<(Arc<Barrier>, Arc<Barrier>)>> = Mutex::new(None);
    pub(crate) fn install(entered: Arc<Barrier>, release: Arc<Barrier>) {
        *GATE.lock().unwrap() = Some((entered, release));
    }
    pub(crate) fn hold() {
        if let Some((entered, release)) = GATE.lock().unwrap().take() {
            entered.wait();
            release.wait();
        }
    }
}
