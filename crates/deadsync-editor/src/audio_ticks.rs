//! MusicManager source tick windows; scheduling and voices stay in the host.
use crate::{Error, audio_preferences::EffectiveAudio};
use std::collections::BTreeSet;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TickKind {
    Assist,
    Beat,
}
#[derive(Clone, Copy, Debug)]
pub struct Head {
    pub row: i64,
    pub chart_time: f64,
    pub step: bool,
    pub fake: bool,
}
pub trait Clock {
    fn chart_from_song(&self, seconds: f64) -> Result<f64, Error>;
    fn song_from_chart(&self, seconds: f64) -> Result<f64, Error>;
    fn row_from_song(&self, seconds: f64) -> Result<f64, Error>;
    fn song_from_row(&self, row: i64) -> Result<f64, Error>;
    fn beat_span_end(
        &self,
        row: i64,
        _sample: i64,
        _attack: f64,
        _sample_rate: u32,
        _window_end: f64,
    ) -> Result<i64, Error> {
        Ok(row)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Tick {
    pub kind: TickKind,
    pub row: i64,
    pub start_sample: i64,
    /// Inclusive compressed beat-row run, stride48; assist events are singletons.
    pub end_row: i64,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Window {
    pub assist: Vec<Tick>,
    pub beat: Vec<Tick>,
}
#[derive(Default)]
struct Rows {
    single: BTreeSet<i64>,
    runs: Vec<(i64, i64)>,
}
impl Rows {
    fn from_ticks(ticks: &[Tick]) -> Self {
        let mut result = Self::default();
        for tick in ticks {
            if tick.row == tick.end_row {
                result.single.insert(tick.row);
            } else {
                result.runs.push((tick.row, tick.end_row));
            }
        }
        result
    }
    fn contains(&self, row: &i64) -> bool {
        if self.single.contains(row) {
            return true;
        }
        let index = self.runs.partition_point(|(_, end)| end < row);
        self.runs
            .get(index)
            .is_some_and(|(start, end)| start <= row && row <= end && row % 48 == 0)
    }
}
#[derive(Clone, Copy)]
pub struct Input {
    pub current_music_samples: i64,
    pub sample_rate: u32,
    pub assist_length: f64,
    pub beat_length: f64,
    pub budget: usize,
}
/// Build once per validated playback snapshot; window queries then start at a
/// binary-searched head instead of scanning an entire chart each frame.
pub struct Timeline {
    heads: Vec<Head>,
}
impl Timeline {
    pub fn new(heads: impl IntoIterator<Item = Head>) -> Result<Self, Error> {
        let mut accepted = Vec::new();
        let mut previous = f64::NEG_INFINITY;
        for (visited, head) in heads.into_iter().enumerate() {
            if visited >= 2_000_000 || head.row < 0 || head.row > i32::MAX as i64 {
                return Err(invalid());
            }
            if !head.step || head.fake {
                continue;
            }
            if !head.chart_time.is_finite()
                || head.chart_time < previous
                || accepted.len() >= 2_000_000
            {
                return Err(invalid());
            }
            previous = head.chart_time;
            accepted.push(head);
        }
        Ok(Self { heads: accepted })
    }
    pub fn window(
        &self,
        clock: &impl Clock,
        audio: EffectiveAudio,
        input: Input,
    ) -> Result<Window, Error> {
        window_inner(clock, &self.heads, audio, input)
    }
}
pub fn window(
    clock: &impl Clock,
    heads: &[Head],
    audio: EffectiveAudio,
    input: Input,
) -> Result<Window, Error> {
    Timeline::new(heads.iter().copied())?.window(clock, audio, input)
}
fn window_inner(
    clock: &impl Clock,
    heads: &[Head],
    audio: EffectiveAudio,
    input: Input,
) -> Result<Window, Error> {
    if input.sample_rate == 0
        || !input.assist_length.is_finite()
        || input.assist_length < 0.0
        || !input.beat_length.is_finite()
        || input.beat_length < 0.0
        || input.budget == 0
    {
        return Err(invalid());
    }
    let input = Input {
        budget: input.budget.min(2_000_000),
        ..input
    };
    let now = input.current_music_samples as f64 / input.sample_rate as f64;
    let (assist, beat) = if audio.assist_skip_beat {
        let beat = beats(clock, audio, input, now, &Rows::default())?;
        let rows = Rows::from_ticks(&beat);
        (assist(clock, heads, audio, input, now, &rows)?, beat)
    } else if audio.beat_skip_assist {
        let assist = assist(clock, heads, audio, input, now, &Rows::default())?;
        let rows = Rows::from_ticks(&assist);
        (assist, beats(clock, audio, input, now, &rows)?)
    } else {
        (
            assist(clock, heads, audio, input, now, &Rows::default())?,
            beats(clock, audio, input, now, &Rows::default())?,
        )
    };
    Ok(Window { assist, beat })
}
fn assist(
    clock: &impl Clock,
    heads: &[Head],
    audio: EffectiveAudio,
    input: Input,
    now: f64,
    skip: &Rows,
) -> Result<Vec<Tick>, Error> {
    if !audio.assist_enabled {
        return Ok(Vec::new());
    }
    let start = clock.chart_from_song(now - input.assist_length)?;
    let end = start + 1.0;
    let mut out = Vec::new();
    let mut previous = 0.0;
    for head in &heads[heads.partition_point(|head| head.chart_time <= start)..] {
        if head.chart_time <= start {
            continue;
        }
        if head.chart_time > end {
            break;
        }
        if !head.step || head.fake {
            continue;
        }
        let time = clock.song_from_chart(head.chart_time)?;
        if !out.is_empty() && time <= previous {
            continue;
        }
        if skip.contains(&head.row) {
            continue;
        }
        previous = time;
        out.push(Tick {
            kind: TickKind::Assist,
            row: head.row,
            end_row: head.row,
            start_sample: sample(time - f64::from(audio.assist_attack), input.sample_rate)?,
        });
        if out.len() > input.budget {
            return Err(invalid());
        }
    }
    Ok(out)
}
fn beats(
    clock: &impl Clock,
    audio: EffectiveAudio,
    input: Input,
    now: f64,
    skip: &Rows,
) -> Result<Vec<Tick>, Error> {
    if !audio.beat_enabled {
        return Ok(Vec::new());
    }
    let start = now - input.beat_length;
    let position = clock.row_from_song(start)?;
    if !position.is_finite() || position < i64::MIN as f64 || position >= i64::MAX as f64 {
        return Err(invalid());
    }
    // C# casts and integer division both truncate toward zero, including lead-in.
    let mut row = (position as i64) / 48 * 48;
    let mut out = Vec::new();
    let mut visited = 0usize;
    loop {
        let time = clock.song_from_row(row)?;
        if !time.is_finite() {
            return Err(invalid());
        }
        if time >= start + 1.0 {
            break;
        }
        let start_sample = sample(time - f64::from(audio.beat_attack), input.sample_rate)?;
        let end = clock.beat_span_end(
            row,
            start_sample,
            f64::from(audio.beat_attack),
            input.sample_rate,
            start + 1.,
        )?;
        let end_time = clock.song_from_row(end)?;
        if end < row
            || (i128::from(end) - i128::from(row)) % 48 != 0
            || end_time >= start + 1.
            || sample(end_time - f64::from(audio.beat_attack), input.sample_rate)? != start_sample
        {
            return Err(invalid());
        }
        // When beat ticks skip assist rows, split around those exact rows.
        // When assist ticks skip beat rows, no skip set is supplied, retaining
        // the complete compressed coverage for membership tests.
        let mut cursor = row;
        for blocked in skip
            .single
            .range(row..=end)
            .copied()
            .filter(|row| row % 48 == 0)
        {
            if cursor < blocked {
                out.push(Tick {
                    kind: TickKind::Beat,
                    row: cursor,
                    end_row: blocked - 48,
                    start_sample,
                });
            }
            cursor = blocked.checked_add(48).ok_or_else(invalid)?;
        }
        if cursor <= end {
            out.push(Tick {
                kind: TickKind::Beat,
                row: cursor,
                end_row: end,
                start_sample,
            });
        }
        visited += 1;
        if visited > input.budget {
            return Err(invalid());
        }
        row = end.checked_add(48).ok_or_else(invalid)?;
    }
    Ok(out)
}
pub fn sample(seconds: f64, rate: u32) -> Result<i64, Error> {
    if rate == 0 {
        return Err(invalid());
    }
    let v = seconds * rate as f64;
    if !v.is_finite() || v < i64::MIN as f64 || v >= i64::MAX as f64 {
        return Err(invalid());
    }
    Ok(v as i64)
}
fn invalid() -> Error {
    Error::InvalidEdit("Editor tick window exceeds supported numeric or scheduling limits".into())
}

/// Producer-side continuity for authoritative voice snapshots. Reset on every
/// transport boundary and source scheduling preference change. Retaining the
/// last past onset is necessary when attack time places it before the next
/// MusicManager lookback window, or after its head leaves that window.
#[derive(Default)]
pub struct VoiceWindows {
    assist: Vec<i64>,
    beat: Vec<i64>,
}
impl VoiceWindows {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn update(
        &mut self,
        window: &Window,
        current_music_sample: i64,
    ) -> Result<(Vec<i64>, Vec<i64>), Error> {
        fn one(
            previous: &[i64],
            ticks: &[Tick],
            current: i64,
        ) -> Result<(Option<i64>, Vec<i64>), Error> {
            let mut past = previous
                .iter()
                .copied()
                .filter(|onset| *onset <= current)
                .max();
            for tick in ticks.iter().filter(|tick| tick.start_sample <= current) {
                past = Some(past.map_or(tick.start_sample, |old| old.max(tick.start_sample)));
            }
            let mut samples = Vec::with_capacity(ticks.len().saturating_add(1));
            samples.extend(past);
            samples.extend(
                ticks
                    .iter()
                    .filter(|tick| tick.start_sample > current)
                    .map(|tick| tick.start_sample),
            );
            samples.sort_unstable();
            samples.dedup();
            if samples.len() > 2_000_001 {
                return Err(invalid());
            }
            Ok((past, samples))
        }
        // Prepare both halves before publishing either, including continuity.
        let (_assist_past, assist) = one(&self.assist, &window.assist, current_music_sample)?;
        let (_beat_past, beat) = one(&self.beat, &window.beat, current_music_sample)?;
        self.assist = assist.clone();
        self.beat = beat.clone();
        Ok((assist, beat))
    }
}
#[cfg(test)]
mod voice_window_tests {
    use super::*;
    fn tick(sample: i64) -> Tick {
        Tick {
            kind: TickKind::Assist,
            row: 0,
            end_row: 0,
            start_sample: sample,
        }
    }
    #[test]
    fn preserves_last_attack_until_reset_without_retriggering_old_future() {
        let mut state = VoiceWindows::default();
        let (a, _) = state
            .update(
                &Window {
                    assist: vec![tick(-1000), tick(100)],
                    beat: vec![],
                },
                0,
            )
            .unwrap();
        assert_eq!(a, [-1000, 100]);
        let (a, _) = state
            .update(
                &Window {
                    assist: vec![tick(100), tick(200)],
                    beat: vec![],
                },
                150,
            )
            .unwrap();
        assert_eq!(a, [100, 200]);
        let (a, _) = state.update(&Window::default(), 300).unwrap();
        assert_eq!(
            a,
            [200],
            "previously scheduled future trigger becomes the retained past voice"
        );
        state.reset();
        assert!(state.update(&Window::default(), -200).unwrap().0.is_empty());
    }
    #[test]
    fn duplicate_onsets_collapse_to_same_audible_restart() {
        let mut state = VoiceWindows::default();
        assert_eq!(
            state
                .update(
                    &Window {
                        assist: vec![tick(1), tick(1)],
                        beat: vec![]
                    },
                    0
                )
                .unwrap()
                .0,
            [1]
        );
    }
}

pub mod playback;
pub use playback::ChartTicks;
