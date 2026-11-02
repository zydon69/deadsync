# DSP: source contract and shared editor audio context

## Final decision (parent validated)

GrooveAuthor UI says DSP changes take effect after application restart; Editor.cs constructs SoundManager once from these preferences. Therefore raw DSP preferences persist and Undo immediately, with restart guidance, and produce no live DSP reconfiguration. The existing whole-PreferencesAudio source oracle covers raw/default/history behavior; no FMOD or hardware oracle is claimed. The former hot-preference draft is separate and is not the porting requirement.

A single lazy EditorAudioSession is transferred unchanged between published edition::State values. Its token and opaque output receipt are independent of document identity. Detached candidates never activate audio. The transfer occurs after the final canonical publish_file_candidate barrier and is non-fallible. New/Open/Reload/Pack/NoSong retain the exact same session; they never apply newer DSP preferences. Only actual CloseEditor/return to gameplay/OS Exit restores the pre-editor output with acknowledgment before ending gains or resuming another transport. No competing Practice owner is introduced. Notes owns this host/session/bridge/UI adaptation.

At context entry, source logical DSP values are captured once. Only Adjustable fields may form a native request; Unsupported/ServerControlled fields remain raw in preferences and are explicitly shown as not applied. Partial backend requests never justify displaying the entire source configuration as applied. Source logical DSP buffer size is distinct from a device period/server quantum; Pulse's queried minreq is the client render/write quantum, not a hardware period or literal buffer count.

## Delivery

- `patches/editor-dsp-context-backends.patch`: native manager/backend capability/request/negotiation and portable DTO module. 14 files.
- `patches/editor-dsp-context-api.patch`: two new runtime modules for exact context initialization/restoration, opaque receipt and injected host transaction tests.
- `patches/editor-dsp-runtime-hooks.py`: pure fail-closed merge helpers runtime/lib/corelib, intended for notes' generator. Needs all three hooks plus SfxCache::rebind_output from notes. Do not apply backend patch alone then compile: StreamReady has new manager/buffer fields that must be retained by AudioEngine.
- Durable module mirrors: `patches/editor-dsp-output-reconfigure.rs`, `patches/editor-dsp-output-transaction.rs`.
- `patches/editor-dsp-output-DRAFT.patch` is an obsolete intermediate checkpoint, NOT for application.

## Receipt/lifecycle

OutputRestoreReceipt is Clone through Arc authority and immutable configuration snapshot. It retains exact InitConfig, the pinned already-selected backend plan, shared MixControls, requested values (including None defaults), and observed effective state required to undo persistent CoreAudio frame-size changes. Capturing a new owner replaces the token even for equal owner/value data. A stable context receipt follows only its own publication epochs; an exact rollback receipt expires on another publication. release_output_lease revokes late commands after actual exit.

The device manager owns an explicit shutdown/join handle. Native shutdown result is reported after native stop/join and decoder teardown. Retained old command senders cannot keep the old manager alive. Native shutdown errors and thread panics are not acknowledgments. No second output is opened after an unconfirmed stop. A mismatched replacement is stopped before rollback. Failed rollback remains unavailable, never a fake restored state.

Initialization/reopen enforces the previous sample rate, channel count, backend and device label, preserving retained SFX sample interpretation. Shared controls and scoped gain overlay survive. SfxCache rebind stops old owned voices and changes only the sender. A fresh PlayedMapReader becomes MusicClock on the host during poll/ACK, not silently on the worker. Epoch and receipt objects are prepared before taking the engine; receipt clones do not renew authority.

## Actual capabilities

- ALSA: requested frames/period count, actual final run parameters queried before ACK; a count is reported only for an exact period/total multiple.
- CoreAudio: query frame-size settable property; frames only if supported; no count. Restore observed original frame size even when original requested setting was None.
- WASAPI: shared custom period requires IAudioClient3 engine-period query; Windows 7 shared fallback reports unsupported for explicit periods. Exclusive aligns requested duration. Actual initialized period is reported, count unsupported.
- Pulse: async libpulse context/stream and nonblocking mainloop; sample spec and final buffer attrs queried before ACK. No dependency on libpulse-simple and no reconstruction of actual attrs from requests. Client minreq frames measured; count unsupported. Bounded/cancellable startup and writable waits.
- JACK: read-only server period; count unsupported; never calls global jack_set_buffer_size. Confirm jack_client_close before reclaiming callback state.
- PipeWire: server quantum control is refused; actual initial observation may remain unknown. Synchronous loop shutdown precedes callback state destruction. Never force global graph quantum.
- FreeBSD remains safely supported for shutdown; new DSP controls are unsupported until a verified mapping exists.

## Validation state

No Cargo/rustc/tests/hardware QA executed by media agent. Rustfmt parsed all isolated Rust files. `git apply --check patches/editor-dsp-context-backends.patch` passed against current primary. Hook generation/parse passed against notes' copy before its generator integrated the hooks; fail-closed repeated application correctly rejects an already-merged copy.

20 new prepared unit tests: DTO contract4; actual manager ACK/join/error4; shared WASAPI request negotiation1; Pulse queried-attribute validation1; common injectable-host lifecycle7; opaque ownership/quiescence guard3. The common host tests exercise the production transaction decision path with a fake host, not native hardware. Parent must execute supported host tests and platform compilation (including Linux/Windows); none is claimed green here.

Official references: [Pulse stream API](https://freedesktop.org/software/pulseaudio/doxygen/stream_8h.html), [Pulse mainloop API](https://freedesktop.org/software/pulseaudio/doxygen/mainloop_8h.html), [Pulse context API](https://freedesktop.org/software/pulseaudio/doxygen/context_8h.html), [JACK server control](https://jackaudio.org/api/group__ServerControl.html), [CoreAudio frame range](https://developer.apple.com/documentation/coreaudio/kaudiodevicepropertybufferframesizerange), [PipeWire properties](https://docs.pipewire.org/devel/page_man_pipewire-props_7.html).


## Recovery after failed initialization

When rollback cannot open output but all stop acknowledgments were confirmed, the pre-editor receipt can reopen output even with engine=None; real handles/clock/cache are created only after actual ready. Shutdown failure or unknown worker panic is quarantined, returns FailedUnavailable/ShutdownFailed, and forbids another open over uncertain callback ownership. The host must expose an explicit no-audio/exit resolution instead of waiting for an impossible restore or claiming success. No source data guard is bypassed.
