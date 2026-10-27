# Semantic chart layout aliases

## Status

Prepared, **not applied**: `layout-alias-hooks.patch` depends on the common
`document::chart_type` declaration in `open-preferences-core-hooks.patch`.
Both patches apply sequentially in a temporary tree of the current sources.
No production file was changed for this lot. No Cargo or GUI validation has run.

New files accompanying the patch:

- `src/document/layout_token.rs`: existing source token span, one private unit.
- `tests/layout_alias_workflows.rs`: four workflows.
- `scripts/editor-layout-alias-reference.py` and the three `layout-aliases.*.json`
  fixtures: real Fumen metadata/full reader oracle, already executed.

Paths above are relative to `crates/deadsync-editor` except `scripts/`.

## Source contract and evidence

The fixture reference records the exact SML/Fumen pins, relevant source blobs,
SDK, program, request/output hashes and assembly identity. Its 370 cases execute
real Fumen readers; 48 include full note loading. They cover all 43 enum types in
hyphen, underscore and ordinal spelling; four Outfox aliases; escaped delimiters;
comments/spacing; unknown and invalid values; and Pump StepF2 ownership.

The Outfox aliases map to `smx-dual`, `smx-full` and `smx-team`. The enum grammar
also admits comma-separated real names by bitwise OR despite not being Flags.
Unknown names and out-of-range ordinals get no native geometry; Rust preserves
those source records rather than copying the source reader's ordinal-999 crash.

Fumen's SSC type parser drops eight padded/commented alias cases that its SM
reader accepts. **Native adaptation:** retain Document's established shared
trim/comment decoding for SSC. Normalize only the resulting semantic descriptor;
never trim or rewrite the source tag on load. The oracle test names and counts
these eight acceptances separately rather than claiming byte/parser equivalence.

## Implementation contract

`parse_parts` canonicalizes only recognized decoded chart type values. Raw SM
headers, SSC tags, source ranges and serialization remain unchanged. Readonly
metadata uses that same tokenizer. Unknown decoded values remain unknown.

The native geometry helpers keep an allocation-free canonical hot path and use
the common table only for aliases. StepF2 detection/minimum owners and the public
palette helpers use that same meaning. Unsupported format families remain
uneditable even when their enum name is known.

An explicit structural layout edit reads the lexical token from its original
range, including escapes. It no longer searches for the canonical descriptor in
raw bytes. The existing decoder and comment skipper determine whitespace/token
boundaries; empty/comment-only fields return an error, never windows(0). Existing
last-STEPSTYPE duplicate policy is unchanged. Surrounding comments remain exact.

## Prepared checks and limits

The four workflows cover all reader fixtures and full owner/lane streams,
effective Snapshot types, public raw alias geometry/StepF2 APIs, escaped/numeric
Pump normalization with Undo after Save, plus Team aliases through views, clock,
music path, palette, note edits, Undo/Redo, save and reopen. Raw unknown property
bytes and original alias spelling survive ordinary note edits. The private unit
covers escaped token spans, comments, Unicode whitespace and empty input.

These Rust checks are prepared, not executed. Music checks validate a synthetic
path/clock only, without decoding audio. Core views/palette checks do not claim
native GUI QA. Explicit layout replacement can replace comments embedded within
the lexical token itself; surrounding comments and untouched properties remain.

## Integration checkpoint
Semantic canonicalization and raw lexical replacement are applied. Four core
workflows passed, including all 370 original-reader cases, escaped StepF2
normalization and SMX team edits/save/reopen. The music-time fixture explicitly
sets SyncOffset=0 instead of mistaking the normal 9ms default for an alias error.
The private lexical-span unit is awaiting the central unit run.
