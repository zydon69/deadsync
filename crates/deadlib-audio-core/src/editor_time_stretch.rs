//! GrooveAuthor Main's callback-local TimeStretcher float PCM boundary.
//!
//! This is separate from codec/output resampling and gameplay pitch handling.
//! The caller owns the source-rate cursor and advances it once per callback by
//! truncating callback_frames * rate, as the original MusicManager does.
use crate::hermite::interpolate;
#[doc(hidden)]
pub fn hermite_interpolate(x0:f32,x1:f32,x2:f32,x3:f32,t:f32)->f32{interpolate(x0,x1,x2,x3,t)}

/// Fill interleaved output with the original four-point Hermite time grid.
///
/// Neither buffer is allocated here. Invalid formats fail before output changes.
/// Source edge clamping applies to the entire interleaved array (not per channel),
/// preserving the original behavior for signed lead-in and after the source end.
pub fn process(
    start_frame: i64,
    input: &[f32],
    sample_rate: u32,
    channels: usize,
    output: &mut [f32],
    rate: f64,
) -> Result<(), &'static str> {
    process_range(start_frame,input,sample_rate,channels,output,rate,0)
}

/// Offset is relative to the entire physical callback, never a scratch chunk.
pub fn process_range(start_frame:i64,input:&[f32],sample_rate:u32,channels:usize,output:&mut[f32],rate:f64,output_offset:usize)->Result<(), &'static str>{
    if sample_rate == 0
        || channels == 0
        || channels > i32::MAX as usize
        || input.is_empty()
        || input.len() > i32::MAX as usize
        || input.len() % channels != 0
        || output.len() % channels != 0
        || !rate.is_finite()
        || rate <= 0.0
    {
        return Err("Invalid editor time-stretch PCM format or rate");
    }
    let time_range = 1.0f32 / sample_rate as f32;
    let start_time = start_frame as f64 / f64::from(sample_rate);
    let last = input.len() as i32 - 1;
    for (frame, destination) in output.chunks_exact_mut(channels).enumerate() {
        let time = start_time + (frame+output_offset) as f64 * rate / f64::from(sample_rate);
        // .NET 10 saturates the float-to-int cast; subsequent integer operations
        // use unchecked Int32 arithmetic in the original source assembly.
        let first = ((time * f64::from(sample_rate)) as i32).wrapping_sub(1);
        let x1_time = f64::from(first.wrapping_add(1)) / f64::from(sample_rate);
        let normalized = ((time - x1_time) / f64::from(time_range)) as f32;
        for (channel, value) in destination.iter_mut().enumerate() {
            let points: [f32; 4] = std::array::from_fn(|point| {
                let index = first
                    .wrapping_add(point as i32)
                    .wrapping_mul(channels as i32)
                    .wrapping_add(channel as i32)
                    .clamp(0, last);
                input[index as usize]
            });
            *value = interpolate(points[0], points[1], points[2], points[3], normalized);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_time_stretcher_matches_1728_float_pcm_cases_exactly() {
        let mut count = 0;
        for line in include_str!("editor_time_stretch/source.bits").lines() {
            let fields = line.split(';').collect::<Vec<_>>();
            let hz = fields[0].parse().unwrap();
            let channels = fields[1].parse().unwrap();
            let start = fields[2].parse().unwrap();
            let rate = fields[3].parse().unwrap();
            let bits = |text: &str| {
                text.split(',')
                    .map(|v| v.parse::<u32>().unwrap())
                    .collect::<Vec<_>>()
            };
            let input = bits(fields[4])
                .into_iter()
                .map(f32::from_bits)
                .collect::<Vec<_>>();
            let expected = bits(fields[5]);
            let mut output = vec![0.0; expected.len()];
            process(start, &input, hz, channels, &mut output, rate).unwrap();
            assert_eq!(
                output.into_iter().map(f32::to_bits).collect::<Vec<_>>(),
                expected,
                "original case {count}: {hz}Hz/{channels}ch start={start}, rate={rate}"
            );
            count += 1;
        }
        assert_eq!(count, 1728);
    }
    #[test]
    fn invalid_requests_leave_the_output_unchanged() {
        for (input, hz, channels, rate) in [
            (&[][..], 48000, 1, 1.0),
            (&[1.0][..], 0, 1, 1.0),
            (&[1.0][..], 48000, 0, 1.0),
            (&[1.0][..], 48000, 2, 1.0),
            (&[1.0][..], 48000, 1, f64::NAN),
        ] {
            let mut output = [0.25; 4];
            assert!(process(0, input, hz, channels, &mut output, rate).is_err());
            assert_eq!(output, [0.25; 4]);
        }
    }
}
