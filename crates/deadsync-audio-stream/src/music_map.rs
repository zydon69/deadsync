use deadlib_audio_core::{
    PlaybackPosMap, PlayedMapReader, bump_music_map_generation, music_map_generation,
    music_track_start_frame,
};
use std::sync::{LazyLock, Mutex};

#[derive(Default)]
struct MusicMapRuntime {
    played: Option<PlayedMapReader>,
    map: PlaybackPosMap,
}

/// Process-global played-position reader restored from the pre-v0.5.654
/// implementation.
///
/// The audio callback remains the sole producer. Application clock snapshots
/// serialize access to the reader and its derived map through this mutex, and
/// timeline resets synchronously discard every queued segment before handing
/// the new generation to the decoder.
static MUSIC_MAP_RUNTIME: LazyLock<Mutex<MusicMapRuntime>> =
    LazyLock::new(|| Mutex::new(MusicMapRuntime::default()));

fn force_music_map_runtime() {
    LazyLock::force(&MUSIC_MAP_RUNTIME);
}

fn install_played_map(played: PlayedMapReader) {
    let mut runtime = MUSIC_MAP_RUNTIME.lock().unwrap();
    runtime.played = Some(played);
    runtime.map.clear();
}

/// Clear the derived map and drain the callback-to-application transport at a
/// music timeline boundary. This intentionally matches the behavior before
/// v0.5.654 rather than deferring cleanup until the next clock snapshot.
#[allow(clippy::redundant_pub_crate)]
#[inline(always)]
pub(crate) fn clear_music_pos_map() -> u64 {
    let mut runtime = MUSIC_MAP_RUNTIME.lock().unwrap();
    let generation = bump_music_map_generation();
    runtime.map.clear();
    while runtime
        .played
        .as_mut()
        .and_then(PlayedMapReader::pop)
        .is_some()
    {}
    generation
}

/// Discard mappings from the previous asset without changing transport time.
/// The callback rejects its PCM by asset epoch before the next ownership read.
pub(crate) fn discard_music_asset_map() {
    let mut runtime=MUSIC_MAP_RUNTIME.lock().unwrap();runtime.map.clear();
    while runtime.played.as_mut().and_then(PlayedMapReader::pop).is_some(){}
}
fn drain_played_map(runtime: &mut MusicMapRuntime) {
    let generation = music_map_generation();
    while let Some((segment_generation, asset_epoch, segment)) =
        runtime.played.as_mut().and_then(PlayedMapReader::pop)
    {
        if segment_generation == generation && asset_epoch==deadlib_audio_core::music_asset_epoch() {
            runtime.map.insert(segment);
        }
    }
}

fn lookup_music_position(stream_frames: f64, sample_rate: u32) -> Option<(f64, f32)> {
    let mut runtime = MUSIC_MAP_RUNTIME.lock().unwrap();
    drain_played_map(&mut runtime);
    runtime
        .map
        .search(stream_frames)
        .map(|(music_seconds, seconds_per_frame)| {
            (
                music_seconds,
                (seconds_per_frame * f64::from(sample_rate)) as f32,
            )
        })
}

fn assist_tick_stream_frame_for_music_seconds(music_seconds: f64) -> Option<u64> {
    if !music_seconds.is_finite() {
        return None;
    }
    let track_frame = {
        let mut runtime = MUSIC_MAP_RUNTIME.lock().unwrap();
        drain_played_map(&mut runtime);
        runtime.map.invert(music_seconds)?
    };
    if !track_frame.is_finite() || track_frame < 0.0 {
        return None;
    }
    Some(music_track_start_frame().saturating_add(track_frame.round() as u64))
}

/// Compatibility handle retained so the later application-owned audio-control
/// refactor does not need to be unwound. The played-position reader and map are
/// global again; this handle stores only the immutable output sample rate.
pub struct MusicClock {
    sample_rate: u32,
}

impl MusicClock {
    pub(crate) fn new(played: PlayedMapReader, sample_rate: u32) -> Self {
        install_played_map(played);
        Self {
            sample_rate: sample_rate.max(1),
        }
    }

