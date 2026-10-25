# Original active-section selection reference

```sh
python3 scripts/editor-generate-spacing-index.py \
  --editor-root /path/to/GrooveAuthor \
  --reference-root /path/to/StepManiaLibrary --dotnet /path/to/dotnet
cargo test -p deadsync-editor --test spacing_index_workflows --jobs 2 --locked
```

Development prerequisites: .NET 10 and the already built pinned Fumen assembly
under the existing reference checkout. Ordinary Rust tests need no .NET/Fumen.

This runner uses the actual Fumen RedBlackTree. The pinned original FindBest,
FindBestByTime, FindBestByPosition and five Ensure/Unset utility members are
extracted unchanged into ignored obj/ sources. Scalar event/config/position
adapters preserve original row-only/time-only comparison behavior. Normal event
tie ordering uses the supplied initialized chart order rather than reproducing
the entire EditorEvent comparator. All rate arrays come from the independently
generated original initialized-state fixtures, preserving their same-row order.

Seven fixtures contain 1113 queries, including empty trees, simultaneous events,
plateaus from negative stops/warps, negative lead-in, exact boundaries and adjacent
double values. Source/member and Fumen assembly hashes are recorded in the
manifest. This proves active-section lookup for valid ordered editor rate data;
it does not validate arbitrary malformed/nonmonotonic input trees or the full
native renderer. The product validates the original row/time ordering invariant
once, then uses binary search for per-frame active lookup.
