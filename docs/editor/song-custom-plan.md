# Custom song metadata in SM

Prepared shared patch `song-custom-hooks.patch`, new helper
`src/song_properties/custom.rs`, three public workflows and one unit oracle.
No shared files applied or Cargo executed.

The actual Fumen metadata reader has executed 24 cases, recorded by
`editor-song-custom-reference.py` and `song-custom.*.json` with source hashes.
SMReader removes the exact SONG marker, then writes Extras with overwrite=true.
Bare and suffixed forms therefore resolve to the **last physical occurrence**;
Data and Version have independent owners. SSC retains suffixed keys separately,
so its song metadata uses bare keys only. GA DeserializeCustomSongData consumes
those canonical Extras; full EditorSong construction is not executed by this probe.

The helper resolves effective source order, including native added properties at
the same insertion point/order used by Document serialization. Reads, SyncOffset
editing and chart deletion share that selection. Mutations write the winning
existing key/range, preserving all losing aliases and unknown JSON fields. New SM
metadata uses original SONG spellings; existing bare-key files remain edited in
place. No automatic rewrite occurs on open.

Native compatibility differences remain explicit: Document already case-folds
bare metadata names. The helper retains that policy and also accepts lowercase
SM SONG markers, which original Fumen does not strip. Three oracle cases are
classified as native case-fold adaptations. Current strict native incomplete/
unknown-version errors remain stricter than GA's tolerant ignore/default path;
this patch does not relax those existing validation contracts.

Tests verify both alias orders, independent fields, empty creation, legacy bare
compatibility, exact Undo/Redo, save/reopen, unknown JSON/vendor preservation and
DefaultChartIndex after deletion. The export writer must reject SyncOffset drift
until these hooks are active, rather than silently changing explicit zero to the
nine-millisecond default.
