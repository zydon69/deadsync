# Main audio fidelity: confirmed remaining difference

## Current integration status

The direct Main PCM path, callback-global interpolation and f64 transport patches
are now applied. The root replay compared 432 physical callbacks against original
music PCM and cursor results exactly; its tick lists were empty. Cargo completed
237 audio tests with no failures and four ignored benchmarks
(`/tmp/deadsync-editor-main-f64-audio.log`). This includes 105 core, 64 facade and
68 stream tests. Native screen and shell compilation after the typed f64 glue
still need their current integration run. Nonempty assist/beat ticks and bounded
SFX retirement remain separate open work. The baseline and sidecar descriptions
below record the integration history, not the current wiring status.

Source: GrooveAuthor `48e18f71655d0f0d86144c63239331833501fda2`, `Sound/MusicManager.cs` 1040–1085 and complete `Sound/TimeStretcher.cs`; Fumen `0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89`.

The source main callback advances its integer sample cursor by `(int)(callbackFrames * MusicRate)` for each callback. For rates not reference-equal to 1 it calls `TimeStretcher.ProcessSound`, using four-point Fumen Hermite interpolation. Its time calculation uses a double start time and output offset, then normalizes by a float `1.0f / sampleRate`. Interleaved sample indices are clamped to the entire input array, including at channel edges. These details are observable and must not be replaced by an equivalent-looking per-frame linear or sinc interpolation.

The current native editor Main prepared-source path still uses the shared decoder/resampler. Disabling preserve-pitch proves rate/pitch isolation but does not prove the original interpolation or per-callback truncation contract. The new dedicated Preview source resampling proof also does not establish this Main contract. This remains an open port requirement. Gameplay retains its existing resampler contract.

## Executed original reference

`scripts/editor_time_stretcher_source_oracle.py` executed the complete unmodified source class against the real pinned Fumen DLL under .NET 10. All 1,728 cases completed: 4 sample rates × 3 channel counts × 3 input lengths × 6 signed/edge start indices × 8 rates. Each output contains exact IEEE float input/output bits and metadata; no independently rewritten interpolation supplies expected samples. The generated reference is `/tmp/deadsync-time-stretcher-root.json`, with adjacent `.reference.json` recording source/DLL/harness/output hashes. The script reproduces this artifact.

This reference proves only the original float PCM interpolation boundary. Native PCM comparison, codec precision, live callback blocks, rate changes, pending source readiness, ownership, outages and complete Main transport integration are not yet verified by it.

## Native interpolation kernel

`deadlib-audio-decode::editor_time_stretch::process` now ports this complete calculation with a shared `hermite::interpolate` primitive. The actual production modules were compiled directly with rustc and their two tests passed: all 1,728 original sample-bit cases, plus atomic rejection of invalid requests. A second original .NET replay reproduced the checked-in `source.bits` exactly; provenance includes its hash. This is not yet wired into the Main callback, and standalone tests do not replace the pending Cargo/transport validation.

## Main transport sidecar ready for parent validation

`patches/editor-main-source.patch` moves the pure kernel and its original fixtures to core, keeps decode API shims, and wires editor Main immutable PCM into physical callbacks with a callback-global interpolation offset and one integer truncation per physical callback. `editor-main-double-rate.patch` and the parent typed glue preserve the original f64 rate end to end for current Main producers. See `patches/editor-main-source-plan.md` for integration order, concurrency/RT contract, executed105 core tests and432 original full-callback PCM/cursor comparisons, and prepared backend tests not yet Cargo-validated. The earlier statement of a disconnected interpolation kernel describes the baseline before these sidecars.

The moved fixture is `crates/deadlib-audio-core/src/editor_time_stretch/source.bits`, with its original provenance beside it. The Preview resample oracle now reads core/hermite.rs and the primary decode/preview.rs; no stale sidecar copy supplies that comparison. Source-domain tick scheduling is a separate confirmed pending requirement; the full Main callback oracle uses empty tick lists/PCM and cannot certify ticks. HostNOW device-return policy remains an explicit host adaptation.