    /// Construct the inert handle used when startup continues without audio.
    #[must_use]
    pub fn without_audio() -> Self {
        force_music_map_runtime();
        Self { sample_rate: 1 }
    }

    pub(crate) const fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub(crate) fn lookup(&self, stream_frames: f64) -> Option<(f64, f32)> {
        lookup_music_position(stream_frames, self.sample_rate)
    }

    /// Source MusicManager compares integral music sample positions. Crossing
    /// an onset at a fractional output frame occurs at ceil(frame), preserving
    /// its integer fragment cursor; frame zero is represented explicitly.
    pub fn editor_tick_trigger(&mut self, music_sample: i64, sample_rate: u32) -> Option<deadlib_audio_core::ScheduledTrigger> {
        if sample_rate == 0 { return None; }
        let seconds = music_sample as f64 / f64::from(sample_rate);
        let relative = {
            let mut runtime=MUSIC_MAP_RUNTIME.lock().unwrap();
            drain_played_map(&mut runtime);
            runtime.map.invert(seconds)?
        };
        if !relative.is_finite() || relative < i64::MIN as f64 || relative >= i64::MAX as f64 { return None; }
        let absolute=i128::from(music_track_start_frame())+i128::from(relative.ceil() as i64);
        if absolute > i128::from(u64::MAX) { return None; }
        Some(if absolute < 0 {
            deadlib_audio_core::ScheduledTrigger {frame:0,skipped_frames:u64::try_from(-absolute).ok()?}
        } else {
            deadlib_audio_core::ScheduledTrigger {frame:absolute as u64,skipped_frames:0}
        })
    }

    /// Source MusicManager schedules ticks against its logical sample cursor
    /// even while no music sample data exists. This does not insert a PCM map.
    pub fn editor_source_tick_trigger(&mut self,music_sample:i64,sample_rate:u32)->Option<deadlib_audio_core::ScheduledTrigger>{
        if let Some(mapped)=self.editor_tick_trigger(music_sample,sample_rate){return Some(mapped);}
        if sample_rate==0{return None;}
        if let Some(control)=deadlib_audio_core::editor_main_configuration(){
            if deadlib_audio_core::music_track_active()&&control.asset_epoch==deadlib_audio_core::music_asset_epoch()&&control.generation==music_map_generation(){
                let cursor=deadlib_audio_core::editor_main_cursor().filter(|c|c.asset_epoch==control.asset_epoch&&c.generation==control.generation&&c.sample_rate_hz==sample_rate);
                let source_frame=cursor.map_or(control.frame,|c|c.source_frame);let physical_frame=cursor.map_or(control.callback_frame,|c|c.callback_frame);
                let offset=((music_sample as f64-source_frame as f64)/control.rate).ceil();
                if !offset.is_finite()||offset<i64::MIN as f64||offset>=i64::MAX as f64{return None;}
                let absolute=i128::from(physical_frame)+i128::from(offset as i64);if absolute>i128::from(u64::MAX){return None;}
                return Some(if absolute<0{deadlib_audio_core::ScheduledTrigger{frame:0,skipped_frames:u64::try_from(-absolute).ok()?}}else{deadlib_audio_core::ScheduledTrigger{frame:absolute as u64,skipped_frames:0}});
            }
        }
        let (origin,rate)=deadlib_audio_core::seeded_music_position(0.)?;
        let relative=((music_sample as f64/f64::from(sample_rate)-origin)/f64::from(rate))*f64::from(self.sample_rate);
        if !relative.is_finite()||relative<i64::MIN as f64||relative>=i64::MAX as f64{return None;}
        let absolute=i128::from(music_track_start_frame())+i128::from(relative.ceil()as i64);
        if absolute>i128::from(u64::MAX){return None;}
        Some(if absolute<0{deadlib_audio_core::ScheduledTrigger{frame:0,skipped_frames:u64::try_from(-absolute).ok()?}}else{deadlib_audio_core::ScheduledTrigger{frame:absolute as u64,skipped_frames:0}})
    }

    pub fn assist_tick_stream_frame(&mut self, music_seconds: f64) -> Option<u64> {
        assist_tick_stream_frame_for_music_seconds(music_seconds)
    }
}
