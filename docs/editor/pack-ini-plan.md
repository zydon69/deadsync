# Pack.ini metadata, dirty state and guarded persistence

## Status and scope

The real original C# oracle has executed successfully: 37 reader cases and six
creation/default cases, with clean pinned sources, Git blob IDs and SHA256
provenance recorded in `pack.reference.json`. The declared model/codec in
`src/pack.rs` and `src/pack/codec.rs` passed all six `pack_metadata_workflows`
tests, including a resumed replay on 2026-10-26. The declared storage adapter
passed all 13 file workflows and six internal failure/race tests on macOS. No user pack file or shared storage adapter
has changed. Shift core six/native ten checks passed before checkpoint
`543e3d106`.

The model provides seven values plus Version, first-wins parsing, injected UTC
creation defaults, original canonical writer output, source-preserving edits,
and prepared snapshots with explicit save acknowledgement. Acknowledgement is a
caller contract, not evidence of disk publication. Missing/new metadata remains
dirty until that acknowledgement; edits after prepare remain dirty against the
saved snapshot. Foreign or obsolete receipts are rejected.

Root owns Save As/storage integration, media owns minimap, and ordering owns
receptors/rendering.
This first Pack increment should implement discovery, seven editable metadata
properties, explicit creation, dirty state and guarded save/external-change
handling. Pack song-library browsing, pack deletion and banner decoding are
separate original features and must remain visible follow-up work rather than
being implied complete by metadata editing.

## References reviewed

GrooveAuthor `48e18f71655d0f0d86144c63239331833501fda2`:

- Whole `StepManiaEditor/EditorItgManiaPack.cs`: property wrapper, cloned saved
  state, creation/save, dirty check, watcher and conflict modal.
- `EditorPack.cs:81–96,141–230,250–310`: display-name fallback, pack creation/save
  queue, pack directory resolved as parent of the active song directory, busy
  guard and reload/disposal.
- `UI/UIPackProperties.cs:92–212`: folder-name/banner display, missing-file
  creation, metadata controls, sync-default precedence, save and explicit
  non-undoable delete confirmation.
- `PackLoadTask.cs:72–125,193–230`: exact Pack.ini name, song enumeration and
  banner fallback ordering.

Fumen `0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89`:

- Whole `Fumen/Converters/ItgManiaPack.cs`: LoadAsync, Save, Matches and Clone.
- Whole `Fumen/Converters/IniFile.cs`: actual parsing and atomic writer path.
- `SMCommon.cs:247–248`: NULL=0.0 seconds, ITG=0.009 seconds.

The original calls into these functions were examined, not inferred solely from
class names or a filename search. The pinned source is present locally through
the existing original-library checkout; a real oracle is proposed below.

## Fields and source value semantics

The exact filename is **Pack.ini**, with one `[Group]` section. Version is a file
field, not an eighth editable pack property.

| INI key | Editor property/type | Loaded/default/new behavior |
| --- | --- | --- |
| Version | integer format marker | Missing or malformed causes original load failure. Future or <1 integer versions warn and parse using known fields. Original writer always emits1. |
| DisplayTitle | Title/string | Missing reads null. New file uses the pack folder name. Display uses a nonempty explicit Title, otherwise folder-name fallback. |
| TranslitTitle | TitleTransliteration/string | Missing null; new file empty. |
| SortTitle | TitleSort/string | Missing null; new file uses the folder title. |
| Series | string | Missing null; new file empty. |
| Year | signed32 integer | Missing/malformed stays0 with warning; original loader accepts signed integers. UI editing has minimum0. New file uses the UTC year. |
| Banner | string/path text | Missing null; new file uses discovered banner path or empty. This field does not authorize moving or copying an asset file. |
| SyncOffset | enum NULL/ITG | Original default is NULL when missing or invalid text. Its Enum.TryParse also accepts numeric enum input; oracle cases must record out-of-range numeric behavior. New-file priority is active-song sync, preferred new-song sync, then ITG. Only recognized 0/9 ms values are chosen by that priority. |

