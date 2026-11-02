# Original profile comparer oracle

Replay from a checkout with the immutable Git objects available:

```sh
python3 scripts/editor_profile_comparer_source_oracle.py \
  --source "$GROOVEAUTHOR_CHECKOUT" \
  --library "$STEPMANIALIBRARY_CHECKOUT" \
  --fumen "$FUMEN_CHECKOUT" \
  --dotnet "$DOTNET10" \
  --output /tmp/profile-comparers.expected.json \
  --provenance /tmp/profile-comparers.provenance.json
```

The script reads `git show` of GrooveAuthor48e18f71655d0f0d86144c63239331833501fda2 and immutable archives of StepManiaLibrarybdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8 and Fumen0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89. It rebuilds actual projects; no prebuilt model DLL or Rust implementation is used as expected output. Existing MIT notices for these source ports remain applicable.

## Executed scope

- Entire original `UIPatternComparer.cs` and `UIPerformedChartComparer.cs` byte-identical, including `SetSortSpecs` and explicit `IComparer.Compare` methods.
- Original table column enum declarations, unchanged.
- Actual library PatternConfig, PerformedChart.Config and nested configuration types, including real `RefreshStepWeightsNormalized` and IsEnabled helpers.
- Original source string-building/getter method bodies extracted unchanged into declared wrapper plumbing.
- 9 Pattern and12 Performed columns, ascending/descending, six real configurations per kind; 1,512 explicit directed pair comparisons plus42 full-list sorts and2 mixed-direction multi-spec sorts.
- Flags cover true/false/null, disabled effective zero, value ties and GUID tie-break, null/empty names, supplementary/private-use Unicode (UTF16 ordinal), all-zero/ratio-equivalent normalized weights, explicit/random/automatic foot choices, source starting/ending footing strings, and source StepSpeedMin ep1-for-both-sides bug.

Fixture and observed provenance are separate under `crates/deadsync-editor/tests/fixtures/profile-comparers/`. Provenance records source/method/archive/harness/stub/fixture hashes and SDK. Native differential test reads these expected results; Cargo was not run by the oracle producer.

## Plumbing and limits

ImGui pointer structs are transport stubs, not a graphical ImGui runtime. Wrapper fields/GUID/Name construction and notification no-op replace full EditorConfig lifecycle. PatternType is supplied as the aligned measure denominator from actual Config.BeatSubDivision*4; GetMeasureSubdivision is an identity transport stub. TravelSpeedMinBPM is assigned directly; original editor speed setter/update callbacks are not under test. These are declared in machine-readable provenance.

Original comparer and original string method logic is not rewritten. This does not execute graphical tables, profile persistence/Undo, or the entire editor. The source initial **ConfigData pre-table** cultural ordering is separate from first visible table ordering. There is no `DefaultSort` on Name (PatternTable67, PerformedTable69), and ImGuiUtils779–786 passes the column flags directly. The first implicit ImGui sort specification has not been executed/established here. Native initial cultural selector order is an accepted keyboard adaptation; this oracle makes no initial UI-order parity claim.
