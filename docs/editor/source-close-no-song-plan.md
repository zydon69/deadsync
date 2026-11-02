# Source Close and explicit NoSong editor state

Isolated `/tmp/deadsync-no-song/{base,work}` against pack coupling Action195 and close-bool/menu-test correction. No primary source applied, Cargo, staging or commit by this agent.

Wait for parent coupling validation before applying:

1. `editor-source-close-no-song.patch` (native presence, file controller/UI, EN/FR, tests).
2. `editor-source-close-no-song-actions.patch` (source Close callback/dispatch, Action195→196).

Both patches passed current-primary apply checks. Existing request/take_application_exit visibility `pub(in crate::screens)` is preserved. Stream/Density/preview implementations are not rewritten.

## Source contract

Pinned Editor.cs OnClose reviews song/pack, then OnCloseNoSave calls CloseSong, resets position and zoom1, while keeping the editor open. UnloadSongResources sets ActiveSong=null and ActivePack.SetSong(null); EditorPack.RefreshInternal clears its pack directory/data when there is no active song. Therefore Close must not be mapped to native CloseEditor, which returns to Practice.

## Native model

`DocumentPresence::{Loaded,NoSong}` explicitly distinguishes the UI/domain state. Existing State helpers require a Workspace; a memory-only empty Workspace is retained as an internal support object in NoSong. It has no destination and cannot be reached through editor Save/SaveAs/recovery/notes. It is never shown as Untitled, recorded in MRU, or claimed to represent an active song. This keeps the existing renderer/settings infrastructure without pretending that an Untitled song and no song are equivalent.

Close is a real source callback with source ID `Close`, configurable separately from CloseEditor. It goes through the same guarded song+pack Save/Discard/Cancel controller and pending durability proof, publishes a detached NoSong State only after the chosen route is valid, and does not set close_ready. The previous song remains on disk; source Close clears active pack/transport/view/history bindings after review. Application preference Snapshot is preserved; song zoom resets to1 as source.

NoSong F1 opens a bounded scrolling File/settings menu with New, Open, Recent, keyboard/density/dark/stream/receptor/note-colour/waveform/navigation settings, CloseEditor and genuine application Exit. There are no enabled song-edit commands. New explicitly creates a real Untitled song; Open restores a loaded song; neither reviews the internal support workspace as unsaved content. Global settings remain accessible. Source Close while NoSong is a no-op with clear help.

Current Save, SaveAs, output settings, recovery/autosave, note commands and NewChart reject NoSong. Automatic recovery only drains an existing worker and starts none. The source callback gate blocks song-specific keyboard commands; bare lanes already use the no-chart input route.

## Validation and tests

Rust source parsed via rustfmt stdout and both patches apply-check succeeded. **No Cargo compilation or Rust test executed by this agent.** Eight new native tests prepared:

- EN/FR Close enters explicit NoSong without a file or returning to Practice.
- Dirty Close Cancel/Discard retains disk contents appropriately.
- Save/recovery/notes refused while global settings and OS Exit work.
- New creates real Untitled; Open loads a real song after NoSong.
- F1 menu and distinct CloseEditor behavior.
- Application preference Arc/density retained and song zoom reset.
- Close Save writes the song, then stays in NoSong editor.
- Pack-only pending boundary: Close stays Loaded until exact durability retry, then NoSong, then OS Exit can become Ready; retry does not republish.

The existing 60-case source oracle in pack-coupling fixtures proves the original OnClose review/continuation decision. The resulting ActiveSong=null and pack clearing were checked directly in pinned Editor.cs/EditorPack.cs; no claim to running the full graphics/resource unload loop.

Remaining next lot: unify Pack Song List transition guards with the common file/pack controller. This patch does not certify that older separate transition path.
