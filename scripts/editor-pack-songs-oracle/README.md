# Original PackSong metadata/list oracle

Run `python3 scripts/editor-pack-songs-reference.py --editor-root /path/to/grooveauthor --reference-root /path/to/StepManiaLibrary --dotnet /path/to/dotnet --output crates/deadsync-editor/tests/fixtures/grooveauthor/pack-songs` from the repository root.

The generator compiles the whole unchanged pinned `PackSong.cs`, including both comparers, against real pinned Fumen metadata readers. The 116 generated directory cases (18 discovery/metadata cases, six numeric boundaries, 45 grammar forms in each SM/SSC format and two mixed-rating cases) exercise format preference, exact lowercase extension discovery, chartless/header-only/bad-note data, ratings, null/transliterated titles, cancellation and en-US/fr-FR list sorting. Generated temporary trees are removed on exit; no user pack is read or written.

`PackLoadTask.cs` and the original UI are reviewed source provenance, not executed GUI/queue behavior. The Program applies their original extension predicate, records actual filesystem enumeration and passes selected file infos into original PackSong. Same-extension selection is host enumeration dependent; the fixture records this rather than inventing a portable order. SDK and Git blob/source/program/assembly/request/output SHA256 details are in `pack-songs.reference.json`.

The source comparator's difficulty-name branch actually queries Chart.Type; this is retained verbatim. Metadata-only Fumen accepts known SSC charts with no NOTES tag, handled by the proposed shared read-only metadata mode while editable Document parsing remains strict. See `docs/editor/pack-song-list-plan.md` before implementing an adapter or claiming reader parity.

The harness records the unchanged original comparer matrix and sorted chart indices. Close finite decimal ratings agree with the native total order; NaN, equal infinities and signed zeros require an explicitly documented native ordering adaptation. Special numeric spellings are compared numerically, including rejected abbreviated inf and accepted full Infinity/NaN forms.