`UIPackProperties.cs:172–180` describes SyncOffset as pack metadata. Changing it
must not rewrite song OFFSET, song editor SyncOffset or chart timing. The
source's wording about ITGmania use is source-era documentation, not a new
current-product claim. No external web fact is needed to implement the format.

## Exact original parser contracts to preserve or classify

`IniFile.LoadAsync` works line by line. It recognizes comment markers only at
the first character (`;`, `#`, `//`, `--`). A group requires `[` at column0 and
`]` at the end of that line. Group names and key lookup are case-sensitive.
Keys are trimmed around the first `=`; the entire remainder is retained as the
value, including spaces and further equals signs. Duplicate keys in a group use
the first occurrence. ItgManiaPack reads the first exactly named `Group` section.

A generic INI crate must not silently replace these rules with case folding,
last-wins duplication or universal value trimming. The original loader warns on
missing optional properties and unsupported versions; these are not grounds for
inventing a blanket read or edit prohibition.

The original writer rebuilds a new Group with known properties, discarding
unknown keys/comments/groups and writing version 1. Rust should retain its
established preservation contract: untouched source bytes remain unchanged,
known edits patch the effective first key in the effective first Group, and
unknown content/duplicate shadows remain intact. Retaining future version bytes
while editing known fields is an explicit preservation difference from the
original writer, not asserted byte-for-byte writer parity. New files use the
original canonical field order and version 1.

Encoding/BOM/newline rules must be established by executed original reader/
writer fixtures before declaring coverage. Include UTF8, BOM, CRLF, invalid
byte sequences and UTF16 samples. Do not presume the SM/SSC RSSP text decoder is
also an equivalent INI parser.

## Missing file, creation and dirty state

- Opening or inspecting a pack without Pack.ini never creates one. Missing,
  unreadable and malformed are separate outcomes; an invalid existing file must
  never be treated as absent and overwritten by an Add action.
- Creation is explicit. It binds to the canonical parent of the active song
  directory and the fixed filename, and fails if a file appeared concurrently.
  Do not scan ancestors guessing a pack root or create directories automatically.
- Creation defaults should be a pure function receiving folder name, discovered
  banner, active/preferred sync and injected UTC year. This makes the source
  priority testable without time-dependent fixtures.
- Original CreateNewPack clones initial values and immediately attempts save.
  A failure still returns the in-memory object, so dirty state alone must not be
  mistaken for disk existence. Rust should model persistence/existence explicitly
  and retain retryable values after a failed creation.
- Dirty compares all seven semantic values against the last successfully saved
  snapshot (`Matches`, `EditorItgManiaPack.cs:229–232`). Missing/null strings and
  explicit empty strings are distinct in original Matches; displaying an empty
  input must not silently convert untouched nulls into edits. The original
  writer serializes null as an empty INI value, a normalization the oracle should
  record separately from draft equality. Read-only discovery,
  banner preview and formatting differences must not mark the simfile dirty.
- Save clones its payload at the beginning. Only successful persistence updates
  the saved snapshot. If live fields change while a prepared save is in flight,
  they remain dirty against that saved snapshot. Do not mark the latest unsaved
  editor object clean just because an older snapshot was saved.
- Pack edits need a separate dirty indicator and explicit Save Pack action;
  original editor can have unsaved song, pack, or both. Global undo integration
  should carry the owning pack identity and values, without turning a pack-only
  edit into a simfile content edit. Design this bridge with root before changing
  Session::Change or close/save policy.

## Proposed storage boundaries and conflict policy

Coordinate with root's Save As work before implementing common disk primitives.
Reuse a small reviewed storage adapter rather than diverging copies of atomic
replacement and conflict checking.

