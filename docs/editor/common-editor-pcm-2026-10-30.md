# Common editor source float PCM

## Boundaries

`PreparedAudioSource::load` and gameplay codec Readers retain their existing i16 contract and allocate no float view. Explicit `load_editor` prepares one shared source float view for Main, Preview, ticks and waveform. Streaming compatibility retains an i16 view; physical editor playback never reads that view when the float source is present. Conversion, allocation, decoding and channel mapping happen on workers, outside audio callbacks.

The standalone waveform pathname fallback uses `load_editor_limited` to bound retained float+i16 PCM by its explicit byte budget, then applies the existing independent packed-pyramid budget. Codec packet scratch stays separately bounded; managed Main waveform reuses the existing immutable source.

WAV reuses the existing RIFF parser; FLAC reuses the probe/codec but copies signed i32 before quantization. Source integer reciprocals are applied after restoring actual container bit depth. FLOAT32 preserves finite bits and signed zero. Nonfinite PCM is refused before immutable publication. Native FLOAT64 remains an explicit conversion capability, outside original FLOAT32 parity.

PCM8 deliberately uses standard centered `(byte-128)/128`. Original Fumen's unsigned byte/255 DC is documented by the unchanged raw parser witness; reproducing that DC is not intended.

MP3/Vorbis retain current native codec/timeline and apply the observed FMOD PCM16 source normalization. This does **not** establish FMOD codec parity: FMOD MODE.DEFAULT exposes different MP3 padding/length in the measured short MP3 fixture. Opus follows the original separate `.opus` packet-concatenation route (no pre-skip, header gain or final-granule trim). libopus and Concentus produce different hashes on the real encoded fixture; equal packet count is not a bit-parity claim.

## Executed original evidence

Pinned Fumen `0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89`, actual bundled FMOD macOS dylib, `NOSOUND_NRT`, `createSound(MODE.DEFAULT)`, original `AllocateSampleBuffer` and `FillSamplesAsync`; no playback channel or physical output. Reproduce with `scripts/editor_fmod_pcm_source_reference.py` and a pinned Fumen Release build.

FNV word hash uses initial1469598103934665603 then `(hash ^ float_bits)*1099511628211` wrapping u64.

| Asset | Original format | Native→48k frames | Original/native float hash |
|---|---|---:|---:|
| editor_assist_tick.wav | PCM24, stereo44100 |18000|507382426822696679|
| editor_beat_tick.wav | PCM16, stereo44100 |5142|13401652033342896737|
| Lossless FLAC from assist | PCM24, stereo44100 |18000|507382426822696679|
| Lossless FLAC from beat | PCM16, stereo44100 |5142|13401652033342896737|

All four hashes are exact actual decode+resample comparisons. This evidence does not establish all codecs, all formats or physical FMOD hardware equivalence.

The raw-parser witness remains `scripts/editor_pcm_parser_source_oracle.py`; PCM8 centering and nonfinite refusal are explicit adaptations, not edits to that original witness.

## ReplayGain

Source SoundManager loads raw MODE.DEFAULT data; editor Main/Preview apply source preferences and fades, without ReplayGain. Editor start/swap/preview and the legacy editor streaming caller therefore establish a new immutable track ID with unity replay gain and exclude that ID from late tagged background analysis. Generic gameplay analysis remains eligible for its own new IDs. Legacy Cut/loop/streaming semantics are retained.

`--opus-metadata-witness` rewrites only OpusHead gain (+6 dB), pre-skip (0) and final granule (0), with valid page CRCs and identical audio packets. Original Concentus on `tests/fixtures/seek.opus` returns44160frames/hash9979172453145087875 for both files. The native editor test requires identical float bits across the same metadata edits and explicitly checks that the unchanged gameplay Reader is affected. This establishes metadata policy; it does not equate Concentus and libopus PCM bits.

## Validation boundary

Standalone rustc decoder suite:40passed,0failed,3ignored; waveform actual module with a minimal Error host:4passed. Original FMOD witness and original Opus metadata-invariance witness executed successfully. Main/Preview real worker→Render ReplayGain tests and the legacy-editor→gameplay eligibility test are included for the parent's Cargo run; they have not been executed by this agent. No Cargo or primary-source mutation was performed.
