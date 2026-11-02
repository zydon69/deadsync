# Profile resolution at the actual document-loading boundary

Source witness: `scripts/editor_profile_load_source_oracle.py`, fixture `tests/fixtures/profile-load/source.expected.json`, pinned GrooveAuthor/Fumen commits recorded by that oracle. The root independently replayed all three original cases exactly before integration.

## One effective owner

`generation::profile_load::Plan` binds the candidate's Session revision token **and original Document source identity**, configured directory and actual profile bytes. It uses the existing `Loaded` catalogue/parser/registration order. The catalogue now optionally returns the precise bytes it parsed, including invalid-profile inputs; those must equal the captured directory snapshot. This prevents a transient read/parse disagreement from incorrectly treating an installed custom profile as missing. Directory membership, exact bytes, directory existence and configured path are checked again at application and publication. Cancellation brackets directory iteration, chunk reads, profile loading and plan application.

A valid chart's unknown expression resolves to Dynamic. Missing marker Pattern/Performed references resolve to Sixteenths/PatternBalanced. Known custom profiles remain selected. A typed repair report provides source-style warnings only for marker substitutions.

The plan surgically rewrites the existing GA metadata property; it does not create a second effective-profile map or add a fallback to any worker. Ordered raw JSON members retain unknown values and duplicate unknown members. Only the effective known GUID fields are replaced. SM uses its existing per-chart GA tag owner; SSC uses its chart owner. Note data, timing owners, chart identity, other chart/song fields and ignored raw SM tags are untouched.

The private Session baseline adoption preserves chart IDs/song-timing ownership, drops derived grid caches and invalidates old review epochs. Expression-only repair stays clean, revision0, with no Undo entry. Marker repair advances the content revision while retaining the original saved revision, so it is dirty with no hidden Undo. Later real edits use ordinary history; Undo returns to the normalized baseline and its correct dirty state. Original raw file bytes remain `FileSession.expected` for external CAS, Save/SaveAs and recovery.

## Product consumers

- Direct `State::open`, `open_selected`, `open_with_profiles` and `open_with_preferences` resolve before creating Workspace/presentation. `open_with_profiles` never opens against the default directory first.
- File/Open, Recent, startup and PublishedOpen workers capture the actual configured profile directory and resolve before `Snapshot::loadable_checked`.
- Pack song workers use the same preparation with their configured directory.
- Prepared workspace/finish and the final File publication barrier revalidate the catalogue; changing configured directory mid-load also refuses publication.
- Final validation occurs before pack/preference ownership transfer, audio teardown or replacement of the active State. Failed/cancelled preparation therefore leaves the old document/history and receipts intact.
- Actual workers, inspectors, copy/duplicate, snapshots/analysis, Save/export and recovery subsequently read the same on-document `ChartData` owner.

No per-frame catalogue I/O or later automatic re-normalization is added. A custom profile which disappears after publication remains an explicit later error/user operation; this loader never overwrites subsequent user choices.

## Recovery

A dirty marker-repair baseline is distinguished from user content by a typed content token (Session identity plus content revision). Only that exact initial baseline may bypass the ordinary dirty recovery refusal. A user edit changes the token and still refuses recovery. A foreign Session cannot reuse the baseline. Undo restoring the exact initial baseline restores the same eligibility.

Before replacing the document with a saved recovery, valid recovered metadata is resolved against the current configured catalogue as a detached candidate, preserving recovered chart IDs, policy, raw baseline CAS and dirty status. No initial-repair exemption survives recovery publication: recovered user content cannot be replaced again by pretending it is only a loader repair. Profile/source checks precede recovery receipt ownership transfer.

## Supplied validation, not executed by sidecar author

Core `profile_load_resolution` covers original three-case matrix × SM/SSC, exact no-load-Undo/dirty distinction, all unknown semantic fields and duplicate/raw-number members, real chart/pattern workers, actual generated apply/Undo/Redo/Save/reopen, present custom profiles, membership/byte/configured-context guards, stale/foreign plans, cancellation, source external change, clean-expression stale-plan invalidation, preexisting recovery and post-load user-edit refusal.

Native `pack_song_list::file_open::profile_resolution_tests` covers direct/configured open and actual File worker publication, changed catalogue/configured-directory/cancel refusal with source state preserved. `pack_song_load::profile_resolution_tests` drives the real pack worker from an actual scanned Row and checks configured profile preservation/publication guard.

The source oracle and tests do not prove unrelated profile-manager filesystem lifecycle, malformed/unknown-version GA metadata repair, or arbitrary generation-output parity. Unknown-version or currently unreadable custom metadata retains its prior loader behavior; this lot does not silently reinterpret it. Field-value preservation is scoped to GA metadata which the existing typed owner can read. Parse errors such as out-of-range unknown numeric JSON remain a separate existing owner limitation.