1. Resolve the pack location from the opened song file. Bind the pack root and
   target identity; no arbitrary path supplied by an edited metadata field may
   become a write target. Banner paths remain metadata and may legally reference
   existing relative paths outside the pack, as in the original asset model.
2. Proposed read limit:4 MiB for Pack.ini, checked before allocation and also on
   streamed reads to handle growth. This is a documented resource bound, not an
   original format limit; files beyond it remain untouched with an explicit
   unsupported outcome. Use bounded line/field work within that total cap rather
   than a tiny arbitrary title-length restriction. Reject CR/LF/NUL in newly
   edited single-line values to prevent INI structure injection; retain loaded
   unknown bytes without flattening them.
3. Store an expected disk state as Missing or exact original bytes. Use the
   existing advisory sidecar convention plus exact compare before and after
   preparing a temporary file. Save in the same directory, preserve existing
   permissions, sync file and directory, and only then report durable completion.
4. Creation uses atomic no-clobber publication. Save refuses external change,
   external deletion and a newly appeared file; it never silently overwrites a
   concurrently edited Pack.ini. A watcher notification being muted does not
   disable save-time checking.
5. Existing symlink handling must be decided with root's canonical-path storage
   adapter: preserve the existing link/target binding, detect retargeting and do
   not accidentally replace a link with a normal file. Do not invent a blanket
   ban on user-managed symlink packs. Parent-path identity and error receipts
   must be covered by storage tests.
6. Distinguish failures before publication from durability failures after rename.
   If a file was published but directory sync failed, retain an explicit receipt
   and dirty/retry state matching actual disk bytes; do not pretend nothing was
   written. The saved semantic snapshot advances only under the chosen durable
   success contract.
7. Perform bounded I/O outside the frame renderer. Refresh on request and an
   asynchronous external-change check can reuse an application service; never
   reread4 MiB every frame. Late results bind to pack/song identity and generation.

These guards extend the original watcher model. Original FileSystemWatcher only
observes Changed/LastWrite and suppresses notices during save and for three
seconds afterward (`EditorItgManiaPack.cs:280–332`). It shows Ignore/Reload and
warns that Reload loses unsaved changes (`:334–365`). Rust's exact CAS checks and
create/delete detection should be documented as preservation/safety behavior,
not falsely claimed as executed original watcher equivalence.

## Proposed small implementation sequence

1. **Original oracle first:** a new .NET development project calls the actual
   pinned Fumen ItgManiaPack.LoadAsync/Save/Clone/Matches assemblies. Record source
   bytes, effective values, clone independence, modified values, written bytes
   and reloaded values. Cover missing Group/Version, malformed/future/version0,
   optional missing/null/empty values, duplicates/case/whitespace/comments,
   equals signs, signed/overflow Year, enum names/numeric/unknown values and the
   encoding cases above. No custom rewritten parser may generate expected data.
2. **Isolated core metadata:** new pack module and tests, initially undeclared.
   A source document + typed draft preserves original structure and semantic
   dirty state. Capture new-file defaults with explicit injected inputs. Keep
   version/unknown preservation differences visible in tests.
3. **Isolated storage:** synthetic temporary pack/song trees only; test missing
   inspection, atomic creation, no-clobber race, saved snapshot/retry, exact
   external edits/deletion, file permissions, symlink binding, malformed file
   preservation, failed writes and bounded reads. No real user pack is modified.
4. **History and native bridge after review:** optional EN/FR keyboard panel for
   seven fields, folder fallback, create/save, dirty and conflict/reload states.
   Pack title/banner changes notify the existing asset adapter. Test field edits,
   sync priority, independent song/pack dirty state, undo/redo, failed-save retry,
   external conflict and save/reopen before claiming this increment complete.

Pack song-table enumeration, load concurrency limits, banner selection/decoding,
explicit deletion and automatic file watching remain separate increments unless
root expands this scope. The original delete is explicitly confirmed and
non-undoable; it is not part of initial metadata/save implementation.


