//! Source MusicManager.RenderPreview's frame envelope. Fade-out takes priority
//! in the overlap; the two ramps are never interpolated over a decoder block.
// MusicManager.GetSampleIndexFromTime casts to long (truncates), not rounds.
pub(super) fn frames(seconds: f64, hz: u32) -> u64 {
    if seconds.is_finite() {
        (seconds.max(0.) * f64::from(hz)) as u64
    } else {
        0
    }
}
#[derive(Clone, Copy)]
pub(super) struct PreviewFade {
    fade_in: u64,
    fade_out: u64,
    total: Option<u64>,
}
impl PreviewFade {
    pub(super) fn new(fade_in: u64, fade_out: u64, total: Option<u64>) -> Option<Self> {
        (fade_in > 0 || (fade_out > 0 && total.is_some())).then_some(Self {
            fade_in,
            fade_out,
            total,
        })
    }
    fn gain(self, frame: u64) -> f32 {
        // Signed subtraction is deliberate: an outgoing ramp longer than the
        // excerpt already attenuates its first sample, matching source longs.
        if let Some(total) = self.total {
            let start = i128::from(total) - i128::from(self.fade_out);
            if self.fade_out > 0 && i128::from(frame) > start {
                return (1.0 - (i128::from(frame) - start) as f64 / self.fade_out as f64)
                    .clamp(0., 1.) as f32;
            }
        }
        if frame < self.fade_in {
            return (frame as f64 / self.fade_in as f64).clamp(0., 1.) as f32;
        }
        1.
    }
    pub(super) fn apply(self, samples: &mut [i16], channels: usize, start: u64) {
        if channels == 0 {
            return;
        }
        for (frame, samples) in samples.chunks_exact_mut(channels).enumerate() {
            let gain = self.gain(start.saturating_add(frame as u64));
            for sample in samples {
                *sample = (f32::from(*sample) * gain).round() as i16;
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_partial_preview_quantizes_absolute_positions_before_subtraction(){
        let start=frames(1.9/48000.,48000);let current=frames(3.01/48000.,48000);let full=frames(5.9/48000.,48000);
        assert_eq!((start,current,full),(1,3,5));
        let offset=current.saturating_sub(start);let remaining=full.saturating_sub(offset);
        assert_eq!((offset,remaining,offset+remaining),(2,3,5));
    }
    #[test]
    fn both_ramps_and_strict_overlap_boundary() {
        assert_eq!(frames(1.9 / 48000., 48000), 1);
        let fade = PreviewFade::new(8, 8, Some(10)).unwrap();
        assert_eq!(
            (0..10).map(|n| fade.gain(n)).collect::<Vec<_>>(),
            vec![0., 0.125, 0.25, 0.875, 0.75, 0.625, 0.5, 0.375, 0.25, 0.125]
        );
        assert_eq!(PreviewFade::new(4, 0, Some(10)).unwrap().gain(10), 1.);
        assert_eq!(PreviewFade::new(0, 20, Some(10)).unwrap().gain(0), 0.5);
    }
    #[test]
    fn chunk_boundaries_partial_reload_offset_and_loop_reset() {
        let fade = PreviewFade::new(4, 4, Some(12)).unwrap();
        let mut whole = vec![10000; 24];
        fade.apply(&mut whole, 2, 0);
        let mut chunks = vec![10000; 24];
        for (start, chunk) in chunks.chunks_mut(6).enumerate() {
            fade.apply(chunk, 2, (start * 3) as u64);
        }
        assert_eq!(chunks, whole);
        let mut partial = vec![10000; 14];
        fade.apply(&mut partial, 2, 5);
        assert_eq!(partial, &whole[10..]);
        let mut next_loop = vec![10000; 24];
        fade.apply(&mut next_loop, 2, 0);
        assert_eq!(next_loop, whole);
    }
}
