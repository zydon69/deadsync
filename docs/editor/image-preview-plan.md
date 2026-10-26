# Banner and CD-title previews: integration plan

This is a plan, not a claim of validated implementation. Source inspected after
the battery interruption on 2026-10-26. The existing untracked
`screens/edition/image_previews.rs` is a CPU prototype and is not declared by its
parent module. No Cargo command was run for this review.

## Resumed implementation status

The sections below preserve the initial review and required contracts. Since that
review, the following implementation has been added to the working tree; focused
validation has now been executed by the parent agent. Confirmed results are
recorded below; they do not constitute full runtime or decoder parity.

- `shell/editor_image_previews.rs` owns one Cache across editor visits. App creates
  it lazily; retries after worker creation failure wait five seconds. No new
  service is created merely because the song properties panel closes.
- Theme `PreviewRequest` carries a document-visit identity and resolved Banner /
  CD-title draft paths. Practice forwards requests and returned views. Requests
  currently run while the SongText panel is visible. Closing the panel clears
  payloads; reopening reuses the worker and two stable texture keys.
- One joined worker owns bounded two-slot requests/results. Same-document path
  replacement retains the previous animation until decoding completes. Empty
  requests or document switches clear it immediately. Stale completions cannot
  publish after the corresponding request generation changes.
- Renderer-thread update unloads retired payloads before uploading frames through
  the ordinary AssetManager API. `unload_texture_keep_handle` preserves identity,
  aliases and sheet metadata. Global file metadata intentionally remains a
  fallback; local uploaded/source dimensions are cleared. Renderer rebuilds
  reupload retained CPU frames. Upload failures retry after two seconds and log
  only changed failures until success.
- The compact panel reserves a 100-pixel header for two proportionally fitted
  images and labels. French and English generic empty/loading/missing/video/error
  feedback is visible; detailed decoder errors are confined to logs. Existing
  keyboard navigation and field F2/F3/F4 actions remain the selection route.
- GIFs follow global application time; videos receive no poster-frame decoding.

Reference checkout was verified as
`48e18f71655d0f0d86144c63239331833501fda2`. Source contracts are drawn from
`EditorImageData.cs`, `Texture/EditorTexture.cs`, `UI/UISongProperties.cs`, and
`MonoGameExtensions/TextureUtils.cs`; this does not claim a completed differential
image decoder oracle.

Confirmed parent terminal results (2026-10-26):

- Asset unload: 3 tests passed.
- CPU preview cache: 7 tests passed.
- Native media UI: 9 English and 9 French tests passed (18 total).
- Shell preview service: 1 test passed; shell library compilation passed.
- Separate real-WAV lifecycle baseline: 1 test passed. This is audio evidence,
  not image-preview coverage.

Commands used for the focused validation:

```text
cargo test -p deadlib-assets --locked --jobs 2 --lib unload_texture_keep_handle
cargo test -p deadsync-theme-simply-love --locked --jobs 2 --lib image_previews
cargo test -p deadsync-theme-simply-love --locked --jobs 2 --test edition_song_assets_workflow --test edition_song_assets_fr_workflow
cargo test -p deadsync-shell --locked --jobs 2 --lib editor_image_previews
```

The added tests cover lifecycle/cancellation/external file reload, frame timing,
GPU identity reuse, draft cancellation, localized states, proportional actor
sizes and header/field separation. Passing tests, native OS dialogs, actual GPU
pixels, GIF decoder parity, total retained CPU peaks and arbitrary display sizes
must not be inferred from their presence. Limits remain 64 MiB input/decoded
source per decode, 16 MiB retained thumbnail frames per role and 1024 frames per
animation; concurrent old/result/decode memory can exceed a single role budget.

## Reference behavior

Reference: GrooveAuthor `EditorImageData.cs`, `Texture/EditorTexture.cs`,
`UI/UISongProperties.cs`, and `MonoGameExtensions/TextureUtils.cs` at the pinned
reference used by the editor delivery matrix.

- Banner and CD title are visible in song properties, including empty slots.
- Their buttons open the corresponding image selector. The simplified native
  menu must provide the same actions through keyboard/pad focus and F2.
- Relative paths resolve against the owning simfile directory. Updating a path
  starts asynchronous loading. Editing the same file externally reloads it.
- An old texture stays visible until its replacement finishes loading; swapping
  occurs outside draw submission. Clearing a path unloads it.
- GIFs animate using total application time, including zero-duration frames.
  Video paths are deliberately not decoded by EditorTexture.
- Layout uses `TextureLayoutMode.Box`: preserve aspect ratio, center, letterbox
  or pillarbox. Small images are enlarged to fit the box as well.
- Path edits and clear/browse remain part of the existing staged properties
  workflow; loading a preview must never apply a draft or dirty the document.

