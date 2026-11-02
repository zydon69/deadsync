# Editor float ticks and ordered source gains

## Delivered contract

`EditorAudioGains { main, music, assist, beat }` contains four independent finite `f32` levels in0..1. The native/Gameplay/Shell glue must preserve them separately. Main renders `((music * musicGain + assist * assistGain) + beat * beatGain) * mainGain`. Assist and beat have independent preallocated scratch buffers and fixed addition order, regardless of command insertion order. Ordinary SFX mix afterwards. Generic gameplay i16 preparation, streaming, panning, retriggering and gains retain their existing semantics.

Preview has a different original contract: MusicManager.cs969 computes `defaultVolume = MusicVolume * MainVolume`, then applies the selected fade, then multiplies PCM. That order is preserved. Legacy EditorPlayMusic remains its existing streaming/Cut route, with premultiplied fallback gains; this does not certify legacy source-Main parity.

`AudioControl::prepare_editor_tick(&Path) -> Option<SfxId>` is startup/control preparation, never called by hot TickWindow. `AudioControl::editor_tick_output_format(&self) -> Option<OutputFormat>` returns engine format orNone. The parent-owned glue prepares the two fixed editor clips at entry and on actual format changes. SfxId retains generic i16 plus optional editor float with a(Hz,channels) witness. Same-format DSP reopen may reuse it; a mismatched float handle is rejected, never silently used as i16. Effective duration comes from float frame count. AudioControl owns generation/asset/voice authority separately from immutable clip format.

## Callback and ownership

One coherent four-field tuple is captured at the beginning of each physical callback. Sequentially consistent atomic sequence/fields, two bounded read attempts and same-valid-owner last-tuple fallback prevent mixed publications. A config completed between2048-frame scratch chunks affects the following physical callback. Invalid/exhausted publication refuses before gain/owner changes; begin preflights before asset ownership mutation. Terminal release clears authority even when the sequence is exhausted. The application audio producer is the sole editor gain writer.

Two scratch buffers are allocated in RenderState construction; callback code does not allocate, load files, lock, wait or grow buffers. A source float tick has one logical restart slot per assist/beat bus. Float scratch assignment preserves signed zero before the source-ordered sum. Generic/legacy additive mixing still shares the same voice implementation. Both old public wrappers and the actual retiring path call common insertion and ordinary-voice processing; there are no copied ordinary mixer pipelines.

Float PCM is retained inside the credited source schedule, together with its source onset window and any legacy i16 PCM storage referenced by the same queued voice. Thus replacement/stale discard/cancel cannot perform final destruction of either large PCM allocation in callback, even when the cache/control producer closes first. The previous actual-ring bound remains unchanged: N=C+32active+1pending, production C128/N161. Returned snapshots are destroyed on control reclamation or after callbacks stop. Both endpoint teardown orders are tested for float and legacy backing storage. Render ownership tests share the existing global audio test guard to avoid fake callbacks racing Main clock fixtures.

## Executed original proof

`scripts/editor_float_ticks_source_reference.py` extracts unchanged GA pin48e18f71655d0f0d86144c63239331833501fda2 SetMusicRate, RenderMusicAndTicks, UpdateTicks, GetNextTickTimes, GetFirstTickInRange, CheckForStartingTick, GetTickValuesForSample, Mix, full SoundPlaybackState and TimeStretcher. It also extracts complete unchanged Fumen0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89 ResampleFloat and HermiteInterpolate. Actual bundled44100Hz assist/beat sample arrays pass through that original resampling.

The native comparison executes the actual core RenderState, real queue/retirement, Main prepared descriptor and both float tick descriptors. Four gain tuples include all nonunit levels, signed cancellation, some zero buses and an all-zero tuple. Rates .3/1.1/1.35, callback sizes257/2049/4097, source origins-3/0/42 and three successive physical callbacks produce324 comparisons. All108 fresh callbacks match the unchanged original float PCM hash and integer cursor. All324 match the diagnostic which changes ONLY physical-tail persistence.144 original tail-repeat differences remain explicit. The unchanged original is retained; no general tolerance is used.

Source's previously documented tail-tracking truncation bug is deliberately corrected: physical tails advance1x without repeating a frame at callback boundaries. Native's previously accepted defined EOF silence is also retained instead of the original repeated final DC value. The exercised reference cases avoid EOF disagreement; they do not conceal it with a tolerance.

## PCM boundary and remaining work

This lot fixes the float resampling/mixing boundary, not raw codec parity. Existing PreparedAudioSource still supplies i16. The actual assist asset is PCM24 stereo44100Hz/16538frames, and the beat is PCM16 stereo44100Hz/4725frames. The reference explicitly conditions on the existing decoder conversion (assist drops8bits), converts that input to float, then performs source float resampling. There is NO i16 requantification after it.

Fumen GetParseSampleFunc has distinct raw16/24/32/FLOAT conversion rules. The next, separate common editor decoder lot must preserve the appropriate input precision and normalization for Main, Preview and ticks together. Parent's original35-pattern parser witness is independent of codec/FM0D execution. Previous Main/Preview kernel oracles were also conditioned on supplied PCM; no full codec or universal audio bit-parity claim is made.

ReplayGain gameplay-global processing still exists on editor start/swap/reload paths; parent confirmed it is absent from the explicit source raw-parser/mixer pipeline. Its removal from editor-only routes belongs to the separate common decoder lot. FMOD itself was not executed. This lot's source PCM oracle explicitly resets music_target_gain to1. Generic gameplay behavior was not removed.

## Validation

- Core standalone:116PASS,0fail,2preexisting ignored, including both partial-publication and completed-between-chunks barriers.
- Original oracle:324 PCM hashes and integer cursors, no general tolerance.
- New core tests: partial gain publication barrier, publication completed between chunks, fixed tuple until next callback; source Preview nonunit default-volume order; invalid/MAX atomic refusal and terminal release; float/legacy PCM retirement under both teardown orders.
- New AudioControl test: real editor assets through prepare_editor_tick/public scoped Main+TickWindow/actual RenderState4097+257 callbacks at1.1/1.35, float bits against independently indexed dry float clips and explicit source gain formula, format refusal, no i16 round trip, effective float duration. Existing generic prepared-byte source-onset test remains.
- Agent does not run Cargo. Parent applies the held glue and executes full core/backend/Shell/native validation.