## Integrated storage increment (validated)

`src/pack/storage.rs` and `tests/pack_storage_workflows.rs` are integrated. The public path is `pack::storage`; the shared
simfile `storage.rs` is unchanged. Thirteen filesystem workflows and six private fault/race tests passed central
validation (the six units in session `84624`). They use synthetic temporary
directories exclusively. The previously declared metadata model's six tests
have passed; native UI validation remains separate.

`FileSession::open(pack_root)` is read-only and returns `None` only for absence.
Malformed, oversized, non-file and dangling-symlink targets are errors. Explicit
`create` prepares a missing-file draft; only `save` publishes it, using atomic
no-clobber. Existing files retain permission bits and unknown bytes. Paths are
bound to canonical pack/target parents and Unix device/inode identities; an
existing Pack.ini symlink survives because its resolved target is replaced.
Retargeting a link, rebinding a directory, deleting/replacing a file, or changing
its bytes prevents save. On non-Unix platforms std's available creation metadata
is a weaker identity fallback, not an inode guarantee.

A persistent advisory sidecar coordinates DeadSync saves to the resolved target.
Existing sidecars are opened without create/truncate, checked against the opened
file identity, and symlink sidecars are refused. The adapter compares exact disk
bytes before and after temporary-file preparation. Arbitrary external processes
do not participate in the lock: the final comparison and overwrite rename are
not a kernel compare-and-swap. The documented residual race does not weaken the
atomic no-clobber guarantee for creation.

`prepare_save` captures immutable bytes and a private adapter generation.
`publish` refuses foreign/stale snapshots, syncs the temporary file, atomically
publishes it, then synchronizes the directory on Unix. The receipt uses the
published temporary file's handle identity rather than a post-rename path read.
Publication followed by a directory-sync failure retains the exact on-disk
baseline and snapshot in an explicit pending state. `retry_durability` verifies
that published binding/bytes and synchronizes without rewriting; only success
acknowledges the saved semantic snapshot. Later live edits remain dirty.
Pre-publication failures leave the prior baseline and dirty draft intact.

Successful save/retry returns an inspectable `Durability`:

- `FileAndDirectory` on Unix: both file and directory synchronization succeeded.
- `FileOnly` elsewhere: file synchronization and atomic publication succeeded;
  directory synchronization is unavailable in this std-based adapter, matching
  the shared storage strategy. This is a narrower guarantee and does not claim
  Windows validation from execution on macOS.

Coverage includes missing inspection, creation/no-clobber races, malformed and
oversized targets, raw metadata preservation, independent prepared/live edits,
foreign/stale snapshots, external modification/deletion/same-bytes replacement,
locking, permission bits, symlink preservation/retargeting/dangling targets,
parent-directory replacement, failures before publication, published durability
failure/retry, unavailable post-publication path metadata and external edits
while a receipt is pending. ACLs/ownership beyond std `Permissions`, hostile
filesystem-wide race resistance, watchers and shared Save As integration remain
outside this isolated storage increment. The native Pack UI is separately
implemented and validated in `pack-native-plan.md`.

## Resumed storage validation

The 19 storage tests include no-clobber creation, exact external-change and inode
checks, symlink preservation/retarget conflict, parent rebinding, permissions,
advisory lock contention, pending publication receipts and retry after directory
sync failure. Receipt identity is captured from the temporary file handle before
publication; a post-rename path metadata lookup is not required to acknowledge
the published payload. Later draft edits remain dirty against that payload.

The API reports `FileAndDirectory` durability on Unix and `FileOnly` elsewhere,
matching the ordinary storage adapter's directory-sync boundary. The macOS run
does not validate Windows. Native Pack properties and close guards are integrated separately and passed
17 native tests (16 EN/FR workflows plus one grouped pending/release/source unit)
and 39 native regressions (help two + general editor 37). These storage tests
remain distinct from that UI validation. Shell compilation is in progress.
