# MP3 source compatibility oracle

`mp3_before_timeline_options.rs` contains the production MP3 reader from commit
`0d1fba4c2890ff4b4423041d201ee3f63c575c87`, before caller-selected timeline
options. Only the original test module is omitted. This GPL-3.0 source remains
unchanged and is compiled only under `cfg(test)`; it is absent from the game.
Its full/production SHA-256 hashes, source path, commit and production fingerprint
are recorded in `mp3_before_timeline_options.json`. A Rust unit assertion pins the
complete copied source against accidental edits. Verify independently with:

```sh
python3 scripts/verify-mp3-reference.py --git
```

The optional Git check requires that historical commit to exist locally. Default
verification uses the included manifest. Cargo tests require neither Git nor
Python and execute the included original source with the locked dependencies.

The untrimmed compatibility test compares full PCM for Info, Xing, headerless and
ID3-prefixed variants, then every emitted sample after ten seeks for each variant.
Fixed sample counts, format metadata, duration bits, cursor targets and final
positions remain asserted for both implementations. Diagnostics report a bounded
sample index instead of printing whole PCM arrays.

The former golden values are preserved in `historical_mp3_hashes.json`. In this
execution environment both the current untrimmed decoder and the executing
original source produced hash 13793718411883935705, while the historical initial
Info hash was 15560969893992054619. This observation does not establish why the
old capture differs or claim portability across every toolchain. Updating those
values would not prove compatibility; executing the pinned original source gives
a directly reproducible comparison here. The historical values remain available
for investigating the old capture.

This proves the caller-selected untrimmed policy against the earlier reader
under the same locked codec dependencies. It is not an independent MP3 decoder
conformance test or a guarantee across future Symphonia/dependency changes.
