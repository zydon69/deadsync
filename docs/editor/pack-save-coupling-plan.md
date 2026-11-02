# Pack/song save coupling and real application Exit

Isolated copy: `/tmp/deadsync-pack-coupling/{base,work}`. No primary source applied, Cargo, staging or commit by this agent.

## Apply order

1. Current primary including chartless output, watcher, Density/Dark and TogglePreview.
2. `editor-pack-coupling.patch` (native orchestration, genuine shell Exit intent, EN/FR labels, durable source fixtures/tests).
3. `editor-pack-coupling-actions.patch` (registry + dispatch only). Prepared against Action191 →193. If notes' two conversion actions have already been applied, adjust only ALL count to195; all four callback additions must remain.

Both patches passed `git apply --check` against current primary when delivered. Existing Stream/Density/preview behavior is not rewritten. Do not apply old chartless patch again.

## Behavior and safety contracts

- File New/Open/Recent/Reload/editor-close review song and pack dirty state together. The source exemption for an Open into the same canonical pack retains the exact Settings object and rebinds its song binding rather than dropping pack edits.
- Keep editing cancels authority. Discard requires the existing separate confirmation. Save pack-only never writes the song. Save song and SaveAs durably save the exact active pack before publishing song output, preventing the existing SaveAs reset from deleting a pack draft.
- Source EditorSong saves song and pack concurrently after preflight. Native deliberately orders pack first because separate durable filesystem operations cannot form a cross-file atomic transaction; a later song failure retains its draft and does not continue closing. A successful pack write is not rolled back. Advanced loss review is prepared before coupled pack publication for ordinary Save; cancelling that review writes neither output.
- Pack guard uses exact instance identity plus checked monotone edit revision and root. Editing then returning to the original text still revokes authority. Reload replaces identity. Song history/Undo and policy revisions keep their existing guards and real publication witness.
- Pack durability retry does not republish. A pack-only Save-and-continue intent completes after the exact retry only if both guards remain current. New song edits (including edit/Undo) cancel continuation. Song+pack workflow failures keep the song and clear continuation; restarting Save is explicit.
- SaveAs and advanced Save reviews snapshot pack authority. Changed pack state requires a fresh review. No hidden draft rewrite, GUID weakening, or adopted external baseline.
- Source SavePackFile is an actual native callback using Pack storage. Missing pack metadata remains its existing explicit creation UI. Default binding is empty as source.
- Source Exit is an actual callback, not editor-close. Public `ApplicationExit::{Pending,Cancelled,Ready}` travels through practice API; Window CloseRequested initiates it, and the host consumes Ready with `event_loop.exit()`. Editor is retained during review. Cancel/failure revokes Exit; Ready is revalidated against history/policy/pack before the next host turn. Exit requested during existing pending song publication waits for its real durability retry.
- Source Close is still a separate subsequent lot: source leaves an editor with no active song, whereas current native CloseEditor returns to Practice. This patch does not map Close to CloseEditor or pretend Untitled is NoSong. Pack Song List has older local guards to unify in the subsequent lot as documented to parent.

## Source oracle actually executed

`scripts/editor-pack-coupling-reference.py` extracted unchanged complete source ShowUnsavedChangesModal, TryInvokePostSaveFunction, OnClose and OnExit at commit48e18f71655d0f0d86144c63239331833501fda2. .NET7 built and ran60 cases: five intents × song dirty × pack dirty × Save/Discard/Cancel. Fixtures and hashes live in `crates/deadsync-editor/tests/fixtures/pack-coupling` delivered by patch.

Property-only/UI-delegate adapters capture actual modal and callback behavior. Save stubs trace the documented song-to-pack coupling; this does not claim execution of the full EditorSong writer, pack disk publication, or native history.

## Validation scope

All isolated Rust files parsed with rustfmt stdout; both patch applicability checks passed. **No Rust compilation or test executed by this agent.** Nine new native tests are prepared (including the60-case source contract fixture): pack-only Save/Discard/Cancel EN/FR, edit-return invalidation, same-pack retention/rebind, distinct Exit/Cancel, durable pack Exit, edit after Ready, song+pack pending refusal preserving song/recovery, and pack-only exact retry with edit/Undo invalidation. Existing SaveAs dirty-pack test is updated to expect an accessible review rather than opaque entry refusal.

Parent should compile core key bindings plus native edition serial, then inspect the small shell Exit integration with its existing tests. Source Close/NoSong and Pack Song List are explicitly subsequent work, not certified complete here.
