# Original PCM parser witness

Fumen pinned `0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89`. `scripts/editor_pcm_parser_source_oracle.py` extracts the unchanged `SoundManager.GetParseSampleFunc` method and executes it on 35 explicit raw samples in .NET10. Root execution passed on 2026-10-30; fixture/provenance under tests/fixtures/audio-pcm-parser.

This proves sample-bit conversion outputs only. Minimal SOUND_FORMAT enum names stand in for the FMOD enum. No FMOD file decoding, container support, timeline, channel mapping, resampling or playback is exercised. PCM8 is unsigned per the observed source method. PCM16/24 use positive signed maximum denominators. Float payloads include infinities and NaN to record raw parser behavior; this is not a downstream safety certificate.

The fixture is intended for the shared editor-only decode correction. Existing generic gameplay i16 codecs must retain their contract. Main/Preview/ticks should consume one shared preparation policy instead of independent normalization patches. Current decode still loses PCM24 precision and no fix is claimed by this witness.
