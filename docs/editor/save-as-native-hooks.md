# Save As hook layers and integration checkpoints

Layers A/B/C/D are now applied and centrally validated. All 49 editor core
units, 32 scoped native editor units (also serialized) and ten EN/FR State
workflows passed. The layer inventory below documents the integration order.

Apply in order at coordinated checkpoints:

1. `editor-media-history-relocation.patch`: declare document/history migration,
   extract unchanged serialize_document body.
2. `editor-save-as-storage.patch`: declare/export storage adapter and explicit
   pending-durability dirty marker. Validate existing history10 + storage11.
   If ReceptorPreferences is already present, also apply the receptor-only ignore
   patch before compiling; use the combined global-variants patch only after
   NoteColorPreferences also exists. Never replay either external effect. The future KeyBindingPreferences variant has a separate optional editor-media-relocation-key-bindings-variant.patch, applied only after the combined global ignore and after that variant exists.
3. `editor-save-as-rebind.patch`: add typed media-only proof, total external
   stamp mapping, Workspace focus-map publication, exact Position/View rebind,
   and poll-only AutoRecovery completion. Its storage export hunk expects layer2.
   Validate Receipt2, Workspace4, recovery-poll1 plus rerun the existing21.
4. `editor-save-as-native.patch`: add optional destination/review/pending panels,
   Ctrl+F12 and menu entries, Buffer/IME/clipboard Target, pack/navigation guards,
   profile history migration and localized EN/FR messages. It depends on layer3
   and the new native modules. It is regenerated against the current NoteColor/Receptor and MinimapOptions base: F1 menu22→23, Save As index22, EmptyMenu9→10, and all new Panel exhaustive arms.

The baseline A/B, current global-variant ignore, regenerated C/D were applied in order to copied files. Only the known old A document.rs declaration context was adapted to include current alias modules. Edition/text-fields/storage parser checks passed with rustfmt skip_children; the live source was not modified and no Cargo was run.

New modules to retain beside those patches:

- Core: history/media_relocation/rebind.rs; chart_focus/rebind.rs;
  chart_views/rebind.rs; workspace/save_as.rs; storage/auto_recovery/poll.rs.
- Native: edition/save_as.rs; edition/profile_edit/save_as.rs.

Native isolated unit targets add3 tests: pending UI boundary/close/open/recovery
and retry retention (EN/FR); modal recovery/dirtyPack guards (EN/FR); detached
profile map stamps while preserving its independent preference serials.
The pending-boundary mock does not certify filesystem durability; the real
storage tests own that proof. UI code never marks content saved by itself.

Install pending external State tests only with layer4:

- patches/save-as-native-tests/support.rs → tests/support/save_as.rs
- patches/save-as-native-tests/en.rs → tests/edition_save_as_workflow.rs
- patches/save-as-native-tests/fr.rs → tests/edition_save_as_fr_workflow.rs

They prepare5 tests per locale: Unicode/IME review/cancel; explicit existing
replacement and Undo; externally changed destination/error keeps editor open;
cross-format rejection; explicit transport stop and stale snapshot exclusion.
Native pending and pack guards are covered by the separate unit boundary tests.
A GPU screenshot/manual gameplay check is not implied.

The first UI keeps the same format and requires an absolute editable path.
Original cross-format writer options, a native file-dialog chooser, RecentFiles
updates and advanced export preferences remain required later work. The previous
recovery artifacts are retained, never deleted without their own review.

## Native corrections and scope
The destination actor owns its String; it does not borrow temporary text as a
static label. Native temporary-root fixtures use canonical paths on macOS. A
missing-Pack test supplies an explicit Version1 Pack.ini before editing it.

The receipt retry notice checks current dirty state: durable older output does
not mean newer edits are saved. Rebind-only diagnostics do not invent pending
disk durability. Two additional native units cover these messages in EN/FR.
No live GPU/audio/physical-input test is claimed. Cross-format writing and
Untitled remain separate unconnected increments.
