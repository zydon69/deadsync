# Live chart presentation order

`chart_order` ports `ChartComparer` from GrooveAuthor commit
`48e18f71655d0f0d86144c63239331833501fda2` (MIT), with its comparison
precedence: chart type, difficulty, rating, nonempty name, nonempty description,
nonfake step count, and resident GUID. Empty text sorts last. Unknown types do
not receive an invented alphabetical tie breaker. DeadSync includes Couple in
the comparator's original type positions; the reference's visible chart list
excludes the two Couple types.

`Snapshot::resident` captures current properties and working note grids, using
the existing editor clock for fake and warp exclusions. StepF2 Pump co-op stored
as `pump-double` compares as the effective `pump-routine` type, matching Fumen.
`Workspace::ordered_charts(collator)` returns original document indices. It does
not move active selection, reorder source sections, save a song, or dirty it.

`Session::chart_ids()` owns the runtime identities. Existing charts keep their
GUIDs when another chart is created, duplicated, generated, or deleted.
Structural document snapshots preserve the exact IDs through undo/redo and
abandoned redo branches. New/reopened/recovered sessions assign new IDs. No ID
is written into a simfile or confused with a generation-profile GUID.

The low-level comparator preserves unchecked signed subtraction from the C#
reference. `Snapshot::indices_with` rejects difficulty/rating ranges wider than
`i32::MAX`, which otherwise produce inconsistent comparison results in upstream
code. This prevents passing that known invalid ordering into Rust's sort.

## Evidence

`chart_order_workflows` has nine passing tests covering SM/SSC live properties,
pending notes, fake/warp exclusions, StepF2, creation/duplication/deletion,
generated batches, undo/redo, failed edits, abandoned redo, and save/reopen.

The committed `chart-order.reference.json` contains 54 chart keys and 8,748
comparisons in `en-US`, `fr-FR`, and `sv-SE`. The fixture generator extracts the
unchanged C# comparator into a property-only adapter and invokes actual .NET
`CurrentCulture` comparisons. Its source checksums and reference commit are
recorded in the fixture. Ordinary Rust tests require neither .NET nor the
reference checkout.

Regenerate with:

```sh
python3 scripts/editor-chart-order-reference.py \
  --reference-root /path/to/grooveauthor \
  --dotnet /path/to/dotnet \
  --output crates/deadsync-editor/tests/fixtures/grooveauthor/chart-order.reference.json
```

## Native navigation and ICU boundary

Ctrl+Tab and Shift+Ctrl+Tab now queue a direction in the native editor. The shell
processes that request through the stateless `NativeChartOrder` adapter, so no
ICU pointer is stored in editor or game state and no collator is opened on frames
without a request. `Workspace::chart_order_snapshot()` supplies the immutable
live inputs. ICU sort keys for every name/description are prepared before the
Rust sorting operation, so allocation/native errors cannot interrupt a sort.

The queued request is bound to Session content identity/revision, active source
chart GUID/index, and the localization revision. Those conditions are checked
before and after native work. Opening a panel, entering IME preedit, losing
focus, changing content, or changing language discards a stale request. The
screen rejects duplicate, missing, or out-of-range source indices before moving
focus. Errors use the localized `ChartOrderFailed` message and retain selection.
Serialization order and the toolbar's source chart number remain unchanged.

The ICU adapter loads system `libicucore.dylib` on macOS, standard ICU SONAMEs and
major-version symbols on Linux, and ICU DLLs from System32 on Windows. An RAII
owner closes the UCollator before unloading its library. Text is passed as UTF-16
with explicit checked lengths, including embedded NUL code units. The default
collator configuration follows [.NET's ICU comparison implementation](https://raw.githubusercontent.com/dotnet/runtime/main/src/native/libs/System.Globalization.Native/pal_collation.c).
Configured UI languages `en`, `fr`, and `sv` map to `en-US`, `fr-FR`, and `sv-SE`;
regional language tags retain their region. This deliberately follows the game
language rather than silently choosing a different operating-system culture.

ICU/CLDR versions can change comparison results between hosts. The backend
exposes its collation version for diagnostics. Missing ICU is an explicit error;
lexical sorting, libc collation, and Windows NLS are not advertised as equivalent
substitutes. Windows/Linux library loading has not yet been exercised on those
operating systems in this work.

A read-only macOS experiment loaded system ICU 78.1 and matched all 121 text
pairs in each of the three fixture cultures. New Rust shell tests compare actual
ICU calls and sort keys against those culture fixtures, exercise contrasting
English/French/Swedish chart order, and cover resource independence, input
validation, embedded NULs, and canonical-equivalent accents. All three shell ICU contract tests passed on macOS, including all 363 real
text comparisons against the .NET fixtures. All 12 native EN/FR tests passed for deferred navigation, invalid permutations,
stale requests, localized errors, source gaps and zero/one editable chart. The
45 affected attack-event/widget and time-spacing regression tests also passed.
These use synthetic inputs and an injected orderer; actual shell ICU execution
is recorded separately.

### Unsupported chart coverage

The unsupported-layout issue identified during source review has been corrected
in code: snapshots load only editable layouts and expose their original source
indices. Transports must return an exhaustive permutation of this eligible set,
rather than assume compact indices starting at zero. Native requests with fewer
than two eligible charts avoid opening ICU. New domain/native tests cover source
gaps, unknown layouts, invalid compact indices, and zero/one editable chart; the
ICU culture-order test now also includes an unsupported source prefix. All nine
domain tests, all 12 native tests and the three actual ICU contract tests
passed. Linux/Windows execution and physical keyboard/runtime review remain
separate validation work.