## Problems found in the CPU prototype

1. `Cache::new` starts a detached worker and creates two fresh global keys via
   `NEXT_CACHE`. Putting a Cache in each editor State lets repeated reopen grow
   texture metadata/handles, and a previous decoder can overlap the next worker.
2. `set_paths` immediately clears the old animation and retires the texture.
   This differs from the original replacement behavior described above.
3. Existing limits bound each role, not total retained memory: while decoding,
   old animations and queued results may coexist. Document and test a total
   bound; do not advertise a 16 MiB service bound from the per-animation limit.
4. Tests inside the untracked module have not been compiled. They demonstrate
   intended coverage only.

## Ownership and API

Add an application-owned service beside `app/option_previews.rs`, constructed
once lazily and retained across editor visits. It owns one long-lived CPU worker
and two stable keys (Banner, CD title). No decoder or filesystem access belongs
in screen view/render code. Preserve the existing service through renderer
recreation; requeue the selected CPU frames if the new AssetManager lacks them.

The screen supplies a small request snapshot:

```text
PreviewRequest { document_identity, paths: [Option<PathBuf>; 2] }
PreviewState { status, key, source_size, has_resident_image }
Service::set_request(Option<PreviewRequest>)
Service::update(total_application_time, assets, backend)
Service::view(role) -> PreviewState
```

Resolve paths from the active SongText draft when open, otherwise committed
properties. A changed document identity invalidates old results even when paths
match. Paths include simfile directory, so Save As requires no special rewrite.
Close sends None, invalidates work, clears results and CPU frames, and unloads
both GPU payloads. Reopen reuses worker and texture identities. If decoding is
uncancellable at a codec boundary, keep its worker occupied until it returns;
never spawn a substitute worker just because the screen closes.

On replacement, retain the currently displayed animation until the result is
accepted. A successful replacement queues the new selected frame; the renderer
retires the previous GPU texture through normal upload replacement. A failed
replacement clears the previous image and shows a localized failure/placeholder
after completion. Missing/unsupported/empty states have explicit placeholders.
No stale upload may survive a request generation change.

Store a JoinHandle in the service worker. Shutdown signals its Condvar and joins
at application service teardown, outside screen navigation. Worker results and
requests remain bounded to the two roles; no unbounded message queue.

## AssetManager support

Add `unload_texture_keep_handle(&str) -> Option<(TextureHandle, T)>` in
`TextureStore<T>` and the RendererTexture forwarding method in AssetManager.
It cancels pending uploads, clears uploaded/source dimensions, removes the GPU
payload, and advances the texture revision when observable content changes.
It **preserves** key/handle maps, case-insensitive aliases, and sprite-sheet
metadata. Unknown keys return None without reserving identities. Reserved keys
without uploaded textures still have pending work cancelled. The shell retires
returned textures with `backend.retire_texture` on the renderer thread.

Do not use `remove_texture` for routine close/reopen: it removes identity, and
the monotonically allocated dense handles then grow on each reopen. Do not
register generated textures; upload frames directly using the shared RGBA API.
Global dimension metadata can retain only these two stable keys.

## Required focused validation

1. CPU: PNG/JPEG transparency/source dimensions, corrupt/missing/video paths,
   bounds, GIF disposal/transparency and unequal/zero timing, source byte limits.
2. Worker: stale request result rejected, clear during decode, rapid song switch,
   creation/change/replacement/deletion, no decoded frame accepted if file stamp
   changed while loading. Synchronize tests through hooks/handshakes rather than
   depending on arbitrary sleep timing.
3. Asset store: unload absent/pending/resident; same handle after unload/reupload;
   no stale pending upload; dimensions/revision invalidated; case aliases retained;
   hundreds of close/reopen cycles leave handle allocation bounded by two.
4. Service: one worker across reopening, two texture identities, stale decode
   drained before next job, old image during loading then atomic swap, renderer
   reset reupload, source files unchanged, total retained CPU budget.
5. Native EN/FR: Banner/CD-title focus, F2 choose, F3 clear, F4 auto-discovery,
   draft cancel/apply, undo/redo, Save As, placeholder and Box layout actors.
   Opening the chooser via preview affordance must target the correct field.
6. Reference fidelity: extract unchanged C# GIF frame-selection and Box layout
   functions in an oracle fixture, compare boundary times and image aspect ratios
   against Rust. Actual GPU pixel output remains a separate runtime check.

Run Cargo serially with `--locked --jobs 2`. Stage this feature only after focused
CPU, asset, service and native targets pass. This increment does not complete the
full GrooveAuthor port; the delivery matrix remains authoritative for remaining
generation, editing, metadata, analysis, audio and runtime work.
